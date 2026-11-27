//! `DEFINE ACCESS` (`orm/src/statements/define_access.rs`): record, JWT and bearer access, the
//! clauses every type takes, and what SurrealDB does with them on sign-up, sign-in and
//! authentication.

use umbriel_tests::{support::statements::*, *};

/// The record access every behaviour test signs in through: users are `person` records found by
/// `$name` and `$pass`.
fn signup_expression() -> Raw {
    Raw::new("CREATE person SET name = $name, pass = crypto::argon2::generate($pass)")
}

fn signin_expression() -> Raw {
    Raw::new("SELECT * FROM person WHERE name = $name AND crypto::argon2::compare(pass, $pass)")
}

fn person(
    guard: &TestDb,
    access: &str,
    name: &str,
    pass: &str,
) -> Record<BTreeMap<String, String>> {
    Record {
        namespace: guard.namespace().to_owned(),
        database: guard.database().to_owned(),
        access: access.to_owned(),
        params: BTreeMap::from([
            ("name".to_owned(), name.to_owned()),
            ("pass".to_owned(), pass.to_owned()),
        ]),
    }
}

/// A guest connection with the test database selected.
async fn guest_in(guard: &TestDb) -> Surreal<Any> {
    let db = guest(Endpoint::Main).await;
    bounded("selecting the test database", async {
        db.use_ns(guard.namespace()).use_db(guard.database()).await
    })
    .await;
    db
}

mod rendering {
    use super::*;

    #[test]
    fn record_forms() {
        assert_sql(
            &define_access("account")
                .on_database()
                .record(
                    umbriel::statements::record_access()
                        .signup(signup_expression())
                        .signin(signin_expression())
                        .with_jwt(
                            jwt_access()
                                .algorithm(JwtAlgorithm::HS512)
                                .key("example-key"),
                        )
                        .with_refresh(),
                )
                .duration_for_token(Duration::from_secs(15 * 60))
                .duration_for_session(Duration::from_secs(12 * 60 * 60)),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD \
             SIGNUP (CREATE person SET name = $name, pass = crypto::argon2::generate($pass)) \
             SIGNIN (SELECT * FROM person WHERE name = $name AND crypto::argon2::compare(pass, $pass)) \
             WITH JWT ALGORITHM HS512 KEY 'example-key', WITH REFRESH \
             DURATION FOR TOKEN 15m, FOR SESSION 12h;",
        );
        assert_sql(
            &define_access("account")
                .on_database()
                .record(
                    umbriel::statements::record_access()
                        .signin(select(All).from(Table::from("user")))
                        .with_refresh(),
                )
                .duration_for_session(Duration::from_secs(60 * 60)),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD SIGNIN (SELECT * FROM user) \
             WITH REFRESH DURATION FOR SESSION 1h;",
        );
    }

    #[test]
    fn jwt_forms() {
        assert_sql(
            &define_access("api").on_namespace().jwt(
                umbriel::statements::jwt_access()
                    .algorithm(JwtAlgorithm::RS256)
                    .key("example-public-key")
                    .audience(["acme-app", "acme-admin"])
                    .with_issuer_key("example-private-key"),
            ),
            "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key' \
             AUDIENCE 'acme-app', 'acme-admin' WITH ISSUER KEY 'example-private-key';",
        );
        assert_sql(
            &define_access("issuer")
                .on_root()
                .jwt(
                    umbriel::statements::jwt_access()
                        .url("https://example.com/jwks.json")
                        .with_issuer(JwtAlgorithm::RS256, "example-private-key"),
                )
                .duration_for_session(Duration::from_secs(60)),
            "DEFINE ACCESS issuer ON ROOT TYPE JWT URL 'https://example.com/jwks.json' \
             WITH ISSUER ALGORITHM RS256 KEY 'example-private-key' DURATION FOR SESSION 1m;",
        );
    }

