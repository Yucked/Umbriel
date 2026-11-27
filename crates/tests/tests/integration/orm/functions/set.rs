//! `set::*` (`orm/src/functions/set.rs`): every function and method form run, the result
//! kinds per function, and the results SurrealDB computes.

use std::collections::BTreeSet;

use umbriel::{
    BoolLike, NumberLike, Operatable, Param, Render, SetLike, StrandLike, closure, functions::set,
};
use umbriel_tests::*;

fn numbers() -> BTreeSet<i64> {
    BTreeSet::from([1, 2, 3])
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    let other = BTreeSet::from([3, 4]);
    holds_all!(
        &db,
        [
            (set::add(numbers(), 4), "<set> [1, 2, 3, 4]"),
            (set::all!(numbers()), "true"),
            (set::any!(numbers(), 2), "true"),
            (set::at(numbers(), 0), "1"),
            (set::complement(numbers(), other.clone()), "<set> [1, 2]"),
            (set::contains(numbers(), 2), "true"),
            (set::difference(numbers(), other.clone()), "<set> [1, 2, 4]"),
            (
                set::filter(numbers(), closure(["n"], |[n]| n.greater_than(1))),
                "<set> [2, 3]"
            ),
            (
                set::find(numbers(), closure(["n"], |[n]| n.greater_than(1))),
                "2"
            ),
            (set::first(numbers()), "1"),
            (
                set::fold(numbers(), 0, closure(["a", "b"], |[a, b]| a.add(b))),
                "6"
            ),
            (set::intersect(numbers(), other.clone()), "<set> [3]"),
            (set::is_empty(BTreeSet::<i64>::new()), "true"),
            (set::join(numbers(), "-"), "'1-2-3'"),
            (set::last(numbers()), "3"),
            (set::len(numbers()), "3"),
            (
                set::map(numbers(), closure(["n"], |[n]| n.multiply(2))),
                "<set> [2, 4, 6]"
            ),
            (set::max(numbers()), "3"),
            (set::min(numbers()), "1"),
            (
                set::reduce(numbers(), closure(["a", "b"], |[a, b]| a.add(b))),
                "6"
            ),
            (set::remove(numbers(), 2), "<set> [1, 3]"),
            (set::slice!(numbers(), 1, 2), "<set> [2]"),
            (set::union(numbers(), other), "<set> [1, 2, 3, 4]"),
        ]
    );
    is_a(&db, &set::flatten(numbers()), "set").await;
}

#[tokio::test]
async fn result_kinds() {
    let (db, _guard) = db().await;
    let len = NumberLike::from(set::len(numbers()));
    let all = BoolLike::from(set::all!(numbers()));
    let joined = StrandLike::from(set::join(numbers(), ","));
    let union = SetLike::from(set::union(numbers(), numbers()));
    for (expression, kind) in [
        (len.to_debug_sql(), "int"),
        (all.to_debug_sql(), "bool"),
        (joined.to_debug_sql(), "string"),
        (union.to_debug_sql(), "set"),
    ] {
        let matches: bool = read(&db, &format!("RETURN type::is_{kind}({expression});")).await;
        assert!(matches, "`{expression}` is a {kind}");
    }
}

#[test]
fn generated_bindings() {
    let nested = set::len(set::add(Param::new("tags"), "x"));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "set::len(set::add($tags, $_p0))");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = failure(&db, "RETURN set::len('text');").await;
    assert!(error.contains("set::len"), "{error}");
}

/// Every mapped method runs on a set and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let s = || SetLike::from(numbers());
    let other = || BTreeSet::from([3, 4]);
    let sum = || closure(["a", "b"], |[a, b]| a.add(b));
    let big = || closure(["v"], |[v]| v.greater_than(1));
    let double = || closure(["v"], |[v]| v.multiply(2));
    same_all!(
        &db,
        [
            (s().add(4), set::add(numbers(), 4)),
            (s().all(None::<i64>), set::all!(numbers())),
            (s().any(Some(2)), set::any!(numbers(), 2)),
            (s().at(1), set::at(numbers(), 1)),
            (s().complement(other()), set::complement(numbers(), other())),
            (s().contains(2), set::contains(numbers(), 2)),
            (s().difference(other()), set::difference(numbers(), other())),
            (s().filter(big()), set::filter(numbers(), big())),
            (s().find(big()), set::find(numbers(), big())),
            (s().first(), set::first(numbers())),
            (s().flatten(), set::flatten(numbers())),
            (s().fold(10, sum()), set::fold(numbers(), 10, sum())),
            (s().intersect(other()), set::intersect(numbers(), other())),
            (s().is_empty(), set::is_empty(numbers())),
            (s().join("-"), set::join(numbers(), "-")),
            (s().last(), set::last(numbers())),
            (s().len(), set::len(numbers())),
            (s().map(double()), set::map(numbers(), double())),
            (s().max(), set::max(numbers())),
            (s().min(), set::min(numbers())),
            (s().reduce(sum()), set::reduce(numbers(), sum())),
            (s().remove(2), set::remove(numbers(), 2)),
            (s().slice(Some(1), Some(3)), set::slice!(numbers(), 1, 3)),
            (s().union(other()), set::union(numbers(), other())),
        ]
    );
}
