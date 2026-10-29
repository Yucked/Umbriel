//! `WITH NOINDEX` and `WITH INDEX …`, which `SELECT`, `UPDATE`, `UPSERT` and `DELETE` write after
//! their targets.

use crate::{Field, Render, Renderer};

use super::select::IndexNames;

/// Which indexes the query planner may use to find a statement's records.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IndexHint {
    /// `WITH NOINDEX`: read through the table iterator.
    NoIndex,
    /// `WITH INDEX a, b`: only these indexes.
    Indexes(Vec<Field>),
}

impl IndexHint {
    pub(crate) fn indexes(indexes: impl Into<IndexNames>) -> Self {
        Self::Indexes(indexes.into().into())
    }
}

/// Writes the clause with its leading space.
impl Render for IndexHint {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::NoIndex => {
                out.push_str(" WITH NOINDEX");
            }
            Self::Indexes(indexes) => {
                out.push_str(" WITH INDEX ").push(indexes);
            }
        }
    }
}
