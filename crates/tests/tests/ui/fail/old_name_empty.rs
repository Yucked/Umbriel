use umbriel::{RecordId, model};

// Regression (UMB-031): `old_name = ""` panicked the derive; a previous name cannot be empty.
#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    #[orm(old_name = "")]
    pub name: String,
}

fn main() {}
