//! `array::*` (`orm/src/functions/array.rs`): every function and method form run, and the
//! results SurrealDB computes.

use umbriel::{ArrayLike, Field, Operatable, Param, Render, ValueLike, closure, functions::array};
use umbriel_tests::*;

/// The forms no server test calls.
#[test]
fn rendering() {
    calls!((array::range!(Field::new("span")), "array::range(span)"));
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (array::add(vec![1, 2], 3), "[1, 2, 3]"),
            (array::add(vec![1, 2], 2), "[1, 2]"),
            (array::all!(vec![1, 2]), "true"),
            (array::all!(vec![1, 0]), "false"),
            (array::any!(vec![0, 1]), "true"),
            (array::append(vec![1, 2], 3), "[1, 2, 3]"),
            (array::at(vec![1, 2, 3], 1), "2"),
            (array::at(vec![1, 2, 3], -1), "3"),
            (
                array::boolean_and(vec![true, false], vec![true, true]),
                "[true, false]"
            ),
            (array::boolean_not(vec![true, false]), "[false, true]"),
            (
                array::boolean_or(vec![true, false], vec![false, false]),
                "[true, false]"
            ),
            (
                array::boolean_xor(vec![true, false], vec![true, true]),
                "[false, true]"
            ),
            (array::clump(vec![1, 2, 3, 4], 2), "[[1, 2], [3, 4]]"),
            (array::combine(vec![1, 2], vec![3]), "[[1, 3], [2, 3]]"),
            (array::complement(vec![1, 2, 3], vec![2]), "[1, 3]"),
            (array::concat!(vec![1], vec![2]), "[1, 2]"),
            (array::concat!(vec![1], vec![2], vec![3]), "[1, 2, 3]"),
            (array::difference(vec![1, 2], vec![2, 3]), "[1, 3]"),
            (array::distinct(vec![1, 1, 2]), "[1, 2]"),
            (array::every!(vec![1, 2]), "true"),
            (array::fill!(vec![1, 2, 3], 0), "[0, 0, 0]"),
            (array::filter_index(vec![1, 2, 1], 1), "[0, 2]"),
            (array::find(vec![1, 2, 3], 2), "2"),
            (array::find_index(vec![1, 2, 3], 2), "1"),
            (array::first(vec![1, 2]), "1"),
            (array::flatten(vec![vec![1], vec![2, 3]]), "[1, 2, 3]"),
            (array::includes!(vec![1, 2], 2), "true"),
            (array::index_of(vec![1, 2], 2), "1"),
            (array::insert!(vec![1, 2, 3, 4], 5, 2), "[1, 2, 5, 3, 4]"),
            (array::intersect(vec![1, 2], vec![2, 3]), "[2]"),
            (array::is_empty(Vec::<i64>::new()), "true"),
            (array::join(vec!["a", "b"], "-"), "'a-b'"),
            (array::last(vec![1, 2]), "2"),
            (array::len(vec![1, 2]), "2"),
            (array::matches(vec![1, 2, 1], 1), "[true, false, true]"),
            (array::max(vec![1, 3]), "3"),
            (array::min(vec![1, 3]), "1"),
            (array::pop(vec![1, 2]), "2"),
            (array::prepend(vec![1], 0), "[0, 1]"),
            (array::push(vec![1], 2), "[1, 2]"),
            (array::range!(1, 4), "[1, 2, 3]"),
            (array::remove(vec![1, 2, 3], 1), "[1, 3]"),
            (array::repeat(1, 3), "[1, 1, 1]"),
            (array::reverse(vec![1, 2]), "[2, 1]"),
            (array::sequence!(3), "[0, 1, 2]"),
            (array::sequence!(1, 3), "[1, 2, 3]"),
            (array::some!(vec![1, 2], 2), "true"),
            (array::sort!(vec![3, 1, 2]), "[1, 2, 3]"),
            (array::sort!(vec![3, 1, 2], "desc"), "[3, 2, 1]"),
            (array::sort_natural!(vec!["a10", "a2"]), "['a2', 'a10']"),
            (array::sort_lexical!(vec!["b", "A", "a"]), "['A', 'a', 'b']"),
            (array::sort::asc(vec![3, 1, 2]), "[1, 2, 3]"),
            (array::sort::desc(vec![3, 1, 2]), "[3, 2, 1]"),
            (array::swap(vec![1, 2, 3], 0, 2), "[3, 2, 1]"),
            (
                array::transpose(vec![vec![1, 2], vec![3, 4]]),
                "[[1, 3], [2, 4]]"
            ),
            (array::union(vec![1, 2], vec![2, 3]), "[1, 2, 3]"),
            (array::windows(vec![1, 2, 3], 2), "[[1, 2], [2, 3]]"),
            (ArrayLike::from(vec![3, 1, 2]).sort_asc(), "[1, 2, 3]"),
            (ValueLike::from(2).repeat(2), "[2, 2]"),
        ]
    );
    for function in [
        array::group(vec![vec![1], vec![1, 2]]),
        array::logical_and(vec![true, false], vec![1, 1]),
        array::logical_or(vec![true, false], vec![1, 1]),
        array::logical_xor(vec![true, false], vec![1, 1]),
        array::slice!(vec![1, 2, 3, 4, 5], 1, 3),
        array::shuffle(vec![1, 2, 3]),
    ] {
        is_a(&db, &function, "array").await;
    }
}

