//! `IF … ELSE`. Syntax: <https://surrealdb.com/docs/surrealql/statements/ifelse>.

use crate::{Conditional, Expression, Filter, QueryChain, Render, Renderer};

use super::{Statement, impl_statement};

/// Creates an IF ELSE statement with compile-time valid transition.
/// The IF ELSE statement can be used as a main statement, or within a parent statement,
/// to return a value depending on whether a condition, or a series of conditions match.
/// The statement allows for multiple ELSE IF expressions, and a final ELSE expression,
/// with no limit to the number of ELSE IF conditional expressions.
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::{if_, order, select}, types::RecordId};
///         # let age = Field::new("age");
/// // Can create simple if statement
/// if_(age.greater_than_or_equal(18))
///     .then("Valid".to_string())
///     .end();
///
/// // A bit complex if else statement
/// # let age = Field::new("age");
/// # let name = Field::new("name");
/// # let country = Field::new("country");
/// if_(age.greater_than_or_equal(18).less_than_or_equal(120))
///     .then("Valid")
///     .else_if(name.is("Alice"))
///     .then("The Alien!")
///     .else_if(cond(country.is("Canada")).or(country.is("Norway")))
///     .then("Cold")
///     .else_("Hot")
///     .end();
///
/// // And even with nested statements
/// # let name = Field::new("name");
/// # let age = Field::new("age");
/// # let country = Field::new("country");
/// # let city = Field::new("city");
/// # let fake_id = RecordId::new("user", "alice");
/// # let fake_id2 = RecordId::new("user", "bob");
///
/// let select1 = select(All)
///     .from(fake_id)
///     .where_(cond(city.is("Prince Edward Island"))
///                 .and(city.is("NewFoundland"))
///                 .or(city.is("Toronto"))
///     )
///     .order_by(order(&age).numeric())
///     .limit(153)
///     .start(10);
///
/// let select2 = select(All)
///     .from(fake_id2)
///     .where_(country.is("INDONESIA"))
///     .order_by(order(&age).numeric())
///     .limit(20)
///     .start(5);
///
///  if_(cond(age.greater_than_or_equal(18)).and(age.less_than_or_equal(120)))
///     .then(select1)
///     .else_if(name.is("Alice"))
///     .then(select2)
///     .else_if(cond(country.is("Canada")).or(country.is("Norway")))
///     .then("Cold")
///     .else_("Hot")
///     .end();
/// ```
pub fn if_(condition: impl Conditional) -> IfStatement {
    IfStatement {
        condition: Filter::new(condition),
    }
}

/// The body of an IF branch: an expression or a chain of statements.
#[derive(Debug, Clone)]
pub enum Body {
    /// An expression
    Expression(Expression),
    /// A chain of statements
    QueryChain(QueryChain),
}

impl<T: Into<Expression>> From<T> for Body {
    fn from(value: T) -> Self {
        Self::Expression(value.into())
    }
}

impl From<QueryChain> for Body {
    fn from(value: QueryChain) -> Self {
        Self::QueryChain(value)
    }
}

impl Render for Body {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::Expression(expr) => expr.render(out),
            Self::QueryChain(body) => body.render(out),
        }
    }
}

#[derive(Debug, Clone)]
struct Branch {
    condition: Filter,
    body: Body,
}

#[derive(Debug, Clone)]
struct Branches {
    if_: Branch,
    else_ifs: Vec<Branch>,
    else_: Option<Body>,
}

/// An IF statement awaiting its body.
#[derive(Debug, Clone)]
pub struct IfStatement {
    condition: Filter,
}

impl IfStatement {
    /// Can be a select statment or any other valid surrealdb Value
    pub fn then(self, body: impl Into<Body>) -> ThenExpression {
        ThenExpression(Branches {
            if_: Branch {
                condition: self.condition,
                body: body.into(),
            },
            else_ifs: vec![],
            else_: None,
        })
    }
}

/// An IF statement with a body, which may continue with `ELSE IF` or `ELSE`.
pub struct ThenExpression(Branches);

impl ThenExpression {
    /// Starts an `ELSE IF` branch.
    pub fn else_if(self, condition: impl Conditional) -> ElseIfStatement {
        ElseIfStatement {
            branches: self.0,
            condition: Filter::new(condition),
        }
    }

    /// Adds the final `ELSE` branch.
    pub fn else_(mut self, body: impl Into<Body>) -> ElseStatement {
        self.0.else_ = Some(body.into());
        ElseStatement(self.0)
    }

    /// Ends the statement.
    pub fn end(self) -> IfElseStatement {
        IfElseStatement(self.0)
    }
}

/// An `ELSE IF` branch awaiting its body.
pub struct ElseIfStatement {
    branches: Branches,
    condition: Filter,
}

impl ElseIfStatement {
    /// Sets the branch's body.
    pub fn then(mut self, body: impl Into<Body>) -> ThenExpression {
        self.branches.else_ifs.push(Branch {
            condition: self.condition,
            body: body.into(),
        });
        ThenExpression(self.branches)
    }
}

/// An IF statement with its final `ELSE` branch.
pub struct ElseStatement(Branches);

impl ElseStatement {
    /// Ends the statement.
    pub fn end(self) -> IfElseStatement {
        IfElseStatement(self.0)
    }
}

/// if else flow builder
#[derive(Debug, Clone)]
pub struct IfElseStatement(Branches);

impl Statement for IfElseStatement {
    fn render_body(&self, out: &mut Renderer) {
        let branches = &self.0;
        out.push_str("IF ")
            .push(&branches.if_.condition)
            .push_str(" \n\t{ ")
            .push(&branches.if_.body)
            .push_str(" }");

        for branch in &branches.else_ifs {
            out.push_str("\nELSE IF ")
                .push(&branch.condition)
                .push_str(" \n\t{ ")
                .push(&branch.body)
                .push_str(" }");
        }

        if let Some(body) = &branches.else_ {
            out.push_str("\nELSE\n\t{ ").push(body).push_str(" }");
        }
    }
}

impl_statement!(IfElseStatement);
