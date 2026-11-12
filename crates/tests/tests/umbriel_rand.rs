//! `rand()` and `rand::*` (`orm/src/functions/rand.rs`): every function rendered with and without
//! its optional arguments, and the kinds of the values SurrealDB generates.

use std::time::Duration;

use umbriel::{Render, functions::{rand as rand_fn, string}};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (rand_fn::rand!(), "rand()"),
        (rand_fn::bool!(), "rand::bool()"),
        (
            rand_fn::duration!(Duration::from_secs(1), Duration::from_secs(60)),
            "rand::duration(1s, 1m)",
        ),
        (rand_fn::enum_!("a", "b"), "rand::enum('a', 'b')"),
        (rand_fn::float!(), "rand::float()"),
        (rand_fn::float!(1, 2), "rand::float(1, 2)"),
        (rand_fn::int!(), "rand::int()"),
        (rand_fn::int!(1, 10), "rand::int(1, 10)"),
        (rand_fn::string!(), "rand::string()"),
        (rand_fn::string!(8), "rand::string(8)"),
        (rand_fn::string!(4, 8), "rand::string(4, 8)"),
        (rand_fn::time!(), "rand::time()"),
        (rand_fn::time!(0, 1_000), "rand::time(0, 1000)"),
        (rand_fn::ulid!(), "rand::ulid()"),
        (rand_fn::ulid!(datetime()), &format!("rand::ulid({})", umbriel::ValueLike::from(datetime()).to_debug_sql())),
        (rand_fn::uuid!(), "rand::uuid()"),
        (rand_fn::uuid::v4!(), "rand::uuid::v4()"),
        (rand_fn::uuid::v7!(), "rand::uuid::v7()"),
        (rand_fn::id!(), "rand::id()"),
        (rand_fn::id!(10), "rand::id(10)"),
        (rand_fn::id!(5, 10), "rand::id(5, 10)"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    is_each!(&db, [
        (rand_fn::rand!(), "float"),
        (rand_fn::bool!(), "bool"),
        (rand_fn::duration!(Duration::from_secs(1), Duration::from_secs(60)), "duration"),
        (rand_fn::float!(1, 2), "float"),
        (rand_fn::int!(1, 10), "int"),
        (rand_fn::string!(8), "string"),
        (rand_fn::time!(), "datetime"),
        (rand_fn::ulid!(), "string"),
        (rand_fn::uuid!(), "uuid"),
        (rand_fn::uuid::v4!(), "uuid"),
        (rand_fn::uuid::v7!(), "uuid"),
        (rand_fn::uuid::v7!(datetime()), "uuid"),
        (rand_fn::id!(), "string"),
    ]);
    holds(&db, &string::len!(rand_fn::string!(8)), "8").await;
    holds(&db, &string::len!(rand_fn::id!(10)), "10").await;
    let picked: String = read(
        &db,
        &format!("RETURN {};", rand_fn::enum_!("a", "b").to_debug_sql()),
    )
    .await;
    assert!(["a", "b"].contains(&picked.as_str()));
    let rolled: i64 = read(&db, &format!("RETURN {};", rand_fn::int!(1, 6).to_debug_sql())).await;
    assert!((1..=6).contains(&rolled), "{rolled}");
}

#[test]
fn generated_bindings() {
    let nested = string::len!(rand_fn::string!(8));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "string::len(rand::string($_p0))");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = call_failure(&db, &rand_fn::int!(10, 1)).await;
    assert!(error.contains("rand::int"), "{error}");
}
