//! Query execution: every query runs through [`execute`], which checks each statement's result
//! before reporting success.

use surrealdb::{
    Connection, IndexedResults, Surreal,
    types::{AlreadyExistsError, NotFoundError, QueryError},
};

use crate::{Error, Query, Result, StatementContext};

/// Turns a database error into Umbriel's error, keeping it as the source.
///
/// Shared by statement execution, which passes the failed statement's context, and the SDK
/// methods models call directly, which pass none. The classification reads only the error's
/// structured details, never its message:
/// - an already-exists error for a record is [`Error::RecordExists`];
/// - a not-found error for a table is [`Error::TableNotFound`];
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
pub(crate) async fn execute(db: &Surreal<impl Connection>, query: Query) -> Result<IndexedResults> {
    let (sql, vars) = query.into_parts();
    let mut results = db
        .query(sql)
        .bind(vars)
        .await
        .map_err(|error| classify(error, None))?;
    // Counted first: `take_errors` removes the failed statements from the results.
    let statement_count = results.num_statements();
    match select_failure(statement_count, results.take_errors()) {
        Some(error) => Err(error),
        None => Ok(results),
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
