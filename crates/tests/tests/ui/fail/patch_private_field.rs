mod account {
    use umbriel::{RecordId, model};

    #[model(node, table = "account")]
    pub struct Account {
        pub id: RecordId<Self>,
        pub name: String,
        balance: u64,
    }
}

// A patch field is as visible as its field: `balance` is private to `account`.
fn main() {
    let _ = account::AccountPatch {
        balance: umbriel::Patch::Set(0),
        ..Default::default()
    };
}
