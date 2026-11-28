use umbriel::{LinkOne, Node, RecordId, model};

// A lightweight edge stores only `id`, `in` and `out`.
#[model(edge, table = "likes")]
#[orm(relation(from = "user", to = "post"), lightweight)]
pub struct Likes<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
    pub weight: u8,
}

fn main() {}
