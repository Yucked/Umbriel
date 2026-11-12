//! Surrealism module calls, `mod::name(…)`.
//!
//! Every `::`-separated segment of the path must be an identifier (`[A-Za-z_][A-Za-z0-9_]*`);
//! otherwise the query fails to render. SurrealDB parses `mod::` calls only when Surrealism
//! modules are enabled; defining modules is not part of this API.
//!
//! ```rust
//! use umbriel::{*, functions::module, kinds};
//! let call = module::call::<kinds::Number, _>("geo::distance", [Field::new("a"), Field::new("b")]);
//! assert_eq!(call.to_debug_sql(), "mod::geo::distance(a, b)");
//! ```

use std::borrow::Cow;

use crate::{Function, ValueLike, kinds::KindOf};

/// `mod::path(args)`, whose result is `K`, the kind the module function returns.
pub fn call<K: KindOf, A: Into<ValueLike>>(
    path: impl Into<Cow<'static, str>>,
    args: impl IntoIterator<Item = A>,
) -> Function<K> {
    Function::module(path.into()).args(args)
}
