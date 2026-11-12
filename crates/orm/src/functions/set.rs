//! Set functions, `set::*`.
//!
//! The functions returning a set are typed [`kinds::Set`]; those returning an element of one
//! are untyped, since the element kind is not known statically.
//!
//! ```rust
//! use umbriel::{*, functions::set};
//! let tags = Field::new("tags");
//! assert_eq!(set::len!(tags.clone()).to_debug_sql(), "set::len(tags)");
//! assert_eq!(SetLike::from(tags).first().to_debug_sql(), "tags.first()");
//! ```

use crate::kinds;

functions! { $ "set" "::", set, set;
    /// The set with `value` added.
    add(set: SetLike, value: ValueLike) -> kinds::Set [SetLike.add];
    /// Whether every element is truthy, or matches `check` (a value or a closure).
    all(set: SetLike; check: ValueLike) -> kinds::Bool [SetLike.all];
    /// Whether any element is truthy, or matches `check` (a value or a closure).
    any(set: SetLike; check: ValueLike) -> kinds::Bool [SetLike.any];
    /// The element at `index`, in the set's order.
    at(set: SetLike, index: NumberLike) -> kinds::Any [SetLike.at];
    /// The elements of `a` that are not in `b`.
    complement(a: SetLike, b: SetLike) -> kinds::Set [SetLike.complement];
    /// Whether the set contains `value`.
    contains(set: SetLike, value: ValueLike) -> kinds::Bool [SetLike.contains];
    /// The elements in only one of the sets.
    difference(a: SetLike, b: SetLike) -> kinds::Set [SetLike.difference];
    /// The elements equal to `check`, or for which the closure `check` is truthy.
    filter(set: SetLike, check: ValueLike) -> kinds::Set [SetLike.filter];
    /// The first element equal to `check`, or for which the closure is truthy.
    find(set: SetLike, check: ValueLike) -> kinds::Any [SetLike.find];
    /// The first element.
    first(set: SetLike) -> kinds::Any [SetLike.first];
    /// The nested sets and arrays flattened one level.
    flatten(set: SetLike) -> kinds::Set [SetLike.flatten];
    /// The closure applied to an accumulator, starting at `init`, and each element.
    fold(set: SetLike, init: ValueLike, closure: ClosureLike) -> kinds::Any [SetLike.fold];
    /// The elements in both sets.
    intersect(a: SetLike, b: SetLike) -> kinds::Set [SetLike.intersect];
    /// Whether the set is empty.
    is_empty(set: SetLike) -> kinds::Bool [SetLike.is_empty];
    /// The elements as strings, joined with `separator`.
    join(set: SetLike, separator: StrandLike) -> kinds::String [SetLike.join];
    /// The last element.
    last(set: SetLike) -> kinds::Any [SetLike.last];
    /// The number of elements.
    len(set: SetLike) -> kinds::Int [SetLike.len];
    /// The closure applied to each element.
    map(set: SetLike, closure: ClosureLike) -> kinds::Set [SetLike.map];
    /// The greatest element.
    max(set: SetLike) -> kinds::Any [SetLike.max];
    /// The least element.
    min(set: SetLike) -> kinds::Any [SetLike.min];
    /// The closure applied to an accumulator, starting at the first element, and each other
    /// element.
    reduce(set: SetLike, closure: ClosureLike) -> kinds::Any [SetLike.reduce];
    /// The set without `value`.
    remove(set: SetLike, value: ValueLike) -> kinds::Set [SetLike.remove];
    /// The elements from the index `start` up to the index `end`, exclusive, or in the range
    /// `start`.
    slice(set: SetLike; start: ValueLike, end: NumberLike) -> kinds::Set [SetLike.slice];
    /// The elements of both sets.
    union(a: SetLike, b: SetLike) -> kinds::Set [SetLike.union];
}
