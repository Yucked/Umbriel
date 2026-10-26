use umbriel::{E, LinkOne, Model, Node, RecordId, Relate, model, statements::relate};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    #[orm(relate(model = "UserLikesPost", connection = "->likes->post"))]
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

// The path ends at a post, so a user's id is not a valid endpoint.
fn main() {
    let _ = relate::<UserLikesPost>(User::with(User::id("alice")).likes__(E).post(User::id("bob")));
}
