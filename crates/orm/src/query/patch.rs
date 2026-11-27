//! Partial updates that keep omitted, `NONE` and `NULL` distinct.

use surrealdb::types::{Kind, SurrealValue, Value};

use crate::{Field, Model, NONE, NULL, Operator, Setter, ValueLike};

/// How a partial update treats one field.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Patch<T> {
    /// Leave the field unchanged.
    #[default]
    Keep,
    /// Remove the field (`NONE`).
    None,
    /// Store `NULL`.
    Null,
    /// Store this value, replacing the current one entirely, nested objects included.
    Set(T),
}

impl<T: SurrealValue> Patch<T> {
    /// The assignment for `field`, or `None` when the field is kept.
    #[doc(hidden)]
    pub fn into_setter(self, field: impl Into<Field>) -> Option<Setter> {
        let value = match self {
            Patch::Keep => return None,
            // Assigning `NONE` deletes the field.
            Patch::None => ValueLike::from(NONE),
            Patch::Null => ValueLike::from(NULL),
            Patch::Set(value) => ValueLike::value(value),
        };
        Some(Setter::new(field.into(), Operator::Equal, value))
    }
}

/// A generated `<Model>Patch`: one [`Patch`] per field of the model.
pub trait ModelPatch: Default {
    /// The model this patch updates.
    type Model: Model;

    /// The assignments for every field that is not [`Patch::Keep`], in declaration order.
    fn into_setters(self) -> Vec<Setter>;
}

/// A value that may be `NULL`, as distinct from absent: a field that may hold `NULL` is
/// `Option<Nullable<T>>`, where `None` is `NONE`, `Some(Nullable::Null)` is `NULL` and
/// `Some(Nullable::Value(v))` is a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Nullable<T> {
    /// `NULL`
    Null,
    /// A value
    Value(T),
}

impl<T: SurrealValue> SurrealValue for Nullable<T> {
    fn kind_of() -> Kind {
        Kind::either(vec![Kind::Null, T::kind_of()])
    }

    fn is_value(value: &Value) -> bool {
        matches!(value, Value::Null) || T::is_value(value)
    }

    fn into_value(self) -> Value {
        match self {
            Nullable::Null => Value::Null,
            Nullable::Value(value) => value.into_value(),
        }
    }

    fn from_value(value: Value) -> Result<Self, surrealdb::types::Error> {
        match value {
            Value::Null => Ok(Nullable::Null),
            value => T::from_value(value).map(Nullable::Value),
        }
    }
}

impl<T> From<T> for Nullable<T> {
    fn from(value: T) -> Self {
        Nullable::Value(value)
    }
}