    #[test]
    fn bearer_forms() {
        assert_sql(
            &define_access("api")
                .on_root()
                .bearer_for_user(umbriel::statements::bearer_access())
                .duration_for_grant(Duration::from_secs(30 * 24 * 60 * 60))
                .duration_for_session(Expiry::Never),
            "DEFINE ACCESS api ON ROOT TYPE BEARER FOR USER \
             DURATION FOR GRANT 4w2d, FOR SESSION NONE;",
        );
        assert_sql(
            &define_access("devices").on_database().bearer_for_record(
                umbriel::statements::bearer_access().with_jwt(
                    jwt_access()
                        .algorithm(JwtAlgorithm::HS256)
                        .key("example-key"),
                ),
            ),
            "DEFINE ACCESS devices ON DATABASE TYPE BEARER FOR RECORD \
             WITH JWT ALGORITHM HS256 KEY 'example-key';",
        );
    }

    #[test]
    fn authenticate_context_and_options() {
        assert_sql(
            &define_access("account")
                .on_database()
                .overwrite()
                .record(umbriel::statements::record_access().signin(signin_expression()))
                .authenticate(Raw::new(
                    "IF $auth.banned { THROW 'banned' } ELSE { $auth }",
                ))
                .context(Raw::new("SELECT VALUE plan FROM ONLY $auth"))
                .comment("record users"),
            "DEFINE ACCESS OVERWRITE account ON DATABASE TYPE RECORD \
             SIGNIN (SELECT * FROM person WHERE name = $name AND crypto::argon2::compare(pass, $pass)) \
             AUTHENTICATE (IF $auth.banned { THROW 'banned' } ELSE { $auth }) \
             CONTEXT (SELECT VALUE plan FROM ONLY $auth) COMMENT 'record users';",
        );
    }

    #[test]
    fn invalid_combinations_fail_to_render() {
        assert_render_error(&define_access("api").on_database(), "needs a type");
        assert_render_error(
            &define_access("api")
                .on_database()
                .jwt(umbriel::statements::jwt_access()),
            "needs `algorithm` and `key`",
        );
        assert_render_error(
            &define_access("api").on_database().jwt(
                umbriel::statements::jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example")
                    .url("https://example.com"),
            ),
            "not both",
        );
        assert_render_error(
            &define_access("api").on_database().jwt(
                umbriel::statements::jwt_access()
                    .algorithm(JwtAlgorithm::RS256)
                    .key("example-public-key")
                    .with_issuer(JwtAlgorithm::HS256, "example"),
            ),
            "the verifying algorithm",
        );
    }
}

mod behaviour {
    use super::*;

    /// Each definition is stored as written: `INFO FOR DB` lists it (A3).
    #[tokio::test]
    async fn definitions_are_stored() {
        let (db, _guard) = db().await;
        run(
            &db,
            define_access("account")
                .on_database()
                .record(
                    umbriel::statements::record_access()
                        .signup(signup_expression())
                        .signin(signin_expression())
                        .with_refresh(),
                )
                .duration_for_session(Duration::from_secs(60 * 60)),
        )
        .await;
        run(
            &db,
            define_access("api")
                .on_database()
                .bearer_for_user(umbriel::statements::bearer_access())
                .duration_for_grant(Duration::from_secs(60 * 60)),
        )
        .await;
        let account = stored_definition(&db, "INFO FOR DB;", "accesses", "account").await;
        assert!(account.contains("TYPE RECORD"), "{account}");
        assert!(account.contains("WITH REFRESH"), "{account}");
        assert!(account.contains("FOR SESSION 1h"), "{account}");
        let api = stored_definition(&db, "INFO FOR DB;", "accesses", "api").await;
        assert!(api.contains("TYPE BEARER FOR USER"), "{api}");
        assert!(api.contains("FOR GRANT 1h"), "{api}");
    }

