use surrealdb::types::{ConnectionError, QueryError};
use thiserror::Error;

/// Errors returned by Umbriel.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The query could not be built from its inputs.
    #[error("invalid query: {0}")]
    QueryBuilder(String),

    /// More records were returned than the operation allows.
    #[error("expected at most {expected} record(s), but more were returned")]
    TooManyItemsReturned {
        /// The maximum number of records the operation accepts.
        expected: usize,
    },

    /// The database rejected or failed to run the query.
    #[error("query failed: {0}")]
    Query(#[source] surrealdb::Error),

    /// A statement of the query failed.
    #[error("statement {index} failed: {source}{}", related_note(.related))]
    Statement {
        /// The position of the statement in the query, from 0.
        index: usize,
        /// The statement's error. Boxed so `Error`, and every `Result` carrying it, stays small.
        #[source]
        source: Box<surrealdb::Error>,
        /// Every other statement error of the response, in statement order. Kept because the
        /// detailed cause can sit on another statement, such as the closing `COMMIT` of a
        /// transaction whose commit failed.
        related: Vec<(usize, surrealdb::Error)>,
    },

    /// A write was refused because a record with the same id already exists, for example a
    /// second `CREATE` of one id.
    #[error("record `{id}` already exists{}", statement_note(.statement.as_ref()))]
    RecordExists {
        /// The existing record's id, in SurrealQL syntax.
        id: String,
        /// The database's error.
        #[source]
        source: Box<surrealdb::Error>,
        /// Where the failure occurred, when it came from a statement of a query rather than an
        /// SDK method.
        statement: Option<StatementContext>,
    },

    /// A statement used a table that is not defined, on a database that does not define tables
    /// implicitly (such as one defined `STRICT`). Since SurrealDB 3.0.0 this is an error rather
    /// than an empty result.
    #[error("table `{table}` does not exist{}", statement_note(.statement.as_ref()))]
    TableNotFound {
        /// The table's name.
        table: String,
        /// The database's error.
        #[source]
        source: Box<surrealdb::Error>,
        /// Where the failure occurred, when it came from a statement of a query rather than an
        /// SDK method.
        statement: Option<StatementContext>,
    },

    /// A transaction conflicted with another and was not committed (SurrealDB's
    /// `TransactionConflict`). [`Error::retry`] is [`Retry::Safe`].
    #[error("transaction conflict{}", statement_note(.statement.as_ref()))]
    Conflict {
        /// The database's error.
        #[source]
        source: Box<surrealdb::Error>,
        /// Where the failure occurred, when it came from a statement of a query rather than an
        /// SDK method.
        statement: Option<StatementContext>,
    },

    /// The database refused the operation for the session: a permission, an authentication
    /// failure, or a capability the server or engine does not grant, such as scripting. The
    /// source's `not_allowed_details()` says which.
    #[error("not allowed{}", statement_note(.statement.as_ref()))]
    NotAllowed {
        /// The database's error.
        #[source]
        source: Box<surrealdb::Error>,
        /// Where the failure occurred, when it came from a statement of a query rather than an
        /// SDK method.
        statement: Option<StatementContext>,
    },

    /// The client's connection was unusable: never connected, already connected, or failed in
    /// transport. The source's `connection_details()` says which; [`Error::retry`] depends on it.
    #[error("connection error{}", statement_note(.statement.as_ref()))]
    Connection {
        /// The database's error.
        #[source]
        source: Box<surrealdb::Error>,
        /// Where the failure occurred, when it came from a statement of a query rather than an
        /// SDK method.
        statement: Option<StatementContext>,
    },

    /// A table name chosen at run time is not one of the model's tables: neither its default
    /// table nor a longer name starting with its `table_prefix`.
    #[error("table `{table}` is not a table of `{model}`")]
    TableNotInFamily {
        /// The rejected name.
        table: String,
        /// The model's default table.
        model: &'static str,
    },

    /// A value returned by the database did not convert into the requested type.
    #[error("unexpected value from the database: {0}")]
    Conversion(#[source] surrealdb::Error),

    /// Fields requested for fetching are not record links.
    #[error(
        "these fields are not record links and cannot be fetched: {0}; only `link_self`, `link_one` and `link_many` fields can be"
    )]
    FieldsUnfetchableNotARecordLink(String),

    /// No record was returned where one was required.
    #[error("no record returned from {0}")]
    RecordNotFound(String),

    /// A migration failed.
    #[cfg(feature = "migrations")]
    #[error(transparent)]
    Migration(Box<crate::migrations::MigrationError>),
}

/// The position of a failed statement within its query, and the other statement errors of the
/// same response.
#[derive(Debug)]
pub struct StatementContext {
    /// The position of the statement in the query, from 0.
    pub index: usize,
    /// Every other statement error of the response, in statement order.
    pub related: Vec<(usize, surrealdb::Error)>,
}

