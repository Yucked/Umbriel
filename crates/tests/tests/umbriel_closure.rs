//! Closures (`orm/src/query/closure.rs`): rendering, the closure-taking functions on the engine,
//! a closure called through a parameter, and the names refused before anything runs.

use std::collections::BTreeSet;

use umbriel::{
    Operatable, Render, Runnable, chain, closure,
    functions::{array, set},
    statements::{let_, return_},
    types::Kind,
};
use umbriel_tests::*;

#[test]
fn closures_render() {
    let doubled = closure(["n"], |[n]| n.multiply(2));
    assert_eq!(doubled.to_debug_sql(), "|$n| $n * 2");
    assert_eq!(
        doubled.clone().typed([("n", Kind::Int)]).to_debug_sql(),
        "|$n: int| $n * 2"
    );
    assert_eq!(
        doubled
            .typed([("n", Kind::Int)])
            .returns(Kind::Int)
            .to_debug_sql(),
        "|$n: int| -> int { $n * 2 }"
    );
    let summed = closure(["total", "n"], |[total, n]| total.add(n))
        .typed([("n", Kind::Either(vec![Kind::Int, Kind::Float]))]);
    assert_eq!(
        summed.to_debug_sql(),
        "|$total, $n: <int | float>| $total + $n"
    );
    for closure in [
        "|$n| $n * 2",
        "|$n: int| $n * 2",
        "|$n: int| -> int { $n * 2 }",
        "|$total, $n: <int | float>| $total + $n",
    ] {
        assert_parses(&format!("RETURN {closure};"));
    }
}

#[tokio::test]
async fn array_map_filter_fold_run() {
    let db = db().await;
    let numbers = vec![1, 2, 3];
    holds_all!(
        &db,
        [
            (
                array::map!(numbers.clone(), closure(["n"], |[n]| n.multiply(2))),
                "[2, 4, 6]"
            ),
            (
                array::filter!(numbers.clone(), closure(["n"], |[n]| n.gt(1))),
                "[2, 3]"
            ),
            (
                array::fold!(
                    numbers.clone(),
                    0,
                    closure(["total", "n"], |[total, n]| total.add(n))
                ),
                "6",
            ),
            (
                array::reduce!(
                    numbers.clone(),
                    closure(["total", "n"], |[total, n]| total.add(n))
                ),
                "6",
            ),
            (
                array::find!(numbers.clone(), closure(["n"], |[n]| n.gt(1))),
                "2"
            ),
            (
                array::all!(numbers.clone(), closure(["n"], |[n]| n.gt(0))),
                "true"
            ),
            (array::any!(numbers, closure(["n"], |[n]| n.gt(2))), "true"),
            (
                set::map!(BTreeSet::from([1, 2]), closure(["n"], |[n]| n.add(1))),
                "<set> [2, 3]",
            ),
        ]
    );
}

#[tokio::test]
async fn a_closure_runs_through_a_parameter() {
    let db = db().await;
    let double = let_("double").equal_to(closure(["n"], |[n]| n.multiply(2)));
    let call = double.get_param().call([3]);
    assert_eq!(call.to_debug_sql(), "$double(3)");
    let result: Option<i64> = chain(double)
        .chain(return_(call))
        .run(&db)
        .await
        .expect("ran")
        .take(1)
        .expect("decodes");
    assert_eq!(result, Some(6));
}

#[test]
fn invalid_parameter_names_are_render_errors() {
    for name in ["", "1n", "a-b", "a b", "$n", "_p0"] {
        let invalid = closure([name], |[n]| n);
        assert_render_error(&invalid, "invalid closure parameter");
    }
    assert_render_error(
        &closure(["n", "n"], |[a, b]| a.add(b)),
        "closure parameter `$n` is repeated",
    );
    assert_render_error(
        &closure(["n"], |[n]| n).typed([("m", Kind::Int)]),
        "closure kind given for `m`",
    );
}

#[tokio::test]
async fn closures_cannot_be_stored() {
    let db = db().await;
    let error = failure(
        &db,
        &format!(
            "CREATE thing:alice SET double = {};",
            closure(["n"], |[n]| n.multiply(2)).to_debug_sql()
        ),
    )
    .await;
    assert!(!error.is_empty());
    let stored: Option<umbriel::types::Value> = read(&db, "RETURN thing:alice.double;").await;
    assert_eq!(stored, None, "nothing is stored");
}
