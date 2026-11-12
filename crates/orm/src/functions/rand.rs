//! Random value functions, `rand()` and `rand::*`.
//!
//! ```rust
//! use umbriel::{*, functions::rand};
//! assert_eq!(rand::int!().to_debug_sql(), "rand::int()");
//! assert_eq!(rand::int!(1, 6).to_debug_sql(), "rand::int(1, 6)");
//! assert_eq!(rand::uuid::v7!().to_debug_sql(), "rand::uuid::v7()");
//! ```

use crate::kinds;

functions! { $ "" "", rand, rand;
    /// A random float from 0 to 1.
    rand() -> kinds::Float;
}

functions! { $ "rand" "::", rand, rand;
    /// A random boolean.
    bool() -> kinds::Bool;
    /// A random duration from `min` to `max`.
    duration(min: DurationLike, max: DurationLike) -> kinds::Duration;
    /// One of the values chosen at random; with one array, one of its elements.
    enum_ = enum(.. values: ValueLike) -> kinds::Any;
    /// A random float, from `min` to `max` when both are given.
    float(; [min: NumberLike, max: NumberLike]) -> kinds::Float;
    /// A random record key: 20 characters, `length` characters, or from `min` to `max`
    /// characters long.
    id(; length: NumberLike, max: NumberLike) -> kinds::String;
    /// A random integer, from `min` to `max` when both are given.
    int(; [min: NumberLike, max: NumberLike]) -> kinds::Int;
    /// A random string: 32 characters, `length` characters, or from `min` to `max` characters
    /// long.
    string(; length: NumberLike, max: NumberLike) -> kinds::String;
    /// A random datetime, from `min` to `max` (datetimes or Unix seconds) when both are given.
    time(; [min: ValueLike, max: ValueLike]) -> kinds::Datetime;
    /// A random ULID, for the time `timestamp` when given.
    ulid(; timestamp: DatetimeLike) -> kinds::String;
    /// A random version 7 UUID, for the time `timestamp` when given.
    uuid(; timestamp: DatetimeLike) -> kinds::Uuid;
}

/// Random UUIDs, `rand::uuid::*`.
pub mod uuid {
    use crate::kinds;

    functions! { $ "rand::uuid" "::", rand_uuid, rand::uuid;
        /// A random version 4 UUID.
        v4() -> kinds::Uuid;
        /// A random version 7 UUID, for the time `timestamp` when given.
        v7(; timestamp: DatetimeLike) -> kinds::Uuid;
    }
}
