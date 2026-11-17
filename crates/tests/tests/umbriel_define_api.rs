//! APIs (`orm/src/statements/define_api.rs`, and `alter_api`/`remove_api`): rendering, checked
//! with SurrealDB's parser, and their effect on a database.

use std::time::Duration;

use umbriel::{
    Runnable,
    statements::{ApiMethod, alter_api, api_action, api_config, define_api, remove_api},
};
use umbriel_tests::*;

#[test]
fn a_definition_writes_its_config_fallback_and_actions() {
    assert_sql(&define_api("/status"), "DEFINE API '/status';");
    assert_sql(
        &define_api("/status")
            .overwrite()
            .for_any(
                api_config()
                    .permissions_full()
                    .middleware("api::timeout", [Duration::from_secs(1)]),
            )
            .fallback("not found")
            .action(api_action([ApiMethod::Get, ApiMethod::Post], "ok"))
            .action(api_action([ApiMethod::Delete], "gone").config(api_config().permissions_none()))
            .comment("Acme Corp status"),
        "DEFINE API OVERWRITE '/status' FOR ANY PERMISSIONS FULL MIDDLEWARE api::timeout(1s) \
         THEN 'not found' FOR GET, POST THEN 'ok' FOR DELETE PERMISSIONS NONE THEN 'gone' \
         COMMENT 'Acme Corp status';",
    );
    // A fallback alone still needs `FOR ANY`.
    assert_sql(
        &define_api("/status").fallback("ok"),
        "DEFINE API '/status' FOR ANY THEN 'ok';",
    );
}

#[test]
fn invalid_actions_and_middleware_fail() {
    assert_render_error(
        &define_api("/status").action(api_action([], "ok")),
        "at least one method",
    );
    for hostile in [
        "timeout",
        "api::",
        "api::timeout; REMOVE TABLE user",
        "1api::timeout",
    ] {
        let statement =
            define_api("/status").for_any(api_config().middleware(hostile, Vec::<i64>::new()));
        assert_render_error(&statement, "invalid");
    }
}

#[test]
fn alter_sets_and_drops_actions_and_the_fallback() {
    assert_sql(
        &alter_api("/status")
            .if_exists()
            .for_any(api_config().permissions_none())
            .fallback("moved")
            .action(api_action([ApiMethod::Put], "ok"))
            .drop_action([ApiMethod::Get, ApiMethod::Trace])
            .comment("v2"),
        "ALTER API IF EXISTS '/status' FOR ANY PERMISSIONS NONE THEN 'moved' FOR PUT THEN 'ok' \
         FOR GET, TRACE DROP THEN COMMENT 'v2';",
    );
    assert_sql(
        &alter_api("/status").drop_fallback().drop_comment(),
        "ALTER API '/status' FOR ANY DROP THEN DROP COMMENT;",
    );
    assert_render_error(
        &alter_api("/status").fallback("x").drop_fallback(),
        "both set and drop THEN",
    );
}

/// Regression: actions are written before drops, so a method both set and dropped would end up
/// dropped whichever call came last.
#[test]
fn a_method_both_set_and_dropped_fails_in_either_order() {
    assert_render_error(
        &alter_api("/status")
            .drop_action([ApiMethod::Get])
            .action(api_action([ApiMethod::Get], "ok")),
        "both set and drop the action of GET",
    );
    assert_render_error(
        &alter_api("/status")
            .action(api_action([ApiMethod::Get, ApiMethod::Post], "ok"))
            .drop_action([ApiMethod::Trace, ApiMethod::Get]),
        "both set and drop the action of GET",
    );
}

#[test]
fn remove_takes_if_exists() {
    assert_sql(
        &remove_api("/status").if_exists(),
        "REMOVE API IF EXISTS '/status';",
    );
}

#[tokio::test]
async fn an_api_is_defined_altered_and_removed() {
    let db = db().await;
    define_api("/status")
        .action(api_action([ApiMethod::Get], "ok"))
        .run(&db)
        .await
        .expect("defined");
    let api = stored_definition(&db, "INFO FOR DB;", "apis", "/status").await;
    // 3.3.4 lists methods in lower case.
    assert!(api.contains("FOR get"), "{api}");

    alter_api("/status")
        .action(api_action([ApiMethod::Post], "created"))
        .comment("Acme Corp status")
        .run(&db)
        .await
        .expect("altered");
    let api = stored_definition(&db, "INFO FOR DB;", "apis", "/status").await;
    assert!(api.contains("FOR post"), "{api}");
    assert!(api.contains("COMMENT 'Acme Corp status'"), "{api}");

    remove_api("/status").run(&db).await.expect("removed");
    assert!(remove_api("/status").run(&db).await.is_err());
}
