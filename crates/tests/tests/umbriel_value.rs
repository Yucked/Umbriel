//! `value::*` (`orm/src/functions/value.rs`): every function and method form rendered, and the
//! results SurrealDB computes.

use std::collections::BTreeMap;

use umbriel::{Field, Operatable, Param, Render, ValueLike, closure, functions::value};
use umbriel_tests::*;

#[test]
fn rendering() {
    let score = Field::new("score");
    let double = || closure(["n"], |[n]| n.multiply(2));
    calls!(
        (value::chain!(score.clone(), double()), "value::chain(score, |$n| $n * 2)"),
        (value::diff!(score.clone(), 1), "value::diff(score, 1)"),
        (value::expect!(score.clone(), double()), "value::expect(score, |$n| $n * 2)"),
        (value::expect!(score.clone(), double(), "too low"), "value::expect(score, |$n| $n * 2, 'too low')"),
        (value::patch!(score.clone(), Field::new("changes")), "value::patch(score, changes)"),
    );
    let score = ValueLike::from(Field::new("score"));
    calls!(
        (score.chain(double()), "score.chain(|$n| $n * 2)"),
        (score.diff(1), "score.diff(1)"),
        (score.expect(double(), None::<&str>), "score.expect(|$n| $n * 2)"),
        (score.patch(Field::new("changes")), "score.patch(changes)"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    let before = BTreeMap::from([("name".to_owned(), "Alice")]);
    let after = BTreeMap::from([("name".to_owned(), "Bob")]);
    holds_all!(&db, [
        (value::chain!(2, closure(["n"], |[n]| n.multiply(3))), "6"),
        (value::expect!(5, closure(["n"], |[n]| n.gt(1))), "5"),
        (
            value::patch!(before.clone(), value::diff!(before.clone(), after)),
            "{ name: 'Bob' }",
        ),
        (ValueLike::from(2).chain(closure(["n"], |[n]| n.add(1))), "3"),
    ]);
    is_a(&db, &value::diff!(before, BTreeMap::<String, &str>::new()), "array").await;
}

#[test]
fn generated_bindings() {
    let nested = value::chain!(Param::new("score"), closure(["n"], |[n]| n.add(10)));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "value::chain($score, |$n| $n + $_p0)");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = call_failure(&db, &value::expect!(0, closure(["n"], |[n]| n.gt(1)), "too low")).await;
    assert!(error.contains("too low"), "{error}");
}
