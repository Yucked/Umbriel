use umbriel::{Model, RecordId, SchemaGetter, model, statements::update};

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

// `RETURN VALUE` takes a field of the statement's own model.
fn main() {
    let _ = update::<User>(User::id("alice")).return_value(Team::schema().name);
}
