//! `LIVE SELECT` and the subscriptions it starts.
//!
//! SurrealDB 3.3.4 parses `LIVE SELECT [DIFF | @fields] FROM @table [WHERE @condition]
//! [FETCH @fields]` (`parse_live_stmt`). Live queries work over WebSocket and embedded engines;
//! the HTTP engine refuses them.

use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::{FutureExt, Stream, StreamExt};
use surrealdb::{
    Connection, Notification, Surreal,
    method::QueryStream,
    opt::PatchOp,
    types::{Action, SerializationError, SurrealValue, Uuid, Value},
};

use crate::{
    Error, Query, Render, Renderer, Result, Runnable,
    execution::{decode, execute},
};

use super::{
    Cardinality, Statement, impl_statement, kill::kill, select::SelectStatement,
    select::SelectStatementInit,
};

// The executor a dropped stream's `KILL` runs on, as the SDK spawns its own tasks.
#[cfg(not(target_family = "wasm"))]
use tokio::spawn;
#[cfg(target_family = "wasm")]
use wasm_bindgen_futures::spawn_local as spawn;

/// Turns a `SELECT` into a `LIVE SELECT` whose notifications decode into the select's output
/// type `O`.
///
/// `O` comes from the select: [`select`](super::select) gives a [`Value`], a model's
/// [`select`](crate::SurrealRead::select) the model, `select_value` and `select_fields` their
/// typed projections. Each notification carries one record, so the select's cardinality is
/// dropped. `LIVE SELECT` takes one table and only `WHERE` and `FETCH` besides the projection:
/// any other clause, `ONLY`, or more than one target fails when the statement is rendered.
///
/// ```rust
/// use umbriel::{*, statements::{live, select}};
///
/// let person = Table::new("person");
/// let statement = live(select(All).from(person.clone()).where_(Field::new("age").greater_than(18)));
/// assert_eq!(statement.to_debug_sql(), "LIVE SELECT * FROM person WHERE age > 18;");
/// assert_eq!(live(select(All).from(person)).diff().to_debug_sql(), "LIVE SELECT DIFF FROM person;");
/// ```
pub fn live<O: SurrealValue, C: Cardinality>(
    select: SelectStatement<O, C>,
) -> LiveSelectStatement<O> {
    let (select, only) = select.into_live_parts();
    LiveSelectStatement {
        select,
        only,
        diff: false,
        decode: decode::<O>,
        output: PhantomData,
    }
}

/// A `LIVE SELECT` statement whose notifications decode into `O`.
#[must_use]
pub struct LiveSelectStatement<O> {
    select: SelectStatementInit,
    only: bool,
    diff: bool,
    decode: fn(Value) -> Result<O>,
    output: PhantomData<fn() -> O>,
}

impl<O: 'static> LiveSelectStatement<O> {
    /// `LIVE SELECT DIFF`: each notification carries the JSON Patch operations of the change
    /// instead of the record. `DIFF` replaces the projection, so the select's must be `*`.
    ///
    /// The operations decode by their `op`: `add`, `remove`, `replace` and `change` (a text
    /// patch, the only form SurrealDB's diff writes for a changed string). Any other `op`, or a
    /// missing or mistyped field, is [`Error::Conversion`].
    pub fn diff(self) -> LiveSelectStatement<Vec<PatchOp>> {
        LiveSelectStatement {
            select: self.select,
            only: self.only,
            diff: true,
            decode: decode_patches,
            output: PhantomData,
        }
    }

    /// Registers the live query on `db` and returns the stream of its notifications.
    ///
    /// # Errors
    ///
    /// The statement's render errors, the database's refusal (such as
    /// [`Error::NotAllowed`]), or the engine's, such as HTTP, which has no live queries.
    pub async fn subscribe<C: Connection>(&self, db: &Surreal<C>) -> Result<LiveStream<O, C>> {
        let query = self.to_query()?;
        let (query_id, stream) = register(db, &query).await?;
        Ok(LiveStream {
            subscription: Subscription::new(db.clone(), query_id),
            query,
            stream,
            decode: self.decode,
            ended: false,
        })
    }
}

impl<O> Statement for LiveSelectStatement<O> {
    fn render_body(&self, out: &mut Renderer) {
        self.select.render_live(self.diff, self.only, out);
    }
}

