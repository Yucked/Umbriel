use umbriel::{RecordId, model, types};

// A plain `table` refuses table values in SurrealDB 3.3.4, so the tables must be named.
#[model(node, table = "audit")]
pub struct Audit {
    pub id: RecordId<Self>,
    pub source: types::Table,
}

fn main() {}
