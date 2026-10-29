use surrealdb::types::{SurrealValue, Table, Value};

use crate::{
    Alias, All, E, Edge, Field, Filter, Fragment, Function, Model, NONE, NULL, Node, Operation,
    Param, Render, Renderer, Setter,
    statements::{
        Cardinality, CreateStatement, DeleteStatement, IfElseStatement, InsertStatement,
        LetStatement, RelateOrUpdateStatement, RelateStatement, ReturnMode, SelectStatement,
        Subquery, UpdateStatement, UpsertStatement,
    },
};

/// Any SurrealQL expression: a bound value, field, parameter, operation, function call or
/// subquery.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueLike(Expr);

/// The common expressions are kept as they are; any other node is shared as a [`Fragment`].
#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Value(Value),
    /// Fixed syntax, such as `NONE` or `*`; empty for no expression.
    Syntax(&'static str),
    Table(Table),
    Field(Field),
    Param(Param),
    Operation(Operation),
    Node(Fragment),
}

impl ValueLike {
    /// Wraps any query node.
    pub fn new(node: impl Render + Send + Sync + 'static) -> Self {
        Self(Expr::Node(Fragment::new(node)))
    }

    /// A value, bound as a parameter when the query is rendered.
    pub fn value(value: impl SurrealValue) -> Self {
        Self(Expr::Value(value.into_value()))
    }

    /// Fixed SurrealQL syntax.
    pub(crate) const fn syntax(text: &'static str) -> Self {
        Self(Expr::Syntax(text))
    }

    /// A table name, written as an identifier rather than bound as a table value.
    pub(crate) fn table(table: Table) -> Self {
        Self(Expr::Table(table))
    }

    /// Whether the expression renders to nothing.
    pub fn is_empty(&self) -> bool {
        matches!(self.0, Expr::Syntax(""))
    }

    /// Renders the expression as an arithmetic operand: an operation is parenthesized so it binds
    /// before the surrounding operator.
    pub(crate) fn render_operand(&self, out: &mut Renderer) {
        if let Expr::Operation(operation) = &self.0 {
            out.push_str("(").push(operation).push_str(")");
        } else {
            self.render(out);
        }
    }
}

impl Render for ValueLike {
    fn render(&self, out: &mut Renderer) {
        match &self.0 {
            Expr::Value(value) => out.push_value(value),
            Expr::Syntax(text) => out.push_str(text),
            Expr::Table(table) => out.push(table),
            Expr::Field(field) => out.push(field),
            Expr::Param(param) => out.push(param),
            Expr::Operation(operation) => out.push(operation),
            Expr::Node(node) => out.push(node),
        };
    }
}

impl From<Operation> for ValueLike {
    fn from(value: Operation) -> Self {
        Self(Expr::Operation(value))
    }
}

impl From<Setter> for ValueLike {
    fn from(value: Setter) -> Self {
        Self(Expr::Operation(value.into()))
    }
}

impl<T: SurrealValue> From<T> for ValueLike {
    fn from(value: T) -> Self {
        Self::value(value)
    }
}

impl From<Field> for ValueLike {
    fn from(value: Field) -> Self {
        Self(Expr::Field(value))
    }
}

impl From<&Field> for ValueLike {
    fn from(value: &Field) -> Self {
        Self(Expr::Field(value.clone()))
    }
}

impl From<Param> for ValueLike {
    fn from(value: Param) -> Self {
        Self(Expr::Param(value))
    }
}

impl From<&Param> for ValueLike {
    fn from(value: &Param) -> Self {
        Self(Expr::Param(value.clone()))
    }
}

/// Implements `From<$source>` by sharing the source as a node.
macro_rules! value_like_from_node {
    ($($source:ty),* $(,)?) => {
        $(
            impl From<$source> for ValueLike {
                fn from(value: $source) -> Self {
                    Self::new(value)
                }
            }
        )*
    };
}

value_like_from_node!(Alias, Filter, Function);

impl From<Subquery> for ValueLike {
    fn from(value: Subquery) -> Self {
        Self(Expr::Node(value.into_fragment()))
    }
}

impl From<LetStatement> for ValueLike {
    fn from(value: LetStatement) -> Self {
        Self(Expr::Param(value.get_param()))
    }
}

impl From<&LetStatement> for ValueLike {
    fn from(value: &LetStatement) -> Self {
        Self(Expr::Param(value.get_param()))
    }
}

impl From<All> for ValueLike {
    fn from(_: All) -> Self {
        Self::syntax("*")
    }
}

impl From<NULL> for ValueLike {
    fn from(_: NULL) -> Self {
        Self::syntax("NULL")
    }
}

impl From<NONE> for ValueLike {
    fn from(_: NONE) -> Self {
        Self::syntax("NONE")
    }
}

impl From<E> for ValueLike {
    fn from(_: E) -> Self {
        Self::syntax("")
    }
}

impl<O: 'static, C: Cardinality> From<SelectStatement<O, C>> for ValueLike {
    fn from(statement: SelectStatement<O, C>) -> Self {
        Self::new(Subquery::from(statement))
    }
}

impl From<IfElseStatement> for ValueLike {
    fn from(statement: IfElseStatement) -> Self {
        Self::new(Subquery::from(statement))
    }
}

/// Implements `From<$statement>` for `ValueLike` by nesting the statement as a subquery, for every
/// return mode and cardinality.
macro_rules! value_like_from_write_statement {
    ($($bound:ident $statement:ident),* $(,)?) => {
        $(
            impl<T: $bound, R: ReturnMode, C: Cardinality> From<$statement<T, R, C>> for ValueLike {
                fn from(statement: $statement<T, R, C>) -> Self {
                    Self::new(Subquery::from(statement))
                }
            }
        )*
    };
}

value_like_from_write_statement!(
    Node CreateStatement,
    Model UpdateStatement,
    Model UpsertStatement,
    Model DeleteStatement,
    Edge RelateStatement,
    Edge RelateOrUpdateStatement,
    Model InsertStatement,
);

/// A macro to create a heterogenous list of anything
/// that can be converted into `ValueLike` including all
/// supported surrealdb types like numbers,
/// dates, field, param, geometry etc, and statements.
/// It creates Vec<ValueLike>` from a list of values.
///
/// # Arguments
/// * `$( $val:expr ),*` - A list of values that can be converted into `ValueLike`
/// # Example
/// ```rust
/// use umbriel::{*, functions::{math, count}, types::Duration};
/// let country = Field::new("country");
/// let age = Field::new("age");
/// let total = AliasName::new("total");
///
/// let values = arr![
///         1,
///         2,
///         3,
///         count!().__as__(total),
///         math::sum!(age),
///         country,
///         54,
///         Duration::from_secs(43)
///     ];
///
///    assert_eq!(
///         values.iter()
///             .map(|v| v.to_debug_sql())
///             .collect::<Vec<_>>()
///             .join(", "),
///         "1, 2, 3, count() AS total, math::sum(age), country, 54, 43s"
///    );
///    ```
#[macro_export]
macro_rules! arr {
    ($( $val:expr ),*) => {{
        vec![
            $( $crate::ValueLike::from($val) ),*
        ]
    }};
}
