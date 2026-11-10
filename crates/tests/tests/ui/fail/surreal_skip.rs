use umbriel::{RecordId, model};

// The SDK's derive ignores `skip`, so the field would still be stored.
#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
    #[surreal(skip)]
    pub session_token: String,
}

fn main() {}
