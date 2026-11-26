use umbriel::{Field, RecordId, SurrealRead, model};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    pub name: String,
}

// A plain `Field` has no declared type, so it cannot be part of a typed projection.
fn main() {
    let _ = Member::select_fields((Field::new("name"),));
}
