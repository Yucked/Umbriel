use crate::{Expression, Renderer};

use super::{Statement, impl_statement};

/// Creates a THROW statement: it fails the query with the given value as its error. Any value
/// can be thrown, such as a message, an object, a parameter or a subquery's result; SurrealDB
/// reports it as text.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::throw};
/// assert_eq!(throw("some error message").to_debug_sql(), "THROW 'some error message';");
/// assert_eq!(throw(Param::new("reason")).to_debug_sql(), "THROW $reason;");
/// ```
pub fn throw(error: impl Into<Expression>) -> ThrowStatement {
    ThrowStatement {
        error: error.into(),
    }
}

/// Represents the THROW statement
pub struct ThrowStatement {
    error: Expression,
}

impl Statement for ThrowStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("THROW ").push(&self.error);
    }
}

impl_statement!(ThrowStatement);
