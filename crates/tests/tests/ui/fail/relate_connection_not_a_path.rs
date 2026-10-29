use umbriel::{LinkOne, Node, RecordId, Relate, model};

// A connection is `->edge->node` over table names; a filter is not a table name.
#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    #[orm(relate(model = "UserLikesPost", connection = "->likes[WHERE score > 1]->post"))]
    pub liked: Relate<Post>,
}

#[model(node, table = "post")]
pub struct Post {
    pub id: RecordId<Self>,
}

#[model(edge, table = "likes")]
pub struct Likes<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
}

pub type UserLikesPost = Likes<User, Post>;

fn main() {}
