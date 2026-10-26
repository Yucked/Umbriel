//! Typed `MERGE` data: a generated `<Model>Merge` holds one [`Merge`] per stored field, and
//! renders the object SurrealDB merges into the record.
//!
//! Unlike [`Patch`](crate::Patch), whose `Set` replaces a field's value whole, a `MERGE` combines
//! objects at every level: a nested object keeps the fields the merge does not mention. Arrays are
//! single values, so an array in a merge replaces the stored array whole.

use std::convert::Infallible;

use surrealdb::types::{SurrealValue, Value};

use crate::{Model, NONE, NULL, Object, Render, Renderer, ValueLike};

/// How a merge treats one field. `F` is the nested merge of an embedded object's fields; for
/// other fields it is [`Infallible`], so `Fields` cannot be written.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Merge<T, F = Infallible> {
    /// Leave the field out of the merge, so it is unchanged.
    #[default]
    Keep,
    /// Merge `NONE` into the field.
    None,
    /// Store `NULL`.
    Null,
    /// Merge this value. A scalar or array replaces the field; an object combines with the stored
    /// object, so stored fields the value lacks survive. A typed object holds every field it
    /// declares, so in practice `Set` overwrites each declared field of a nested object.
    Set(T),
    /// Merge only the nested object's fields that are not `Keep`.
    Fields(F),
}

impl<T, F> Merge<T, F> {
    /// Whether the field is left out of the merge.
    pub fn is_keep(&self) -> bool {
        matches!(self, Merge::Keep)
    }
}

impl<T: SurrealValue, F: MergeFields> Merge<T, F> {
    /// Adds the field's entry under `name`, unless it is kept.
    #[doc(hidden)]
    pub fn push_entry(self, name: &'static str, object: &mut MergeObject) {
        let value = match self {
            Merge::Keep => return,
            Merge::None => ValueLike::from(NONE),
            Merge::Null => ValueLike::from(NULL),
            Merge::Set(value) => ValueLike::value(value),
            Merge::Fields(fields) => {
                let mut nested = MergeObject::default();
                fields.push_entries(&mut nested);
                ValueLike::new(nested)
            }
        };
        object.entries.push((name, value));
    }
}

/// How a merge treats a flattened object, whose fields are stored as the model's own.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum FlatMerge<T, F> {
    /// Leave every field of the object out of the merge.
    #[default]
    Keep,
    /// Merge every field the object's value holds.
    Set(T),
    /// Merge only the object's fields that are not `Keep`.
    Fields(F),
}

impl<T: Object, F: MergeFields> FlatMerge<T, F> {
    /// Adds the object's entries as the model's own.
    #[doc(hidden)]
    pub fn push_entries(self, object: &mut MergeObject) {
        match self {
            FlatMerge::Keep => {}
            FlatMerge::Fields(fields) => fields.push_entries(object),
            FlatMerge::Set(value) => {
                // A hand-written `SurrealValue` may convert to something other than an object:
                // the statement then fails rather than merge nothing.
                let mut values = match value.into_value() {
                    Value::Object(values) => values.into_inner(),
                    other => {
                        object.errors.push(format!(
                            "a flattened object must convert into an object, not {:?}",
                            other.kind()
                        ));
                        return;
                    }
                };
                T::STORED_FIELDS.for_each(&mut |name| {
                    if let Some(value) = values.remove(name) {
                        object.entries.push((name, ValueLike::value(value)));
                    }
                });
            }
        }
    }
}

/// The fields of a generated `<Model>Merge` or `<Object>Merge`.
pub trait MergeFields {
    /// Adds an entry for every field that is not `Keep`, in declaration order.
    #[doc(hidden)]
    fn push_entries(self, object: &mut MergeObject);
}

impl MergeFields for Infallible {
    fn push_entries(self, _: &mut MergeObject) {
        match self {}
    }
}

/// A generated `<Model>Merge`: one [`Merge`] per stored field of the model.
pub trait ModelMerge: MergeFields + Default {
    /// The model this merge updates.
    type Model: Model;

    /// The object `MERGE` combines into the record. It is empty when every field is `Keep`.
    fn into_object(self) -> MergeObject
    where
        Self: Sized,
    {
        let mut object = MergeObject::default();
        self.push_entries(&mut object);
        object
    }
}

/// The object a merge renders: `{ 'name': value, … }`, with each value bound.
#[derive(Debug, Clone, Default)]
pub struct MergeObject {
    entries: Vec<(&'static str, ValueLike)>,
    errors: Vec<String>,
}

impl MergeObject {
    /// Whether every field was kept.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.errors.is_empty()
    }
}

impl Render for MergeObject {
    fn render(&self, out: &mut Renderer) {
        for error in &self.errors {
            out.push_error(error.as_str());
        }
        out.push_str("{");
        for (index, (name, value)) in self.entries.iter().enumerate() {
            out.push_str(if index > 0 { ", " } else { " " });
            // Keys are written as string literals, so any stored name is a valid key.
            out.push_inlined(&ValueLike::value((*name).to_owned()))
                .push_str(": ")
                .push(value);
        }
        out.push_str(if self.entries.is_empty() { "}" } else { " }" });
    }
}
