//! Type conversion and checking functions, `type::*`.
//!
//! Their method forms are on [`ValueLike`](crate::ValueLike), since SurrealDB dispatches them on
//! every value: `type::int(x)` is `x.to_int()`, `type::is_int(x)` is `x.is_int()` and `type::of(x)`
//! is `x.type_of()`. On a string, SurrealDB dispatches `.is_datetime()`, `.is_uuid()` and
//! `.is_record()` to the `string::is_*` functions instead, which check the string's content.
//! `type::string`'s method is `to_string_value` in Rust, rendering `.to_string()`.
//!
//! ```rust
//! use umbriel::{*, functions::type_};
//! let age = Field::new("age");
//! assert_eq!(type_::int!(age.clone()).to_debug_sql(), "type::int(age)");
//! assert_eq!(ValueLike::from(age).to_int().to_debug_sql(), "age.to_int()");
//! ```

use crate::kinds;

functions! { $ "type" "::", type, type_;
    /// The value converted to an array.
    array(value: ValueLike) -> kinds::Array [ValueLike.to_array];
    /// The value converted to a boolean.
    bool(value: ValueLike) -> kinds::Bool [ValueLike.to_bool];
    /// The value converted to bytes.
    bytes(value: ValueLike) -> kinds::Bytes [ValueLike.to_bytes];
    /// The value converted to a datetime.
    datetime(value: ValueLike) -> kinds::Datetime [ValueLike.to_datetime];
    /// The value converted to a decimal.
    decimal(value: ValueLike) -> kinds::Decimal [ValueLike.to_decimal];
    /// The value converted to a duration.
    duration(value: ValueLike) -> kinds::Duration [ValueLike.to_duration];
    /// The field of the current record at the path `field`.
    field(field: StrandLike) -> kinds::Any;
    /// The fields of the current record at the paths `fields`.
    fields(fields: ArrayLike) -> kinds::Array;
    /// The value converted to a float.
    float(value: ValueLike) -> kinds::Float [ValueLike.to_float];
    /// The value converted to a geometry.
    geometry(value: ValueLike) -> kinds::Geometry [ValueLike.to_geometry];
    /// The value converted to an integer.
    int(value: ValueLike) -> kinds::Int [ValueLike.to_int];
    /// The value converted to a number.
    number(value: ValueLike) -> kinds::Number [ValueLike.to_number];
    /// The name of the value's type, e.g. `'int'`.
    of(value: ValueLike) -> kinds::String [ValueLike.type_of];
    /// The value converted to a point.
    point(value: ValueLike) -> kinds::Point [ValueLike.to_point];
    /// The value converted to a range.
    range(value: ValueLike) -> kinds::Range [ValueLike.to_range];
    /// A record id: `value` converted to one, or the record of `table` with the key `value`
    /// when `table` is given (`type::record('user', 'alice')`).
    record(value: ValueLike; table: ValueLike) -> kinds::Record [ValueLike.to_record];
    /// The value converted to a set.
    set(value: ValueLike) -> kinds::Set [ValueLike.to_set];
    /// The value converted to a string.
    string(value: ValueLike) -> kinds::String [ValueLike.to_string_value = to_string];
    /// The value converted to a string, replacing invalid UTF-8 in bytes.
    string_lossy(value: ValueLike) -> kinds::String [ValueLike.to_string_lossy];
    /// The value converted to a table.
    table(value: ValueLike) -> kinds::Table;
    /// The value converted to a UUID.
    uuid(value: ValueLike) -> kinds::Uuid [ValueLike.to_uuid];
    /// Whether the value is an array.
    is_array(value: ValueLike) -> kinds::Bool [ValueLike.is_array];
    /// Whether the value is a boolean.
    is_bool(value: ValueLike) -> kinds::Bool [ValueLike.is_bool];
    /// Whether the value is bytes.
    is_bytes(value: ValueLike) -> kinds::Bool [ValueLike.is_bytes];
    /// Whether the value is a geometry collection.
    is_collection(value: ValueLike) -> kinds::Bool [ValueLike.is_collection];
    /// Whether the value is a datetime.
    is_datetime(value: ValueLike) -> kinds::Bool [ValueLike.is_datetime];
    /// Whether the value is a decimal.
    is_decimal(value: ValueLike) -> kinds::Bool [ValueLike.is_decimal];
    /// Whether the value is a duration.
    is_duration(value: ValueLike) -> kinds::Bool [ValueLike.is_duration];
    /// Whether the value is a float.
    is_float(value: ValueLike) -> kinds::Bool [ValueLike.is_float];
    /// Whether the value is a geometry.
    is_geometry(value: ValueLike) -> kinds::Bool [ValueLike.is_geometry];
    /// Whether the value is an integer.
    is_int(value: ValueLike) -> kinds::Bool [ValueLike.is_int];
    /// Whether the value is a line.
    is_line(value: ValueLike) -> kinds::Bool [ValueLike.is_line];
    /// Whether the value is a multiline.
    is_multiline(value: ValueLike) -> kinds::Bool [ValueLike.is_multiline];
    /// Whether the value is a multipoint.
    is_multipoint(value: ValueLike) -> kinds::Bool [ValueLike.is_multipoint];
    /// Whether the value is a multipolygon.
    is_multipolygon(value: ValueLike) -> kinds::Bool [ValueLike.is_multipolygon];
    /// Whether the value is `NONE`.
    is_none(value: ValueLike) -> kinds::Bool [ValueLike.is_none];
    /// Whether the value is `NULL`.
    is_null(value: ValueLike) -> kinds::Bool [ValueLike.is_null];
    /// Whether the value is a number.
    is_number(value: ValueLike) -> kinds::Bool [ValueLike.is_number];
    /// Whether the value is an object.
    is_object(value: ValueLike) -> kinds::Bool [ValueLike.is_object];
    /// Whether the value is a point.
    is_point(value: ValueLike) -> kinds::Bool [ValueLike.is_point];
    /// Whether the value is a polygon.
    is_polygon(value: ValueLike) -> kinds::Bool [ValueLike.is_polygon];
    /// Whether the value is a range.
    is_range(value: ValueLike) -> kinds::Bool [ValueLike.is_range];
    /// Whether the value is a record id, of `table` when given.
    is_record(value: ValueLike; table: StrandLike) -> kinds::Bool [ValueLike.is_record];
    /// Whether the value is a set.
    is_set(value: ValueLike) -> kinds::Bool [ValueLike.is_set];
    /// Whether the value is a string.
    is_string(value: ValueLike) -> kinds::Bool [ValueLike.is_string];
    /// Whether the value is a UUID.
    is_uuid(value: ValueLike) -> kinds::Bool [ValueLike.is_uuid];
}
