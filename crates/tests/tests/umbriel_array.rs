//! `array::*` (`orm/src/functions/array.rs`): every function and method form rendered, and the
//! results SurrealDB computes.

use umbriel::{
    ArrayLike, Field, Operatable, Param, Render, ValueLike, arr, closure, functions::array,
};
use umbriel_tests::*;

#[test]
fn rendering() {
    let goals = Field::new("goals");
    calls!(
        (array::add!(vec![1, 2], 3), "array::add([1, 2], 3)"),
        (array::add!(goals.clone(), 3), "array::add(goals, 3)"),
        (array::all!(vec![1, 2]), "array::all([1, 2])"),
        (array::any!(Param::new("goals")), "array::any($goals)"),
        (array::append!(vec![1, 2], 3), "array::append([1, 2], 3)"),
        (array::at!(vec![1, 2], 1), "array::at([1, 2], 1)"),
        (
            array::boolean_and!(vec![true], vec![false]),
            "array::boolean_and([true], [false])"
        ),
        (
            array::boolean_not!(vec![true]),
            "array::boolean_not([true])"
        ),
        (
            array::boolean_or!(vec![true], vec![false]),
            "array::boolean_or([true], [false])"
        ),
        (
            array::boolean_xor!(vec![true], vec![false]),
            "array::boolean_xor([true], [false])"
        ),
        (
            array::clump!(vec![1, 2, 3, 4], 2),
            "array::clump([1, 2, 3, 4], 2)"
        ),
        (
            array::combine!(vec![1, 2], vec![3]),
            "array::combine([1, 2], [3])"
        ),
        (
            array::complement!(vec![1, 2], vec![2]),
            "array::complement([1, 2], [2])"
        ),
        (array::concat!(vec![1], vec![2]), "array::concat([1], [2])"),
        (
            array::difference!(vec![1, 2], vec![2]),
            "array::difference([1, 2], [2])"
        ),
        (array::distinct!(vec![1, 1]), "array::distinct([1, 1])"),
        (array::every!(goals.clone()), "array::every(goals)"),
        (array::fill!(goals.clone(), 0), "array::fill(goals, 0)"),
        (
            array::fill!(goals.clone(), 0, 1, 2),
            "array::fill(goals, 0, 1, 2)"
        ),
        (array::filter!(goals.clone(), 1), "array::filter(goals, 1)"),
        (
            array::filter_index!(vec![1, 2, 1], 1),
            "array::filter_index([1, 2, 1], 1)"
        ),
        (array::find!(vec![1, 2], 2), "array::find([1, 2], 2)"),
        (
            array::find_index!(vec![1, 2], 2),
            "array::find_index([1, 2], 2)"
        ),
        (array::first!(vec![1, 2]), "array::first([1, 2])"),
        (
            array::flatten!(vec![vec![1], vec![2, 3]]),
            "array::flatten([[1], [2, 3]])"
        ),
        (
            array::fold!(goals.clone(), 0, closure(["a", "b"], |[a, b]| a.add(b))),
            "array::fold(goals, 0, |$a, $b| $a + $b)",
        ),
        (
            array::group!(vec![vec![1], vec![1, 2]]),
            "array::group([[1], [1, 2]])"
        ),
        (
            array::includes!(goals.clone(), 1),
            "array::includes(goals, 1)"
        ),
        (
            array::index_of!(goals.clone(), 1),
            "array::index_of(goals, 1)"
        ),
        (
            array::insert!(vec![1, 2], 5, 1),
            "array::insert([1, 2], 5, 1)"
        ),
        (array::insert!(vec![1, 2], 5), "array::insert([1, 2], 5)"),
        (
            array::intersect!(vec![1, 2], vec![2]),
            "array::intersect([1, 2], [2])"
        ),
        (array::is_empty!(goals.clone()), "array::is_empty(goals)"),
        (
            array::join!(vec!["a", "b"], "-"),
            "array::join(['a', 'b'], '-')"
        ),
        (array::last!(vec![1, 2]), "array::last([1, 2])"),
        (array::len!(goals.clone()), "array::len(goals)"),
        (
            array::logical_and!(vec![true], vec![1]),
            "array::logical_and([true], [1])"
        ),
        (
            array::logical_or!(vec![true], vec![1]),
            "array::logical_or([true], [1])"
        ),
        (
            array::logical_xor!(vec![true], vec![1]),
            "array::logical_xor([true], [1])"
        ),
        (
            array::map!(goals.clone(), closure(["n"], |[n]| n.add(1))),
            "array::map(goals, |$n| $n + 1)",
        ),
        (array::matches!(vec![1, 2], 1), "array::matches([1, 2], 1)"),
        (array::max!(vec![1, 2]), "array::max([1, 2])"),
        (array::min!(vec![1, 2]), "array::min([1, 2])"),
        (array::pop!(vec![1, 2]), "array::pop([1, 2])"),
        (array::prepend!(vec![1], 0), "array::prepend([1], 0)"),
        (array::push!(vec![1], 2), "array::push([1], 2)"),
        (array::range!(1, 4), "array::range(1, 4)"),
        (array::range!(Field::new("span")), "array::range(span)"),
        (
            array::reduce!(goals.clone(), closure(["a", "b"], |[a, b]| a.add(b))),
            "array::reduce(goals, |$a, $b| $a + $b)",
        ),
        (array::remove!(vec![1, 2], 0), "array::remove([1, 2], 0)"),
        (array::repeat!("a", 3), "array::repeat('a', 3)"),
        (array::reverse!(vec![1, 2]), "array::reverse([1, 2])"),
        (array::sequence!(3), "array::sequence(3)"),
        (array::sequence!(1, 3), "array::sequence(1, 3)"),
        (array::shuffle!(goals.clone()), "array::shuffle(goals)"),
        (
            array::slice!(vec![1, 2, 3], 1, 2),
            "array::slice([1, 2, 3], 1, 2)"
        ),
        (array::some!(goals.clone(), 1), "array::some(goals, 1)"),
        (array::sort!(vec![2, 1]), "array::sort([2, 1])"),
        (
            array::sort!(vec![2, 1], "asc"),
            "array::sort([2, 1], 'asc')"
        ),
        (
            array::sort!(vec![2, 1], "desc"),
            "array::sort([2, 1], 'desc')"
        ),
        (
            array::sort!(vec![2, 1], false),
            "array::sort([2, 1], false)"
        ),
        (
            array::sort_lexical!(goals.clone()),
            "array::sort_lexical(goals)"
        ),
        (
            array::sort_natural!(goals.clone(), "desc"),
            "array::sort_natural(goals, 'desc')"
        ),
        (
            array::sort_natural_lexical!(goals.clone()),
            "array::sort_natural_lexical(goals)"
        ),
        (array::sort::asc!(vec![2, 1]), "array::sort::asc([2, 1])"),
        (array::sort::desc!(vec![2, 1]), "array::sort::desc([2, 1])"),
        (
            array::swap!(goals.clone(), 0, 1),
            "array::swap(goals, 0, 1)"
        ),
        (
            array::transpose!(vec![vec![1, 2], vec![3, 4]]),
            "array::transpose([[1, 2], [3, 4]])"
        ),
        (array::union!(vec![1], vec![2]), "array::union([1], [2])"),
        (
            array::windows!(goals.clone(), 2),
            "array::windows(goals, 2)"
        ),
        (array::len!(arr![1, goals]), "array::len([1, goals])"),
    );
}

