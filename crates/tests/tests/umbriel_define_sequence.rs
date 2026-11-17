//! Sequences (`orm/src/statements/define_sequence.rs`, and `alter_sequence`/`remove_sequence`):
//! rendering, checked with SurrealDB's parser, and their effect on a database.

use std::time::Duration;

use surrealdb::types::{Object, Value};
use umbriel::{
    Param, Runnable,
    statements::{alter_sequence, define_sequence, remove_sequence},
};
use umbriel_tests::*;

#[test]
fn a_definition_writes_its_clauses_in_grammar_order() {
    assert_sql(
        &define_sequence("order_number"),
        "DEFINE SEQUENCE order_number;",
    );
    assert_sql(
        &define_sequence("order_number")
            .if_not_exists()
            .batch(100)
            .start(-5)
            .timeout(Duration::from_secs(5)),
        "DEFINE SEQUENCE IF NOT EXISTS order_number BATCH 100 START -5 TIMEOUT 5s;",
    );
    assert_sql(
        &define_sequence(Param::new("name")).overwrite().start(1),
        "DEFINE SEQUENCE OVERWRITE $name START 1;",
    );
}

#[test]
fn alter_sets_or_clears_the_timeout() {
    assert_sql(
        &alter_sequence("order_number").timeout(Duration::from_millis(1500)),
        "ALTER SEQUENCE order_number TIMEOUT 1s500ms;",
    );
    assert_sql(
        &alter_sequence("order_number").if_exists().drop_timeout(),
        "ALTER SEQUENCE IF EXISTS order_number TIMEOUT NONE;",
    );
    assert_sql(
        &alter_sequence("order_number"),
        "ALTER SEQUENCE order_number;",
    );
    assert_render_error(
        &alter_sequence("order_number")
            .timeout(Duration::from_secs(1))
            .drop_timeout(),
        "both set and drop TIMEOUT",
    );
}

#[test]
fn remove_takes_if_exists() {
    assert_sql(
        &remove_sequence("order_number").if_exists(),
        "REMOVE SEQUENCE IF EXISTS order_number;",
    );
}

/// The sequence's `INFO FOR DB STRUCTURE` entry.
async fn structure(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) -> Object {
    let info: Object = read(db, "INFO FOR DB STRUCTURE;").await;
    let Some(Value::Array(sequences)) = info.get("sequences") else {
        panic!("`sequences` is listed: {info:?}");
    };
    match sequences.first() {
        Some(Value::Object(sequence)) => sequence.clone(),
        other => panic!("one sequence is defined: {other:?}"),
    }
}

#[tokio::test]
async fn a_sequence_issues_values_and_alter_keeps_them() {
    let db = db().await;
    define_sequence("order_number")
        .batch(10)
        .start(100)
        .run(&db)
        .await
        .expect("defined");
    let first: i64 = read(&db, "RETURN sequence::nextval('order_number');").await;
    let second: i64 = read(&db, "RETURN sequence::nextval('order_number');").await;
    assert_eq!(first, 100);
    assert!(second > first);

    alter_sequence("order_number")
        .timeout(Duration::from_secs(5))
        .run(&db)
        .await
        .expect("altered");
    assert!(matches!(
        structure(&db).await.get("timeout"),
        Some(Value::Duration(_))
    ));
    let third: i64 = read(&db, "RETURN sequence::nextval('order_number');").await;
    assert!(third > second, "{third} follows {second}");

    alter_sequence("order_number")
        .drop_timeout()
        .run(&db)
        .await
        .expect("altered");
    assert!(matches!(
        structure(&db).await.get("timeout"),
        None | Some(Value::None)
    ));

    remove_sequence("order_number")
        .run(&db)
        .await
        .expect("removed");
    assert!(!succeeds(&db, "RETURN sequence::nextval('order_number');").await);
}

#[tokio::test]
async fn defining_an_existing_sequence_without_overwrite_fails() {
    let db = db().await;
    define_sequence("order_number")
        .run(&db)
        .await
        .expect("defined");
    let error = define_sequence("order_number")
        .run(&db)
        .await
        .expect_err("it exists")
        .to_string();
    assert!(error.contains("already exists"), "{error}");
}
