//! The data clause shared by the write statements. Each builder keeps an `Option<DataClause>`, so
//! a statement without one is explicit, and exposes only the clauses its statement documents.

use surrealdb::types::{self, Value};

use crate::{Field, Model, PatchOp, Render, Renderer, Setter, ValueLike};

#[derive(Debug, Clone)]
pub(crate) enum DataClause {
    /// `CONTENT value`: the whole record, with the record ids it holds, which the statement checks
    /// against its targets.
    Content { value: ValueLike, ids: ContentIds },
    /// `MERGE value`: combined into the record, nested objects recursively.
    Merge(ValueLike),
    /// `PATCH [ops]`: JSON Patch operations.
    Patch(Vec<PatchOp>),
    /// `REPLACE value`: the whole record, respecting `READONLY` fields.
    Replace(ValueLike),
    /// `SET assignments`.
    Set(Vec<Setter>),
    /// `UNSET fields`.
    Unset(Vec<Field>),
}

/// The record ids a `CONTENT` model holds: its own, and for an edge the `in` and `out` it sets.
#[derive(Debug, Clone)]
pub(crate) struct ContentIds {
    pub(crate) id: types::RecordId,
    pub(crate) r#in: Option<types::RecordId>,
    pub(crate) out: Option<types::RecordId>,
}

impl DataClause {
    /// `CONTENT` with a whole model.
    pub(crate) fn content<T: Model>(content: T) -> Self {
        let id = content.record_id().into();
        let value = content.into_value();
        // An unset link is `NONE` and a fetched one an object; only a record id is a known endpoint.
        let link = |name: &str| match &value {
            Value::Object(object) => match object.get(name) {
                Some(Value::RecordId(id)) => Some(id.clone()),
                _ => None,
            },
            _ => None,
        };
        let ids = ContentIds {
            id,
            r#in: link("in"),
            out: link("out"),
        };
        Self::Content {
            value: ValueLike::value(value),
            ids,
        }
    }

    /// The record ids a `CONTENT` clause holds; `None` for any other clause.
    pub(crate) fn content_ids(data: Option<&Self>) -> Option<&ContentIds> {
        match data {
            Some(Self::Content { ids, .. }) => Some(ids),
            _ => None,
        }
    }

    /// Adds assignments, extending earlier ones; any other clause is replaced.
    pub(crate) fn set(data: Option<Self>, setters: Vec<Setter>) -> Self {
        match data {
            Some(Self::Set(mut existing)) => {
                existing.extend(setters);
                Self::Set(existing)
            }
            _ => Self::Set(setters),
        }
    }

    /// Adds fields to remove, extending earlier ones; any other clause is replaced.
    pub(crate) fn unset(data: Option<Self>, fields: Vec<Field>) -> Self {
        match data {
            Some(Self::Unset(mut existing)) => {
                existing.extend(fields);
                Self::Unset(existing)
            }
            _ => Self::Unset(fields),
        }
    }

    /// Adds patch operations, extending earlier ones; any other clause is replaced.
    pub(crate) fn patch(data: Option<Self>, ops: Vec<PatchOp>) -> Self {
        match data {
            Some(Self::Patch(mut existing)) => {
                existing.extend(ops);
                Self::Patch(existing)
            }
            _ => Self::Patch(ops),
        }
    }
}

/// Renders the clause with its leading space.
impl Render for DataClause {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::Content { value, .. } => {
                out.push_str(" CONTENT ").push(value);
            }
            Self::Merge(merge) => {
                out.push_str(" MERGE ").push(merge);
            }
            Self::Patch(ops) => {
                out.push_str(" PATCH [").push(ops).push_str("]");
            }
            Self::Replace(replacement) => {
                out.push_str(" REPLACE ").push(replacement);
            }
            Self::Set(setters) => {
                out.push_str(" SET ").push(setters);
            }
            Self::Unset(fields) => {
                out.push_str(" UNSET ").push(fields);
            }
        }
    }
}

/// Fields that can be removed with `UNSET`: one field or a list of them.
pub struct Unsettables(pub(crate) Vec<Field>);

impl<F: Into<Field>> From<F> for Unsettables {
    fn from(field: F) -> Self {
        Self(vec![field.into()])
    }
}

impl<F: Into<Field>> From<Vec<F>> for Unsettables {
    fn from(fields: Vec<F>) -> Self {
        Self(fields.into_iter().map(Into::into).collect())
    }
}

impl<F: Into<Field>, const N: usize> From<[F; N]> for Unsettables {
    fn from(fields: [F; N]) -> Self {
        Self(fields.into_iter().map(Into::into).collect())
    }
}
