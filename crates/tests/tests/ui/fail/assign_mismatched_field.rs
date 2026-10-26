use umbriel::{RecordId, SchemaGetter, SetterAssignable, model};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
    pub age: u8,
}

// A field stands for a value of its own type only: `age` is a `u8`, not a `String`.
fn main() {
    let user = User::schema();
    let _ = user.name.equal_to(user.age);
}
