use std::{fmt, marker::PhantomData};

use surrealdb::types::Kind;

use crate::{
    Render, Renderer, ValueLike,
    kinds::{self, KindOf},
};

/// A cast, `<kind> value`, typed by the kind `K` it converts to.
pub struct Cast<K: KindOf> {
    kind: Kind,
    value: ValueLike,
    marker: PhantomData<fn() -> K>,
}

/// `<K> value`: converts `value` to the kind of the marker `K`, e.g.
/// `cast::<kinds::Array<kinds::Int>>(tags)` renders `<array<int>> tags`. The result enters the
/// typed expressions `K` does (see [`kinds`]).
///
/// ```rust
/// use umbriel::{*, kinds};
/// assert_eq!(cast::<kinds::Int>("5").to_debug_sql(), "<int> '5'");
/// ```
pub fn cast<K: KindOf>(value: impl Into<ValueLike>) -> Cast<K> {
    Cast {
        kind: K::kind(),
        value: value.into(),
        marker: PhantomData,
    }
}

/// `<kind> value` for a kind known only at run time, or one no marker names: a union
/// (`int | string`, tried in order), a literal or a fixed-length array.
///
/// The result is untyped: it enters [`ValueLike`] and every typed expression, unchecked, so the
/// kind is the caller's to keep consistent.
pub fn cast_dynamic(kind: Kind, value: impl Into<ValueLike>) -> Cast<kinds::Any> {
    Cast {
        kind,
        value: value.into(),
        marker: PhantomData,
    }
}

impl<K: KindOf> Clone for Cast<K> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            value: self.value.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: KindOf> fmt::Debug for Cast<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cast")
            .field("kind", &self.kind)
            .field("value", &self.value)
            .finish()
    }
}

impl<K: KindOf> PartialEq for Cast<K> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.value == other.value
    }
}

/// The value is parenthesized when it is an operation, since a cast binds tighter than any
/// binary operator.
impl<K: KindOf> Render for Cast<K> {
    fn render(&self, out: &mut Renderer) {
        out.push_str("<").push_syntax(&self.kind).push_str("> ");
        self.value.render_operand(out);
    }
}

/// A cast is an expression, usable wherever a value is.
impl<K: KindOf> From<Cast<K>> for ValueLike {
    fn from(cast: Cast<K>) -> Self {
        ValueLike::new(cast)
    }
}
