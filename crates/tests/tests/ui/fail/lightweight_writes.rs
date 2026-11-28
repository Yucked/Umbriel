use umbriel::{
    LinkOne, Node, RecordId, SurrealCrud, model,
    statements::{insert_relation, relate},
};

#[model(node, table = "user")]
#[derive(Clone)]
pub struct User {
    pub id: RecordId<Self>,
}

#[model(node, table = "post")]
#[derive(Clone)]
pub struct Post {
    pub id: RecordId<Self>,
}

#[model(edge, table = "likes")]
#[orm(relation(from = "user", to = "post"), lightweight, relax_table)]
#[derive(Clone)]
pub struct LikesEdge<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
}

type Likes = LikesEdge<User, Post>;

// A lightweight edge stores no records: it cannot be saved, inserted, or related with data.
async fn writes(db: &umbriel::Surreal<umbriel::engine::any::Any>, likes: Likes) {
    let _ = likes.clone().save(db).await;
    let _ = insert_relation::<Likes>(likes.clone());
    let _ = relate::<Likes>(umbriel::Field::new("user:alice->likes->post:hello")).content(likes);
}

fn main() {}
