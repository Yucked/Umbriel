// Statement syntax
// REBUILD INDEX [ IF EXISTS ] @name ON [ TABLE ] @table [ CONCURRENTLY ]

use surrealdb::types::Table;

use crate::{Renderer, TableIndex};

use super::{Statement, impl_definition};

/// Rebuilds an index from the table's records: `REBUILD INDEX [IF EXISTS] @name ON TABLE @table
/// [CONCURRENTLY]`. SurrealDB reads the names as identifiers only, so neither may be a parameter,
/// and the table is required: a statement without one fails to render. Rebuilding an index
/// marked for removal (`ALTER INDEX … PREPARE REMOVE`) fails.
///
/// ```rust
/// use umbriel::{*, statements::rebuild_index};
///
/// let statement = rebuild_index("post_content")
///     .on_table(Table::new("post"))
///     .if_exists()
///     .concurrently();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "REBUILD INDEX IF EXISTS post_content ON TABLE post CONCURRENTLY;"
/// );
/// ```
pub fn rebuild_index(index: impl Into<TableIndex>) -> RebuildIndexStatement {
    RebuildIndexStatement {
        index: index.into(),
        table: None,
        if_exists: false,
        concurrently: false,
    }
}

/// A `REBUILD INDEX` statement.
#[must_use]
pub struct RebuildIndexStatement {
    index: TableIndex,
    table: Option<Table>,
    if_exists: bool,
    concurrently: bool,
}

impl RebuildIndexStatement {
    /// The table the index is on.
    pub fn on_table(mut self, table: impl Into<Table>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// `IF EXISTS`: rebuilding an index that does not exist succeeds instead of failing.
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }

    /// `CONCURRENTLY`: the statement returns before the rebuild finishes, which runs in the
    /// background; `INFO FOR INDEX` reports its progress.
    pub fn concurrently(mut self) -> Self {
        self.concurrently = true;
        self
    }
}

impl Statement for RebuildIndexStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("REBUILD INDEX");
        if self.if_exists {
            out.push_str(" IF EXISTS");
        }
        out.push_str(" ").push(&self.index);
        match &self.table {
            Some(table) => {
                out.push_str(" ON TABLE ").push(table);
            }
            None => {
                out.push_error("REBUILD INDEX needs the index's table: call `on_table`");
            }
        }
        if self.concurrently {
            out.push_str(" CONCURRENTLY");
        }
    }
}

impl_definition!(RebuildIndexStatement);
