use umbriel::{LinkOne, Links, RecordId, SurrealRead, model};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    #[orm(link_one = "Wallet")]
    pub wallet: LinkOne<Wallet>,
}

#[model(node, table = "wallet")]
pub struct Wallet {
    pub id: RecordId<Self>,
    #[orm(link_one = "Bank")]
    pub bank: LinkOne<Bank>,
}

#[model(node, table = "bank")]
pub struct Bank {
    pub id: RecordId<Self>,
}

// A wallet's path cannot be fetched on a statement that decodes members.
fn main() {
    let _ = Member::select().fetch_link(Wallet::links().bank);
}
