//! Markers for the SurrealQL kind an expression evaluates to.
//!
//! A [`Function`](crate::Function), [`Cast`](crate::Cast) or [`Constant`](crate::Constant) carries
//! one of these markers, so its result only enters the typed expressions that can hold it: a
//! `Function<kinds::Int>` (such as `string::len`) is a [`NumberLike`](crate::NumberLike) but not a
//! [`StrandLike`](crate::StrandLike). [`Any`] is the result of a call whose kind is not known
//! statically (`fn::`, `ml::`, `mod::` and embedded JavaScript calls, and functions returning an
//! element of a collection); it enters every typed expression, unchecked.
//!
//! Every marker gives its SurrealQL kind through [`KindOf::kind`], rendered as in
//! `DEFINE FIELD … TYPE`.

use std::marker::PhantomData;

use surrealdb::types::{GeometryKind, Kind, Table as TableName};

use crate::{
    ArrayLike, BoolLike, BytesLike, ClosureLike, DatetimeLike, DurationLike, GeometryLike, Model,
    NumberLike, ObjectLike, RecordIdLike, SetLike, StrandLike, TableLike,
};

mod sealed {
    pub trait Sealed {}
}

/// A kind marker: the SurrealQL kind an expression evaluates to. Sealed: the markers are the ones
/// in this module.
pub trait KindOf: sealed::Sealed + 'static {
    /// The SurrealQL kind, e.g. `array<int>` for `Array<Int>`.
    fn kind() -> Kind;
}

/// A marker whose results may enter the typed expression `T`. Sealed through [`KindOf`].
pub trait Enters<T>: KindOf {}

/// The tables a [`Record`] marker names: a model's table, or none for [`AnyRecord`].
pub trait RecordTables: 'static {
    /// The tables of the `record<…>` kind; empty for any table.
    fn tables() -> Vec<TableName>;
}

/// A record of any table: `record`.
#[derive(Debug, Clone, Copy)]
pub struct AnyRecord;

impl RecordTables for AnyRecord {
    fn tables() -> Vec<TableName> {
        vec![]
    }
}

impl<M: Model + 'static> RecordTables for M {
    fn tables() -> Vec<TableName> {
        vec![M::table()]
    }
}

/// Defines unit markers and their kinds.
macro_rules! markers {
    ($($(#[$attr:meta])* $name:ident => $kind:expr;)*) => {
        $(
            $(#[$attr])*
            #[derive(Debug, Clone, Copy)]
            pub struct $name;

            impl sealed::Sealed for $name {}

            impl KindOf for $name {
                fn kind() -> Kind {
                    $kind
                }
            }
        )*
    };
}

markers! {
    /// Any value: `any`. Its results enter every typed expression, unchecked.
    Any => Kind::Any;
    /// `bool`
    Bool => Kind::Bool;
    /// `int`
    Int => Kind::Int;
    /// `float`
    Float => Kind::Float;
    /// `decimal`
    Decimal => Kind::Decimal;
    /// `number`
    Number => Kind::Number;
    /// `string`
    String => Kind::String;
    /// `datetime`
    Datetime => Kind::Datetime;
    /// `duration`
    Duration => Kind::Duration;
    /// `uuid`
    Uuid => Kind::Uuid;
    /// `bytes`
    Bytes => Kind::Bytes;
    /// `object`
    Object => Kind::Object;
    /// `geometry`
    Geometry => Kind::Geometry(vec![]);
    /// `geometry<point>`
    Point => Kind::Geometry(vec![GeometryKind::Point]);
    /// `table`
    Table => Kind::Table(vec![]);
    /// `regex`
    Regex => Kind::Regex;
    /// `range`
    Range => Kind::Range;
}

/// `record<table>` for a model, or `record` for [`AnyRecord`].
pub struct Record<T: RecordTables = AnyRecord>(PhantomData<fn() -> T>);

impl<T: RecordTables> sealed::Sealed for Record<T> {}

impl<T: RecordTables> KindOf for Record<T> {
    fn kind() -> Kind {
        Kind::Record(T::tables())
    }
}

/// `array<T>`
pub struct Array<T: KindOf = Any>(PhantomData<fn() -> T>);

impl<T: KindOf> sealed::Sealed for Array<T> {}

impl<T: KindOf> KindOf for Array<T> {
    fn kind() -> Kind {
        Kind::Array(Box::new(T::kind()), None)
    }
}

/// `set<T>`
pub struct Set<T: KindOf = Any>(PhantomData<fn() -> T>);

impl<T: KindOf> sealed::Sealed for Set<T> {}

impl<T: KindOf> KindOf for Set<T> {
    fn kind() -> Kind {
        Kind::Set(Box::new(T::kind()), None)
    }
}

/// `option<T>`, which is `none | T`. Its results enter only [`ValueLike`](crate::ValueLike), since
/// the value may be `NONE`. Not named `Option`: a second public `Option` makes rustc write
/// `std::option::Option` in every diagnostic.
pub struct Optional<T: KindOf>(PhantomData<fn() -> T>);

impl<T: KindOf> sealed::Sealed for Optional<T> {}

impl<T: KindOf> KindOf for Optional<T> {
    fn kind() -> Kind {
        Kind::Either(vec![Kind::None, T::kind()])
    }
}

/// Declares which markers enter which typed expressions.
macro_rules! enters {
    ($($marker:ty => $($target:ty),+;)*) => {
        $($(impl Enters<$target> for $marker {})+)*
    };
}

enters! {
    Any => NumberLike, StrandLike, BoolLike, ArrayLike, SetLike, BytesLike, RecordIdLike,
        GeometryLike, DatetimeLike, DurationLike, ObjectLike, TableLike, ClosureLike;
    Int => NumberLike;
    Float => NumberLike;
    Decimal => NumberLike;
    Number => NumberLike;
    String => StrandLike;
    Bool => BoolLike;
    Bytes => BytesLike;
    Geometry => GeometryLike;
    Point => GeometryLike;
    Datetime => DatetimeLike;
    Duration => DurationLike;
    Object => ObjectLike;
    Table => TableLike;
}

impl<T: KindOf> Enters<ArrayLike> for Array<T> {}

impl<T: KindOf> Enters<SetLike> for Set<T> {}

impl<T: RecordTables> Enters<RecordIdLike> for Record<T> {}
