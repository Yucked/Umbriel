use crate::{DurationLike, Ident, Name, Renderer};

use super::{DefineOptions, Statement, impl_define_kind, impl_definition};

/// Defines a sequence: `DEFINE SEQUENCE [OVERWRITE | IF NOT EXISTS] @name [BATCH @batch]
/// [START @start] [TIMEOUT @duration]`. SurrealDB defaults to `BATCH 1000` and `START 0`.
///
/// SurrealDB stores no comment for a sequence, so this builder has none.
///
/// [`overwrite`](DefineSequenceStatement::overwrite) on an existing sequence resets the values
/// it has allocated, so values already issued may be issued again. To change only its timeout,
/// use [`alter_sequence`](super::alter_sequence()), which keeps them.
///
/// ```rust
/// use std::time::Duration;
/// use umbriel::{*, statements::define_sequence};
///
/// let statement = define_sequence("order_number")
///     .batch(100)
///     .start(1000)
///     .timeout(Duration::from_secs(5));
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE SEQUENCE order_number BATCH 100 START 1000 TIMEOUT 5s;"
/// );
/// ```
pub fn define_sequence(name: impl Into<Name<Ident>>) -> DefineSequenceStatement {
    DefineSequenceStatement {
        name: name.into(),
        options: DefineOptions::default(),
        batch: None,
        start: None,
        timeout: None,
    }
}

/// A `DEFINE SEQUENCE` statement.
pub struct DefineSequenceStatement {
    name: Name<Ident>,
    options: DefineOptions,
    batch: Option<u32>,
    start: Option<i64>,
    timeout: Option<DurationLike>,
}

impl DefineSequenceStatement {
    /// `BATCH n`: how many values a node reserves at a time.
    pub fn batch(mut self, batch: u32) -> Self {
        self.batch = Some(batch);
        self
    }

    /// `START n`: the first value issued.
    pub fn start(mut self, start: i64) -> Self {
        self.start = Some(start);
        self
    }

    /// `TIMEOUT @duration`: how long `sequence::nextval` waits to reserve a batch. SurrealDB
    /// 3.3.4 parses it as an expression, like every other `TIMEOUT`.
    pub fn timeout(mut self, timeout: impl Into<DurationLike>) -> Self {
        self.timeout = Some(timeout.into());
        self
    }
}

impl Statement for DefineSequenceStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE SEQUENCE");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.name);
        if let Some(batch) = self.batch {
            out.push_str(" BATCH ").push_display(&batch);
        }
        if let Some(start) = self.start {
            out.push_str(" START ").push_display(&start);
        }
        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push(timeout);
        }
    }
}

impl_define_kind!(DefineSequenceStatement);
impl_definition!(DefineSequenceStatement);
