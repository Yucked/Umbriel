//! `count()`.
//!
//! ```rust
//! use umbriel::*;
//! assert_eq!(count!().to_debug_sql(), "count()");
//! assert_eq!(count!(Field::new("verified")).to_debug_sql(), "count(verified)");
//! ```

use crate::kinds;

functions! { $ "" "", functions, ;
    /// `1`, or with a value: `1` when it is truthy and `0` otherwise, or the length of an array.
    /// In a `GROUP BY` selection it counts the rows.
    count(; value: ValueLike) -> kinds::Int;
}