    /// JWT access on the root and on the namespace is stored at that level, with its algorithm
    /// and audience (A3). Both outlive the test's database, so they get names of their own and
    /// are removed afterwards.
    #[tokio::test]
    async fn jwt_definitions_are_stored_on_root_and_namespace() {
        let (db, guard) = db().await;
        let name = format!("jwt_{}", guard.database());
        let jwt = || {
            umbriel::statements::jwt_access()
                .algorithm(JwtAlgorithm::HS512)
                .key("example-key")
                .audience(["api.example.com"])
        };
        run(&db, define_access(name.clone()).on_root().jwt(jwt())).await;
        run(&db, define_access(name.clone()).on_namespace().jwt(jwt())).await;
        let on_root = stored_definition(&db, "INFO FOR ROOT;", "accesses", &name).await;
        let on_namespace = stored_definition(&db, "INFO FOR NS;", "accesses", &name).await;
        run(&db, remove_access(name.clone()).on_root()).await;
        run(&db, remove_access(name.clone()).on_namespace()).await;
        for stored in [on_root, on_namespace] {
            assert!(stored.contains("TYPE JWT"), "{stored}");
            assert!(stored.contains("ALGORITHM HS512"), "{stored}");
            assert!(stored.contains("api.example.com"), "{stored}");
        }
    }