impl_statement!([O: 'static] LiveSelectStatement<O>);

/// A change a live query reports.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveEvent<O> {
    /// A record was created.
    Create(O),
    /// A record was updated.
    Update(O),
    /// A record was deleted; the value is the record as it was.
    Delete(O),
    /// The live query's `WHERE` or projection failed to evaluate. The value is SurrealDB's
    /// message, never decoded as `O`. SurrealDB 3.3.4 skips such records without sending this
    /// (`doc/lives.rs`), but the SDK's `Action` has it, so a stream reports it if it comes.
    Error(Value),
}

/// The stream of a live query's notifications, from [`LiveSelectStatement::subscribe`].
///
/// Each item is a [`LiveEvent`], or an error:
/// - a notification whose record doesn't decode into `O` is [`Error::Conversion`], and the
///   stream goes on;
/// - a transport failure is the stream's last item: the stream ends after it.
///
/// The stream ends when the live query does: after [`kill`](Self::kill), or with the connection.
/// Invalidating the session also ends the live query on the server, but SurrealDB 3.3.4 sends
/// no `Killed` notification for it, so the stream stays open and receives nothing: drop it.
///
/// Dropping the stream ends the live query as best effort: unless a [`kill`](Self::kill)
/// succeeded, it sends a `KILL` through a clone of the handle that subscribed (the same
/// namespace, database and sign-in), from a task of its own, and
/// nothing reports a failure. It does so after the stream ended too: an end or a transport
/// failure stops the notifications without confirming that the server removed the query, and
/// SurrealDB 3.3.4 accepts a `KILL` of an id it no longer holds. [`kill`](Self::kill) runs the
/// `KILL` and returns its result. The SDK's stream sends a `KILL` on drop as well, but
/// SurrealDB 3.3.4's SDK runs it in a new, empty session (`From<Arc<Inner>> for Surreal`), which
/// the server refuses; it only logs the failure. On native targets the stream must be dropped
/// inside a Tokio runtime, which the SDK's own drop needs too.
///
/// The SDK replays sign-in, `USE` and variables when a WebSocket reconnects, but not live
/// queries: after a reconnect the stream stays open and receives nothing, and the SDK gives no
/// signal of the reconnect. [`resubscribe`](Self::resubscribe) registers the query again.
///
/// ```rust,no_run
/// use futures::StreamExt;
/// use umbriel::{All, Table, statements::{LiveEvent, live, select}};
///
/// # async fn watch(db: &umbriel::Surreal<umbriel::engine::any::Any>) -> umbriel::Result<()> {
/// let mut notes = live(select(All).from(Table::from("note"))).subscribe(db).await?;
/// while let Some(event) = notes.next().await {
///     match event? {
///         LiveEvent::Create(note) | LiveEvent::Update(note) => println!("now {note:?}"),
///         LiveEvent::Delete(note) => println!("gone {note:?}"),
///         LiveEvent::Error(message) => println!("the live query failed: {message:?}"),
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[must_use = "streams do nothing unless you poll them"]
pub struct LiveStream<O, C: Connection> {
    subscription: Subscription<C>,
    query: Query,
    stream: QueryStream<Notification<Value>>,
    decode: fn(Value) -> Result<O>,
    /// The SDK stream ended or failed, so it is no longer polled. Whether a `KILL` is still owed
    /// is the subscription's.
    ended: bool,
}

impl<O, C: Connection> LiveStream<O, C> {
    /// The live query's id, as `KILL` takes it. It changes with [`resubscribe`](Self::resubscribe).
    pub fn query_id(&self) -> Uuid {
        self.subscription.query_id
    }

    /// Ends the live query with `KILL` and returns its result.
    ///
    /// After a successful `KILL` the stream's drop sends none of its own; after a failed one it
    /// still sends its best-effort `KILL`. The SDK ends its stream
    /// on SurrealDB's `Killed` notification. If that has already arrived when the `KILL` returns,
    /// the SDK's stream is dropped with nothing left to end. If not, the SDK sends a second
    /// `KILL` on drop, which fails as described on [`LiveStream`] and changes nothing. Umbriel
    /// doesn't wait for the notification: without a timer of its own the wait couldn't be bounded.
    ///
    /// # Errors
    ///
    /// The `KILL`'s failure, such as [`Error::NotAllowed`] for a session that doesn't own the
    /// query. SurrealDB 3.3.4 accepts a `KILL` of an id it no longer holds.
    pub async fn kill(mut self) -> Result<()> {
        self.subscription.end().await?;
        drain_ready(&mut self.stream);
        Ok(())
    }

    /// Registers the same live query again, then ends the old one, and delivers from the new
    /// one. Use it after the connection was re-established, which loses live queries.
    ///
    /// - If registering fails, `self` is unchanged: the same query id and the same stream.
    /// - On success, `self` switches to the new query before anything else is awaited. The old
    ///   query is then ended with `KILL`. Notifications still buffered on the old stream are
    ///   discarded.
    /// - If the future is dropped while the registration is in flight, the server may register a
    ///   query whose id never reaches the client, as with any dropped `LIVE` request. If it is
    ///   dropped later, `self` already holds the new query, and the old one, owned by the future,
    ///   is ended on drop as a dropped stream's is: a best-effort `KILL` through a clone of the
    ///   handle.
    /// - If the old query's `KILL` fails, its best-effort `KILL` is still sent when the future
    ///   returns.
    ///
    /// Events between the loss of the old query and the new registration are lost, and while
    /// both are registered an event can arrive on either, so events around the switch can be
    /// lost or repeated. No history is recovered; changefeeds (`SHOW CHANGES`) keep one.
    ///
    /// # Errors
    ///
    /// The registration's failure, with `self` unchanged; or [`Error::StaleSubscription`] when
    /// the new query is in place but the `KILL` of the old one failed, for example because the
    /// connection dropped. After a reconnect the server no longer holds the old query, and
    /// SurrealDB 3.3.4 accepts the `KILL` of an id it no longer holds, so resubscribing succeeds.
    pub async fn resubscribe(&mut self) -> Result<()> {
        let (query_id, stream) = register(&self.subscription.db, &self.query).await?;
        // The new query lives in the session of the handle it was registered on, which the
        // stream keeps: dropping a handle ends its session and the session's live queries
        // (`Drop for Surreal`). The old query's `KILL` runs on a clone, a session of its own.
        let old_id = std::mem::replace(&mut self.subscription.query_id, query_id);
        self.subscription.armed = true;
        // From here the old query is owned by this future: dropping it sends the `KILL`.
        let mut old = Subscription::new(self.subscription.db.clone(), old_id);
        let mut old_stream = std::mem::replace(&mut self.stream, stream);
        self.ended = false;
        match old.end().await {
            Ok(()) => {
                drain_ready(&mut old_stream);
                Ok(())
            }
            Err(source) => Err(Error::StaleSubscription {
                query_id: old.query_id,
                source: Box::new(source),
            }),
        }
    }
}

/// A registered live query that is ended with a `KILL` through a clone of the subscribing handle
/// when dropped, until a `KILL` succeeds. The SDK's stream can't be relied on for it (see
/// [`LiveStream`]).
///
/// Each clone of a `Surreal` handle is a session of its own, and dropping a handle ends its
/// session together with the session's live queries. So `db` must outlive any query registered
/// on it: `resubscribe` registers on the stream's handle and keeps it, and gives the old query a
/// clone to `KILL` from.
struct Subscription<C: Connection> {
    db: Surreal<C>,
    query_id: Uuid,
    /// No successful `KILL` yet, so the drop sends one.
    armed: bool,
}

impl<C: Connection> Subscription<C> {
    fn new(db: Surreal<C>, query_id: Uuid) -> Self {
        Self {
            db,
            query_id,
            armed: true,
        }
    }

    /// Runs the `KILL` and disarms the drop on success.
    async fn end(&mut self) -> Result<()> {
        kill(self.query_id).run(&self.db).await?;
        self.armed = false;
        Ok(())
    }
}

impl<C: Connection> Drop for Subscription<C> {
    /// Sends the `KILL` from a task of its own unless one succeeded.
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let db = self.db.clone();
        let query_id = self.query_id;
        spawn(async move {
            // The owner is gone, so no caller is left to take the result; `kill` and
            // `resubscribe` return theirs.
            let _ = kill(query_id).run(&db).await;
        });
    }
}

