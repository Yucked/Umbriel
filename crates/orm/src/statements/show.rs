//! `SHOW CHANGES`: the changes a changefeed recorded, typed as [`ChangeSet`]s.
//!
//! The shapes are SurrealDB 3.3.4's (`surrealdb-datastore` `values/changefeed.rs`). With
//! `INCLUDE ORIGINAL`, an update carries the JSON patch that turns the record after the change
//! back into the record before it: current to previous, the reverse of the update itself. A
//! create is recorded as a plain update, since there is nothing to revert to.

use surrealdb::types::{
    ConversionError, Datetime, Kind, Object, RecordId, SurrealValue, Table, Value,
};

use crate::{Error, Executor, Render, Renderer, Result, execution::execute};

use super::{DiffOp, Statement, impl_statement};

/// Creates a SHOW CHANGES statement builder, reading a table's changes from `since` on. The
/// table needs a changefeed (see
/// [`DefineTableStatement::changefeed`](super::DefineTableStatement::changefeed)).
///
/// Examples
/// ```rust
/// use umbriel::{*, statements::show_changes_for_table};
///
/// let statement = show_changes_for_table("reading", 5u64).limit(10);
/// assert_eq!(statement.to_query().unwrap().sql(), "SHOW CHANGES FOR TABLE reading SINCE 5 LIMIT 10;");
///
/// let timestamp = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
/// let statement = show_changes_for_table("reading", timestamp);
/// assert_eq!(
///     statement.to_debug_sql(),
///     "SHOW CHANGES FOR TABLE reading SINCE d'2023-11-14T22:13:20Z';"
/// );
/// ```
pub fn show_changes_for_table(
    table: impl Into<Table>,
    since: impl Into<ChangesSince>,
) -> ShowChangesStatement {
    ShowChangesStatement {
        table: Some(table.into()),
        since: since.into(),
        limit: None,
    }
}

/// Creates a SHOW CHANGES statement builder, reading the changes of every table of the
/// database from `since` on. The database needs a changefeed (see
/// [`DefineDatabaseStatement::changefeed`](super::DefineDatabaseStatement::changefeed)).
///
/// ```rust
/// use umbriel::{*, statements::show_changes_for_database};
///
/// assert_eq!(
///     show_changes_for_database(0u64).to_debug_sql(),
///     "SHOW CHANGES FOR DATABASE SINCE 0;"
/// );
/// ```
pub fn show_changes_for_database(since: impl Into<ChangesSince>) -> ShowChangesStatement {
    ShowChangesStatement {
        table: None,
        since: since.into(),
        limit: None,
    }
}

/// Where `SHOW CHANGES` starts reading. SurrealQL takes it, and the limit, only as literals.
#[derive(Debug, Clone)]
pub enum ChangesSince {
    /// Changes at or after a point in time.
    Timestamp(Datetime),
    /// Changes at or after a versionstamp.
    Versionstamp(u64),
}

impl From<Datetime> for ChangesSince {
    fn from(timestamp: Datetime) -> Self {
        Self::Timestamp(timestamp)
    }
}

impl From<chrono::DateTime<chrono::Utc>> for ChangesSince {
    fn from(timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        Self::Timestamp(timestamp.into())
    }
}

impl From<u64> for ChangesSince {
    fn from(versionstamp: u64) -> Self {
        Self::Versionstamp(versionstamp)
    }
}

/// A `SHOW CHANGES` statement, for a table or the whole database.
pub struct ShowChangesStatement {
    table: Option<Table>,
    since: ChangesSince,
    limit: Option<u32>,
}

impl ShowChangesStatement {
    /// Sets the LIMIT clause for the SHOW CHANGES statement.
    pub fn limit(mut self, limit: u32) -> ShowChangesStatement {
        self.limit = Some(limit);
        self
    }

    /// Runs the statement and decodes the change sets, with each record as `R`, e.g. a model or
    /// [`Object`].
    ///
    /// # Errors
    ///
    /// The database error, or [`Error::Conversion`] when a record does not decode as `R`, a
    /// versionstamp does not fit a `u64`, or a change set does not have the shape SurrealDB 3.3.4
    /// gives it.
    pub async fn execute<R: SurrealValue>(&self, db: &impl Executor) -> Result<Vec<ChangeSet<R>>> {
        let mut results = execute(db, self.to_query()?).await?;
        match results.take::<Value>(0).map_err(Error::Conversion)? {
            Value::Array(sets) => sets.into_iter().map(change_set).collect(),
            other => Err(unexpected(
                any_array(),
                &other,
                "SHOW CHANGES returns an array",
            )),
        }
    }
}

