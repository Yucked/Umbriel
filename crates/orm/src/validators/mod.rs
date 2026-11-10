//! Compile-time checks emitted by the model macros: each `assert_*` function only compiles when
//! its type argument satisfies the corresponding marker trait.

use std::collections::{BTreeSet, HashSet};

use surrealdb::types::{self, SurrealValue};

use crate::{LinkMany, LinkOne, LinkSelf, Node, Object, RecordId};

/// Checks that struct has all the fields specified
#[macro_export]
macro_rules! assert_fields {
    ($ty:ty: $($field:ident),+) => {
        {
            let _ = |_: $ty| {};
            let _ = |val: $ty| {
                $(
                    let _ = val.$field;
                )+
            };
        }
    };
}
pub use assert_fields;

#[macro_export]
/// Checks that two types are equal
macro_rules! assert_type_eq_all {
    ($a:ty, $b:ty) => {{
        // A closure, unlike a nested fn, may name the enclosing item's generic parameters.
        let _ = |value: $a| -> $b { value };
    }};
}

pub use assert_type_eq_all;

#[macro_export]
/// Checks that a type implements a trait
macro_rules! assert_impl_one {
    ($ty:ty: $trait:path) => {{
        struct AssertTraitImpl<T: $trait>(::std::marker::PhantomData<T>);
        AssertTraitImpl::<$ty>(::std::marker::PhantomData);
    }};
}

pub use assert_impl_one;

/// Declares a marker trait, its implementors and its `assert_type_is_*`/`assert_value_is_*`
/// checks.
macro_rules! kind_check {
    (
        $(#[$attr:meta])*
        $trait_name:ident, $message:literal, $type_fn:ident, $value_fn:ident
    ) => {
        $(#[$attr])*
        #[diagnostic::on_unimplemented(
            message = $message,
            label = "the Rust type does not match the declared database type",
            note = "change the field's Rust type or its `#[orm(ty = ...)]`"
        )]
        pub trait $trait_name {}

        #[doc = concat!("Compiles only when `T` implements [`", stringify!($trait_name), "`].")]
        pub fn $type_fn<T: $trait_name + ?Sized>() {}

        #[doc = concat!("Compiles only when the value's type implements [`", stringify!($trait_name), "`].")]
        pub fn $value_fn<T: $trait_name>(_value: T) {}
    };
}

macro_rules! impl_marker {
    ($trait_name:ident: $($ty:ty),* $(,)?) => {
        $(impl $trait_name for $ty {})*
    };
}

kind_check!(
    /// Types holding a SurrealDB `number`.
    IsNumber, "`{Self}` cannot hold a SurrealDB `number`",assert_type_is_number, assert_value_is_number
);
impl_marker!(IsNumber: i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,
    types::Number, types::Decimal);

