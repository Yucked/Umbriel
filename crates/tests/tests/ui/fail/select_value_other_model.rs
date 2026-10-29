use umbriel::{SchemaGetter, SurrealCrud, model, RecordId};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

#[model(node, table = "team")]
pub struct Team {
    pub id: RecordId<Self>,
    pub name: String,
}

// A model's `select_value` takes a field of that model.
fn main() {
    let _ = User::select_value(Team::schema().name);
}
