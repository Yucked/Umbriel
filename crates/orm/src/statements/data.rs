//! The data clause shared by the write statements. Each builder keeps an `Option<DataClause>`, so
//! a statement without one is explicit, and exposes only the clauses its statement documents.

use crate::{Field, PatchOp, Render, Renderer, Setter, ValueLike};

#[derive(Debug, Clone)]
pub(crate) enum DataClause {
    /// `CONTENT value`: the whole record.
    Content(ValueLike),
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

impl DataClause {
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
            Self::Content(content) => {
                out.push_str(" CONTENT ").push(content);
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
