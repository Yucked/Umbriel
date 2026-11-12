use std::sync::Arc;

use crate::{
    Aliasable, ArrayLike, Conditional, GeometryLike, NumberLike, Ordinal, Render, Renderer, Setter,
    StrandLike, ValueLike,
};

/// A SurrealQL binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// `=`
    Equal,
    /// `!=`
    NotEqual,
    /// `==`
    Exact,
    /// `?=`
    AnyEqual,
    /// `*=`
    AllEqual,
    /// `<`
    LessThan,
    /// `<=`
    LessThanOrEqual,
    /// `>`
    MoreThan,
    /// `>=`
    MoreThanOrEqual,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `**`
    Pow,
    /// `&&`
    TruthyAnd,
    /// `||`
    TruthyOr,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `IS`
    Is,
    /// `IS NOT`
    IsNot,
    /// `CONTAINS`
    Contain,
    /// `CONTAINSNOT`
    NotContain,
    /// `CONTAINSALL`
    ContainAll,
    /// `CONTAINSANY`
    ContainAny,
    /// `CONTAINSNONE`
    ContainNone,
    /// `INSIDE`
    Inside,
    /// `NOTINSIDE`
    NotInside,
    /// `ALLINSIDE`
    AllInside,
    /// `ANYINSIDE`
    AnyInside,
    /// `NONEINSIDE`
    NoneInside,
    /// `OUTSIDE`
    Outside,
    /// `INTERSECTS`
    Intersects,
    /// `@@`
    Matches,
    /// `+=`
    Inc,
    /// `-=`
    Dec,
    /// `%`
    Modulo,
    /// `??`
    NullCoalescing,
    /// `?:`
    TernaryCondition,
}

impl Operator {
    /// The operator's SurrealQL token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::Exact => "==",
            Self::AnyEqual => "?=",
            Self::AllEqual => "*=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::MoreThan => ">",
            Self::MoreThanOrEqual => ">=",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Pow => "**",
            Self::TruthyAnd => "&&",
            Self::TruthyOr => "||",
            Self::And => "AND",
            Self::Or => "OR",
            Self::Is => "IS",
            Self::IsNot => "IS NOT",
            Self::Contain => "CONTAINS",
            Self::NotContain => "CONTAINSNOT",
            Self::ContainAll => "CONTAINSALL",
            Self::ContainAny => "CONTAINSANY",
            Self::ContainNone => "CONTAINSNONE",
            Self::Inside => "INSIDE",
            Self::NotInside => "NOTINSIDE",
            Self::AllInside => "ALLINSIDE",
            Self::AnyInside => "ANYINSIDE",
            Self::NoneInside => "NONEINSIDE",
            Self::Outside => "OUTSIDE",
            Self::Intersects => "INTERSECTS",
            Self::Matches => "@@",
            Self::Inc => "+=",
            Self::Dec => "-=",
            Self::Modulo => "%",
            Self::NullCoalescing => "??",
            Self::TernaryCondition => "?:",
        }
    }
}

impl Render for Operator {
    fn render(&self, out: &mut Renderer) {
        out.push_str(self.as_str());
    }
}

/// Defines the operations that can be performed on a field. Shared, since operations are reused as
/// operands, conditions and projections.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation(Arc<OperationNode>);

#[derive(Debug, PartialEq)]
enum OperationNode {
    /// `lhs <operator> rhs`
    Binary {
        lhs: ValueLike,
        operator: Operator,
        rhs: ValueLike,
    },
    /// `lhs <operator> rhs` with operation operands parenthesized, so each binds first.
    Arithmetic {
        lhs: ValueLike,
        operator: Operator,
        rhs: ValueLike,
    },
    /// `(lower < operand AND operand < upper)`, or with `<=` when inclusive.
    Between {
        lower: Ordinal,
        operand: ValueLike,
        upper: Ordinal,
        inclusive: bool,
    },
    /// `operand @reference@ terms`; the reference is part of the operator, so it is written as a
    /// literal.
    MatchesWithRef {
        operand: ValueLike,
        reference: u8,
        terms: StrandLike,
    },
    /// `(lhs ?? rhs)` or `(lhs ?: rhs)`: parenthesized as a whole, since these bind more loosely
    /// than `OR`, and with operation operands parenthesized too.
    Nullish {
        lhs: ValueLike,
        operator: Operator,
        rhs: ValueLike,
    },
    /// `!operand`, `!!operand` or `-operand`, with an operation operand parenthesized.
    Prefix {
        operator: &'static str,
        operand: ValueLike,
    },
}