// Nothing in the stream relies on being pinned: the SDK stream it polls is `Unpin`, and the
// subscription and query are only borrowed. So it's `Unpin` whatever the connection type.
impl<O, C: Connection> Unpin for LiveStream<O, C> {}

impl<O, C: Connection> Stream for LiveStream<O, C> {
    type Item = Result<LiveEvent<O>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.ended {
            return Poll::Ready(None);
        }
        match this.stream.poll_next_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                this.ended = true;
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(error))) => {
                this.ended = true;
                Poll::Ready(Some(Err(Error::from(error))))
            }
            Poll::Ready(Some(Ok(notification))) => {
                match live_event(notification.action, notification.data, this.decode) {
                    Some(item) => Poll::Ready(Some(item)),
                    None => {
                        this.ended = true;
                        Poll::Ready(None)
                    }
                }
            }
        }
    }
}

/// The event a notification with `action` and `data` reports: `None` for `Killed`, the live
/// query's end.
pub(crate) fn live_event<O>(
    action: Action,
    data: Value,
    decode: fn(Value) -> Result<O>,
) -> Option<Result<LiveEvent<O>>> {
    Some(match action {
        Action::Create => decode(data).map(LiveEvent::Create),
        Action::Update => decode(data).map(LiveEvent::Update),
        Action::Delete => decode(data).map(LiveEvent::Delete),
        Action::Error => Ok(LiveEvent::Error(data)),
        Action::Killed => return None,
    })
}

