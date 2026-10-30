//! LET (`orm/src/statements/let_.rs`): typed parameters, rendered with each value's SurrealDB
//! type and checked by the database.

use umbriel::{
    Model, RecordId, Render, Runnable, chain,
    statements::{let_, return_},
};
use umbriel_tests::*;

#[test]
fn a_typed_let_renders_the_value_type() {
    assert_sql(
        &let_("count").typed::<i64>().equal_to(5),
        "LET $count: int = 5;",
    );
    assert_sql(
        &let_("tags").typed::<Vec<i64>>().equal_to(vec![1, 2]),
        "LET $tags: array<int> = [1, 2];",
    );
    assert_sql(
        &let_("nickname").typed::<Option<String>>().equal_to("Al"),
        "LET $nickname: none | string = 'Al';",
    );
    assert_sql(
        &let_("owner")
            .typed::<RecordId<User>>()
            .equal_to(User::id("alice")),
        "LET $owner: record<user> = user:alice;",
    );
}

#[tokio::test]
async fn a_typed_let_accepts_its_type() {
    let db = db().await;
    let mut results = chain(let_("count").typed::<i64>().equal_to(5))
        .chain(return_(umbriel::Param::new("count")))
        .run(db.clone())
        .await
        .expect("ran");
    let count: Option<i64> = results.take(1).expect("int");
    assert_eq!(count, Some(5));
}

/// 3.3.4 coerces a typed `LET` value to its kind and fails with `SetCoerce` when it cannot
/// (`dbs/executor.rs`).
#[tokio::test]
async fn a_typed_let_rejects_another_type() {
    let db = db().await;
    let refused = let_("count")
        .typed::<i64>()
        .equal_to("five")
        .run(db.clone())
        .await;
    assert!(refused.is_err(), "{refused:?}");
}

/// A model's kind is an object literal type; whether SurrealDB accepts it as written depends on
/// #28 (T01).
#[tokio::test]
async fn a_model_typed_let() {
    let db = db().await;
    let organization = Organization {
        id: Organization::id("acme"),
        name: "Acme Corp".to_owned(),
        ..Default::default()
    };
    let statement = let_("org").typed::<Organization>().equal_to(organization);
    assert_parses(&statement.to_debug_sql());
    statement.run(db.clone()).await.expect("accepted");
}
