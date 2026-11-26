use umbriel::{LinkOne, Links, RecordId, model};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    #[orm(link_one = "Wallet")]
    pub wallet: LinkOne<Wallet>,
    #[orm(link_one = "Bank")]
    pub employer: LinkOne<Bank>,
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

// `employer` ends at a bank, so the next hop must start at a bank, not a wallet.
fn main() {
    let _ = Member::links().employer.then(Wallet::links().bank);
}