    /// Sign-up issues a token and sign-in with the right credentials succeeds. A wrong password,
    /// an unknown access method and a `THROW` in `AUTHENTICATE` refuse the sign-in (A4, A8).
    /// SurrealDB 3.3.4 reports a `SIGNIN` that finds no record as "No record was returned",
    /// without the `NotAllowed` kind, so only the refusal is checked.
    #[tokio::test]
    async fn signup_and_signin() {
        let (db, guard) = db().await;
        run(
            &db,
            define_access("account").on_database().record(
                umbriel::statements::record_access()
                    .signup(signup_expression())
                    .signin(signin_expression()),
            ),
        )
        .await;
        let alice = guest_in(&guard).await;
        alice
            .signup(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect("signed up");
        let bob = guest_in(&guard).await;
        bob.signin(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect("signed in");

        for credentials in [
            person(&guard, "account", "Alice", "wrong-password"),
            person(&guard, "missing", "Alice", "example-password"),
        ] {
            guest_in(&guard)
                .await
                .signin(credentials)
                .await
                .expect_err("refused");
        }

        run(
            &db,
            define_access("account")
                .on_database()
                .overwrite()
                .record(
                    umbriel::statements::record_access()
                        .signup(signup_expression())
                        .signin(signin_expression()),
                )
                .authenticate(Raw::new("THROW 'accounts are closed'")),
        )
        .await;
        let refused = guest_in(&guard)
            .await
            .signin(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect_err("refused by AUTHENTICATE");
        assert!(
            refused.to_string().contains("accounts are closed"),
            "{refused}"
        );
    }

    /// `WITH REFRESH` gives sign-in a refresh token, and without it there is none (A5).
    #[tokio::test]
    async fn refresh_tokens_follow_with_refresh() {
        let (db, guard) = db().await;
        run(
            &db,
            define_access("account").on_database().record(
                umbriel::statements::record_access()
                    .signup(signup_expression())
                    .signin(signin_expression())
                    .with_refresh(),
            ),
        )
        .await;
        run(
            &db,
            define_access("plain").on_database().record(
                umbriel::statements::record_access()
                    .signup(signup_expression())
                    .signin(signin_expression()),
            ),
        )
        .await;
        let refreshed = guest_in(&guard)
            .await
            .signup(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect("signed up");
        assert!(
            refreshed.refresh.is_some(),
            "WITH REFRESH issues a refresh token"
        );
        let plain = guest_in(&guard)
            .await
            .signup(person(&guard, "plain", "Bob", "example-password"))
            .await
            .expect("signed up");
        assert!(plain.refresh.is_none(), "no WITH REFRESH, no refresh token");

        let session = guest_in(&guard).await;
        session
            .authenticate(refreshed)
            .await
            .expect("the refresh token is exchanged");
    }

    /// After `invalidate`, a permission-checked read is refused as `NotAllowed`: the session is
    /// anonymous again (A6).
    #[tokio::test]
    async fn invalidate_ends_the_session() {
        let (db, guard) = db().await;
        run(
            &db,
            define_access("account").on_database().record(
                umbriel::statements::record_access()
                    .signup(signup_expression())
                    .signin(signin_expression()),
            ),
        )
        .await;
        assert!(
            succeeds(
                &db,
                "DEFINE TABLE note PERMISSIONS FOR select WHERE $auth != NONE; \
                 CREATE note:one SET text = 'hello';"
            )
            .await
        );
        let alice = guest_in(&guard).await;
        alice
            .signup(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect("signed up");
        let notes: Vec<Value> = read(&alice, "SELECT * FROM note;").await;
        assert_eq!(notes.len(), 1, "a signed-in user reads the note");
        alice.invalidate().await.expect("invalidated");
        let error = Raw::new("SELECT * FROM note;")
            .run(&alice)
            .await
            .expect_err("an invalidated session is refused");
        assert!(matches!(error, Error::NotAllowed { .. }), "{error:?}");
    }

    /// `CONTEXT` puts its value in `$session.data`, and SurrealDB refuses one that could write (A8).
    #[tokio::test]
    async fn context_is_the_sessions_data() {
        let (db, guard) = db().await;
        run(
            &db,
            define_access("account")
                .on_database()
                .record(
                    umbriel::statements::record_access()
                        .signup(signup_expression())
                        .signin(signin_expression()),
                )
                .context(Raw::new("{ plan: 'pro' }")),
        )
        .await;
        let alice = guest_in(&guard).await;
        alice
            .signup(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect("signed up");
        let plan: String = read(&alice, "RETURN $session.data.plan;").await;
        assert_eq!(plan, "pro");

        let writing = define_access("writer")
            .on_database()
            .record(umbriel::statements::record_access().signin(signin_expression()))
            .context(Raw::new("CREATE audit"));
        assert!(
            writing.run(&db).await.is_err(),
            "a writing CONTEXT is refused"
        );
    }

    /// The access methods whose tokens `authenticate` takes.
    fn record_with_audience(
        audience: &[&str],
    ) -> umbriel::statements::DefineAccessStatement<OnDatabase> {
        define_access("account").on_database().overwrite().record(
            umbriel::statements::record_access()
                .signup(signup_expression())
                .signin(signin_expression())
                .with_jwt(
                    jwt_access()
                        .algorithm(JwtAlgorithm::HS512)
                        .key("example-shared-key")
                        .audience(audience.iter().copied()),
                ),
        )
    }

    /// `AUDIENCE` with tokens SurrealDB signs itself: a record access signs `aud` into what it
    /// issues, and redefining the method changes what it accepts (A9). Mirrors the cases of
    /// `test_token_with_audience` in `surrealdb-core` 3.3.4's `iam/verify.rs`.
    #[tokio::test]
    async fn audience_is_signed_and_checked() {
        let (db, guard) = db().await;
        run(&db, record_with_audience(&["acme-app"])).await;
        let token = guest_in(&guard)
            .await
            .signup(person(&guard, "account", "Alice", "example-password"))
            .await
            .expect("signed up")
            .access;
        let unaudienced = {
            run(&db, record_with_audience(&[])).await;
            guest_in(&guard)
                .await
                .signup(person(&guard, "account", "Bob", "example-password"))
                .await
                .expect("signed up")
                .access
        };

        let accepts = |audience: &'static [&'static str],
                       token: surrealdb::opt::auth::AccessToken| {
            let db = db.clone();
            let guard_ns = guard.namespace().to_owned();
            let guard_db = guard.database().to_owned();
            async move {
                run(&db, record_with_audience(audience)).await;
                let session = guest(Endpoint::Main).await;
                bounded("selecting the test database", async {
                    session.use_ns(guard_ns).use_db(guard_db).await
                })
                .await;
                session.authenticate(token).await
            }
        };

        accepts(&["acme-app"], token.clone())
            .await
            .expect("a matching audience is accepted");
        accepts(&["other", "acme-app"], token.clone())
            .await
            .expect("a list naming the audience is accepted");
        let mismatched = accepts(&["other"], token).await.expect_err("refused");
        assert!(
            matches!(Error::from(mismatched), Error::NotAllowed { .. }),
            "a mismatched audience is not allowed"
        );
        let missing = accepts(&["acme-app"], unaudienced)
            .await
            .expect_err("refused");
        assert!(
            matches!(Error::from(missing), Error::NotAllowed { .. }),
            "a token without the required audience is not allowed"
        );
    }
}
