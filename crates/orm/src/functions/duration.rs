//! Duration functions and the constant `duration::MAX`.
//!
//! ```rust
//! use umbriel::{*, functions::duration};
//! let elapsed = Field::new("elapsed");
//! assert_eq!(duration::days!(elapsed.clone()).to_debug_sql(), "duration::days(elapsed)");
//! assert_eq!(DurationLike::from(elapsed).days().to_debug_sql(), "elapsed.days()");
//! ```

use crate::{Constant, kinds};

functions! { $ "duration" "::", duration, duration;
    /// The whole days in the duration.
    days(duration: DurationLike) -> kinds::Int [DurationLike.days];
    /// The whole hours in the duration.
    hours(duration: DurationLike) -> kinds::Int [DurationLike.hours];
    /// The whole microseconds in the duration.
    micros(duration: DurationLike) -> kinds::Int [DurationLike.micros];
    /// The whole milliseconds in the duration.
    millis(duration: DurationLike) -> kinds::Int [DurationLike.millis];
    /// The whole minutes in the duration.
    mins(duration: DurationLike) -> kinds::Int [DurationLike.mins];
    /// The nanoseconds in the duration.
    nanos(duration: DurationLike) -> kinds::Int [DurationLike.nanos];
    /// The whole seconds in the duration.
    secs(duration: DurationLike) -> kinds::Int [DurationLike.secs];
    /// The whole weeks in the duration.
    weeks(duration: DurationLike) -> kinds::Int [DurationLike.weeks];
    /// The whole years in the duration.
    years(duration: DurationLike) -> kinds::Int [DurationLike.years];
    /// A duration of `days` days.
    from_days(days: NumberLike) -> kinds::Duration;
    /// A duration of `hours` hours.
    from_hours(hours: NumberLike) -> kinds::Duration;
    /// A duration of `micros` microseconds.
    from_micros(micros: NumberLike) -> kinds::Duration;
    /// A duration of `millis` milliseconds.
    from_millis(millis: NumberLike) -> kinds::Duration;
    /// A duration of `mins` minutes.
    from_mins(mins: NumberLike) -> kinds::Duration;
    /// A duration of `nanos` nanoseconds.
    from_nanos(nanos: NumberLike) -> kinds::Duration;
    /// A duration of `secs` seconds.
    from_secs(secs: NumberLike) -> kinds::Duration;
    /// A duration of `weeks` weeks.
    from_weeks(weeks: NumberLike) -> kinds::Duration;
}

/// `duration::MAX`, the longest duration SurrealDB holds.
pub const MAX: Constant<kinds::Duration> = Constant::new("duration::MAX");
