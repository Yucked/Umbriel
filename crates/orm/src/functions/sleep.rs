//! `sleep()`: pauses the query.
//!
//! ```rust
//! use umbriel::{*, functions::sleep};
//! let pause = sleep::sleep!(std::time::Duration::from_millis(5));
//! assert_eq!(pause.to_debug_sql(), "sleep(5ms)");
//! ```

use crate::kinds;

functions! { $ "" "", sleep, sleep;
    /// Pauses for `duration`; the result is `NONE`.
    sleep(duration: DurationLike) -> kinds::Any;
}
