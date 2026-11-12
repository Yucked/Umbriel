//! Embedded JavaScript functions, `function(args) { … }`. SurrealDB runs them only when built with
//! its `scripting` feature (Umbriel's `scripting` feature).

use crate::{Function, ValueLike};

/// `function(args) body`: embedded JavaScript, called with `args`, which the body reads as
/// `arguments`. The body is written verbatim and not checked; the result is untyped, and
/// [`Function::typed`] gives it the kind the script returns.
pub fn function_fn<T: Into<ValueLike>>(args: Vec<T>, body: impl Into<String>) -> Function {
    Function::script(body.into()).args(args)
}

/// `function(args) { … }`: embedded JavaScript. The arguments are any values or expressions; the
/// body is written verbatim, either as Rust tokens in braces or as string literals.
///
/// ```rust
/// use umbriel::{*, functions::function};
/// let doubled = function!((Field::new("score")), "{ return arguments[0] * 2; }");
/// assert_eq!(doubled.to_debug_sql(), "function(score) { return arguments[0] * 2; }");
/// ```
#[macro_export]
macro_rules! function {
    (($($arg:expr),*), {$($code:tt)*}) => {
        $crate::functions::function_fn(
            ::std::vec![$($crate::ValueLike::from($arg)),*],
            ::core::stringify!({$($code)*}),
        )
    };
    (($($arg:expr),*), $($code:tt)*) => {
        $crate::functions::function_fn(
            ::std::vec![$($crate::ValueLike::from($arg)),*],
            ::core::concat!($($code)*),
        )
    };
}
pub use function;
