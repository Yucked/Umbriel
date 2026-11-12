//! `math::*` (`orm/src/functions/math.rs`): every function and method form rendered, and the
//! results SurrealDB computes.

use umbriel::{
    Field, NumberLike, Param, Render, ValueLike,
    functions::{array, math},
};
use umbriel_tests::*;

#[test]
fn rendering() {
    let score = Field::new("score");
    let scores = Field::new("scores");
    calls!(
        (math::abs!(-5), "math::abs(-5)"),
        (math::abs!(score.clone()), "math::abs(score)"),
        (math::acos!(score.clone()), "math::acos(score)"),
        (math::acot!(score.clone()), "math::acot(score)"),
        (math::asin!(score.clone()), "math::asin(score)"),
        (math::atan!(score.clone()), "math::atan(score)"),
        (math::bottom!(scores.clone(), 2), "math::bottom(scores, 2)"),
        (math::ceil!(1.5), "math::ceil(1.5f)"),
        (math::clamp!(score.clone(), 0, 10), "math::clamp(score, 0, 10)"),
        (math::cos!(score.clone()), "math::cos(score)"),
        (math::cot!(score.clone()), "math::cot(score)"),
        (math::deg2rad!(score.clone()), "math::deg2rad(score)"),
        (math::fixed!(1.2345, 2), "math::fixed(1.2345f, 2)"),
        (math::floor!(1.5), "math::floor(1.5f)"),
        (math::interquartile!(scores.clone()), "math::interquartile(scores)"),
        (math::lerp!(0, 10, 0.5), "math::lerp(0, 10, 0.5f)"),
        (math::lerpangle!(0, 90, 0.5), "math::lerpangle(0, 90, 0.5f)"),
        (math::ln!(score.clone()), "math::ln(score)"),
        (math::log!(score.clone(), 3), "math::log(score, 3)"),
        (math::log10!(score.clone()), "math::log10(score)"),
        (math::log2!(score.clone()), "math::log2(score)"),
        (math::max!(vec![1, 2]), "math::max([1, 2])"),
        (math::mean!(vec![1, 2]), "math::mean([1, 2])"),
        (math::median!(vec![1, 2]), "math::median([1, 2])"),
        (math::midhinge!(scores.clone()), "math::midhinge(scores)"),
        (math::min!(vec![1, 2]), "math::min([1, 2])"),
        (math::mode!(vec![1, 2]), "math::mode([1, 2])"),
        (math::nearestrank!(scores.clone(), 50), "math::nearestrank(scores, 50)"),
        (math::percentile!(scores.clone(), 50), "math::percentile(scores, 50)"),
        (math::pow!(2, 3), "math::pow(2, 3)"),
        (math::product!(vec![1, 2]), "math::product([1, 2])"),
        (math::rad2deg!(score.clone()), "math::rad2deg(score)"),
        (math::round!(1.5), "math::round(1.5f)"),
        (math::sign!(score.clone()), "math::sign(score)"),
        (math::sin!(score.clone()), "math::sin(score)"),
        (math::spread!(scores.clone()), "math::spread(scores)"),
        (math::sqrt!(16), "math::sqrt(16)"),
        (math::stddev!(scores.clone()), "math::stddev(scores)"),
        (math::sum!(Param::new("scores")), "math::sum($scores)"),
        (math::tan!(score), "math::tan(score)"),
        (math::top!(scores.clone(), 2), "math::top(scores, 2)"),
        (math::trimean!(scores.clone()), "math::trimean(scores)"),
        (math::variance!(scores), "math::variance(scores)"),
    );
}

#[test]
fn method_rendering() {
    let score = NumberLike::from(Field::new("score"));
    calls!(
        (score.abs(), "score.abs()"),
        (score.acos(), "score.acos()"),
        (score.acot(), "score.acot()"),
        (score.asin(), "score.asin()"),
        (score.atan(), "score.atan()"),
        (score.ceil(), "score.ceil()"),
        (score.cos(), "score.cos()"),
        (score.cot(), "score.cot()"),
        (score.deg2rad(), "score.deg2rad()"),
        (score.floor(), "score.floor()"),
        (score.ln(), "score.ln()"),
        (score.log(2), "score.log(2)"),
        (score.log10(), "score.log10()"),
        (score.log2(), "score.log2()"),
        (score.rad2deg(), "score.rad2deg()"),
        (score.round(), "score.round()"),
        (score.sign(), "score.sign()"),
        (score.sin(), "score.sin()"),
        (score.tan(), "score.tan()"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(&db, [
        (math::abs!(-5), "5"),
        (array::sort!(math::bottom!(vec![3, 1, 2], 2)), "[1, 2]"),
        (math::ceil!(1.2), "2"),
        (math::clamp!(15, 0, 10), "10"),
        (math::fixed!(1.2345, 2), "1.23"),
        (math::floor!(1.8), "1"),
        (math::lerp!(0, 10, 0.5), "5"),
        (math::log10!(100), "2"),
        (math::log2!(8), "3"),
        (math::max!(vec![1, 4, 2]), "4"),
        (math::mean!(vec![1, 2, 3]), "2"),
        (math::median!(vec![1, 5, 3]), "3"),
        (math::min!(vec![4, 1, 2]), "1"),
        (math::mode!(vec![1, 2, 2]), "2"),
        (math::pow!(2, 3), "8"),
        (math::product!(vec![2, 3, 4]), "24"),
        (math::round!(1.5), "2"),
        (math::sign!(-3), "-1"),
        (math::spread!(vec![1, 5]), "4"),
        (math::sqrt!(16), "4"),
        (math::sum!(vec![1, 2, 3]), "6"),
        (array::sort!(math::top!(vec![3, 1, 2], 2)), "[2, 3]"),
        (NumberLike::from(-5).abs(), "5"),
        (NumberLike::from(2.7).floor(), "2"),
    ]);
    for function in [
        math::acos!(0.5),
        math::acot!(1),
        math::asin!(0.5),
        math::atan!(1),
        math::cos!(0),
        math::cot!(1),
        math::deg2rad!(180),
        math::interquartile!(vec![1, 2, 3, 4]),
        math::lerpangle!(0, 90, 0.5),
        math::ln!(2),
        math::log!(8, 2),
        math::midhinge!(vec![1, 2, 3, 4]),
        math::nearestrank!(vec![1, 2, 3, 4], 50),
        math::percentile!(vec![1, 2, 3, 4], 50),
        math::rad2deg!(1),
        math::sin!(1),
        math::stddev!(vec![1, 2, 3, 4]),
        math::tan!(1),
        math::trimean!(vec![1, 2, 3, 4]),
        math::variance!(vec![1, 2, 3, 4]),
    ] {
        is_a(&db, &function, "number").await;
    }
}

#[test]
fn generated_bindings() {
    let nested = math::round!(math::pow!(Param::new("base"), 2));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "math::round(math::pow($base, $_p0))");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = call_failure(&db, &math::abs!(NumberLike::from(Field::from_expr(ValueLike::from("five"))))).await;
    assert!(error.contains("math::abs"), "{error}");
}
