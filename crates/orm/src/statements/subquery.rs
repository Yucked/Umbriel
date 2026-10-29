use crate::{Block, Edge, Fragment, Model, Node, Render, Renderer};

use super::{
    Cardinality, CreateStatement, DeleteStatement, IfElseStatement, InsertStatement,
    RelateOrUpdateStatement, RelateStatement, ReturnMode, ReturnStatement, SelectStatement,
    Statement, UpdateStatement, UpsertStatement,
};

/// A statement nested inside another query, rendered in parentheses with its own values.
#[derive(Debug, Clone, PartialEq)]
pub struct Subquery(Fragment);

/// `(statement)`, without the statement's trailing `;`.
struct Parenthesized<S>(S);

impl<S: Statement> Render for Parenthesized<S> {
    fn render(&self, out: &mut Renderer) {
        out.push_str("(");
        self.0.render_body(out);
        out.push_str(")");
    }
}

impl Subquery {
    fn of(statement: impl Statement + Send + Sync + 'static) -> Self {
        Self(Fragment::new(Parenthesized(statement)))
    }

    /// The subquery as a node another path or expression holds.
    pub(crate) fn into_fragment(self) -> Fragment {
        self.0
    }
}

impl Render for Subquery {
    fn render(&self, out: &mut Renderer) {
        out.push(&self.0);
    }
}

impl From<Block> for Subquery {
    fn from(block: Block) -> Self {
        Self(Fragment::new(block))
    }
}

// Separate arms: an optional `[..]` before a `ty` is ambiguous, since a type can start with `[`.
macro_rules! subquery_from {
    ([$($generics:tt)*] $statement:ty) => {
        impl<$($generics)*> From<$statement> for Subquery {
            fn from(statement: $statement) -> Self {
                Self::of(statement)
            }
        }
    };
    ($statement:ty) => {
        impl From<$statement> for Subquery {
            fn from(statement: $statement) -> Self {
                Self::of(statement)
            }
        }
    };
}

subquery_from!([O: 'static, C: Cardinality] SelectStatement<O, C>);
subquery_from!(IfElseStatement);
subquery_from!(ReturnStatement);
subquery_from!([T: Node, R: ReturnMode, C: Cardinality] CreateStatement<T, R, C>);
subquery_from!([T: Model, R: ReturnMode, C: Cardinality] UpdateStatement<T, R, C>);
subquery_from!([T: Model, R: ReturnMode, C: Cardinality] UpsertStatement<T, R, C>);
subquery_from!([T: Model, R: ReturnMode, C: Cardinality] DeleteStatement<T, R, C>);
subquery_from!([T: Edge, R: ReturnMode, C: Cardinality] RelateStatement<T, R, C>);
subquery_from!([T: Edge, R: ReturnMode, C: Cardinality] RelateOrUpdateStatement<T, R, C>);
subquery_from!([T: Model, R: ReturnMode, C: Cardinality] InsertStatement<T, R, C>);
