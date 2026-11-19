//! Configs (`orm/src/statements/define_config.rs`, and the `remove_config_*` builders):
//! rendering, checked with SurrealDB's parser, and where each config is stored.

use surrealdb::types::{Object, Value};
use umbriel::{
    Runnable,
    statements::{
        api_config, define_config_api, define_config_default, define_config_graphql,
        remove_config_api, remove_config_default, remove_config_graphql,
    },
};
use umbriel_tests::*;

#[test]
fn the_api_config_writes_its_config() {
    assert_sql(&define_config_api(api_config()), "DEFINE CONFIG API;");
    assert_sql(
        &define_config_api(
            api_config()
                .permissions_full()
                .middleware("api::req::max_body", ["1mb"]),
        )
        .if_not_exists(),
        "DEFINE CONFIG IF NOT EXISTS API PERMISSIONS FULL MIDDLEWARE api::req::max_body('1mb');",
    );
}

#[test]
fn the_graphql_config_writes_its_clauses() {
    assert_sql(&define_config_graphql(), "DEFINE CONFIG GRAPHQL;");
    assert_sql(
        &define_config_graphql()
            .overwrite()
            .tables_exclude(["audit"])
            .functions_include(["math::add_one", "greet"])
            .depth(4)
            .complexity(100)
            .introspection(false),
        "DEFINE CONFIG OVERWRITE GRAPHQL TABLES EXCLUDE audit FUNCTIONS INCLUDE \
         fn::math::add_one, fn::greet DEPTH 4 COMPLEXITY 100 INTROSPECTION NONE;",
    );
    assert_sql(
        &define_config_graphql()
            .tables_none()
            .functions_none()
            .introspection(true),
        "DEFINE CONFIG GRAPHQL TABLES NONE FUNCTIONS NONE INTROSPECTION AUTO;",
    );
    assert_render_error(
        &define_config_graphql().tables_include(Vec::<String>::new()),
        "at least one name",
    );
    assert_render_error(
        &define_config_graphql().functions_include(["fn::greet"]),
        "pass the name without `fn::`",
    );
}

#[test]
fn the_default_config_needs_a_namespace_or_database() {
    assert_sql(
        &define_config_default().database("app"),
        "DEFINE CONFIG DEFAULT DATABASE app;",
    );
    assert_render_error(&define_config_default(), "needs a namespace");
}

#[test]
fn removal_names_the_config() {
    assert_sql(&remove_config_api(), "REMOVE CONFIG API;");
    assert_sql(
        &remove_config_graphql().if_exists(),
        "REMOVE CONFIG IF EXISTS GRAPHQL;",
    );
    assert_sql(&remove_config_default(), "REMOVE CONFIG DEFAULT;");
}

#[tokio::test]
async fn database_configs_are_defined_and_removed() {
    let (db, _guard) = db().await;
    define_config_api(api_config().permissions_full())
        .run(&db)
        .await
        .expect("defined");
    define_config_graphql()
        .tables_auto()
        .run(&db)
        .await
        .expect("defined");
    let api = stored_definition(&db, "INFO FOR DB;", "configs", "API").await;
    assert!(api.starts_with("API"), "{api}");
    let graphql = stored_definition(&db, "INFO FOR DB;", "configs", "GraphQL").await;
    assert!(graphql.starts_with("GRAPHQL"), "{graphql}");

    remove_config_api().run(&db).await.expect("removed");
    remove_config_graphql().run(&db).await.expect("removed");
    let info: Object = read(&db, "INFO FOR DB;").await;
    assert!(
        matches!(info.get("configs"), Some(Value::Object(configs)) if configs.is_empty()),
        "{info:?}"
    );
}

/// The default config is stored on the root, where a 3.x server defines one when it starts. It
/// is server-wide, so the test reads it rather than redefining or removing it.
#[tokio::test]
async fn the_default_config_is_stored_on_the_root() {
    let (db, _guard) = db().await;
    let root: Object = read(&db, "INFO FOR ROOT STRUCTURE;").await;
    assert!(
        matches!(root.get("defaults"), Some(Value::Object(defaults)) if !defaults.is_empty()),
        "{root:?}"
    );
}

/// Pins the upstream limitation the builders' docs cite: `ALTER CONFIG` drops its comment.
#[tokio::test]
async fn alter_config_drops_its_comment() {
    let (db, _guard) = db().await;
    define_config_api(api_config().permissions_full())
        .run(&db)
        .await
        .expect("defined");
    assert!(
        succeeds(
            &db,
            "ALTER CONFIG API PERMISSIONS FULL COMMENT 'Acme Corp';"
        )
        .await
    );
    let api = stored_definition(&db, "INFO FOR DB;", "configs", "API").await;
    assert!(!api.contains("COMMENT"), "{api}");
}