/// What a failure says about running the operation again. Umbriel never retries anything
/// itself; the advice is for the caller's own retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Retry {
    /// The failed transaction can be restarted from its beginning: it conflicted with another
    /// and committed nothing. This covers that transaction only: writes that committed before
    /// it, such as earlier statements of the same batch outside it, stay applied, so replaying a
    /// larger batch is not safe.
    Safe,
    /// There isn't enough information to tell whether the write applied: the transport failed,
    /// the server reported a timeout, a commit's outcome is unknown, or the SDK gave the failure
    /// no structured kind at all. Check the database before writing again.
    Uncertain,
    /// No retry is recommended. This is not a guarantee that nothing applied.
    Never,
}

impl Error {
    /// What this failure says about running the operation again; see [`Retry`].
    ///
    /// Read only from the error's variant and the SDK error's structured details, never its
    /// message. A failure is [`Retry::Never`] only when it is known not to depend on an unknown
    /// write; anything the SDK leaves unclassified is [`Retry::Uncertain`]:
    /// - [`Error::Conflict`] is [`Retry::Safe`];
    /// - failures before anything is sent ([`Error::QueryBuilder`], a model's table or field
    ///   checks, a client that was never connected or is already connected) are
    ///   [`Retry::Never`];
    /// - server refusals are [`Retry::Never`]: [`Error::RecordExists`],
    ///   [`Error::TableNotFound`], [`Error::NotAllowed`], and SDK errors whose kind is
    ///   validation, thrown, not-allowed, not-found, already-exists, configuration, or a
    ///   statement that was not executed (`QueryError::NotExecuted`);
    /// - failures after a successful run ([`Error::Conversion`],
    ///   [`Error::TooManyItemsReturned`], [`Error::RecordNotFound`]) are [`Retry::Never`]: the
    ///   operation finished, so running it again repeats it;
    /// - every other SDK error is [`Retry::Uncertain`]: a failed transport, a timeout, a
    ///   cancellation, an unknown commit outcome (SurrealDB 3.3.4 reports it as a query error
    ///   without a kind), and errors without structured details, such as an embedded
    ///   transaction handle's failed `commit`;
    /// - a migration failure takes the advice of the database error it wraps, and is
    ///   [`Retry::Never`] otherwise (files, names, prompts and checksums).
    #[must_use]
    pub fn retry(&self) -> Retry {
        match self {
            Self::Conflict { .. } => Retry::Safe,
            Self::Connection { source, .. } => sdk_advice(source),
            Self::Query(source) => sdk_advice(source),
            Self::Statement { source, .. } => sdk_advice(source),
            Self::QueryBuilder(_)
            | Self::TooManyItemsReturned { .. }
            | Self::RecordExists { .. }
            | Self::TableNotFound { .. }
            | Self::NotAllowed { .. }
            | Self::TableNotInFamily { .. }
            | Self::Conversion(_)
            | Self::FieldsUnfetchableNotARecordLink(_)
            | Self::RecordNotFound(_) => Retry::Never,
            #[cfg(feature = "migrations")]
            Self::Migration(error) => match error.as_ref() {
                crate::migrations::MigrationError::ProblemWithQuery(error) => error.retry(),
                crate::migrations::MigrationError::DbError(source) => sdk_advice(source),
                // Every other migration error is a local failure: files, names, checksums,
                // prompts or the migration state, none of which depends on an unknown write.
                _ => Retry::Never,
            },
        }
    }
}

/// The advice an SDK error's structured kind supports. Only kinds that say the operation was
/// refused, not run, or never sent are `Never`; everything else, a missing kind included, is
/// `Uncertain`.
fn sdk_advice(source: &surrealdb::Error) -> Retry {
    if matches!(
        source.query_details(),
        Some(QueryError::TransactionConflict)
    ) {
        return Retry::Safe;
    }
    if matches!(
        source.connection_details(),
        Some(ConnectionError::Uninitialised | ConnectionError::AlreadyConnected)
    ) || matches!(source.query_details(), Some(QueryError::NotExecuted))
        || source.is_validation()
        || source.is_thrown()
        || source.is_not_allowed()
        || source.is_not_found()
        || source.is_already_exists()
        || source.is_configuration()
    {
        return Retry::Never;
    }
    Retry::Uncertain
}

/// Classifies an SDK error the way Umbriel's own execution does, so the errors of SDK calls such
/// as `begin`, `commit` and `cancel` convert with `?` into the same variants.
impl From<surrealdb::Error> for Error {
    fn from(error: surrealdb::Error) -> Self {
        crate::execution::classify(error, None)
    }
}

#[cfg(feature = "migrations")]
impl From<crate::migrations::MigrationError> for Error {
    fn from(error: crate::migrations::MigrationError) -> Self {
        Self::Migration(Box::new(error))
    }
}

fn related_note(related: &[(usize, surrealdb::Error)]) -> String {
    match related.len() {
        0 => String::new(),
        1 => " (1 related statement error)".to_owned(),
        count => format!(" ({count} related statement errors)"),
    }
}

fn statement_note(statement: Option<&StatementContext>) -> String {
    statement.map_or_else(String::new, |statement| {
        format!(
            " (statement {}{})",
            statement.index,
            related_note(&statement.related)
        )
    })
}

/// The result type returned by Umbriel.
pub type Result<T, E = Error> = std::result::Result<T, E>;
