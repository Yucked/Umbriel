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