/// Runs the `LIVE SELECT` and takes its query id and notification stream.
async fn register<C: Connection>(
    db: &Surreal<C>,
    query: &Query,
) -> Result<(Uuid, QueryStream<Notification<Value>>)> {
    let mut results = execute(db, query.clone()).await?;
    let query_id = results
        .take::<Option<Uuid>>(0)
        .map_err(Error::Conversion)?
        .ok_or_else(|| conversion("LIVE SELECT returned no query id".to_owned()))?;
    let stream = results.stream::<Notification<Value>>(0)?;
    Ok((query_id, stream))
}

/// Takes the notifications already delivered, without waiting, so a `Killed` that has arrived
/// ends the SDK stream and its drop has nothing left to `KILL`.
fn drain_ready(stream: &mut QueryStream<Notification<Value>>) {
    while let Some(Some(_)) = stream.next().now_or_never() {}
}

fn conversion(message: String) -> Error {
    Error::Conversion(surrealdb::Error::serialization(
        message,
        SerializationError::Deserialization,
    ))
}

/// Decodes a `DIFF` notification's operations by their `op`. The SDK's own `PatchOp` decoder is
/// untagged, and `add` and `replace` have the same fields, so it can't tell them apart.
pub(crate) fn decode_patches(value: Value) -> Result<Vec<PatchOp>> {
    let Value::Array(operations) = value else {
        return Err(conversion(format!(
            "a DIFF notification is an array of operations, not {value:?}"
        )));
    };
    operations.into_iter().map(decode_patch).collect()
}

fn decode_patch(operation: Value) -> Result<PatchOp> {
    let Value::Object(mut fields) = operation else {
        return Err(conversion(format!(
            "a JSON Patch operation is an object, not {operation:?}"
        )));
    };
    let text = |fields: &mut surrealdb::types::Object, name: &str| match fields.remove(name) {
        Some(Value::String(text)) => Ok(text),
        other => Err(conversion(format!(
            "a JSON Patch operation's `{name}` is a string, not {other:?}"
        ))),
    };
    let op = text(&mut fields, "op")?;
    let path = text(&mut fields, "path")?;
    let value = |fields: &mut surrealdb::types::Object| {
        fields
            .remove("value")
            .ok_or_else(|| conversion(format!("the JSON Patch `{op}` at `{path}` has no value")))
    };
    Ok(match op.as_str() {
        "add" => PatchOp::Add {
            value: value(&mut fields)?,
            path,
        },
        "replace" => PatchOp::Replace {
            value: value(&mut fields)?,
            path,
        },
        "remove" => PatchOp::Remove { path },
        "change" => PatchOp::Change {
            value: text(&mut fields, "value")?,
            path,
        },
        other => {
            return Err(conversion(format!(
                "`{other}` is not a JSON Patch operation a DIFF notification carries"
            )));
        }
    })
}
