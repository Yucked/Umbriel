//! Query execution: every query runs through [`execute`], which checks each statement's result
//! before reporting success.

use surrealdb::{
    Connection, IndexedResults, Surreal,
    method::{Create, Delete, Insert, Query as SdkQuery, Select, Transaction, Upsert, WithStats},
    opt::{CreateResource, IntoResource},
    types::{AlreadyExistsError, NotFoundError, QueryError, SurrealValue, Value},
};

use crate::{Error, Query, Result, StatementContext};

mod sealed {
    pub trait Sealed {}
}

impl<C: Connection> sealed::Sealed for Surreal<C> {}
impl<C: Connection> sealed::Sealed for Transaction<C> {}

/// Where Umbriel runs its queries and record operations: an SDK client ([`Surreal`]) or an open
/// SDK transaction ([`Transaction`]). Every execution API borrows one, so a shared client, such
/// as one held in an `Arc` and passed as `arc.as_ref()`, is never cloned.
///
/// Through a [`Transaction`], statements and model methods run inside it and persist only when
/// it commits. The trait is sealed: those two types are its only implementors. Its methods
/// forward to the SDK methods of the same name.
pub trait Executor: sealed::Sealed + Sync {
    /// The SDK connection the handle runs on.
    type Connection: Connection;

    /// The SDK's `query`, with the text given as a `String`.
    fn query(&self, sql: String) -> SdkQuery<'_, Self::Connection>;

    /// The SDK's `select`.
    fn select<O>(&self, resource: impl IntoResource<O>) -> Select<'_, Self::Connection, O>;

    /// The SDK's `create`.
    fn create<R>(&self, resource: impl CreateResource<R>) -> Create<'_, Self::Connection, R>;

    /// The SDK's `insert`.
    fn insert<O>(&self, resource: impl IntoResource<O>) -> Insert<'_, Self::Connection, O>;

    /// The SDK's `upsert`.
    fn upsert<O>(&self, resource: impl IntoResource<O>) -> Upsert<'_, Self::Connection, O>;

    /// The SDK's `delete`.
    fn delete<O>(&self, resource: impl IntoResource<O>) -> Delete<'_, Self::Connection, O>;
}

/// Implements [`Executor`] for an SDK handle type by forwarding to its inherent methods.
macro_rules! impl_executor {
    ($handle:ident) => {
        impl<C: Connection> Executor for $handle<C> {
            type Connection = C;

            fn query(&self, sql: String) -> SdkQuery<'_, C> {
                $handle::query(self, sql)
            }

            fn select<O>(&self, resource: impl IntoResource<O>) -> Select<'_, C, O> {
                $handle::select(self, resource)
            }

            fn create<R>(&self, resource: impl CreateResource<R>) -> Create<'_, C, R> {
                $handle::create(self, resource)
            }

            fn insert<O>(&self, resource: impl IntoResource<O>) -> Insert<'_, C, O> {
                $handle::insert(self, resource)
            }

            fn upsert<O>(&self, resource: impl IntoResource<O>) -> Upsert<'_, C, O> {
                $handle::upsert(self, resource)
            }

            fn delete<O>(&self, resource: impl IntoResource<O>) -> Delete<'_, C, O> {
                $handle::delete(self, resource)
            }
        }
    };
}

impl_executor!(Surreal);
impl_executor!(Transaction);

/// Turns a database error into Umbriel's error, keeping it as the source.
///
/// Shared by statement execution, which passes the failed statement's context, and the SDK
/// methods called directly (record operations, `begin`, `commit`, `cancel`, through
/// `From<surrealdb::Error>`), which pass none. The classification reads only the error's
/// structured details, never its message:
/// - an already-exists error for a record is [`Error::RecordExists`];
/// - a not-found error for a table is [`Error::TableNotFound`];
/// - a transaction conflict is [`Error::Conflict`];
/// - a not-allowed error of any kind is [`Error::NotAllowed`];
/// - a connection error of any kind is [`Error::Connection`];
/// - anything else is [`Error::Statement`] with a context, or [`Error::Query`] without one.
#[doc(hidden)]
pub fn classify(source: surrealdb::Error, statement: Option<StatementContext>) -> Error {
    if let Some(AlreadyExistsError::Record { id }) = source.already_exists_details() {
        let id = id.clone();
        return Error::RecordExists {
            id,
            source: Box::new(source),
            statement,
        };
    }
    if let Some(NotFoundError::Table { name }) = source.not_found_details() {
        let table = name.clone();
        return Error::TableNotFound {
            table,
            source: Box::new(source),
            statement,
        };
    }
    if matches!(
        source.query_details(),
        Some(QueryError::TransactionConflict)
    ) {
        return Error::Conflict {
            source: Box::new(source),
            statement,
        };
    }
    if source.is_not_allowed() {
        return Error::NotAllowed {
            source: Box::new(source),
            statement,
        };
    }
    if source.is_connection() {
        return Error::Connection {
            source: Box::new(source),
            statement,
        };
    }
    match statement {
        Some(StatementContext { index, related }) => Error::Statement {
            index,
            source: Box::new(source),
            related,
        },
        None => Error::Query(source),
    }
}

