//! A consumer with `umbriel` as its only dependency: everything it needs to connect, run queries
//! and use a transaction comes from Umbriel's re-exports. It needs no runtime to compile; an
//! application awaits `run` on its own.

use std::sync::Arc;

use umbriel::{
    All, Executor, Model, Raw, RecordId, Result, Runnable, Surreal, SurrealCrud, SurrealCrudNode,
    engine::any::{self, Any},
    model,
    opt::Config,
    statements::{Returning, select},
};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

fn alice() -> User {
    User {
        id: User::id("alice"),
        name: "Alice".to_owned(),
    }
}

/// Works the same on a client and inside an SDK transaction.
async fn store(db: &impl Executor) -> Result<Vec<User>> {
    alice().create(db).await?;
    User::insert_many(
        db,
        vec![User {
            id: User::id("bob"),
            name: "Bob".to_owned(),
        }],
    )
    .await?;
    select(All)
        .from(User::table())
        .decode::<User>()
        .execute(db)
        .await
}

/// Connects to an in-memory database, then runs statements, model methods and a transaction,
/// returning the users the transaction committed.
///
/// # Errors
///
/// Any database error, classified as `umbriel::Error`.
pub async fn run() -> Result<Vec<User>> {
    let db: Surreal<Any> = any::connect(("mem://".to_owned(), Config::new())).await?;
    db.use_ns("app").use_db("app").await?;

    // A shared client is borrowed, never cloned.
    let shared = Arc::new(db);
    store(shared.as_ref()).await?;

    // `begin` takes the client by value: a transaction starts from a clone, a session of its own.
    let tx = shared.as_ref().clone().begin().await?;
    Raw::new("DELETE user;").run(&tx).await?;
    let users = store(&tx).await?;
    tx.commit().await?;
    Ok(users)
}