impl Statement for ShowChangesStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("SHOW CHANGES FOR ");
        match &self.table {
            Some(table) => {
                out.push_str("TABLE ").push(table);
            }
            None => {
                out.push_str("DATABASE");
            }
        }
        out.push_str(" SINCE ");
        match &self.since {
            ChangesSince::Timestamp(timestamp) => out.push_syntax(timestamp),
            ChangesSince::Versionstamp(versionstamp) => out.push_display(versionstamp),
        };
        if let Some(limit) = self.limit {
            out.push_str(" LIMIT ").push_display(&limit);
        }
    }
}

impl_statement!(ShowChangesStatement);

/// The changes one transaction made, at one versionstamp.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ChangeSet<R = Object> {
    /// Where the transaction sits in the changefeed. `SHOW CHANGES … SINCE` takes it.
    pub versionstamp: u64,
    /// The changes, in the order they were made.
    pub changes: Vec<TableChange<R>>,
}

/// One change of a [`ChangeSet`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TableChange<R = Object> {
    /// A record was created or updated: the record after the change. Without `INCLUDE
    /// ORIGINAL`, every write gives this; with it, only a create does.
    Update(R),
    /// A record was updated, on a changefeed with `INCLUDE ORIGINAL`.
    Updated {
        /// The record after the change.
        current: R,
        /// The JSON patch that turns `current` back into the record before the change. This
        /// is the reverse of the update: applying it to `current` gives the previous record.
        revert: Vec<DiffOp>,
    },
    /// A record was deleted.
    Delete {
        /// The deleted record's id.
        id: RecordId,
        /// The deleted record, on a changefeed with `INCLUDE ORIGINAL`.
        original: Option<R>,
    },
    /// A table was defined: its definition, as `INFO … STRUCTURE` lists it.
    DefineTable(Object),
}

fn unexpected(expected: Kind, value: &Value, context: &str) -> Error {
    Error::Conversion(
        ConversionError::from_value(expected, value)
            .with_context(context.to_owned())
            .into(),
    )
}

/// The kind of an array of any length and element.
fn any_array() -> Kind {
    Kind::Array(Box::new(Kind::Any), None)
}

fn decode<T: SurrealValue>(value: Value) -> Result<T> {
    T::from_value(value).map_err(Error::Conversion)
}

/// The object `value` holds, or a conversion error naming `context`.
fn object(value: Value, context: &str) -> Result<Object> {
    match value {
        Value::Object(object) => Ok(object),
        other => Err(unexpected(Kind::Object, &other, context)),
    }
}

fn change_set<R: SurrealValue>(value: Value) -> Result<ChangeSet<R>> {
    let mut set = object(value, "a change set is an object")?;
    let versionstamp = decode(set.remove("versionstamp").unwrap_or(Value::None))?;
    let Some(Value::Array(changes)) = set.remove("changes") else {
        return Err(unexpected(
            any_array(),
            &Value::None,
            "a change set has an array of `changes`",
        ));
    };
    Ok(ChangeSet {
        versionstamp,
        changes: changes
            .into_iter()
            .map(table_change)
            .collect::<Result<_>>()?,
    })
}

fn table_change<R: SurrealValue>(value: Value) -> Result<TableChange<R>> {
    let mut change = object(value, "a change is an object")?;
    if let Some(update) = change.remove("update") {
        return match change.remove("current") {
            Some(current) => Ok(TableChange::Updated {
                current: decode(current)?,
                revert: decode(update)?,
            }),
            None => Ok(TableChange::Update(decode(update)?)),
        };
    }
    if let Some(delete) = change.remove("delete") {
        let mut delete = object(delete, "a delete is an object")?;
        return Ok(TableChange::Delete {
            id: decode(delete.remove("id").unwrap_or(Value::None))?,
            original: delete.remove("original").map(decode).transpose()?,
        });
    }
    match change.remove("define_table") {
        Some(Value::Object(table)) => Ok(TableChange::DefineTable(table)),
        _ => Err(unexpected(
            Kind::Object,
            &Value::Object(change),
            "a change is an `update`, a `delete` or a `define_table`",
        )),
    }
}