kind_check!(
    /// Types holding a SurrealDB `int`.
    IsInt, "`{Self}` cannot hold a SurrealDB `int`",assert_type_is_int, assert_value_is_int
);
impl_marker!(IsInt: i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

/// Integer types, for bounds on generic `int` fields.
pub use IsInt as Int;

kind_check!(
    /// Types holding a SurrealDB `float`.
    IsFloat, "`{Self}` cannot hold a SurrealDB `float`",assert_type_is_float, assert_value_is_float
);
impl_marker!(IsFloat: f32, f64);

kind_check!(
    /// Types holding a SurrealDB `decimal`.
    IsDecimal, "`{Self}` cannot hold a SurrealDB `decimal`",assert_type_is_decimal, assert_value_is_decimal
);
impl_marker!(IsDecimal: f32, f64, types::Decimal);

kind_check!(
    /// Types holding a SurrealDB `string`.
    IsString, "`{Self}` cannot hold a SurrealDB `string`",assert_type_is_string, assert_value_is_string
);
impl<T: Into<String>> IsString for T {}

kind_check!(
    /// Types holding a SurrealDB `bool`.
    IsBool, "`{Self}` cannot hold a SurrealDB `bool`",assert_type_is_bool, assert_value_is_bool
);
impl_marker!(IsBool: bool, &bool);

kind_check!(
    /// Types holding a SurrealDB `duration`.
    IsDuration, "`{Self}` cannot hold a SurrealDB `duration`",assert_type_is_duration, assert_value_is_duration
);
impl_marker!(IsDuration: types::Duration, std::time::Duration);

kind_check!(
    /// Types holding a SurrealDB `uuid`.
    IsUuid, "`{Self}` cannot hold a SurrealDB `uuid`",assert_type_is_uuid, assert_value_is_uuid
);
impl_marker!(IsUuid: types::Uuid, uuid::Uuid);

kind_check!(
    /// Types holding a SurrealDB `datetime`.
    IsDatetime, "`{Self}` cannot hold a SurrealDB `datetime`",assert_type_is_datetime, assert_value_is_datetime
);
impl_marker!(IsDatetime: types::Datetime, chrono::DateTime<chrono::Utc>);

kind_check!(
    /// Types holding a SurrealDB `bytes`.
    IsBytes, "`{Self}` cannot hold SurrealDB `bytes`",assert_type_is_bytes, assert_value_is_bytes
);
impl_marker!(IsBytes: types::Bytes);

kind_check!(
    /// Types holding a SurrealDB `range`.
    IsRange, "`{Self}` cannot hold a SurrealDB `range`",assert_type_is_range, assert_value_is_range
);
impl_marker!(IsRange: types::Range);

kind_check!(
    /// Types holding a SurrealDB `geometry`.
    IsGeometry, "`{Self}` cannot hold a SurrealDB `geometry`",assert_type_is_geometry, assert_value_is_geometry
);
impl_marker!(IsGeometry: types::Geometry, geo::Geometry<f64>, geo::Point<f64>,
    geo::LineString<f64>, geo::Polygon<f64>, geo::MultiPoint<f64>, geo::MultiLineString<f64>,
    geo::MultiPolygon<f64>);

kind_check!(
    /// Types holding a SurrealDB `record`.
    IsRecordId, "`{Self}` cannot hold a SurrealDB `record`",assert_type_is_record_id, assert_value_is_record_id
);
impl_marker!(IsRecordId: types::RecordId);
impl<T> IsRecordId for RecordId<T> {}
impl<N: Node> IsRecordId for LinkOne<N> {}
impl<N: Node> IsRecordId for LinkSelf<N> {}

kind_check!(
    /// Types holding any SurrealDB value.
    IsAny, "`{Self}` does not implement `SurrealValue`",assert_type_is_any, assert_value_is_any
);
impl<T: SurrealValue> IsAny for T {}

kind_check!(
    /// Types holding a SurrealDB `option`.
    IsOption, "`{Self}` cannot hold a SurrealDB `option`; use `Option<T>`",assert_type_is_option, assert_value_is_option
);
impl<T> IsOption for Option<T> {}
impl<T> IsOption for &Option<T> {}

kind_check!(
    /// Types holding `NULL` or another value.
    IsNull, "`{Self}` cannot hold `NULL`",assert_type_is_null, assert_value_is_null
);
impl<T> IsNull for Option<T> {}
impl<T> IsNull for crate::Nullable<T> {}

kind_check!(
    /// Types holding a SurrealDB `array`.
    IsArray, "`{Self}` cannot hold a SurrealDB `array`",assert_type_is_array, assert_value_is_array
);
impl<T> IsArray for Vec<T> {}
impl<T> IsArray for &Vec<T> {}
impl<T, const N: usize> IsArray for [T; N] {}
impl<T> IsArray for [T] {}
impl<T> IsArray for &[T] {}
impl IsArray for types::Array {}
impl<N: Node> IsArray for LinkMany<N> {}

kind_check!(
    /// Types holding a SurrealDB `set`.
    IsSet, "`{Self}` cannot hold a SurrealDB `set`",assert_type_is_set, assert_value_is_set
);
impl<T> IsSet for HashSet<T> {}
impl<T> IsSet for &HashSet<T> {}
impl<T> IsSet for BTreeSet<T> {}
impl<T> IsSet for &BTreeSet<T> {}
impl IsSet for types::Set {}

kind_check!(
    /// `Vec` types.
    IsVec, "`{Self}` is not a `Vec`",assert_type_is_vec, assert_value_is_vec
);
impl<T> IsVec for Vec<T> {}
impl<T> IsVec for &Vec<T> {}

kind_check!(
    /// `HashSet` types.
    IsHashSet, "`{Self}` is not a `HashSet`",assert_type_is_hashset, assert_value_is_hashset
);
impl<T> IsHashSet for HashSet<T> {}
impl<T> IsHashSet for &HashSet<T> {}

kind_check!(
    /// Embedded objects declared with `#[umbriel::model(object)]`.
    IsObject, "`{Self}` is not an embedded object; declare it with `#[umbriel::model(object)]`",assert_type_is_object, assert_value_is_object
);
impl<T: Object> IsObject for T {}

kind_check!(
    /// Nodes declared with `#[umbriel::model(node)]`.
    IsNode, "`{Self}` is not a node; declare it with `#[umbriel::model(node)]`",assert_type_is_node, assert_value_is_node
);
impl<T: Node> IsNode for T {}

kind_check!(
    /// Record links of any kind.
    IsLink, "`{Self}` is not a record link",assert_type_is_link, assert_value_is_link
);
impl<N: Node> IsLink for LinkOne<N> {}
impl<N: Node> IsLink for LinkSelf<N> {}
impl<N: Node> IsLink for LinkMany<N> {}

kind_check!(
    /// One-to-one record links.
    IsLinkOne, "`{Self}` is not a `LinkOne`",assert_type_is_link_one, assert_value_is_link_one
);
impl<N: Node> IsLinkOne for LinkOne<N> {}

kind_check!(
    /// Self-referential record links.
    IsLinkSelf, "`{Self}` is not a `LinkSelf`",assert_type_is_link_self, assert_value_is_link_self
);
impl<N: Node> IsLinkSelf for LinkSelf<N> {}

kind_check!(
    /// One-to-many record links.
    IsLinkMany, "`{Self}` is not a `LinkMany`",assert_type_is_link_many, assert_value_is_link_many
);
impl<N: Node> IsLinkMany for LinkMany<N> {}

/// Validate that type is iterable at compile time
pub fn assert_type_is_iterable<T: IntoIterator>() {}

/// Compiles only when both arrays have the same length.
pub fn assert_same_length_arrays<T, const N: usize>(_array1: [T; N], _array2: [T; N]) {}

/// Checks that all idents are unique.
#[macro_export]
macro_rules! check_unique_idents {
    // Base case: single element, always unique
    ($_ident:ident) => {};

    // Recursive case: check head against the rest and recurse
    ($head:ident, $($tail:ident),+ $(,)?) => {
        // Generate a unique constant for $head
        $crate::internal_tools::paste! {
            // The name keeps the field's own case: upper-casing it would make `a` and `A` collide.
            #[allow(non_upper_case_globals)]
            const [<UNIQUE_ $head>]: () = ();
        }

        // Recurse with the tail
        $crate::check_unique_idents!($($tail),*);
    };
}

/// Checks that it is false
#[macro_export]
macro_rules! assert_not {
    ($e:expr) => {
        assert!(!$e)
    };
}
pub use assert_not;
