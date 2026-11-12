//! Record id functions, `record::*`.
//!
//! ```rust
//! use umbriel::{*, functions::record};
//! let owner = Field::new("owner");
//! assert_eq!(record::tb!(owner.clone()).to_debug_sql(), "record::tb(owner)");
//! assert_eq!(RecordIdLike::from(owner).id().to_debug_sql(), "owner.id()");
//! ```

use crate::kinds;

functions! { $ "record" "::", record, record;
    /// Whether the record exists.
    exists(record: RecordIdLike) -> kinds::Bool [RecordIdLike.exists];
    /// The key of a record id.
    id(record: RecordIdLike) -> kinds::Any [RecordIdLike.id];
    /// Whether the record is an edge.
    is_edge(record: RecordIdLike) -> kinds::Bool [RecordIdLike.is_edge];
    /// The table of a record id.
    table(record: RecordIdLike) -> kinds::String [RecordIdLike.table];
    /// The table of a record id; the same as `table`.
    tb(record: RecordIdLike) -> kinds::String [RecordIdLike.tb];
}