/// Runs a rendered query and checks every statement's result.
pub(crate) async fn execute(db: &impl Executor, query: Query) -> Result<IndexedResults> {
    let (sql, vars) = query.into_parts();
    let mut results = db.query(sql).bind(vars).await?;
    check(&mut results)?;
    Ok(results)
}

/// [`execute`], with the SDK's per-statement statistics.
pub(crate) async fn execute_with_stats(
    db: &impl Executor,
    query: Query,
) -> Result<WithStats<IndexedResults>> {
    let (sql, vars) = query.into_parts();
    let mut response = db.query(sql).bind(vars).with_stats().await?;
    check(&mut response.0)?;
    Ok(response)
}

/// Fails with the response's chosen statement failure, if it holds one.
fn check(results: &mut IndexedResults) -> Result<()> {
    // Counted first: `take_errors` removes the failed statements from the results.
    let statement_count = results.num_statements();
    match select_failure(statement_count, results.take_errors()) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Decodes the first statement's result as one value: `NONE` is `None`, anything else decodes as
/// `V`, an array included.
///
/// Takes the raw value because the SDK's `take::<Option<V>>` unwraps a one-element array and
/// rejects longer ones, which breaks every `V` that is itself an array, such as a `RETURN DIFF`
/// patch.
pub(crate) fn take_one<V: SurrealValue>(mut results: IndexedResults) -> Result<Option<V>> {
    match results.take::<Value>(0).map_err(Error::Conversion)? {
        Value::None => Ok(None),
        value => V::from_value(value).map(Some).map_err(Error::Conversion),
    }
}

/// Chooses the error to report for a response of `statement_count` statements, given the failed
/// ones by index; `None` when the response holds no failure.
///
/// The choice uses only statement indexes and error kinds, never message text:
/// - an error that is neither `NotExecuted` nor `Cancelled` is a failure;
/// - a `Cancelled` error is excused only when the next result that is not `Cancelled` succeeded,
///   which is the `NONE` of an intentional `CANCEL TRANSACTION`; a timeout or other
///   cancellation fails the closing statement too, so it is reported;
/// - `NotExecuted` errors are excused while a failure explains them;
/// - when only `NotExecuted` errors remain, the last one of the first run of consecutive failed
///   statements is chosen. This is a selection policy, not an identification of the statement:
///   SurrealDB puts a failed commit's cause on the closing `COMMIT`, after the rollback errors of
///   the statements before it.
///
/// The lowest-index failure is chosen; every other error is returned with it as context. Each run
/// of consecutive cancelled statements is classified once, so after sorting, one pass suffices.
#[doc(hidden)]
pub fn select_failure(
    statement_count: usize,
    errors: impl IntoIterator<Item = (usize, surrealdb::Error)>,
) -> Option<Error> {
    let mut errors = errors.into_iter().collect::<Vec<_>>();
    errors.sort_unstable_by_key(|(index, _)| *index);

    let is_cancelled =
        |error: &surrealdb::Error| matches!(error.query_details(), Some(QueryError::Cancelled));

    let mut not_executed = None;
    let mut primary = None;
    let mut position = 0;
    while position < errors.len() {
        match errors[position].1.query_details() {
            Some(QueryError::NotExecuted) => {
                not_executed = not_executed.or(Some(position));
                position += 1;
            }
            Some(QueryError::Cancelled) => {
                // Classify the whole run of consecutive cancelled statements at once: it is
                // intentional only when the statement after it ran and succeeded.
                let mut last = position;
                while let Some((next, error)) = errors.get(last + 1)
                    && *next == errors[last].0 + 1
                    && is_cancelled(error)
                {
                    last += 1;
                }
                let after = errors[last].0 + 1;
                let succeeded_after = after < statement_count
                    && errors.get(last + 1).is_none_or(|(next, _)| *next != after);
                if !succeeded_after {
                    primary = Some(position);
                    break;
                }
                position = last + 1;
            }
            _ => {
                primary = Some(position);
                break;
            }
        }
    }

    let primary = primary.or_else(|| {
        // The last error of the run of consecutive failed statements starting there.
        let first = not_executed?;
        let run = errors[first..]
            .windows(2)
            .take_while(|pair| pair[1].0 == pair[0].0 + 1)
            .count();
        Some(first + run)
    })?;

    let (index, source) = errors.remove(primary);
    Some(classify(
        source,
        Some(StatementContext {
            index,
            related: errors,
        }),
    ))
}
