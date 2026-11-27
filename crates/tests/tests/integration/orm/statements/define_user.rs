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

/// A synthetic SCRAM-SHA-256 verifier of the shape SurrealDB 3.3.4 accepts: 4096 iterations, a
/// salt, and 32-byte stored and server keys.
pub(super) const SCRAM_VERIFIER: &str = "SCRAM-SHA-256$4096:MDEyMzQ1Njc4OWFiY2RlZg==\
    $nQpbZ77WudtqufPwikHXGRt6g2QJ4zns8bZLw273DRM=:jn2amWP1q1h+jgjy0YTO14S6/F02SV7taipOeB7ef20=";

/// `PASSSCRAM` renders beside the password, the server accepts a valid verifier, and refuses
/// one that isn't (S27).
#[tokio::test]
async fn a_scram_verifier_is_defined() {
    assert_sql(
        &define_user("frank")
            .on_database()
            .password("example-password")
            .passscram(SCRAM_VERIFIER),
        &format!(
            "DEFINE USER frank ON DATABASE PASSWORD 'example-password' PASSSCRAM '{SCRAM_VERIFIER}';"
        ),
    );
    let (db, _guard) = db().await;
    run(
        &db,
        define_user("frank")
            .on_database()
            .password("example-password")
            .passscram(SCRAM_VERIFIER),
    )
    .await;
    let refused = define_user("grace")
        .on_database()
        .password("example-password")
        .passscram("SCRAM-SHA-256$1:bad")
        .run(&db)
        .await;
    assert!(refused.is_err(), "an invalid verifier is refused");
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
