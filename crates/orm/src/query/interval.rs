use crate::{StrandLike, keywords::keywords};

keywords! {
    /// Represents a time interval.
    #[derive(Debug, Clone, Copy)]
    pub enum Interval {
        /// Year
        Year => "year",
        /// Month
        Month => "month",
        /// Week
        Week => "week",
        /// Day
        Day => "day",
        /// Hour
        Hour => "hour",
        /// Minute
        Minute => "minute",
        /// Second
        Second => "second",
    }
}

impl From<Interval> for StrandLike {
    fn from(value: Interval) -> Self {
        value.as_str().to_owned().into()
    }
}
