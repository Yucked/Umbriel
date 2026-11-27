//! `DEFINE USER` (`orm/src/statements/define_user.rs`): each level, password or hash, and role.

use umbriel_tests::{support::statements::*, *};

#[test]
fn user_definitions() {
    assert_sql(
        &define_user("alice")
            .on_root()
            .password("example-password")
            .role(UserRole::Owner),
        "DEFINE USER alice ON ROOT PASSWORD 'example-password' ROLES OWNER;",
    );
    assert_sql(
        &define_user("bob")
            .on_namespace()
            .passhash("$argon2id$example")
            .role(UserRole::Editor),
        "DEFINE USER bob ON NAMESPACE PASSHASH '$argon2id$example' ROLES EDITOR;",
    );
    assert_sql(
        &define_user("carol")
            .on_database()
            .password("example")
            .role(UserRole::Viewer),
        "DEFINE USER carol ON DATABASE PASSWORD 'example' ROLES VIEWER;",
    );
}

#[test]
fn roles_and_durations() {
    assert_sql(
        &define_user("dave")
            .on_database()
            .password("example")
            .role(UserRole::Editor)
            .role(UserRole::Viewer)
            .duration_for_token(Duration::from_secs(5 * 60))
            .duration_for_session(Expiry::Never)
            .comment("reporting"),
        "DEFINE USER dave ON DATABASE PASSWORD 'example' ROLES EDITOR, VIEWER \
         DURATION FOR TOKEN 5m, FOR SESSION NONE COMMENT 'reporting';",
    );
}

/// The durations are stored with the user (A11).
#[tokio::test]
async fn durations_are_stored() {
    let (db, _guard) = db().await;
    run(
        &db,
        define_user("erin")
            .on_database()
            .password("example-password")
            .duration_for_token(Duration::from_secs(5 * 60))
            .duration_for_session(Duration::from_secs(60 * 60)),
    )
    .await;
    let stored = stored_definition(&db, "INFO FOR DB;", "users", "erin").await;
    assert!(stored.contains("FOR TOKEN 5m"), "{stored}");
    assert!(stored.contains("FOR SESSION 1h"), "{stored}");
}
