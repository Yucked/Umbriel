//! RETURN (`orm/src/statements/return_.rs`): result shapes in a chain, and `RETURN … FETCH`.

use std::collections::BTreeMap;

use surrealdb::types::Value;
use umbriel::{
    All, Field, Model, Param, Raw, Runnable, ValueLike, chain,
    statements::{let_, return_, select},
};
use umbriel_tests::*;

/// Each statement of a chain has its own result, `LET` included, and a `RETURN` gives its
/// value as it is.
#[tokio::test]
async fn result_shapes_in_a_chain() {
    let db = db().await;
    let mut results = chain(let_("n").equal_to(2))
        .chain(return_(Field::from(Param::new("n")).multiply(3)))
        .chain(return_(vec![1, 2]))
        .run(db.clone())
        .await
        .expect("ran");
    assert_eq!(results.num_statements(), 3);
    let product: Option<i64> = results.take(1).expect("int");
    assert_eq!(product, Some(6));
    let list: Vec<i64> = results.take(2).expect("array");
    assert_eq!(list, [1, 2]);
}

#[tokio::test]
async fn return_fetch_replaces_links() {
    let db = db().await;
    Raw::new(
        "CREATE user:alice SET name = 'Alice'; \
         CREATE organization:acme SET name = 'Acme Corp', users = [user:alice];",
    )
    .run(db.clone())
    .await
    .expect("seeded");
    let acme = || Field::from_expr(ValueLike::from(Organization::id("acme"))).all();

    let mut results = return_(acme())
        .fetch(Field::new("users"))
        .run(db.clone())
        .await
        .expect("ran");
    let direct: BTreeMap<String, Vec<BTreeMap<String, Value>>> = results.take(0).expect("object");
    assert_eq!(
        direct["users"][0].get("name"),
        Some(&Value::String("Alice".to_owned()))
    );

    let org = let_("org").equal_to(select(All).from_only(Organization::id("acme")));
    let mut results = chain(org.clone())
        .chain(return_(org.get_param()).fetch(Field::new("users")))
        .run(db.clone())
        .await
        .expect("ran");
    let through_let: BTreeMap<String, Vec<BTreeMap<String, Value>>> =
        results.take(1).expect("object");
    assert_eq!(
        through_let["users"][0].get("name"),
        Some(&Value::String("Alice".to_owned()))
    );
}