#[tokio::test]
async fn generated_bindings() {
    let nested = array::len(array::push(Param::new("goals"), "x"));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "array::len(array::push($goals, $_p0))");
    assert_eq!(vars.len(), 1);
    let (db, _guard) = db().await;
    let len: Option<i64> = bound(&db, &array::len(vec![1, 2, 3])).await;
    assert_eq!(len, Some(3));
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(
        &db,
        &array::len(ArrayLike::from(Field::from_expr(ValueLike::from("text")))),
    )
    .await;
    assert!(error.contains("array::len"), "{error}");
}

/// Every mapped method runs on an array and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let n = || ArrayLike::from(vec![3, 1, 2, 1]);
    let nv = || vec![3, 1, 2, 1];
    let b = || ArrayLike::from(vec![true, false]);
    let bv = || vec![true, false];
    let o = || vec![1, 4];
    let nested = || ArrayLike::from(vec![vec![1, 2], vec![3, 4]]);
    let nestedv = || vec![vec![1, 2], vec![3, 4]];
    let sum = || closure(["a", "b"], |[a, b]| a.add(b));
    let big = || closure(["v"], |[v]| v.greater_than(1));
    same_all!(
        &db,
        [
            (n().add(5), array::add(nv(), 5)),
            (n().all(None::<i64>), array::all!(nv())),
            (n().any(Some(2)), array::any!(nv(), 2)),
            (n().append(5), array::append(nv(), 5)),
            (n().at(1), array::at(nv(), 1)),
            (
                b().boolean_and(vec![true, true]),
                array::boolean_and(bv(), vec![true, true])
            ),
            (b().boolean_not(), array::boolean_not(bv())),
            (
                b().boolean_or(vec![false, false]),
                array::boolean_or(bv(), vec![false, false])
            ),
            (
                b().boolean_xor(vec![true, true]),
                array::boolean_xor(bv(), vec![true, true])
            ),
            (n().clump(2), array::clump(nv(), 2)),
            (n().combine(o()), array::combine(nv(), o())),
            (n().complement(o()), array::complement(nv(), o())),
            (n().concat([o()]), array::concat!(nv(), o())),
            (n().difference(o()), array::difference(nv(), o())),
            (n().distinct(), array::distinct(nv())),
            (n().every(Some(big())), array::every!(nv(), big())),
            (n().fill(0, Some(1), Some(3)), array::fill!(nv(), 0, 1, 3)),
            (n().filter(big()), array::filter(nv(), big())),
            (n().filter_index(1), array::filter_index(nv(), 1)),
            (n().find(big()), array::find(nv(), big())),
            (n().find_index(2), array::find_index(nv(), 2)),
            (n().first(), array::first(nv())),
            (nested().flatten(), array::flatten(nestedv())),
            (n().fold(10, sum()), array::fold(nv(), 10, sum())),
            (nested().group(), array::group(nestedv())),
            (n().includes(Some(2)), array::includes!(nv(), 2)),
            (n().index_of(2), array::index_of(nv(), 2)),
            (n().insert(9, Some(1)), array::insert!(nv(), 9, 1)),
            (n().intersect(o()), array::intersect(nv(), o())),
            (n().is_empty(), array::is_empty(nv())),
            (n().join("-"), array::join(nv(), "-")),
            (n().last(), array::last(nv())),
            (n().len(), array::len(nv())),
            (
                b().logical_and(vec![1, 1]),
                array::logical_and(bv(), vec![1, 1])
            ),
            (
                b().logical_or(vec![1, 1]),
                array::logical_or(bv(), vec![1, 1])
            ),
            (
                b().logical_xor(vec![1, 1]),
                array::logical_xor(bv(), vec![1, 1])
            ),
            (
                n().map(closure(["v"], |[v]| v.multiply(2))),
                array::map(nv(), closure(["v"], |[v]| v.multiply(2)))
            ),
            (n().matches(1), array::matches(nv(), 1)),
            (n().max(), array::max(nv())),
            (n().min(), array::min(nv())),
            (n().pop(), array::pop(nv())),
            (n().prepend(0), array::prepend(nv(), 0)),
            (n().push(0), array::push(nv(), 0)),
            (n().reduce(sum()), array::reduce(nv(), sum())),
            (n().remove(0), array::remove(nv(), 0)),
            (n().reverse(), array::reverse(nv())),
            (n().slice(Some(1), Some(3)), array::slice!(nv(), 1, 3)),
            (n().some(Some(2)), array::some!(nv(), 2)),
            (n().sort(Some("desc")), array::sort!(nv(), "desc")),
            (n().sort_lexical(None::<i64>), array::sort_lexical!(nv())),
            (n().sort_natural(None::<i64>), array::sort_natural!(nv())),
            (
                n().sort_natural_lexical(None::<i64>),
                array::sort_natural_lexical!(nv())
            ),
            (n().sort_asc(), array::sort::asc(nv())),
            (n().sort_desc(), array::sort::desc(nv())),
            (n().swap(0, 1), array::swap(nv(), 0, 1)),
            (nested().transpose(), array::transpose(nestedv())),
            (n().union(o()), array::union(nv(), o())),
            (n().windows(2), array::windows(nv(), 2)),
        ]
    );
    // Random order, so only the kind and the length are compared.
    is_a(&db, &n().shuffle(), "array").await;
    holds(&db, &array::len(n().shuffle()), "4").await;
}
