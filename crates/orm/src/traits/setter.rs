use std::{marker::PhantomData, ops::Deref};

use surrealdb::types::SurrealValue;

use crate::{
    Block, Conditional, Edge, Field, Filter, IdiomPart, Model, Node, NumberLike, Operator, Param,
    Render, Renderer, ValueLike,
    statements::{
        Cardinality, CreateStatement, DeleteStatement, IfElseStatement, InsertStatement,
        LetStatement, RelateOrUpdateStatement, RelateStatement, ReturnMode, SelectStatement,
        Subquery, UpdateStatement, UpsertStatement,
    },
};

/// One element of an array field, typed by the element: `tags.at(0)` or
/// `tags.filter(cond)` (`tags[0]`, `tags[WHERE …]`), assignable like a field of type `T`.
#[derive(Debug, Clone)]
pub struct ElementField<T> {
    field: Field,
    element: PhantomData<fn() -> T>,
}

impl<T> ElementField<T> {
    fn new(field: Field) -> Self {
        Self {
            field,
            element: PhantomData,
        }
    }
}

impl<T> Deref for ElementField<T> {
    type Target = Field;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<T> From<ElementField<T>> for Field {
    fn from(value: ElementField<T>) -> Self {
        value.field
    }
}

impl<T> From<&ElementField<T>> for Field {
    fn from(value: &ElementField<T>) -> Self {
        value.field.clone()
    }
}

impl<T> From<ElementField<T>> for ValueLike {
    fn from(value: ElementField<T>) -> Self {
        value.field.into()
    }
}

impl<T> Render for ElementField<T> {
    fn render(&self, out: &mut Renderer) {
        out.push(&self.field);
    }
}

impl<T: SurrealValue> SetterAssignable<T> for ElementField<T> {}

impl<T: SurrealValue + Into<NumberLike>> SetterNumeric<T> for ElementField<T> {}

/// An assignment, e.g. `name = $_p0`, used in `SET` clauses.
#[derive(Debug, Clone, PartialEq)]
pub struct Setter {
    field: Field,
    operator: Operator,
    value: ValueLike,
}

impl Setter {
    pub(crate) fn new(field: Field, operator: Operator, value: ValueLike) -> Self {
        Self {
            field,
            operator,
            value,
        }
    }

