//! `DEFINE ACCESS` (`orm/src/statements/define_access.rs`): record and JWT access, and the combinations SurrealDB refuses.

use umbriel_tests::{support::statements::*, *};

#[test]
fn access_definitions() {
    let signup = Raw::new("CREATE user SET email = $email, pass = crypto::argon2::generate($pass)");
    let signin = Raw::new(
        "SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)",
    );
    assert_sql(
        &define_access("account").on_database().record(
            record_access()
                .signup(signup)
                .signin(signin)
                .with_jwt(JwtAlgorithm::HS512, "example-key")
                .duration_for_token(Duration::from_secs(15 * 60))
                .duration_for_session(Duration::from_secs(12 * 60 * 60)),
        ),
        "DEFINE ACCESS account ON DATABASE TYPE RECORD \
             SIGNUP (CREATE user SET email = $email, pass = crypto::argon2::generate($pass)) \
             SIGNIN (SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)) \
             WITH JWT ALGORITHM HS512 KEY 'example-key' \
             DURATION FOR TOKEN 15m, FOR SESSION 12h;",
    );
    assert_sql(
        &define_access("account").on_database().record(
            record_access()
                .signin(select(All).from(Table::from("user")))
                .duration_for_session(Duration::from_secs(60 * 60)),
        ),
        "DEFINE ACCESS account ON DATABASE TYPE RECORD SIGNIN (SELECT * FROM user) \
             DURATION FOR SESSION 1h;",
    );
    assert_sql(
        &define_access("api").on_namespace().jwt(
            jwt_access()
                .algorithm(JwtAlgorithm::RS256)
                .key("example-public-key"),
        ),
        "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key';",
    );
    assert_sql(
        &define_access("issuer").on_root().jwt(
            jwt_access()
                .url("https://example.com/jwks.json")
                .duration_for_session(Duration::from_secs(60)),
        ),
        "DEFINE ACCESS issuer ON ROOT TYPE JWT URL 'https://example.com/jwks.json' \
             DURATION FOR SESSION 1m;",
    );
}

#[test]
fn define_access_rejects_invalid_combinations() {
    assert_render_error(
        &define_access("account").on_root().record(record_access()),
        "only be defined on a database",
    );
    assert_render_error(
        &define_access("account")
            .on_namespace()
            .record(record_access()),
        "only be defined on a database",
    );
    assert_render_error(
        &define_access("api").jwt(jwt_access().url("https://example.com")),
        "needs a level",
    );
    assert_render_error(&define_access("api").on_database(), "needs a type");
    assert_render_error(
        &define_access("api").on_database().jwt(jwt_access()),
        "needs `algorithm` and `key`",
    );
    assert_render_error(
        &define_access("api").on_database().jwt(
            jwt_access()
                .algorithm(JwtAlgorithm::HS256)
                .key("example")
                .url("https://example.com"),
        ),
        "not both",
    );
}
