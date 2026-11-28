use umbriel::{LinkOne, Node, RecordId, model};

// `enforced` is a clause of `TYPE RELATION`, which only `relation` asks for.
#[model(edge, table = "likes")]
#[orm(enforced)]
pub struct Likes<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
}

fn main() {}