#[test]
fn method_rendering() {
    let tags = ArrayLike::from(Field::new("tags"));
    let other = || Field::new("other");
    let sum = || closure(["a", "b"], |[a, b]| a.add(b));
    calls!(
        (tags.add(1), "tags.add(1)"),
        (tags.all(None::<i64>), "tags.all()"),
        (tags.any(Some(1)), "tags.any(1)"),
        (tags.append(1), "tags.append(1)"),
        (tags.at(0), "tags.at(0)"),
        (tags.boolean_and(other()), "tags.boolean_and(other)"),
        (tags.boolean_not(), "tags.boolean_not()"),
        (tags.boolean_or(other()), "tags.boolean_or(other)"),
        (tags.boolean_xor(other()), "tags.boolean_xor(other)"),
        (tags.clump(2), "tags.clump(2)"),
        (tags.combine(other()), "tags.combine(other)"),
        (tags.complement(other()), "tags.complement(other)"),
        (tags.concat([other()]), "tags.concat(other)"),
        (tags.difference(other()), "tags.difference(other)"),
        (tags.distinct(), "tags.distinct()"),
        (tags.every(None::<i64>), "tags.every()"),
        (tags.fill(0, Some(1), None::<i64>), "tags.fill(0, 1)"),
        (tags.filter(1), "tags.filter(1)"),
        (tags.filter_index(1), "tags.filter_index(1)"),
        (tags.find(1), "tags.find(1)"),
        (tags.find_index(1), "tags.find_index(1)"),
        (tags.first(), "tags.first()"),
        (tags.flatten(), "tags.flatten()"),
        (tags.fold(0, sum()), "tags.fold(0, |$a, $b| $a + $b)"),
        (tags.group(), "tags.group()"),
        (tags.includes(Some(1)), "tags.includes(1)"),
        (tags.index_of(1), "tags.index_of(1)"),
        (tags.insert(1, Some(0)), "tags.insert(1, 0)"),
        (tags.intersect(other()), "tags.intersect(other)"),
        (tags.is_empty(), "tags.is_empty()"),
        (tags.join(","), "tags.join(',')"),
        (tags.last(), "tags.last()"),
        (tags.len(), "tags.len()"),
        (tags.logical_and(other()), "tags.logical_and(other)"),
        (tags.logical_or(other()), "tags.logical_or(other)"),
        (tags.logical_xor(other()), "tags.logical_xor(other)"),
        (tags.map(closure(["n"], |[n]| n)), "tags.map(|$n| $n)"),
        (tags.matches(1), "tags.matches(1)"),
        (tags.max(), "tags.max()"),
        (tags.min(), "tags.min()"),
        (tags.pop(), "tags.pop()"),
        (tags.prepend(1), "tags.prepend(1)"),
        (tags.push(1), "tags.push(1)"),
        (tags.reduce(sum()), "tags.reduce(|$a, $b| $a + $b)"),
        (tags.remove(0), "tags.remove(0)"),
        (tags.reverse(), "tags.reverse()"),
        (tags.shuffle(), "tags.shuffle()"),
        (tags.slice(Some(1), Some(2)), "tags.slice(1, 2)"),
        (tags.some(Some(1)), "tags.some(1)"),
        (tags.sort(Some("desc")), "tags.sort('desc')"),
        (tags.sort_lexical(None::<i64>), "tags.sort_lexical()"),
        (tags.sort_natural(None::<i64>), "tags.sort_natural()"),
        (
            tags.sort_natural_lexical(None::<i64>),
            "tags.sort_natural_lexical()"
        ),
        (tags.sort_asc(), "tags.sort_asc()"),
        (tags.sort_desc(), "tags.sort_desc()"),
        (tags.swap(0, 1), "tags.swap(0, 1)"),
        (tags.transpose(), "tags.transpose()"),
        (tags.union(other()), "tags.union(other)"),
        (tags.windows(2), "tags.windows(2)"),
        (
            ValueLike::from(Field::new("input")).repeat(3),
            "input.repeat(3)"
        ),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(
        &db,
        [
            (array::add!(vec![1, 2], 3), "[1, 2, 3]"),
            (array::add!(vec![1, 2], 2), "[1, 2]"),
            (array::all!(vec![1, 2]), "true"),
            (array::all!(vec![1, 0]), "false"),
            (array::any!(vec![0, 1]), "true"),
            (array::append!(vec![1, 2], 3), "[1, 2, 3]"),
            (array::at!(vec![1, 2, 3], 1), "2"),
            (array::at!(vec![1, 2, 3], -1), "3"),
            (
                array::boolean_and!(vec![true, false], vec![true, true]),
                "[true, false]"
            ),
            (array::boolean_not!(vec![true, false]), "[false, true]"),
            (
                array::boolean_or!(vec![true, false], vec![false, false]),
                "[true, false]"
            ),
            (
                array::boolean_xor!(vec![true, false], vec![true, true]),
                "[false, true]"
            ),
            (array::clump!(vec![1, 2, 3, 4], 2), "[[1, 2], [3, 4]]"),
            (array::combine!(vec![1, 2], vec![3]), "[[1, 3], [2, 3]]"),
            (array::complement!(vec![1, 2, 3], vec![2]), "[1, 3]"),
            (array::concat!(vec![1], vec![2]), "[1, 2]"),
            (array::concat!(vec![1], vec![2], vec![3]), "[1, 2, 3]"),
            (array::difference!(vec![1, 2], vec![2, 3]), "[1, 3]"),
            (array::distinct!(vec![1, 1, 2]), "[1, 2]"),
            (array::every!(vec![1, 2]), "true"),
            (array::fill!(vec![1, 2, 3], 0), "[0, 0, 0]"),
            (array::filter_index!(vec![1, 2, 1], 1), "[0, 2]"),
            (array::find!(vec![1, 2, 3], 2), "2"),
            (array::find_index!(vec![1, 2, 3], 2), "1"),
            (array::first!(vec![1, 2]), "1"),
            (array::flatten!(vec![vec![1], vec![2, 3]]), "[1, 2, 3]"),
            (array::includes!(vec![1, 2], 2), "true"),
            (array::index_of!(vec![1, 2], 2), "1"),
            (array::insert!(vec![1, 2, 3, 4], 5, 2), "[1, 2, 5, 3, 4]"),
            (array::intersect!(vec![1, 2], vec![2, 3]), "[2]"),
            (array::is_empty!(Vec::<i64>::new()), "true"),
            (array::join!(vec!["a", "b"], "-"), "'a-b'"),
            (array::last!(vec![1, 2]), "2"),
            (array::len!(vec![1, 2]), "2"),
            (array::matches!(vec![1, 2, 1], 1), "[true, false, true]"),
            (array::max!(vec![1, 3]), "3"),
            (array::min!(vec![1, 3]), "1"),
            (array::pop!(vec![1, 2]), "2"),
            (array::prepend!(vec![1], 0), "[0, 1]"),
            (array::push!(vec![1], 2), "[1, 2]"),
            (array::range!(1, 4), "[1, 2, 3]"),
            (array::remove!(vec![1, 2, 3], 1), "[1, 3]"),
            (array::repeat!(1, 3), "[1, 1, 1]"),
            (array::reverse!(vec![1, 2]), "[2, 1]"),
            (array::sequence!(3), "[0, 1, 2]"),
            (array::sequence!(1, 3), "[1, 2, 3]"),
            (array::some!(vec![1, 2], 2), "true"),
            (array::sort!(vec![3, 1, 2]), "[1, 2, 3]"),
            (array::sort!(vec![3, 1, 2], "desc"), "[3, 2, 1]"),
            (array::sort_natural!(vec!["a10", "a2"]), "['a2', 'a10']"),
            (array::sort_lexical!(vec!["b", "A", "a"]), "['A', 'a', 'b']"),
            (array::sort::asc!(vec![3, 1, 2]), "[1, 2, 3]"),
            (array::sort::desc!(vec![3, 1, 2]), "[3, 2, 1]"),
            (array::swap!(vec![1, 2, 3], 0, 2), "[3, 2, 1]"),
            (
                array::transpose!(vec![vec![1, 2], vec![3, 4]]),
                "[[1, 3], [2, 4]]"
            ),
            (array::union!(vec![1, 2], vec![2, 3]), "[1, 2, 3]"),
            (array::windows!(vec![1, 2, 3], 2), "[[1, 2], [2, 3]]"),
            (ArrayLike::from(vec![3, 1, 2]).sort_asc(), "[1, 2, 3]"),
            (ValueLike::from(2).repeat(2), "[2, 2]"),
        ]
    );
    for function in [
        array::group!(vec![vec![1], vec![1, 2]]),
        array::logical_and!(vec![true, false], vec![1, 1]),
        array::logical_or!(vec![true, false], vec![1, 1]),
        array::logical_xor!(vec![true, false], vec![1, 1]),
        array::slice!(vec![1, 2, 3, 4, 5], 1, 3),
        array::shuffle!(vec![1, 2, 3]),
    ] {
        is_a(&db, &function, "array").await;
    }
}

#[tokio::test]
async fn generated_bindings() {
    let nested = array::len!(array::push!(Param::new("goals"), "x"));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "array::len(array::push($goals, $_p0))");
    assert_eq!(vars.len(), 1);
    let db = db().await;
    let len: Option<i64> = bound(&db, &array::len!(vec![1, 2, 3])).await;
    assert_eq!(len, Some(3));
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = call_failure(
        &db,
        &array::len!(ArrayLike::from(Field::from_expr(ValueLike::from("text")))),
    )
    .await;
    assert!(error.contains("array::len"), "{error}");
}

