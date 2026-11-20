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
