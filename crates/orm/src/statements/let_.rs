use std::borrow::Cow;

use surrealdb::types::{Kind, SurrealValue};

use crate::{Clause, Conditional, Expression, Operatable, Param, Renderer, SchemaGetter};

use super::{Statement, impl_statement};

/// Builds LET statement.
///
/// The LET statement sets and stores a value which can then be used in a subsequent query.
/// A parameter can store any value, including the result of a query.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::{let_, select}};
/// # let user = Table::new("user");
/// // You can assign a value surrealdb value
/// let_("name").equal_to(5);
///
/// // and even a select statement
/// let_("users").equal_to(select(All).from(user));
/// ```
///
/// An invalid name (see [`Param::check`]) fails when the query is rendered.
pub fn let_(name: impl Into<Cow<'static, str>>) -> LetStatement {
    LetStatement {
        parameter: Param::new(name),
        kind: None,
        value: None,
    }
}

#[macro_export]
/// Macro for creating a LET statement
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::{let_, select}};
/// # let ref user = Table::new("user");
/// // You can assign a value surrealdb value
/// let_!(name = 5);
/// // or
/// let_statement!(name = 5);
///
/// // and even a select statement
/// let_!(users = select(All).from(user));
/// // or
/// let_statement!(users = select(All).from(user));
/// ```
macro_rules! let_statement {
    ($param: ident =  $expr: expr) => {
        let $param = $crate::statements::let_(stringify!($param)).equal_to($expr);
    };
}

pub use let_statement as let_;

/// Let statement builder
#[derive(Debug, Clone)]
pub struct LetStatement {
    parameter: Param,
    kind: Option<Kind>,
    value: Option<Expression>,
}

/// As an operand or condition, a `LET` statement stands for the parameter it defines.
impl Operatable for LetStatement {}

impl Conditional for LetStatement {}

impl LetStatement {
    /// Assigned expression
    /// Examples
    /// ```rust
    /// use umbriel::{*, statements::{let_, select}};
    /// # let user = Table::new("user");
    /// // You can assign a value surrealdb value
    /// # let_("name")
    /// .equal_to(5);
    ///
    /// // and even a select statement
    /// # let_("users")
    /// .equal_to(select(All).from(user));
    pub fn equal_to(mut self, value: impl Into<Expression>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// `LET $name: type = …`: the value must be of `V`'s SurrealDB type, or the statement fails.
    ///
    /// The type is `V`'s own kind, such as `int`, `option<string>`, `array<int>` or
    /// `record<user>`. A model's kind is an object literal type, whose rendering is still being
    /// settled (Umbriel #28).
    ///
    /// # Example
    ///
    /// ```
    /// use umbriel::{*, statements::let_};
    ///
    /// let statement = let_("count").typed::<i64>().equal_to(5);
    /// assert_eq!(statement.to_debug_sql(), "LET $count: int = 5;");
    /// ```
    pub fn typed<V: SurrealValue>(mut self) -> Self {
        self.kind = Some(V::kind_of());
        self
    }

    /// helper function for getting assigned param from the LET statement
    pub fn get_param(&self) -> Param {
        self.parameter.clone()
    }

    /// For traversing from the param
    pub fn with_path<T: SchemaGetter>(&self, clause: impl Into<Clause>) -> T::Schema {
        self.parameter.with_path::<T>(clause)
    }
}

impl Statement for LetStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("LET ").push(&self.parameter);
        if let Some(kind) = &self.kind {
            out.push_str(": ").push(kind);
        }
        if let Some(value) = &self.value {
            out.push_str(" = ").push(value);
        }
    }
}

impl_statement!(LetStatement);
