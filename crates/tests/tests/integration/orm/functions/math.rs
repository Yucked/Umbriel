//! `math::*` (`orm/src/functions/math.rs`): every function and method form run, and the
//! results SurrealDB computes.

use umbriel::{
    Field, NumberLike, Param, Render, ValueLike,
    functions::{array, math},
};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (math::abs(-5), "5"),
            (array::sort!(math::bottom(vec![3, 1, 2], 2)), "[1, 2]"),
            (math::ceil(1.2), "2"),
            (math::clamp(15, 0, 10), "10"),
            (math::fixed(1.2345, 2), "1.23"),
            (math::floor(1.8), "1"),
            (math::lerp(0, 10, 0.5), "5"),
            (math::log10(100), "2"),
            (math::log2(8), "3"),
            (math::max(vec![1, 4, 2]), "4"),
            (math::mean(vec![1, 2, 3]), "2"),
            (math::median(vec![1, 5, 3]), "3"),
            (math::min(vec![4, 1, 2]), "1"),
            (math::mode(vec![1, 2, 2]), "2"),
            (math::pow(2, 3), "8"),
            (math::product(vec![2, 3, 4]), "24"),
            (math::round(1.5), "2"),
            (math::sign(-3), "-1"),
            (math::spread(vec![1, 5]), "4"),
            (math::sqrt(16), "4"),
            (math::sum(vec![1, 2, 3]), "6"),
            (array::sort!(math::top(vec![3, 1, 2], 2)), "[2, 3]"),
            (NumberLike::from(-5).abs(), "5"),
            (NumberLike::from(2.7).floor(), "2"),
        ]
    );
    for function in [
        math::acos(0.5),
        math::acot(1),
        math::asin(0.5),
        math::atan(1),
        math::cos(0),
        math::cot(1),
        math::deg2rad(180),
        math::interquartile(vec![1, 2, 3, 4]),
        math::lerpangle(0, 90, 0.5),
        math::ln(2),
        math::log(8, 2),
        math::midhinge(vec![1, 2, 3, 4]),
        math::nearestrank(vec![1, 2, 3, 4], 50),
        math::percentile(vec![1, 2, 3, 4], 50),
        math::rad2deg(1),
        math::sin(1),
        math::stddev(vec![1, 2, 3, 4]),
        math::tan(1),
        math::trimean(vec![1, 2, 3, 4]),
        math::variance(vec![1, 2, 3, 4]),
    ] {
        is_a(&db, &function, "number").await;
    }
}

#[test]
fn generated_bindings() {
    let nested = math::round(math::pow(Param::new("base"), 2));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "math::round(math::pow($base, $_p0))");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(
        &db,
        &math::abs(NumberLike::from(Field::from_expr(ValueLike::from("five")))),
    )
    .await;
    assert!(error.contains("math::abs"), "{error}");
}

/// Every mapped method runs on a number and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let x = || NumberLike::from(0.5);
    same_all!(
        &db,
        [
            (NumberLike::from(-5).abs(), math::abs(-5)),
            (x().acos(), math::acos(0.5)),
            (NumberLike::from(2).acot(), math::acot(2)),
            (x().asin(), math::asin(0.5)),
            (x().atan(), math::atan(0.5)),
            (NumberLike::from(1.2).ceil(), math::ceil(1.2)),
            (x().cos(), math::cos(0.5)),
            (x().cot(), math::cot(0.5)),
            (NumberLike::from(180).deg2rad(), math::deg2rad(180)),
            (NumberLike::from(1.8).floor(), math::floor(1.8)),
            (NumberLike::from(2).ln(), math::ln(2)),
            (NumberLike::from(8).log(2), math::log(8, 2)),
            (NumberLike::from(100).log10(), math::log10(100)),
            (NumberLike::from(8).log2(), math::log2(8)),
            (NumberLike::from(1).rad2deg(), math::rad2deg(1)),
            (NumberLike::from(1.5).round(), math::round(1.5)),
            (NumberLike::from(-3).sign(), math::sign(-3)),
            (x().sin(), math::sin(0.5)),
            (x().tan(), math::tan(0.5)),
        ]
    );
}
