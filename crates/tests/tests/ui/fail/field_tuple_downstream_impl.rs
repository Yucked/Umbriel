use umbriel::{FieldTuple, RecordId, ValueLike, model};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    pub name: String,
}

// Another crate can't implement `FieldTuple`, even for its own model: the seal requires every
// element to be one of the model's schema fields.
impl FieldTuple<Member, String> for (u8,) {
    fn into_array(self) -> ValueLike {
        ValueLike::value(self.0)
    }
}

fn main() {}