impl Operation {
    fn new(node: OperationNode) -> Self {
        Self(Arc::new(node))
    }

    pub(crate) fn binary(lhs: ValueLike, operator: Operator, rhs: ValueLike) -> Self {
        Self::new(OperationNode::Binary { lhs, operator, rhs })
    }

    pub(crate) fn arithmetic(lhs: ValueLike, operator: Operator, rhs: ValueLike) -> Self {
        Self::new(OperationNode::Arithmetic { lhs, operator, rhs })
    }

    fn nullish(lhs: ValueLike, operator: Operator, rhs: ValueLike) -> Self {
        Self::new(OperationNode::Nullish { lhs, operator, rhs })
    }

    fn prefix(operator: &'static str, operand: ValueLike) -> Self {
        Self::new(OperationNode::Prefix { operator, operand })
    }
}

impl Render for Operation {
    fn render(&self, out: &mut Renderer) {
        match &*self.0 {
            OperationNode::Binary { lhs, operator, rhs } => {
                out.push(lhs)
                    .push_str(" ")
                    .push(operator)
                    .push_str(" ")
                    .push(rhs);
            }
            OperationNode::Arithmetic { lhs, operator, rhs } => {
                lhs.render_operand(out);
                out.push_str(" ").push(operator).push_str(" ");
                rhs.render_operand(out);
            }
            // SurrealQL does not chain relational operators, so the operand is compared twice.
            OperationNode::Between {
                lower,
                operand,
                upper,
                inclusive,
            } => {
                let operator = if *inclusive { " <= " } else { " < " };
                out.push_str("(")
                    .push(lower)
                    .push_str(operator)
                    .push(operand)
                    .push_str(" AND ")
                    .push(operand)
                    .push_str(operator)
                    .push(upper)
                    .push_str(")");
            }
            OperationNode::MatchesWithRef {
                operand,
                reference,
                terms,
            } => {
                out.push(operand)
                    .push_str(" @")
                    .push_display(reference)
                    .push_str("@ ")
                    .push(terms);
            }
            OperationNode::Nullish { lhs, operator, rhs } => {
                out.push_str("(");
                lhs.render_operand(out);
                out.push_str(" ").push(operator).push_str(" ");
                rhs.render_operand(out);
                out.push_str(")");
            }
            OperationNode::Prefix { operator, operand } => {
                out.push_str(operator);
                operand.render_operand(out);
            }
        }
    }
}

/// `(lower < operand AND operand < upper)`, or with `<=` when inclusive.
fn between(operand: ValueLike, lower: Ordinal, upper: Ordinal, inclusive: bool) -> Operation {
    Operation::new(OperationNode::Between {
        lower,
        operand,
        upper,
        inclusive,
    })
}

impl Aliasable for Operation {}

impl Operatable for Operation {}

impl Conditional for Operation {}

impl From<Setter> for Operation {
    fn from(setter: Setter) -> Self {
        let (field, operator, value) = setter.into_parts();
        Self::binary(field.into(), operator, value)
    }
}

