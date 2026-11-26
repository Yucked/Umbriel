use umbriel::{LinkOne, Links, RecordId, model};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    pub name: String,
    #[orm(link_one = "Wallet")]
    pub wallet: LinkOne<Wallet>,
}

#[model(node, table = "wallet")]
pub struct Wallet {
    pub id: RecordId<Self>,
}

// `name` is not a link, so it has no link path.
fn main() {
    let _ = Member::links().name;
}
