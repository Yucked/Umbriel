A typed ORM for SurrealDB 3: models, queries, schema, graph, transactions and migrations.

Umbriel turns Rust structs into SurrealDB models with a typed schema. Queries are built from
typed paths instead of strings, values are always bound rather than written into the query text,
and every statement's result is checked before success is reported.

# Setup

`umbriel` is the only dependency an application needs, plus an async runtime to await its
futures. It re-exports the SDK client and connection types ([`Surreal`], [`Connection`],
[`engine`], [`opt`], [`IndexedResults`], [`Transaction`]), the SDK's value types as [`types`],
and its own macros.

Features pass the SDK's engines and transports through under the SDK's names. The default is
`protocol-ws` with `rustls`. `protocol-http`, `protocol-grpc`, `native-tls`, `kv-mem`,
`kv-rocksdb`, `kv-surrealkv`, `kv-indxdb`, `kv-tikv`, `scripting`, `http`, `jwks`, `ml` and
`cjk` are opt-in. `parse` enables `umbriel::parse::value`, and `migrations` enables `umbriel::migrations` and its
command line.

Endpoints, namespaces, databases and credentials come from the application's configuration. The
credential types are in [`opt::auth`](crate::opt::auth).

# Example

```rust,no_run
use std::sync::Arc;

use umbriel::{
    All, Executor, Model, RecordId, Result, Surreal, SurrealCrud, SurrealCrudNode,
    engine::any::{self, Any},
    model,
    statements::{Returning, select},
};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

/// Runs the same on a client and inside an SDK transaction.
async fn store(db: &impl Executor) -> Result<Vec<User>> {
    User { id: User::id("alice"), name: "Alice".to_owned() }.create(db).await?;
    User::insert_many(db, vec![User { id: User::id("bob"), name: "Bob".to_owned() }]).await?;
    select(All).from(User::table()).decode::<User>().execute(db).await
}

async fn run(url: &str) -> Result<Vec<User>> {
    let db: Surreal<Any> = any::connect(url).await?;
    db.use_ns("app").use_db("app").await?;

    // Execution borrows its handle, so a shared client is never cloned.
    let shared = Arc::new(db);
    store(shared.as_ref()).await?;

    // An SDK transaction persists only when it commits.
    let tx = shared.as_ref().clone().begin().await?;
    let users = store(&tx).await?;
    tx.commit().await?;
    Ok(users)
}
# fn main() {}
```

# Where to go next

- **Models:** the [`model`] attribute, which covers record ids, stored types, the record
  operations, partial updates, and `NONE` versus `NULL`.
- **Statements and queries:** the [`statements`] module, which covers write statements and their
  return modes, typed `SELECT`, clauses, control flow and schema definitions.
- **Running queries:** [`Executor`] covers clients, sessions, statistics, SDK transactions and
  abandoned writes.
- **Errors:** [`Error`] covers how failures are classified, and [`Error::retry`] says whether an
  operation can safely run again.
- **Functions:** the [`functions`] module holds SurrealDB's built-in functions, typed by their
  result kind ([`kinds`]).
- **Migrations:** the `umbriel::migrations` module, behind the `migrations` feature.
