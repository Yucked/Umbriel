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

/// Writes the data-clause builder methods shared by `CREATE`, `UPDATE`, `UPSERT` and `RELATE`:
/// `content`, `set`, `merge`, `patch`, `replace`, `unset`, `timeout`, and the private `with_mode`
/// that changes the return mode.
///
/// The statement is `$statement<T, R, C>` with a `body` holding `data: Option<DataClause>` and
/// `timeout: Option<DurationLike>`, `mode: R` and `cardinality: PhantomData<C>`; `$bound` is what
/// `T` must implement. `$noun` names what the statement writes in the generated docs.
macro_rules! data_clause_methods {
    ($bound:path, $statement:ident, $noun:literal) => {
        impl<T: $bound, R: $crate::statements::ReturnMode, C: $crate::statements::Cardinality>
            $statement<T, R, C>
        {
            #[doc = concat!(
                "The ", $noun, "'s data as a whole model (`CONTENT`). `CONTENT` bypasses ",
                "`READONLY` fields; [`replace`](Self::replace) respects them. The model's ids ",
                "must agree with the statement's targets, or the statement fails when it is ",
                "rendered."
            )]
            pub fn content(mut self, content: T) -> Self {
                self.body.data =
                    Some($crate::statements::data::DataClause::content(content));
                self
            }

            #[doc = concat!(
                "Assignments to the ", $noun, "'s fields (`SET`): `=`, and `+=`/`-=` to ",
                "increment, decrement, append or remove. Repeated calls add to the assignments."
            )]
            pub fn set(mut self, settables: impl Into<Vec<$crate::Setter>>) -> Self {
                self.body.data = Some($crate::statements::data::DataClause::set(
                    self.body.data.take(),
                    settables.into(),
                ));
                self
            }

            #[doc = concat!(
                "Merges the non-`Keep` fields of a generated `<Model>Merge` into the ", $noun,
                " (`MERGE`). Nested objects combine at every level; arrays are replaced whole. ",
                "A merge with every field `Keep` still writes `MERGE {}`."
            )]
            pub fn merge(mut self, merge: impl $crate::ModelMerge<Model = T>) -> Self {
                self.body.data = Some($crate::statements::data::DataClause::Merge(
                    $crate::ValueLike::new(merge.into_object()),
                ));
                self
            }

            #[doc = concat!(
                "JSON Patch operations on the ", $noun, " (`PATCH`). Repeated calls add to the ",
                "operations; one that fails, such as a failed `test` or a `replace` of a path ",
                "that does not exist, fails the statement."
            )]
            pub fn patch(mut self, patch_op: impl Into<Vec<$crate::PatchOp>>) -> Self {
                self.body.data = Some($crate::statements::data::DataClause::patch(
                    self.body.data.take(),
                    patch_op.into(),
                ));
                self
            }

            #[doc = concat!(
                "Caution! Replaces the ", $noun, "'s fields with the given value, removing every ",
                "field it does not contain (`REPLACE`). Unlike `CONTENT` it respects `READONLY` ",
                "fields."
            )]
            pub fn replace(mut self, replacement: impl $crate::types::SurrealValue) -> Self {
                self.body.data = Some($crate::statements::data::DataClause::Replace(
                    $crate::ValueLike::value(replacement),
                ));
                self
            }

            #[doc = concat!(
                "Removes fields from the ", $noun, " (`UNSET`). Repeated calls add to the ",
                "fields. Fields with a `DEFAULT` or `VALUE` clause are computed after the data ",
                "clause."
            )]
            pub fn unset(
                mut self,
                fields: impl Into<$crate::statements::data::Unsettables>,
            ) -> Self {
                self.body.data = Some($crate::statements::data::DataClause::unset(
                    self.body.data.take(),
                    fields.into().0,
                ));
                self
            }

            /// `TIMEOUT @duration`, bound as a query parameter.
            pub fn timeout(mut self, duration: impl Into<$crate::DurationLike>) -> Self {
                self.body.timeout = Some(duration.into());
                self
            }

            fn with_mode<M: $crate::statements::ReturnMode>(self, mode: M) -> $statement<T, M, C> {
                $statement {
                    body: self.body,
                    mode,
                    cardinality: ::std::marker::PhantomData,
                }
            }
        }
    };
}

pub(crate) use data_clause_methods;
