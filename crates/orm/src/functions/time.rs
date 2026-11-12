//! Time functions and constants, `time::*`.
//!
//! The functions that read one part of a datetime take it optionally: without it, they read the
//! current time.
//!
//! ```rust
//! use umbriel::{*, functions::time};
//! let created = Field::new("created");
//! assert_eq!(time::year!(created.clone()).to_debug_sql(), "time::year(created)");
//! assert_eq!(time::year!().to_debug_sql(), "time::year()");
//! assert_eq!(DatetimeLike::from(created).year().to_debug_sql(), "created.year()");
//! assert_eq!(time::EPOCH.to_debug_sql(), "time::EPOCH");
//! ```

use crate::{Constant, kinds};

functions! { $ "time" "::", time, time;
    /// The datetime rounded up to a multiple of `duration`.
    ceil(datetime: DatetimeLike, duration: DurationLike) -> kinds::Datetime
        [DatetimeLike.ceil];
    /// The day of the month.
    day(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.day];
    /// The datetime rounded down to a multiple of `duration`.
    floor(datetime: DatetimeLike, duration: DurationLike) -> kinds::Datetime
        [DatetimeLike.floor];
    /// The datetime formatted with a `strftime`-style `format`.
    format_ = format(datetime: DatetimeLike, format: StrandLike) -> kinds::String
        [DatetimeLike.format];
    /// The datetime truncated to the start of its `interval` (`'year'`, `'month'`, `'week'`,
    /// `'day'`, `'hour'`, `'minute'` or `'second'`).
    group(datetime: DatetimeLike, interval: StrandLike) -> kinds::Datetime [DatetimeLike.group];
    /// The hour.
    hour(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.hour];
    /// Whether the year is a leap year.
    is_leap_year(; datetime: DatetimeLike) -> kinds::Bool [DatetimeLike.is_leap_year];
    /// The latest datetime.
    max(datetimes: ArrayLike) -> kinds::Datetime;
    /// The microseconds since the Unix epoch.
    micros(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.micros];
    /// The milliseconds since the Unix epoch.
    millis(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.millis];
    /// The earliest datetime.
    min(datetimes: ArrayLike) -> kinds::Datetime;
    /// The minute.
    minute(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.minute];
    /// The month.
    month(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.month];
    /// The nanoseconds since the Unix epoch.
    nano(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.nano];
    /// The current datetime.
    now() -> kinds::Datetime;
    /// The datetime rounded to the nearest multiple of `duration`.
    round(datetime: DatetimeLike, duration: DurationLike) -> kinds::Datetime
        [DatetimeLike.round];
    /// The second.
    second(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.second];
    /// The datetime with its day of the month set to `day`.
    set_day(datetime: DatetimeLike, day: NumberLike) -> kinds::Datetime [DatetimeLike.set_day];
    /// The datetime with its hour set to `hour`.
    set_hour(datetime: DatetimeLike, hour: NumberLike) -> kinds::Datetime
        [DatetimeLike.set_hour];
    /// The datetime with its minute set to `minute`.
    set_minute(datetime: DatetimeLike, minute: NumberLike) -> kinds::Datetime
        [DatetimeLike.set_minute];
    /// The datetime with its month set to `month`.
    set_month(datetime: DatetimeLike, month: NumberLike) -> kinds::Datetime
        [DatetimeLike.set_month];
    /// The datetime with its nanoseconds set to `nanosecond`.
    set_nanosecond(datetime: DatetimeLike, nanosecond: NumberLike) -> kinds::Datetime
        [DatetimeLike.set_nanosecond];
    /// The datetime with its second set to `second`.
    set_second(datetime: DatetimeLike, second: NumberLike) -> kinds::Datetime
        [DatetimeLike.set_second];
    /// The datetime with its year set to `year`.
    set_year(datetime: DatetimeLike, year: NumberLike) -> kinds::Datetime
        [DatetimeLike.set_year];
    /// The local timezone offset of the server.
    timezone() -> kinds::String;
    /// The seconds since the Unix epoch.
    unix(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.unix];
    /// The day of the week, Monday as 1.
    wday(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.wday];
    /// The ISO week of the year.
    week(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.week];
    /// The day of the year.
    yday(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.yday];
    /// The year.
    year(; datetime: DatetimeLike) -> kinds::Int [DatetimeLike.year];
    /// The datetime `micros` microseconds after the Unix epoch.
    from_micros(micros: NumberLike) -> kinds::Datetime;
    /// The datetime `millis` milliseconds after the Unix epoch.
    from_millis(millis: NumberLike) -> kinds::Datetime;
    /// The datetime `nanos` nanoseconds after the Unix epoch.
    from_nanos(nanos: NumberLike) -> kinds::Datetime;
    /// The datetime `secs` seconds after the Unix epoch.
    from_secs(secs: NumberLike) -> kinds::Datetime;
    /// The datetime a ULID encodes.
    from_ulid(ulid: StrandLike) -> kinds::Datetime;
    /// The datetime `secs` seconds after the Unix epoch; the same as `from_secs`.
    from_unix(secs: NumberLike) -> kinds::Datetime;
    /// The datetime a version 7 UUID encodes.
    from_uuid(uuid: ValueLike) -> kinds::Datetime;
}

/// `time::EPOCH`, the Unix epoch.
pub const EPOCH: Constant<kinds::Datetime> = Constant::new("time::EPOCH");

/// `time::MINIMUM`, the earliest datetime SurrealDB holds.
pub const MINIMUM: Constant<kinds::Datetime> = Constant::new("time::MINIMUM");

/// `time::MAXIMUM`, the latest datetime SurrealDB holds.
pub const MAXIMUM: Constant<kinds::Datetime> = Constant::new("time::MAXIMUM");
