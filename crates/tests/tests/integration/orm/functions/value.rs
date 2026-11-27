//! `value::*` (`orm/src/functions/value.rs`): every function and method form run, and the
//! results SurrealDB computes.

use std::collections::BTreeMap;

use umbriel::{Operatable, Param, Render, ValueLike, closure, functions::value};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    let before = BTreeMap::from([("name".to_owned(), "Alice")]);
    let after = BTreeMap::from([("name".to_owned(), "Bob")]);
    holds_all!(
        &db,
        [
            (value::chain(2, closure(["n"], |[n]| n.multiply(3))), "6"),
            (
                value::expect!(5, closure(["n"], |[n]| n.greater_than(1))),
                "5"
            ),
            (
                value::patch(before.clone(), value::diff(before.clone(), after)),
                "{ name: 'Bob' }",
            ),
            (
                ValueLike::from(2).chain(closure(["n"], |[n]| n.add(1))),
                "3"
            ),
        ]
    );
    is_a(
        &db,
        &value::diff(before, BTreeMap::<String, &str>::new()),
        "array",
    )
    .await;
}

#[test]
fn generated_bindings() {
    let nested = value::chain(Param::new("score"), closure(["n"], |[n]| n.add(10)));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "value::chain($score, |$n| $n + $_p0)");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(
        &db,
        &value::expect!(0, closure(["n"], |[n]| n.greater_than(1)), "too low"),
    )
    .await;
    assert!(error.contains("too low"), "{error}");
}

/// Every mapped method runs on a value and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let before = || BTreeMap::from([("name".to_owned(), "Alice")]);
    let after = || BTreeMap::from([("name".to_owned(), "Bob")]);
    let triple = || closure(["n"], |[n]| n.multiply(3));
    let positive = || closure(["n"], |[n]| n.greater_than(0));
    same_all!(
        &db,
        [
            (
                ValueLike::from(2).chain(triple()),
                value::chain(2, triple())
            ),
            (
                ValueLike::from(before()).diff(after()),
                value::diff(before(), after())
            ),
            (
                ValueLike::from(5).expect(positive(), None::<&str>),
                value::expect!(5, positive())
            ),
            (
                ValueLike::from(before()).patch(value::diff(before(), after())),
                value::patch(before(), value::diff(before(), after())),
            ),
        ]
    );
}
