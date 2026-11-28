use umbriel::{LinkOne, Node, RecordId, model};

// `relation` sets the table's `TYPE`, so `kind` would set it twice.
#[model(edge, table = "likes")]
#[orm(relation, kind = "normal")]
pub struct Likes<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
}

fn main() {}
