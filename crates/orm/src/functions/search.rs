//! Full-text search and result fusion functions, `search::*`.
//!
//! `score`, `highlight` and `offsets` read the match of a `@n@` predicate in the same query
//! ([`matches_with`](crate::Operatable::matches_with)), so their reference `n` is a literal. A
//! query calling one without a `MATCHES` predicate in its `WHERE` fails when SurrealDB plans it.
//! `linear` and `rrf` fuse the results of several searches, such as a KNN search and a
//! full-text search.
//!
//! ```rust
//! use umbriel::{*, functions::search};
//! assert_eq!(search::score!(1).to_debug_sql(), "search::score(1)");
//! assert_eq!(
//!     search::highlight!("<b>", "</b>", 1, true).to_debug_sql(),
//!     "search::highlight('<b>', '</b>', 1, true)"
//! );
//! assert_eq!(
//!     search::analyze!("english", "Hello world").to_debug_sql(),
//!     "search::analyze('english', 'Hello world')"
//! );
//! ```

use crate::{BoolLike, Function, Render, Renderer, StrandLike, ValueLike, kinds};

/// The reference number of a `@n@` matches predicate. SurrealDB only accepts it as a literal.
struct PredicateRef(u8);

impl Render for PredicateRef {
    fn render(&self, out: &mut Renderer) {
        out.push_display(&self.0);
    }
}

fn reference(reference: u8) -> ValueLike {
    ValueLike::new(PredicateRef(reference))
}

fn partial_arg(partial: impl Into<BoolLike>) -> ValueLike {
    let partial: BoolLike = partial.into();
    partial.into()
}

/// `search::score(reference)`: the relevance score of the record for the `@reference@`
/// predicate; `NONE` when its index has no scorer or the record has no score for it.
pub fn score_fn(predicate_ref_number: u8) -> Function<kinds::Optional<kinds::Float>> {
    Function::new("search::score").arg(reference(predicate_ref_number))
}

/// `search::offsets(reference [, partial])`: the positions of the matched terms, an object from
/// each value's position in the field to its `{ s, e }` spans. `partial` includes partial
/// matches; `NONE` when no `@reference@` predicate matched the record.
pub fn offsets_fn(
    predicate_ref_number: u8,
    partial: Option<impl Into<BoolLike>>,
) -> Function<kinds::Optional<kinds::Object>> {
    Function::new("search::offsets")
        .arg(reference(predicate_ref_number))
        .optional("partial", partial.map(partial_arg))
}

/// `search::highlight(prefix, suffix, reference [, partial])`: the field with each matched term
/// between `prefix` and `suffix`. One string for a single value, an array of strings for an
/// array field, and `NONE` for no value. The index needs `HIGHLIGHTS`.
pub fn highlight_fn(
    prefix: impl Into<StrandLike>,
    suffix: impl Into<StrandLike>,
    predicate_ref_number: u8,
    partial: Option<impl Into<BoolLike>>,
) -> Function<kinds::Optional<kinds::Either<kinds::String, kinds::Array<kinds::String>>>> {
    Function::new("search::highlight")
        .arg(prefix.into())
        .arg(suffix.into())
        .arg(reference(predicate_ref_number))
        .optional("partial", partial.map(partial_arg))
}

/// `search::score(reference)`. See [`score_fn`].
#[macro_export]
macro_rules! search_score {
    ($reference:expr $(,)?) => {
        $crate::functions::search::score_fn($reference)
    };
}
pub use search_score as score;

/// `search::offsets(reference [, partial])`. See [`offsets_fn`].
#[macro_export]
macro_rules! search_offsets {
    ($reference:expr $(,)?) => {
        $crate::functions::search::offsets_fn($reference, ::core::option::Option::<bool>::None)
    };
    ($reference:expr, $partial:expr $(,)?) => {
        $crate::functions::search::offsets_fn($reference, ::core::option::Option::Some($partial))
    };
}
pub use search_offsets as offsets;

/// `search::highlight(prefix, suffix, reference [, partial])`. See [`highlight_fn`].
#[macro_export]
macro_rules! search_highlight {
    ($prefix:expr, $suffix:expr, $reference:expr $(,)?) => {
        $crate::functions::search::highlight_fn(
            $prefix,
            $suffix,
            $reference,
            ::core::option::Option::<bool>::None,
        )
    };
    ($prefix:expr, $suffix:expr, $reference:expr, $partial:expr $(,)?) => {
        $crate::functions::search::highlight_fn(
            $prefix,
            $suffix,
            $reference,
            ::core::option::Option::Some($partial),
        )
    };
}
pub use search_highlight as highlight;

functions! { $ "search" "::", search, search;
    /// The tokens the analyzer named `analyzer` makes of `text`. `NONE` when either argument is
    /// not a string.
    analyze(analyzer: StrandLike, text: StrandLike) -> kinds::Optional<kinds::Array<kinds::String>>;
    /// The `limit` best records of `results` (a list of result lists, each record with an `id`),
    /// by the weighted sum of their scores normalized with `norm` (`'minmax'` or `'zscore'`).
    /// `weights` has one number per result list. `NONE` when the query is cancelled.
    linear(results: ArrayLike, weights: ArrayLike, limit: NumberLike, norm: StrandLike)
        -> kinds::Optional<kinds::Array<kinds::Object>>;
    /// The `limit` best records of `results` by reciprocal rank fusion, with the constant
    /// `constant` (60 when left out). Each record gets `rrf_score`. `NONE` when the query is
    /// cancelled.
    rrf(results: ArrayLike, limit: NumberLike; constant: NumberLike)
        -> kinds::Optional<kinds::Array<kinds::Object>>;
}
