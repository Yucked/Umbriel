use surrealdb::types::Duration;

use crate::Renderer;

use super::{Statement, impl_statement};

/// Creates a SLEEP statement.
///
/// SurrealDB parses only a duration literal here, so it takes a duration, never a field or
/// parameter; the `sleep()` function in [`functions::sleep`](crate::functions::sleep) takes an
/// expression.
///
/// Examples
///
/// ```rust
/// use std::time::Duration;
/// use umbriel::{*, statements::sleep};
///
/// assert_eq!(sleep(Duration::from_secs(43)).to_debug_sql(), "SLEEP 43s;");
/// ```
pub fn sleep(duration: impl Into<Duration>) -> SleepStatement {
    SleepStatement {
        duration: duration.into(),
    }
}

/// Sleep statement initialization builder
pub struct SleepStatement {
    duration: Duration,
}

impl Statement for SleepStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("SLEEP ").push_syntax(&self.duration);
    }
}

impl_statement!(SleepStatement);
