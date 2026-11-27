//! `DEFINE PARAM` (`orm/src/statements/define_param.rs`): its single-form `PERMISSIONS`, and
//! what they let a record user read.

use std::collections::BTreeMap;

use surrealdb::{opt::auth::Record, types::Value};
use umbriel::{
    Operatable, Param, Raw, Render, Runnable,
    statements::{define_access, define_param, record_access},
};
use umbriel_tests::*;

/// A record user may read a `PERMISSIONS FULL` param, and gets nothing for a `PERMISSIONS NONE`
/// one (S23, left by #13 for #15's record-access setup).
#[tokio::test]
async fn permissions_decide_what_a_record_user_reads() {
    let (db, guard) = db().await;
    for definition in [
        define_param(&Param::new("public"))
            .value("Hello")
            .permissions_full(),
        define_param(&Param::new("secret"))
            .value("Hidden")
            .permissions_none(),
    ] {
        definition.run(&db).await.expect("defined");
    }
    define_access("account")
        .on_database()
        .record(
            record_access()
                .signup(Raw::new("CREATE person SET name = $name"))
                .signin(Raw::new("SELECT * FROM person WHERE name = $name")),
        )
        .run(&db)
        .await
        .expect("defined");

    let user = guest(Endpoint::Main).await;
    bounded(
        "signing up",
        user.signup(Record {
            namespace: guard.namespace().to_owned(),
            database: guard.database().to_owned(),
            access: "account".to_owned(),
            params: BTreeMap::from([("name".to_owned(), "Alice".to_owned())]),
        }),
    )
    .await;
    bounded("selecting the test database", async {
        user.use_ns(guard.namespace())
            .use_db(guard.database())
            .await
    })
    .await;
    let public: String = read(&user, "RETURN $public;").await;
    assert_eq!(public, "Hello");
    let secret: Value = read(&user, "RETURN $secret;").await;
    assert_eq!(secret, Value::None, "PERMISSIONS NONE hides the param");
}

#[test]
fn permissions_take_none_full_or_a_condition() {
    let greeting = Param::new("greeting");
    assert_sql(
        &define_param(&greeting).value("Hello").permissions_none(),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS NONE;",
    );
    assert_sql(
        &define_param(&greeting).value("Hello").permissions_full(),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS FULL;",
    );
    assert_sql(
        &define_param(&greeting)
            .value("Hello")
            .permissions_where(Param::new("public").is(true)),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS WHERE $public IS true;",
    );
}

#[test]
fn the_last_permission_wins() {
    let statement = define_param(&Param::new("greeting"))
        .value("Hello")
        .permissions_none()
        .permissions_full();
    assert_eq!(
        statement.to_debug_sql(),
        "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS FULL;"
    );
}

#[test]
fn permissions_follow_the_comment() {
    assert_sql(
        &define_param(&Param::new("greeting"))
            .value("Hello")
            .comment("shown on sign-in")
            .permissions_none(),
        "DEFINE PARAM $greeting VALUE 'Hello' COMMENT 'shown on sign-in' PERMISSIONS NONE;",
    );
}
