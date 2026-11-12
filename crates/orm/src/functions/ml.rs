//! SurrealML model calls, `ml::name<version>(…)`.
//!
//! Every `::`-separated segment of the model path must be an identifier
//! (`[A-Za-z_][A-Za-z0-9_]*`); otherwise the query fails to render and nothing of the path is
//! written. The version is exactly `major.minor.patch` ([`MlVersion`]).
//!
//! Running a model needs SurrealDB built with its `ml` feature, which Umbriel does not enable:
//! without it, SurrealDB refuses every `ml::` call with "Machine learning computation is not
//! enabled."
//!
//! ```rust
//! use umbriel::{*, functions::ml, kinds};
//! let version: MlVersion = "1.0.0".parse().expect("a version");
//! let call = ml::call::<kinds::Float, _>("insurance::prediction", version, [Field::new("age")]);
//! assert_eq!(call.to_debug_sql(), "ml::insurance::prediction<1.0.0>(age)");
//! ```

use std::borrow::Cow;

use crate::{Function, MlVersion, ValueLike, kinds::KindOf};

/// `ml::path<version>(args)`, whose result is `K`, the kind the model returns.
pub fn call<K: KindOf, A: Into<ValueLike>>(
    path: impl Into<Cow<'static, str>>,
    version: MlVersion,
    args: impl IntoIterator<Item = A>,
) -> Function<K> {
    Function::model(path.into(), version).args(args)
}
