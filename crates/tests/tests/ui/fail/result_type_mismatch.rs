use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    Model, RecordId, SchemaGetter, model,
    statements::{Returning, delete, update},
};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

// The return mode decides the result type: `RETURN NONE` returns `()`, `RETURN VALUE name`
// returns names, and `DELETE` returns nothing by default.
async fn run(db: &Surreal<Db>) -> umbriel::Result<()> {
    let _: Vec<User> = update::<User>(User::id("alice")).return_none().execute(db).await?;
    let _: Vec<User> = update::<User>(User::id("alice"))
        .return_value(User::schema().name)
        .execute(db)
        .await?;
    let _: Vec<User> = delete::<User>(User::id("alice")).execute(db).await?;
    Ok(())
}

fn main() {
    let _ = run;
}
