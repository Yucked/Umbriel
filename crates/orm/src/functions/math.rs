//! Math functions and constants, `math::*`.
//!
//! Results that keep the kind of their input (an `int` stays an `int`) are typed
//! [`kinds::Number`]; the constants are `float`s SurrealDB evaluates at its own precision.
//!
//! ```rust
//! use umbriel::{*, functions::math};
//! let score = Field::new("score");
//! assert_eq!(math::abs!(score.clone()).to_debug_sql(), "math::abs(score)");
//! assert_eq!(NumberLike::from(score).round().to_debug_sql(), "score.round()");
//! assert_eq!(math::PI.to_debug_sql(), "math::PI");
//! ```

use crate::{Constant, kinds};

functions! { $ "math" "::", math, math;
    /// The absolute value.
    abs(number: NumberLike) -> kinds::Number [NumberLike.abs];
    /// The arccosine, in radians.
    acos(number: NumberLike) -> kinds::Number [NumberLike.acos];
    /// The arccotangent, in radians.
    acot(number: NumberLike) -> kinds::Number [NumberLike.acot];
    /// The arcsine, in radians.
    asin(number: NumberLike) -> kinds::Number [NumberLike.asin];
    /// The arctangent, in radians.
    atan(number: NumberLike) -> kinds::Number [NumberLike.atan];
    /// The `count` least numbers.
    bottom(numbers: ArrayLike, count: NumberLike) -> kinds::Array<kinds::Number>;
    /// Rounded up to an integer.
    ceil(number: NumberLike) -> kinds::Number [NumberLike.ceil];
    /// `number` limited to the range `min` to `max`.
    clamp(number: NumberLike, min: NumberLike, max: NumberLike) -> kinds::Number;
    /// The cosine of an angle in radians.
    cos(number: NumberLike) -> kinds::Number [NumberLike.cos];
    /// The cotangent of an angle in radians.
    cot(number: NumberLike) -> kinds::Number [NumberLike.cot];
    /// Degrees converted to radians.
    deg2rad(number: NumberLike) -> kinds::Number [NumberLike.deg2rad];
    /// Rounded to `places` decimal places. SurrealDB has no method form of it.
    fixed(number: NumberLike, places: NumberLike) -> kinds::Number;
    /// Rounded down to an integer.
    floor(number: NumberLike) -> kinds::Number [NumberLike.floor];
    /// The interquartile range.
    interquartile(numbers: ArrayLike) -> kinds::Number;
    /// The linear interpolation from `from` to `to` by `factor`.
    lerp(from: NumberLike, to: NumberLike, factor: NumberLike) -> kinds::Number;
    /// The angular linear interpolation, in degrees, from `from` to `to` by `factor`.
    lerpangle(from: NumberLike, to: NumberLike, factor: NumberLike) -> kinds::Number;
    /// The natural logarithm.
    ln(number: NumberLike) -> kinds::Number [NumberLike.ln];
    /// The logarithm in `base`.
    log(number: NumberLike, base: NumberLike) -> kinds::Number [NumberLike.log];
    /// The base-10 logarithm.
    log10(number: NumberLike) -> kinds::Number [NumberLike.log10];
    /// The base-2 logarithm.
    log2(number: NumberLike) -> kinds::Number [NumberLike.log2];
    /// The greatest number.
    max(numbers: ArrayLike) -> kinds::Number;
    /// The mean.
    mean(numbers: ArrayLike) -> kinds::Number;
    /// The median.
    median(numbers: ArrayLike) -> kinds::Number;
    /// The midhinge.
    midhinge(numbers: ArrayLike) -> kinds::Number;
    /// The least number.
    min(numbers: ArrayLike) -> kinds::Number;
    /// The most frequent number.
    mode(numbers: ArrayLike) -> kinds::Number;
    /// The nearest-rank `percentile`.
    nearestrank(numbers: ArrayLike, percentile: NumberLike) -> kinds::Number;
    /// The `percentile`.
    percentile(numbers: ArrayLike, percentile: NumberLike) -> kinds::Number;
    /// `number` raised to `power`. SurrealDB has no method form of it.
    pow(number: NumberLike, power: NumberLike) -> kinds::Number;
    /// The product.
    product(numbers: ArrayLike) -> kinds::Number;
    /// Radians converted to degrees.
    rad2deg(number: NumberLike) -> kinds::Number [NumberLike.rad2deg];
    /// Rounded to the nearest integer.
    round(number: NumberLike) -> kinds::Number [NumberLike.round];
    /// `-1`, `0` or `1` as the number is negative, zero or positive.
    sign(number: NumberLike) -> kinds::Number [NumberLike.sign];
    /// The sine of an angle in radians.
    sin(number: NumberLike) -> kinds::Number [NumberLike.sin];
    /// The difference between the greatest and least numbers.
    spread(numbers: ArrayLike) -> kinds::Number;
    /// The square root.
    sqrt(number: NumberLike) -> kinds::Number;
    /// The standard deviation.
    stddev(numbers: ArrayLike) -> kinds::Number;
    /// The sum.
    sum(numbers: ArrayLike) -> kinds::Number;
    /// The tangent of an angle in radians.
    tan(number: NumberLike) -> kinds::Number [NumberLike.tan];
    /// The `count` greatest numbers.
    top(numbers: ArrayLike, count: NumberLike) -> kinds::Array<kinds::Number>;
    /// The trimean.
    trimean(numbers: ArrayLike) -> kinds::Number;
    /// The variance.
    variance(numbers: ArrayLike) -> kinds::Number;
}

/// Defines math constants: each is the SurrealQL constant, written by name.
macro_rules! constants {
    ($($(#[$attr:meta])* $name:ident;)*) => {
        $(
            $(#[$attr])*
            pub const $name: Constant<kinds::Float> =
                Constant::new(::core::concat!("math::", ::core::stringify!($name)));
        )*
    };
}

constants! {
    /// `math::E`, Euler's number.
    E;
    /// `math::FRAC_1_PI`, 1/π.
    FRAC_1_PI;
    /// `math::FRAC_1_SQRT_2`, 1/√2.
    FRAC_1_SQRT_2;
    /// `math::FRAC_2_PI`, 2/π.
    FRAC_2_PI;
    /// `math::FRAC_2_SQRT_PI`, 2/√π.
    FRAC_2_SQRT_PI;
    /// `math::FRAC_PI_2`, π/2.
    FRAC_PI_2;
    /// `math::FRAC_PI_3`, π/3.
    FRAC_PI_3;
    /// `math::FRAC_PI_4`, π/4.
    FRAC_PI_4;
    /// `math::FRAC_PI_6`, π/6.
    FRAC_PI_6;
    /// `math::FRAC_PI_8`, π/8.
    FRAC_PI_8;
    /// `math::INFINITY`, positive infinity.
    INFINITY;
    /// `math::LN_10`, ln 10.
    LN_10;
    /// `math::LN_2`, ln 2.
    LN_2;
    /// `math::LOG10_2`, log₁₀ 2.
    LOG10_2;
    /// `math::LOG10_E`, log₁₀ e.
    LOG10_E;
    /// `math::LOG2_10`, log₂ 10.
    LOG2_10;
    /// `math::LOG2_E`, log₂ e.
    LOG2_E;
    /// `math::NEG_INFINITY`, negative infinity.
    NEG_INFINITY;
    /// `math::PI`, π.
    PI;
    /// `math::SQRT_2`, √2.
    SQRT_2;
    /// `math::TAU`, τ = 2π.
    TAU;
}
