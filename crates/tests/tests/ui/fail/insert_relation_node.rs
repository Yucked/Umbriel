use umbriel::{Model, RecordId, model, statements::insert_relation};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

// `INSERT RELATION` inserts edges, not nodes.
fn main() {
    let _ = insert_relation(User {
        id: User::id("alice"),
        name: "Alice".to_owned(),
    });
}
