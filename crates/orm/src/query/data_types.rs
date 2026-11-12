use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use surrealdb::types::{self, SurrealValue};

use crate::{
    Cast, Closure, Constant, Edge, Field, Function, Model, Node, Operation, Param, RecordId,
    Render, Renderer, ValueLike,
    kinds::Enters,
    statements::{
        Cardinality, CreateStatement, DeleteStatement, IfElseStatement, InsertStatement,
        LetStatement, RelateOrUpdateStatement, RelateStatement, ReturnMode, SelectStatement,
        Subquery, UpdateStatement, UpsertStatement,
    },
};

/// Database index name
pub type IndexName = Field;

/// Represents the surrealdb NULL value
#[derive(Debug, Clone, Copy)]
pub struct NULL;

impl Render for NULL {
    fn render(&self, out: &mut Renderer) {
        out.push_str("NULL");
    }
}

/// Represents the surrealdb NONE value
#[derive(Debug, Clone, Copy)]
pub struct NONE;

impl Render for NONE {
    fn render(&self, out: &mut Renderer) {
        out.push_str("NONE");
    }
}

/// Defines an expression type restricted to one kind of value: the listed Rust types bind as
/// values, and fields, parameters, operations and subqueries stand for a value of that kind. A
/// function call, cast or constant enters it only when its kind marker does (see
/// [`kinds`](crate::kinds)).
macro_rules! typed_expression {
    ($(#[$attr:meta])* $name:ident: $($value:ty),* $(,)?) => {
        $(#[$attr])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(ValueLike);

        impl Render for $name {
            fn render(&self, out: &mut Renderer) {
                self.0.render(out);
            }
        }

        impl From<$name> for ValueLike {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        $(
            impl From<$value> for $name {
                fn from(value: $value) -> Self {
                    Self(ValueLike::value(value))
                }
            }
        )*

        typed_expression!(@expressions $name: Field, &Field, Param, &Param, LetStatement, &LetStatement,
            Operation, Subquery, IfElseStatement);

        impl<K: Enters<$name>> From<Function<K>> for $name {
            fn from(value: Function<K>) -> Self {
                Self(value.into())
            }
        }

        impl<K: Enters<$name>> From<Cast<K>> for $name {
            fn from(value: Cast<K>) -> Self {
                Self(value.into())
            }
        }

        impl<K: Enters<$name>> From<Constant<K>> for $name {
            fn from(value: Constant<K>) -> Self {
                Self(value.into())
            }
        }

        impl<O: 'static, C: Cardinality> From<SelectStatement<O, C>> for $name {
            fn from(statement: SelectStatement<O, C>) -> Self {
                Self(statement.into())
            }
        }

        typed_expression!(@statements $name: Node CreateStatement, Model UpdateStatement,
            Model UpsertStatement, Model DeleteStatement, Edge RelateStatement,
            Edge RelateOrUpdateStatement, Model InsertStatement);
    };
    (@statements $name:ident: $($bound:ident $statement:ident),*) => {
        $(
            impl<T: $bound, R: ReturnMode, C: Cardinality> From<$statement<T, R, C>> for $name {
                fn from(statement: $statement<T, R, C>) -> Self {
                    Self(statement.into())
                }
            }
        )*
    };
    (@expressions $name:ident: $($source:ty),*) => {
        $(
            impl From<$source> for $name {
                fn from(value: $source) -> Self {
                    Self(value.into())
                }
            }
        )*
    };
}

typed_expression!(
    /// A number, or an expression standing for one.
    NumberLike: i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64, types::Number,
        types::Decimal
);

typed_expression!(
    /// A string, or an expression standing for one.
    StrandLike: String, &str
);

typed_expression!(
    /// A geometry, or an expression standing for one.
    GeometryLike: types::Geometry, geo::Point<f64>, geo::LineString<f64>, geo::Polygon<f64>,
        geo::MultiPoint<f64>, geo::MultiLineString<f64>, geo::MultiPolygon<f64>
);

typed_expression!(
    /// A record id, or an expression standing for one.
    RecordIdLike: types::RecordId
);

typed_expression!(
    /// A duration, or an expression standing for one.
    DurationLike: types::Duration, std::time::Duration
);

typed_expression!(
    /// A datetime, or an expression standing for one.
    DatetimeLike: types::Datetime, chrono::DateTime<chrono::Utc>
);

typed_expression!(
    /// A table value, or an expression standing for one.
    TableLike: types::Table
);

typed_expression!(
    /// An object, or an expression standing for one.
    ObjectLike: types::Object
);

typed_expression!(
    /// A boolean, or an expression standing for one.
    BoolLike: bool
);

typed_expression!(
    /// An array, or an expression standing for one. Sets are not arrays: see [`SetLike`].
    ArrayLike: types::Array
);

typed_expression!(
    /// A set, or an expression standing for one. `HashSet` and `BTreeSet` bind as sets, not as
    /// the arrays the SDK writes them as; arrays and `Vec`s are not sets.
    SetLike: types::Set
);

typed_expression!(
    /// Bytes, or an expression standing for them. `Vec<u8>` and `&[u8]` bind as bytes here, where
    /// they would otherwise be `array<int>`.
    BytesLike: types::Bytes
);

typed_expression!(
    /// A closure, or an expression standing for one: a parameter holding it, or a call whose
    /// result is untyped.
    ClosureLike:
);

impl From<geo::Geometry<f64>> for GeometryLike {
    fn from(value: geo::Geometry<f64>) -> Self {
        Self(ValueLike::value(types::Geometry::from(value)))
    }
}

impl<T: Model> From<RecordId<T>> for RecordIdLike {
    fn from(value: RecordId<T>) -> Self {
        Self(ValueLike::value(value))
    }
}

impl<T: Model> From<&RecordId<T>> for RecordIdLike {
    fn from(value: &RecordId<T>) -> Self {
        Self(ValueLike::value(value.clone()))
    }
}

impl<V: SurrealValue> From<BTreeMap<String, V>> for ObjectLike {
    fn from(value: BTreeMap<String, V>) -> Self {
        Self(ValueLike::value(value))
    }
}

impl<V: SurrealValue> From<HashMap<String, V>> for ObjectLike {
    fn from(value: HashMap<String, V>) -> Self {
        Self(ValueLike::value(value))
    }
}

impl<T: SurrealValue> From<Vec<T>> for ArrayLike {
    fn from(value: Vec<T>) -> Self {
        Self(ValueLike::value(value))
    }
}

impl<const N: usize, T: SurrealValue> From<[T; N]> for ArrayLike {
    fn from(value: [T; N]) -> Self {
        Self(ValueLike::value(Vec::from(value)))
    }
}

impl<const N: usize, T: SurrealValue + Clone> From<&[T; N]> for ArrayLike {
    fn from(value: &[T; N]) -> Self {
        Self(ValueLike::value(value.to_vec()))
    }
}

/// An array literal whose elements are expressions, e.g. `[1, age, $limit]`.
impl From<Vec<ValueLike>> for ArrayLike {
    fn from(values: Vec<ValueLike>) -> Self {
        Self(ValueLike::new(ArrayLiteral(values)))
    }
}

struct ArrayLiteral(Vec<ValueLike>);

impl Render for ArrayLiteral {
    fn render(&self, out: &mut Renderer) {
        out.push_str("[").push(&self.0).push_str("]");
    }
}

/// The set of `values`, converted element by element.
fn set_of<T: SurrealValue>(values: impl IntoIterator<Item = T>) -> SetLike {
    let values: Vec<types::Value> = values.into_iter().map(SurrealValue::into_value).collect();
    SetLike(ValueLike::value(types::Set::from(values)))
}

impl<T: SurrealValue> From<HashSet<T>> for SetLike {
    fn from(values: HashSet<T>) -> Self {
        set_of(values)
    }
}

impl<T: SurrealValue> From<BTreeSet<T>> for SetLike {
    fn from(values: BTreeSet<T>) -> Self {
        set_of(values)
    }
}

impl From<Vec<u8>> for BytesLike {
    fn from(bytes: Vec<u8>) -> Self {
        Self(ValueLike::value(types::Bytes::from(bytes)))
    }
}

impl From<&[u8]> for BytesLike {
    fn from(bytes: &[u8]) -> Self {
        Self(ValueLike::value(types::Bytes::from(bytes.to_vec())))
    }
}

impl From<Closure> for ClosureLike {
    fn from(closure: Closure) -> Self {
        Self(closure.into())
    }
}
