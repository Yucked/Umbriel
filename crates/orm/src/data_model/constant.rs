use std::{fmt, marker::PhantomData};

use crate::{Render, Renderer, ValueLike, kinds::KindOf};

/// A SurrealQL constant, e.g. `math::PI`, typed by the kind `K` of its value. SurrealDB evaluates
/// it, so its value has SurrealDB's precision; the constants are in
/// [`functions::math`](crate::functions::math), [`functions::time`](crate::functions::time) and
/// [`functions::duration`](crate::functions::duration).
pub struct Constant<K: KindOf> {
    name: &'static str,
    marker: PhantomData<fn() -> K>,
}

impl<K: KindOf> Constant<K> {
    /// The constant `name`, written as SurrealDB spells it.
    pub(crate) const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
}

impl<K: KindOf> Clone for Constant<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: KindOf> Copy for Constant<K> {}

impl<K: KindOf> fmt::Debug for Constant<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Constant").field(&self.name).finish()
    }
}

impl<K: KindOf> PartialEq for Constant<K> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

/// Written by name, without parentheses: `math::PI`.
impl<K: KindOf> Render for Constant<K> {
    fn render(&self, out: &mut Renderer) {
        out.push_str(self.name);
    }
}

/// A constant is an expression, usable wherever a value is.
impl<K: KindOf> From<Constant<K>> for ValueLike {
    fn from(constant: Constant<K>) -> Self {
        ValueLike::new(constant)
    }
}
