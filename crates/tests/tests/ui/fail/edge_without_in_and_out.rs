use umbriel::{LinkOne, Node, RecordId, model};

// UMB-030: an edge names its endpoints' type parameters `In` and `Out`.
#[model(edge, table = "likes")]
pub struct Likes<From: Node, To: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "From")]
    pub r#in: LinkOne<From>,
    #[orm(link_one = "To")]
    pub out: LinkOne<To>,
}

fn main() {}
