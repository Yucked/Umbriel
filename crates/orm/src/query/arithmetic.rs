use std::ops::{Add, Div, Mul, Sub};

use crate::{Field, NumberLike, Operation, Operator, ValueLike, statements::LetStatement};

macro_rules! impl_arithmetic {
    ($lhs:ty => $($trait:ident :: $method:ident ($operator:ident)),* $(,)?) => {
        $(
            impl<T: Into<NumberLike>> $trait<T> for $lhs {
                type Output = Operation;

                fn $method(self, rhs: T) -> Self::Output {
                    let rhs: NumberLike = rhs.into();
                    Operation::arithmetic(ValueLike::from(self), Operator::$operator, rhs.into())
                }
            }
        )*
    };
}

// A `LET` statement stands for its parameter; an operation on the left is parenthesized so it
// binds before the new operator.
macro_rules! impl_all_arithmetic {
    ($($lhs:ty),* $(,)?) => {
        $(impl_arithmetic!($lhs => Add::add(Add), Sub::sub(Sub), Mul::mul(Mul), Div::div(Div));)*
    };
}

impl_all_arithmetic!(LetStatement, &LetStatement, Field, &Field, Operation);