/// Defines the operations that can be performed on a field or param or some other types
/// it is implemented for. The left operand is the node's expression form, so a `LET` statement
/// stands for its parameter.
pub trait Operatable: Sized + Render + Clone + Into<ValueLike> {
    /// `=`. Return a new `operation` that checks whether the field is equal to the specified value
    ///
    /// # Arguments
    ///
    /// * `value` - The value to check for equality. Could be a value, `Field` or `Param`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// let query = age.equal(25);
    /// assert_eq!(query.to_debug_sql(), "age = 25");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_field = Field::new("valid_age_field");
    /// let query = age.equal(valid_age_field);
    /// assert_eq!(query.to_debug_sql(), "age = valid_age_field");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_param = Param::new("valid_age_param");
    /// let query = age.equal(valid_age_param);
    /// assert_eq!(query.to_debug_sql(), "age = $valid_age_param");
    /// ```
    fn equal<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Equal, value)
    }

    /// `!=`. Return a new `DbQuery` that checks whether the field is not equal to the specified value
    ///
    /// # Arguments
    ///
    /// * `value` - The value to check for inequality. Could be a value, `Field` or `Param`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// let query = age.not_equal(25);
    /// assert_eq!(query.to_debug_sql(), "age != 25");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_field = Field::new("valid_age_field");
    /// let query = age.not_equal(valid_age_field);
    /// assert_eq!(query.to_debug_sql(), "age != valid_age_field");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_param = Param::new("valid_age_param");
    /// let query = age.not_equal(valid_age_param);
    /// assert_eq!(query.to_debug_sql(), "age != $valid_age_param");
    /// ```
    fn not_equal<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::NotEqual, value)
    }

    /// `!=`. Alias for `not_equal`. Return a new `DbQuery` that checks whether the field is not equal to the specified value
    ///
    /// # Arguments
    ///
    /// * `value` - The value to check for inequality. Could be a value, `Field` or `Param`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// let query = age.neq(25);
    /// assert_eq!(query.to_debug_sql(), "age != 25");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_field = Field::new("valid_age_field");
    /// let query = age.neq(valid_age_field);
    /// assert_eq!(query.to_debug_sql(), "age != valid_age_field");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_param = Param::new("valid_age_param");
    /// let query = age.neq(valid_age_param);
    /// assert_eq!(query.to_debug_sql(), "age != $valid_age_param");
    /// ```
    fn neq<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::NotEqual, value)
    }

    /// Constructs a query that checks whether the value of the column is exactly equal to the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against. Could be a value, `Field` or `Param`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// let query = age.exactly_equal(25);
    /// assert_eq!(query.to_debug_sql(), "age == 25");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_field = Field::new("valid_age_field");
    /// let query = age.exactly_equal(valid_age_field);
    /// assert_eq!(query.to_debug_sql(), "age == valid_age_field");
    ///
    /// # let age = Field::new("age");
    /// # let valid_age_param = Param::new("valid_age_param");
    /// let query = age.exactly_equal(valid_age_param);
    /// assert_eq!(query.to_debug_sql(), "age == $valid_age_param");
    /// ```
    fn exactly_equal<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
        Self: Sized,
    {
        self.operate(Operator::Exact, value)
    }

    /// Check whether any value in a arraa\y is equal to another value.
    ///
    /// # Arguments
    ///
    /// * `values` - An array of values to be checked for equality with the column. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// let query = friends.any_equal("Alice");
    /// assert_eq!(query.to_debug_sql(), "friends ?= 'Alice'");
    ///
    /// # let friends = Field::new("friends");
    /// # let valid_friends_field = Field::new("valid_friends_field");
    /// let query = friends.any_equal(valid_friends_field);
    /// assert_eq!(query.to_debug_sql(), "friends ?= valid_friends_field");
    ///
    /// # let friends = Field::new("friends");
    /// # let valid_friends_param = Param::new("valid_friends_param");
    /// let query = friends.any_equal(valid_friends_param);
    /// assert_eq!(query.to_debug_sql(), "friends ?= $valid_friends_param");
    /// ```
    fn any_equal<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::AnyEqual, value)
    }

    /// `*=` Check whether all values in an array is equals to another value.
    ///
    /// # Arguments
    ///
    /// * `values` - An array of values to be checked for equality with the column. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// friends.all_equal("Alice");
    /// ```
    fn all_equal<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::AllEqual, value)
    }

    /// `<` Check whether the value of the field is less than the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.less_than(30);
    /// ```
    fn less_than<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::LessThan, value)
    }

    /// `<`. Alias for `less_than`. Check whether the value of the field is less than the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.lt(30);
    /// ```
    fn lt<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::LessThan, value)
    }

    /// `<=` Check whether the value of the field is less than or equal to the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.less_than_or_equal(30);
    /// ```
    fn less_than_or_equal<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::LessThanOrEqual, value)
    }

    /// `<=`. Alias for `less_than_or_equal`. Check whether the value of the field is less than or equal to the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.lte(30);
    /// ```
    fn lte<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::LessThanOrEqual, value)
    }

    /// `>` Check whether the value of the field is greater than the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a number, geometry, datetime or duration, a `Field` or a `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.greater_than(30);
    /// ```
    fn greater_than<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::MoreThan, value)
    }

    /// `>`. Alias for `greater_than`. Check whether the value of the field is greater than the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a number, geometry, datetime or duration, a `Field` or a `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.gt(30);
    /// ```
    fn gt<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::MoreThan, value)
    }

    /// `>=` Check whether the value of the field is greater than or equal to the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a number, geometry, datetime or duration, a `Field` or a `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.greater_than_or_equal(30);
    /// ```
    fn greater_than_or_equal<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::MoreThanOrEqual, value)
    }

    /// `>=` Alias for `greater_than_or_equal`. Check whether the value of the field is greater than or equal to the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compare against the field. Could be a number, geometry, datetime or duration, a `Field` or a `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.gte(30);
    /// ```
    fn gte<T>(&self, value: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        let value: Ordinal = value.into();
        self.operate(Operator::MoreThanOrEqual, value)
    }

    /// `+` Adds a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be added to the current query. Could be `types::Value`, `Field` or `Param
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.add(5);
    /// ```
    fn add<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Add, value)
    }

    /// `+` Alias for `add`. Adds a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be added to the current query. Could be `types::Value`, `Field` or `Param
    ///
    /// # Example
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.plus(5);
    /// ```
    fn plus<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Add, value)
    }

    /// `-` Subtracts a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be subtract to the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.subtract(5);
    /// ```
    fn subtract<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Sub, value)
    }

    /// `-`. Alias for `subtract`. Subtracts a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be subtract to the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.minus(5);
    /// ```
    fn minus<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Sub, value)
    }

    /// `*` Multiply a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be multiply to the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.multiply(5);
    /// ```
    fn multiply<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Mul, value)
    }

    /// `*` Alias for `multiply`. Multiply a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be multiply to the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.mul(5);
    /// ```
    fn mul<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Mul, value)
    }

    /// `/` Divide a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be divide to the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.divide(5);
    /// ```
    fn divide<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Div, value)
    }

    /// `/` Alias for `divide`. Divide a value to the current query.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be divide to the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    /// ```rust
    /// # use umbriel::*;
    /// let age = Field::new("age");
    /// age.div(5);
    /// ```
    fn div<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Div, value)
    }

    /// `**` Raise the current query to the power of the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be raised to the power of the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let age = Field::new("age");
    /// age.power(5);
    /// ```
    fn power<T>(&self, value: T) -> Operation
    where
        T: Into<NumberLike>,
    {
        self.operate(Operator::Pow, value.into())
    }

    /// `**` Alias for `power`. Raise the current query to the power of the given value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be raised to the power of the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let age = Field::new("age");
    /// age.pow(5);
    /// ```
    fn pow<T>(&self, value: T) -> Operation
    where
        T: Into<NumberLike>,
    {
        self.operate(Operator::Pow, value.into())
    }

    /// `&&` Checks whether two values are truthy.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.truthy_and(5);
    /// ```
    fn truthy_and<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::TruthyAnd, value)
    }

    /// `||` Checks whether either of two values are truthy.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.truthy_or(5);
    /// ```
    fn truthy_or<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::TruthyOr, value)
    }

    /// `AND` Checks whether two values are truthy.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.and(5);
    /// ```
    fn and<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::And, value)
    }

    /// `OR` Checks whether either of two values are truthy.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.or(5);
    /// ```
    fn or<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Or, value)
    }

    /// `IS` or `=` Check whether two values are equal.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.is(5);
    /// ```
    fn is<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Is, value)
    }

    /// `IS NOT` or `!=` Check whether two values are not equal.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.is_not(5);
    /// ```
    fn is_not<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::IsNot, value)
    }

    /// `CONTAINS` or `∋` Check whether a value contains another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be a value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// friends.contains("Charlie");
    ///
    /// # let polygon = Field::new("polygon");
    /// polygon.contains("Charlie");
    /// ```
    fn contains<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        let value: ValueLike = value.into();
        self.operate(Operator::Contain, value)
    }

    /// `CONTAINSNOT` or `∌` Check whether a value does not contain another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be any value, `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// friends.contains_not("Charlie");
    /// ```
    fn contains_not<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::NotContain, value)
    }

    /// `CONTAINSALL` or `⊇` Check whether a value contains all of multiple values.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be `Array, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// friends.contains_all(&["Charlie", "Charlie"]);
    /// ```
    fn contains_all<T>(&self, value: T) -> Operation
    where
        T: Into<ArrayLike>,
    {
        let value = value.into();
        self.operate(Operator::ContainAll, value)
    }

    /// `CONTAINSANY` or `⊃` Check whether a value contains any of multiple values.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be `Array, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// friends.contains_any(&["Charlie", "Charlie"]);
    /// ```
    fn contains_any<T>(&self, value: T) -> Operation
    where
        T: Into<ArrayLike>,
    {
        let value = value.into();
        self.operate(Operator::ContainAny, value)
    }

    /// `CONTAINSNONE` or `⊅` Check whether a value does not contain none of multiple values.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be `Array, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let friends = Field::new("friends");
    /// friends.contains_none(&["Charlie", "Charlie"]);
    /// ```
    fn contains_none<T>(&self, value: T) -> Operation
    where
        T: Into<ArrayLike>,
    {
        let value = value.into();
        self.operate(Operator::ContainNone, value)
    }

    /// `INSIDE` or `∈` or `IN` Check whether a value is contained within another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be any supported surrealdb value, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.inside(10);
    /// ```
    fn inside<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Inside, value)
    }

    /// `IN` or `∈` or `INSIDE` Check whether a value is contained within another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be any supported surrealdb value, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// # let age = Field::new("age");
    /// age.in_(10);
    /// ```
    fn in_<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::Inside, value)
    }

    /// `NOTINSIDE` or `∉` or `NOT IN` Check whether a value is not contained within another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be any supported surrealdb value, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// use umbriel::types::Geometry;
    /// use geo::polygon;
    /// let point = Field::new("point");
    /// let polygon = Geometry::Polygon(polygon!(
    ///             exterior: [
    ///                 (x: -111., y: 45.),
    ///                 (x: -111., y: 41.),
    ///                 (x: -104., y: 41.),
    ///                 (x: -104., y: 45.),
    ///             ],
    ///             interiors: [
    ///                 [
    ///                     (x: -110., y: 44.),
    ///                     (x: -110., y: 42.43),
    ///                     (x: -105., y: 42.),
    ///                     (x: -105., y: 44.),
    ///                 ],
    ///             ],
    ///         ));
    /// point.not_inside(polygon);
    /// ```
    fn not_inside<T>(&self, value: T) -> Operation
    where
        T: Into<ValueLike>,
    {
        self.operate(Operator::NotInside, value)
    }

    /// `ALLINSIDE` or `⊆` Check whether all of multiple values are contained within another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be `Array, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let numbers = Field::new("numbers");
    /// numbers.all_inside(&[10, 20, 10]);
    /// ```
    fn all_inside<T>(&self, value: T) -> Operation
    where
        T: Into<ArrayLike>,
    {
        let value = value.into();
        self.operate(Operator::AllInside, value)
    }

    /// `ANYINSIDE` or `⊂` Check whether any of multiple values are contained within another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be `Array, `Field` or `Param.C
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let numbers = Field::new("numbers");
    /// numbers.any_inside(&[10, 20, 10]);
    /// ```
    fn any_inside<T>(&self, value: T) -> Operation
    where
        T: Into<ArrayLike>,
    {
        let value = value.into();
        self.operate(Operator::AnyInside, value)
    }

    /// `NONEINSIDE` or `⊄` Check whether none of multiple values are contained within another value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be `Array, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let numbers = Field::new("numbers");
    /// numbers.none_inside(vec![10, 20, 10]);
    /// ```
    fn none_inside<T>(&self, value: T) -> Operation
    where
        T: Into<ArrayLike>,
    {
        let value = value.into();
        self.operate(Operator::NoneInside, value)
    }

    /// `OUTSIDE` Check whether a geometry value is outside another geometry value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be Geometry, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// use geo::polygon;
    ///
    /// let location = Field::new("location");
    /// let polygon_variable = polygon!(
    ///        exterior: [
    ///        (x: -0.38314819, y: 51.37692386),
    ///        (x: 0.1785278, y: 51.37692386),
    ///
    ///        (x: 0.1785278, y: 51.61460570),
    ///        (x: -0.38314819, y: 51.61460570),
    ///        (x: -0.38314819, y: 51.37692386),
    ///        ],
    ///        interiors: [],
    ///     );
    /// location.outside(polygon_variable);
    /// ```
    fn outside<T>(&self, value: T) -> Operation
    where
        T: Into<GeometryLike>,
    {
        let value: GeometryLike = value.into();
        self.operate(Operator::Outside, value)
    }

    /// `INTERSECTS` Check whether a geometry value intersects annother geometry value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to compared with the current query. Could be Geometry, `Field` or `Param.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    /// use geo::polygon;
    ///
    /// let location = Field::new("location");
    /// let polygon_variable = polygon!(
    ///        exterior: [
    ///        (x: -0.38314819, y: 51.37692386),
    ///        (x: 0.1785278, y: 51.37692386),
    ///
    ///        (x: 0.1785278, y: 51.61460570),
    ///        (x: -0.38314819, y: 51.61460570),
    ///        (x: -0.38314819, y: 51.37692386),
    ///        ],
    ///        interiors: [],
    ///     );
    /// location.intersects(polygon_variable);
    /// ```
    fn intersects<T>(&self, value: T) -> Operation
    where
        T: Into<GeometryLike>,
    {
        let value: GeometryLike = value.into();
        self.operate(Operator::Intersects, value)
    }

    /// `@@` Checks whether the terms are found in a full-text indexed field
    ///
    /// # Arguments
    ///
    /// * `terms` - The terms to search for in the full-text indexed field.
    ///   can also be a `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    ///
    /// # let content = Field::new("content");
    /// let query = content.matches("hello world");
    /// assert_eq!(query.to_debug_sql(), "content @@ 'hello world'");
    /// ```
    fn matches<T>(&self, terms: T) -> Operation
    where
        T: Into<StrandLike>,
    {
        self.operate(Operator::Matches, terms.into())
    }

    /// `@[ref]@` Same as `matches` but using the matches operator with a reference checks whether the terms are found,
    /// highlights the searched terms, and computes the full-text score.
    ///
    /// # Arguments
    ///
    /// * `reference` - The match reference number, used by `search::highlight` and `search::score`.
    ///   It is part of the operator, so it is written as a literal.
    /// * `terms` - The terms to search for in the full-text indexed field.
    ///   can also be a `Field` or `Param`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    ///
    /// # let content = Field::new("content");
    ///
    /// let query = content.matches_with_ref(1, "hello world");
    /// assert_eq!(query.to_debug_sql(), "content @1@ 'hello world'");
    /// ```
    fn matches_with_ref<T>(&self, reference: u8, terms: T) -> Operation
    where
        T: Into<StrandLike>,
    {
        Operation::new(OperationNode::MatchesWithRef {
            operand: self.clone().into(),
            reference,
            terms: terms.into(),
        })
    }

    /// Check whether the value of the field is between the given lower and upper bounds.
    ///
    /// # Arguments
    ///
    /// * `lower_bound` - The lower bound to compare against the field.
    /// * `upper_bound` - The upper bound to compare against the field.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    ///
    /// let age = Field::new("age");
    /// assert_eq!(age.between(18, 30).to_debug_sql(), "(18 < age AND age < 30)");
    /// ```
    fn between<T>(&self, lower_bound: T, upper_bound: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        between(
            self.clone().into(),
            lower_bound.into(),
            upper_bound.into(),
            false,
        )
    }

    /// Check whether the value of the field is between the given lower and upper bounds.
    ///
    /// # Arguments
    ///
    /// * `lower_bound` - The lower bound to compare against the field.
    /// * `upper_bound` - The upper bound to compare against the field.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use umbriel::*;
    ///
    /// let age = Field::new("age");
    /// assert_eq!(age.within(18, 30).to_debug_sql(), "(18 <= age AND age <= 30)");
    /// ```
    fn within<T>(&self, lower_bound: T, upper_bound: T) -> Operation
    where
        T: Into<Ordinal>,
    {
        between(
            self.clone().into(),
            lower_bound.into(),
            upper_bound.into(),
            true,
        )
    }

    /// `(self ?? value)`: `self` unless it is `NONE` or `NULL`, otherwise `value`. Written in
    /// parentheses, since `??` binds more loosely than every other operator.
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let nickname = Field::new("nickname");
    /// assert_eq!(nickname.coalesce("anonymous").to_debug_sql(), "(nickname ?? 'anonymous')");
    /// ```
    fn coalesce<T: Into<ValueLike>>(&self, value: T) -> Operation {
        Operation::nullish(self.clone().into(), Operator::NullCoalescing, value.into())
    }

    /// `(self ?: value)`: `self` when it is truthy, otherwise `value`. Written in parentheses, since
    /// `?:` binds more loosely than every other operator.
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let name = Field::new("name");
    /// assert_eq!(name.or_else_truthy("unnamed").to_debug_sql(), "(name ?: 'unnamed')");
    /// ```
    fn or_else_truthy<T: Into<ValueLike>>(&self, value: T) -> Operation {
        Operation::nullish(self.clone().into(), Operator::TernaryCondition, value.into())
    }

    /// `self % value`: the remainder, with the sign of `self`.
    ///
    /// ```rust
    /// # use umbriel::*;
    /// let count = Field::new("count");
    /// assert_eq!(count.modulo(3).to_debug_sql(), "count % 3");
    /// ```
    fn modulo<T: Into<ValueLike>>(&self, value: T) -> Operation {
        Operation::arithmetic(self.clone().into(), Operator::Modulo, value.into())
    }

    /// `self <operator> value`.
    fn operate(&self, operator: Operator, value: impl Into<ValueLike>) -> Operation {
        Operation::binary(self.clone().into(), operator, value.into())
    }
}

/// `!value`: `true` when `value` is falsy. Also SurrealDB's `not()` function.
///
/// ```rust
/// # use umbriel::*;
/// assert_eq!(not(Field::new("active")).to_debug_sql(), "!active");
/// ```
pub fn not(value: impl Into<ValueLike>) -> Operation {
    Operation::prefix("!", value.into())
}

/// `!!value`: whether `value` is truthy, as a boolean.
///
/// ```rust
/// # use umbriel::*;
/// assert_eq!(truthy(Field::new("tags")).to_debug_sql(), "!!tags");
/// ```
pub fn truthy(value: impl Into<ValueLike>) -> Operation {
    Operation::prefix("!!", value.into())
}

/// `-value`: the negated number or duration.
///
/// ```rust
/// # use umbriel::*;
/// let balance = Field::new("balance");
/// assert_eq!(negate(balance.add(1)).to_debug_sql(), "-(balance + 1)");
/// ```
pub fn negate(value: impl Into<ValueLike>) -> Operation {
    Operation::prefix("-", value.into())
}
