use umbriel::{RecordId, SchemaGetter, SurrealRead, model, statements::SelectStatement};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    pub name: String,
    pub age: i64,
}

// The tuple holds the fields' types in order, so swapping them does not compile.
fn main() {
    let _: SelectStatement<(i64, String)> =
        Member::select_fields((Member::schema().name, Member::schema().age));
}
