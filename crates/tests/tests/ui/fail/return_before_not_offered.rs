use umbriel::{
    Model, RecordId, model,
    statements::{CreateTarget, create, insert},
};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

// SurrealDB documents `RETURN BEFORE` as `NONE` for `CREATE` and `INSERT`, so neither offers it.
fn main() {
    let alice = || User {
        id: User::id("alice"),
        name: "Alice".to_owned(),
    };
    let _ = create::<User>(CreateTarget::table()).content(alice()).return_before();
    let _ = insert(alice()).return_before();
}
