use umbriel::{LinkMany, LinkOne, LinkSelf, RecordId, model};

/// A club member, with links of every kind for link paths and nested `FETCH`: a wallet, a renamed
/// employer link, and a mentor of the same table.
#[model(node, table = "member")]
#[derive(Debug, Clone)]
pub struct Member {
    pub id: RecordId<Self>,
    pub name: String,

    #[orm(link_one = "Wallet")]
    pub wallet: LinkOne<Wallet>,

    #[orm(link_one = "Bank")]
    #[surreal(rename = "worksFor")]
    pub employer: LinkOne<Bank>,

    #[orm(link_self = "Member")]
    pub mentor: LinkSelf<Member>,
}

/// A member's wallet, held at a bank: the second hop of `wallet.bank`.
#[model(node, table = "wallet")]
#[derive(Debug, Clone)]
pub struct Wallet {
    pub id: RecordId<Self>,
    pub balance: i64,

    #[orm(link_one = "Bank")]
    pub bank: LinkOne<Bank>,
}

/// A bank: the end of every member path.
#[model(node, table = "bank")]
#[derive(Debug, Clone)]
pub struct Bank {
    pub id: RecordId<Self>,
    pub name: String,
}

/// A club and its members: a link list to fan out over.
#[model(node, table = "club")]
#[derive(Debug, Clone)]
pub struct Club {
    pub id: RecordId<Self>,
    pub name: String,

    #[orm(link_many = "Member")]
    pub members: LinkMany<Member>,
}
