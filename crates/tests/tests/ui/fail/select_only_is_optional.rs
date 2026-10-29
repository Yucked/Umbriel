use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All, Model, RecordId, model,
    statements::{Returning, select},
};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

// `FROM ONLY` returns one record or none, never a list.
async fn run(db: &Surreal<Db>) -> umbriel::Result<()> {
    let _: Vec<User> = select(All)
        .from_only(User::id("alice"))
        .decode::<User>()
        .execute(db)
        .await?;
    Ok(())
}

fn main() {
    let _ = run;
}
