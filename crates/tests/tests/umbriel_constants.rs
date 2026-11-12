//! SurrealQL constants (`orm/src/data_model/constant.rs`): written by name without parentheses,
//! evaluated by SurrealDB at its own precision.

use std::f64::consts;

use umbriel::{
    Render,
    functions::{duration, math, time},
    types::Value,
};
use umbriel_tests::*;

#[tokio::test]
async fn constants_render_without_parentheses_and_evaluate() {
    let db = db().await;
    let floats = [
        (math::E, "math::E", consts::E),
        (math::FRAC_1_PI, "math::FRAC_1_PI", consts::FRAC_1_PI),
        (
            math::FRAC_1_SQRT_2,
            "math::FRAC_1_SQRT_2",
            consts::FRAC_1_SQRT_2,
        ),
        (math::FRAC_2_PI, "math::FRAC_2_PI", consts::FRAC_2_PI),
        (
            math::FRAC_2_SQRT_PI,
            "math::FRAC_2_SQRT_PI",
            consts::FRAC_2_SQRT_PI,
        ),
        (math::FRAC_PI_2, "math::FRAC_PI_2", consts::FRAC_PI_2),
        (math::FRAC_PI_3, "math::FRAC_PI_3", consts::FRAC_PI_3),
        (math::FRAC_PI_4, "math::FRAC_PI_4", consts::FRAC_PI_4),
        (math::FRAC_PI_6, "math::FRAC_PI_6", consts::FRAC_PI_6),
        (math::FRAC_PI_8, "math::FRAC_PI_8", consts::FRAC_PI_8),
        (math::INFINITY, "math::INFINITY", f64::INFINITY),
        (math::LN_10, "math::LN_10", consts::LN_10),
        (math::LN_2, "math::LN_2", consts::LN_2),
        (math::LOG10_2, "math::LOG10_2", consts::LOG10_2),
        (math::LOG10_E, "math::LOG10_E", consts::LOG10_E),
        (math::LOG2_10, "math::LOG2_10", consts::LOG2_10),
        (math::LOG2_E, "math::LOG2_E", consts::LOG2_E),
        (math::NEG_INFINITY, "math::NEG_INFINITY", f64::NEG_INFINITY),
        (math::PI, "math::PI", consts::PI),
        (math::SQRT_2, "math::SQRT_2", consts::SQRT_2),
        (math::TAU, "math::TAU", consts::TAU),
    ];
    for (constant, name, expected) in floats {
        assert_eq!(constant.to_debug_sql(), name);
        let value: f64 = read(&db, &format!("RETURN {name};")).await;
        assert_eq!(value, expected, "{name}");
    }

    assert_eq!(time::EPOCH.to_debug_sql(), "time::EPOCH");
    let epoch: bool = read(&db, "RETURN time::EPOCH = d'1970-01-01T00:00:00Z';").await;
    assert!(epoch);
    for (name, rendered) in [
        (time::MINIMUM.to_debug_sql(), "time::MINIMUM"),
        (time::MAXIMUM.to_debug_sql(), "time::MAXIMUM"),
    ] {
        assert_eq!(name, rendered);
        let is_datetime: bool = read(&db, &format!("RETURN type::is_datetime({name});")).await;
        assert!(is_datetime, "{name}");
    }
    let ordered: bool = read(
        &db,
        "RETURN time::MINIMUM < time::EPOCH AND time::EPOCH < time::MAXIMUM;",
    )
    .await;
    assert!(ordered);

    assert_eq!(duration::MAX.to_debug_sql(), "duration::MAX");
    let max: Value = read(&db, "RETURN duration::MAX;").await;
    assert!(matches!(max, Value::Duration(_)), "{max:?}");

    let rounded = math::round!(math::PI);
    assert_eq!(rounded.to_debug_sql(), "math::round(math::PI)");
    holds(&db, &rounded, "3").await;
    holds(&db, &time::year!(time::EPOCH), "1970").await;
}
