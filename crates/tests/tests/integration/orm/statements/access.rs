//! The `ACCESS` statement (`orm/src/statements/access.rs`): every operation renders and parses,
//! and a bearer grant's lifecycle on the server.

use umbriel_tests::{support::statements::*, *};

#[test]
fn every_operation_renders() {
    let cases = [
        (
            access("api")
                .on(Level::Database)
                .grant_for_user("alice")
                .to_debug_sql(),
            "ACCESS api ON DATABASE GRANT FOR USER alice;",
        ),
        (
            access("devices")
                .grant_for_record(SpaceShip::id("apollo"))
                .to_debug_sql(),
            "ACCESS devices GRANT FOR RECORD space_ship:apollo;",
        ),
        (
            access("api").show_all().to_debug_sql(),
            "ACCESS api SHOW ALL;",
        ),
        (
            access("api").show_grant("abc123").to_debug_sql(),
            "ACCESS api SHOW GRANT abc123;",
        ),
        (
            access("api")
                .show_where(Field::new("subject.user").equal("alice"))
                .to_debug_sql(),
            "ACCESS api SHOW WHERE subject.user = 'alice';",
        ),
        (
            access("api").on(Level::Root).revoke_all().to_debug_sql(),
            "ACCESS api ON ROOT REVOKE ALL;",
        ),
        (
            access("api").revoke_grant("abc123").to_debug_sql(),
            "ACCESS api REVOKE GRANT abc123;",
        ),
        (
            access("api").purge_expired().to_debug_sql(),
            "ACCESS api PURGE EXPIRED;",
        ),
        (
            access("api")
                .purge_revoked()
                .purge_expired()
                .for_(Duration::from_secs(60 * 60).into())
                .to_debug_sql(),
            "ACCESS api PURGE EXPIRED, REVOKED FOR 1h;",
        ),
    ];
    for (sql, expected) in cases {
        assert_eq!(sql, expected);
        assert_parses(&sql);
    }
}

/// A10: a grant returns its key once, the key signs in, `SHOW` lists the grant, `REVOKE` stops
/// the key signing in, and `PURGE REVOKED` removes it. `DURATION FOR GRANT` shows as the grant's
/// expiry, read without waiting for it.
#[tokio::test]
async fn a_bearer_grant_lives_until_revoked_and_purged() {
    let (db, guard) = db().await;
    run(
        &db,
        define_user("alice")
            .on_database()
            .password("example-password")
            .role(UserRole::Viewer),
    )
    .await;
    run(
        &db,
        define_access("api")
            .on_database()
            .bearer_for_user(bearer_access())
            .duration_for_grant(Duration::from_secs(24 * 60 * 60)),
    )
    .await;

    let grant = access("api")
        .on(Level::Database)
        .grant_for_user("alice")
        .execute(&db)
        .await
        .expect("granted");
    assert_eq!(grant.ac, "api");
    assert_eq!(grant.kind, "bearer");
    assert_eq!(grant.subject.user.as_deref(), Some("alice"));
    assert!(
        grant.expiration.is_some(),
        "DURATION FOR GRANT sets an expiry"
    );
    assert!(grant.revocation.is_none());

    let signin = |key: String| {
        let namespace = guard.namespace().to_owned();
        let database = guard.database().to_owned();
        async move {
            guest(Endpoint::Main)
                .await
                .signin(Record {
                    namespace,
                    database,
                    access: "api".to_owned(),
                    params: BTreeMap::from([("key".to_owned(), key)]),
                })
                .await
        }
    };
    signin(grant.grant.key.clone())
        .await
        .expect("the key signs in");

    let shown = access("api")
        .show_grant(grant.id.clone())
        .execute(&db)
        .await
        .expect("shown");
    assert_eq!(shown.id, grant.id);
    assert_ne!(shown.grant.key, grant.grant.key, "SHOW redacts the key");
    let all = access("api").show_all().execute(&db).await.expect("listed");
    assert_eq!(all.len(), 1);

    let revoked = access("api")
        .revoke_grant(grant.id.clone())
        .execute(&db)
        .await
        .expect("revoked");
    assert!(revoked.revocation.is_some());
    let refused = signin(grant.grant.key.clone()).await.expect_err("revoked");
    assert!(
        matches!(Error::from(refused), Error::NotAllowed { .. }),
        "a revoked key is not allowed"
    );

    let purged = access("api")
        .purge_revoked()
        .execute(&db)
        .await
        .expect("purged");
    assert_eq!(purged.len(), 1);
    let all = access("api").show_all().execute(&db).await.expect("listed");
    assert!(all.is_empty(), "{all:?}");
}
