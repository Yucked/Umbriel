//! Value functions, `value::*`. Their method forms are on [`ValueLike`](crate::ValueLike).
//!
//! ```rust
//! use umbriel::{*, functions::value};
//! let doubled = ValueLike::from(Field::new("score"))
//!     .chain(closure(["n"], |[n]| n.multiply(2)));
//! assert_eq!(doubled.to_debug_sql(), "score.chain(|$n| $n * 2)");
//! assert_eq!(value::diff!(1, 2).to_debug_sql(), "value::diff(1, 2)");
//! ```

use crate::kinds;

functions! { $ "value" "::", value, value;
    /// The closure applied to the value.
    chain(value: ValueLike, closure: ClosureLike) -> kinds::Any [ValueLike.chain];
    /// The JSON Patch operations that turn `from` into `to`.
    diff(from: ValueLike, to: ValueLike) -> kinds::Array [ValueLike.diff];
    /// The value when the closure is truthy for it; otherwise an error, with `message` when
    /// given.
    expect(value: ValueLike, closure: ClosureLike; message: ValueLike) -> kinds::Any
        [ValueLike.expect];
    /// The value with the JSON Patch operations `patch` applied.
    patch(value: ValueLike, patch: ValueLike) -> kinds::Any [ValueLike.patch];
}
