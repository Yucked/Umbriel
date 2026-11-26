use umbriel::{All, LinkOne, Links, Model, RecordId, model, statements::select};

#[model(node, table = "member")]
pub struct Member {
    pub id: RecordId<Self>,
    #[orm(link_one = "Wallet")]
    pub wallet: LinkOne<Wallet>,
}

#[model(node, table = "wallet")]
pub struct Wallet {
    pub id: RecordId<Self>,
}

// `select(All).from(...)` decodes untyped values, so it offers no typed link fetch.
fn main() {
    let _ = select(All).from(Member::table()).fetch_link(Member::links().wallet);
}