    pub(crate) fn into_parts(self) -> (Field, Operator, ValueLike) {
        (self.field, self.operator, self.value)
    }
}

impl Render for Setter {
    fn render(&self, out: &mut Renderer) {
        out.push(&self.field)
            .push_str(" ")
            .push(&self.operator)
            .push_str(" ")
            .push(&self.value);
    }
}

impl Conditional for Setter {}

impl From<Setter> for Vec<Setter> {
    fn from(value: Setter) -> Self {
        vec![value]
    }
}

/// The right-hand side of an assignment: a value of the field's type, or an expression.
pub enum SetterArg<T> {
    /// A value, bound as a parameter.
    Value(T),
    /// A field, parameter, block or subquery.
    Expression(ValueLike),
}

// `V: SurrealValue` keeps this disjoint from the expression impls below: those types are local and
// never values.
impl<T: SurrealValue, V: Into<T> + SurrealValue> From<V> for SetterArg<T> {
    fn from(value: V) -> Self {
        Self::Value(value.into())
    }
}

/// Implements `From<$source>` for `SetterArg` by rendering the source as an expression.
macro_rules! setter_arg_from_expression {
    ($($source:ty),* $(,)?) => {
        $(
            impl<T> From<$source> for SetterArg<T> {
                fn from(value: $source) -> Self {
                    Self::Expression(value.into())
                }
            }
        )*
    };
}

setter_arg_from_expression!(
    Field,
    &Field,
    Param,
    &Param,
    LetStatement,
    &LetStatement,
    Subquery,
    SelectStatement,
    IfElseStatement,
);

impl<T> From<Block> for SetterArg<T> {
    fn from(value: Block) -> Self {
        Self::Expression(Subquery::from(value).into())
    }
}

impl<T> From<&Block> for SetterArg<T> {
    fn from(value: &Block) -> Self {
        Self::Expression(Subquery::from(value.clone()).into())
    }
}

/// Implements `From<$statement>` for `SetterArg` by nesting the write statement as a subquery,
/// for every return mode and cardinality.
macro_rules! setter_arg_from_write_statement {
    ($($bound:ident $statement:ident),* $(,)?) => {
        $(
            impl<T, V: $bound, R: ReturnMode, C: Cardinality> From<$statement<V, R, C>>
                for SetterArg<T>
            {
                fn from(value: $statement<V, R, C>) -> Self {
                    Self::Expression(value.into())
                }
            }
        )*
    };
}

setter_arg_from_write_statement!(
    Node CreateStatement,
    Model UpdateStatement,
    Model UpsertStatement,
    Model DeleteStatement,
    Edge RelateStatement,
    Edge RelateOrUpdateStatement,
    Model InsertStatement,
);

fn assign<T: SurrealValue>(
    field: &Field,
    operator: Operator,
    value: impl Into<SetterArg<T>>,
) -> Setter {
    let value = match value.into() {
        SetterArg::Value(value) => ValueLike::value(value),
        SetterArg::Expression(expression) => expression,
    };
    Setter::new(field.clone(), operator, value)
}

/// A trait for assigning values to a field used in `SET`
/// function in create and update statements.
pub trait SetterAssignable<T: SurrealValue>
where
    Self: std::ops::Deref<Target = Field>,
{
    /// Assigns the given value to the field.
    fn equal_to(&self, value: impl Into<SetterArg<T>>) -> Setter {
        assign(self.deref(), Operator::Equal, value)
    }

    /// Derefs to field type.
    fn to_field(&self) -> Field {
        self.deref().clone()
    }
}

/// A trait for incrementing or decrementing values to a field used in `SET`
/// function in create and update statements.
pub trait SetterNumeric<T: SurrealValue>
where
    Self: std::ops::Deref<Target = Field>,
{
    /// Increments the value of the field by the given value.
    fn increment_by(&self, value: impl Into<SetterArg<T>>) -> Setter {
        assign(self.deref(), Operator::Inc, value)
    }

    /// Decrements the value of the field by the given value.
    fn decrement_by(&self, value: impl Into<SetterArg<T>>) -> Setter {
        assign(self.deref(), Operator::Dec, value)
    }

    /// Derefs to field type.
    fn to_field(&self) -> Field {
        self.deref().clone()
    }
}

/// Setter for array fields.
pub trait SetterArray<T: SurrealValue>
where
    Self: std::ops::Deref<Target = Field>,
{
    /// Appends the given value to the array.
    fn append(&self, value: impl Into<SetterArg<T>>) -> Setter {
        assign(self.deref(), Operator::Inc, value)
    }

    /// Removes the given value from the array.
    fn remove(&self, value: impl Into<SetterArg<T>>) -> Setter {
        assign(self.deref(), Operator::Dec, value)
    }

    /// The element at `index`, `field[index]`, assignable as a `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use umbriel::*;
    /// # struct Tags(Field);
    /// # impl std::ops::Deref for Tags { type Target = Field; fn deref(&self) -> &Field { &self.0 } }
    /// # impl SetterArray<String> for Tags {}
    /// # let tags = Tags(Field::new("tags"));
    /// let first = tags.at(0).equal_to("rust".to_owned());
    /// assert_eq!(first.to_debug_sql(), "tags[0] = 'rust'");
    /// ```
    fn at(&self, index: impl Into<NumberLike>) -> ElementField<T> {
        let mut field = self.deref().clone();
        field.push(IdiomPart::Index(index.into()));
        ElementField::new(field)
    }

    /// The elements matching `condition`, `field[WHERE condition]`, assignable as a `T`.
    fn filter(&self, condition: impl Conditional) -> ElementField<T> {
        let mut field = self.deref().clone();
        field.push(IdiomPart::Where(Filter::new(condition)));
        ElementField::new(field)
    }

    /// Derefs to field type.
    fn to_field(&self) -> Field {
        self.deref().clone()
    }
}
