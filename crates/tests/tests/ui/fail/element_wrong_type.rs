use umbriel::{RecordId, SchemaGetter, SetterArray, SetterAssignable, model};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub tags: Vec<String>,
}

// An element of `tags` is a `String`.
fn main() {
    let _ = User::schema().tags.at(0).equal_to(5u8);
}
