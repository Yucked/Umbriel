

# Borrowed handles

Every execution API borrows an `Executor`: [`Runnable::run`](crate::Runnable::run),
`get_data` and `run_with_stats`, statement `execute` methods, every model method, `TableOf`
methods, and `Function::run`. Umbriel never clones the client. In the SDK, a `clone()` opens a
new server session and replays the old session's setup onto it. A client shared as
`Arc<Surreal<Any>>` is passed as `arc.as_ref()`.

Migrations take `&Surreal<impl Connection>` only, since each migration runs its own
`BEGIN … COMMIT`.

# Sessions

A cloned client has its own session. Changing the namespace or database of one doesn't move the
other, and each keeps its own sign-in. A clone starts from its original's session: the embedded
engine copies it, and a remote engine replays the setup the SDK recorded through `use_ns`,
`use_db` and sign-in.

**`USE` statements are request-scoped.** A `USE` statement's selection lasts for the rest of its
own request on every engine, and it is never written back to the session. Only the SDK's
`use_ns` and `use_db` change the session. So [`use_`](crate::statements::use_) belongs inside a
chain or transaction, and switching a handle is `db.use_ns(…).use_db(…)`. Each `USE` result
decodes as [`UseResult`](crate::statements::UseResult).

SDK client configuration, session variables, `wait_for`, `health()` and `version()` are called
directly on the SDK handle.

# Results and statistics

[`Runnable::run`](crate::Runnable::run) checks every statement of the response, and fails on the
first failure.

[`Runnable::get_data`](crate::Runnable::get_data) decodes the first statement's result as it is:
`NONE` is `None`, and anything else must decode as `T`. A statement without `ONLY` returns a
list, so ask for `Vec<User>`, or write the statement with `ONLY`.

[`Runnable::run_with_stats`](crate::Runnable::run_with_stats) returns the SDK's
[`WithStats`](crate::WithStats), checked as `run` checks a response. Each statement's
[`Stats`](crate::Stats) holds its execution time.

The futures are `Send` but not `Sync`, as the SDK's have been since 3.3.0. They can be spawned on
a multi-threaded runtime, but not shared by reference across threads.

# SDK transactions

An open SDK [`Transaction`](crate::Transaction) is an `Executor`, so statements and model methods
run inside it, across several requests, and persist only when it commits:

```rust,no_run
use umbriel::{Model, RecordId, Result, Surreal, SurrealCrudNode, engine::any::Any, model};

#[model(node, table = "user")]
pub struct User {
    pub id: RecordId<Self>,
    pub name: String,
}

async fn add_alice(db: &Surreal<Any>) -> Result<()> {
    // `begin` takes the client by value: a transaction starts from a clone, a session of its own.
    let tx = db.clone().begin().await?;
    User { id: User::id("alice"), name: "Alice".to_owned() }.create(&tx).await?;
    tx.commit().await?;
    Ok(())
}
# fn main() {}
```

- **Ownership.** `commit` and `cancel` consume the handle and return the client. Their errors
  convert with `?` and are classified like any other.
- **Drop.** A handle dropped without `commit` or `cancel` commits nothing and reports nothing.
  Its session ends, and the SDK releases the open transaction with it. SDK 3.3.4 offers no way to
  observe that release.
- **Cancellation.** On the embedded engine, a query future dropped part-way marks the
  transaction abandoned. The next `commit` rolls it back and fails, with an error that has no
  structured kind ([`Retry::Uncertain`](crate::Retry::Uncertain)).
- **Engines.** Transactions work on the WebSocket and embedded engines only: the server refuses
  `begin` over HTTP. SurrealDB allows at most 64 open transactions per connection.
- **Conflicts** surface at `commit`. Over WebSocket, a commit conflict is
  [`Error::Conflict`](crate::Error::Conflict) ([`Retry::Safe`](crate::Retry::Safe)): restart the
  whole transaction. **On the embedded engine it is [`Error::Query`](crate::Error::Query) with
  [`Retry::Uncertain`](crate::Retry::Uncertain)**, because the engine drops the conflict's kind.
  `SELECT … FOR UPDATE` takes no lock; a conflicting write fails the commit the same way.

# Abandoned writes

Umbriel never retries a write. If the caller drops an execution future, or its own timeout fires
while a write is in flight, the write may or may not have applied, and Umbriel reports nothing
about it. [`Error::retry`](crate::Error::retry) gives the advice an error itself supports.