/// Every mapped method runs on an array and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let db = db().await;
    let n = || ArrayLike::from(vec![3, 1, 2, 1]);
    let nv = || vec![3, 1, 2, 1];
    let b = || ArrayLike::from(vec![true, false]);
    let bv = || vec![true, false];
    let o = || vec![1, 4];
    let nested = || ArrayLike::from(vec![vec![1, 2], vec![3, 4]]);
    let nestedv = || vec![vec![1, 2], vec![3, 4]];
    let sum = || closure(["a", "b"], |[a, b]| a.add(b));
    let big = || closure(["v"], |[v]| v.gt(1));
    same_all!(
        &db,
        [
            (n().add(5), array::add!(nv(), 5)),
            (n().all(None::<i64>), array::all!(nv())),
            (n().any(Some(2)), array::any!(nv(), 2)),
            (n().append(5), array::append!(nv(), 5)),
            (n().at(1), array::at!(nv(), 1)),
            (
                b().boolean_and(vec![true, true]),
                array::boolean_and!(bv(), vec![true, true])
            ),
            (b().boolean_not(), array::boolean_not!(bv())),
            (
                b().boolean_or(vec![false, false]),
                array::boolean_or!(bv(), vec![false, false])
            ),
            (
                b().boolean_xor(vec![true, true]),
                array::boolean_xor!(bv(), vec![true, true])
            ),
            (n().clump(2), array::clump!(nv(), 2)),
            (n().combine(o()), array::combine!(nv(), o())),
            (n().complement(o()), array::complement!(nv(), o())),
            (n().concat([o()]), array::concat!(nv(), o())),
            (n().difference(o()), array::difference!(nv(), o())),
            (n().distinct(), array::distinct!(nv())),
            (n().every(Some(big())), array::every!(nv(), big())),
            (n().fill(0, Some(1), Some(3)), array::fill!(nv(), 0, 1, 3)),
            (n().filter(big()), array::filter!(nv(), big())),
            (n().filter_index(1), array::filter_index!(nv(), 1)),
            (n().find(big()), array::find!(nv(), big())),
            (n().find_index(2), array::find_index!(nv(), 2)),
            (n().first(), array::first!(nv())),
            (nested().flatten(), array::flatten!(nestedv())),
            (n().fold(10, sum()), array::fold!(nv(), 10, sum())),
            (nested().group(), array::group!(nestedv())),
            (n().includes(Some(2)), array::includes!(nv(), 2)),
            (n().index_of(2), array::index_of!(nv(), 2)),
            (n().insert(9, Some(1)), array::insert!(nv(), 9, 1)),
            (n().intersect(o()), array::intersect!(nv(), o())),
            (n().is_empty(), array::is_empty!(nv())),
            (n().join("-"), array::join!(nv(), "-")),
            (n().last(), array::last!(nv())),
            (n().len(), array::len!(nv())),
            (
                b().logical_and(vec![1, 1]),
                array::logical_and!(bv(), vec![1, 1])
            ),
            (
                b().logical_or(vec![1, 1]),
                array::logical_or!(bv(), vec![1, 1])
            ),
            (
                b().logical_xor(vec![1, 1]),
                array::logical_xor!(bv(), vec![1, 1])
            ),
            (
                n().map(closure(["v"], |[v]| v.multiply(2))),
                array::map!(nv(), closure(["v"], |[v]| v.multiply(2)))
            ),
            (n().matches(1), array::matches!(nv(), 1)),
            (n().max(), array::max!(nv())),
            (n().min(), array::min!(nv())),
            (n().pop(), array::pop!(nv())),
            (n().prepend(0), array::prepend!(nv(), 0)),
            (n().push(0), array::push!(nv(), 0)),
            (n().reduce(sum()), array::reduce!(nv(), sum())),
            (n().remove(0), array::remove!(nv(), 0)),
            (n().reverse(), array::reverse!(nv())),
            (n().slice(Some(1), Some(3)), array::slice!(nv(), 1, 3)),
            (n().some(Some(2)), array::some!(nv(), 2)),
            (n().sort(Some("desc")), array::sort!(nv(), "desc")),
            (n().sort_lexical(None::<i64>), array::sort_lexical!(nv())),
            (n().sort_natural(None::<i64>), array::sort_natural!(nv())),
            (
                n().sort_natural_lexical(None::<i64>),
                array::sort_natural_lexical!(nv())
            ),
            (n().sort_asc(), array::sort::asc!(nv())),
            (n().sort_desc(), array::sort::desc!(nv())),
            (n().swap(0, 1), array::swap!(nv(), 0, 1)),
            (nested().transpose(), array::transpose!(nestedv())),
            (n().union(o()), array::union!(nv(), o())),
            (n().windows(2), array::windows!(nv(), 2)),
        ]
    );
    // Random order, so only the kind and the length are compared.
    is_a(&db, &n().shuffle(), "array").await;
    holds(&db, &array::len!(n().shuffle()), "4").await;
}
