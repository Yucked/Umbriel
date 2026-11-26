use umbriel::{RecordId, SchemaGetter, SurrealRead, model};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    pub name: String,
}

#[model(node, table = "wallet")]
pub struct Wallet {
    pub id: RecordId<Self>,
    pub balance: i64,
}

// A wallet's field cannot be selected from members.
fn main() {
    let _ = Member::select_fields((Member::schema().name, Wallet::schema().balance));
}
