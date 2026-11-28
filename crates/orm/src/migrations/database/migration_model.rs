use std::{collections::BTreeMap, fmt::Display};

use crate::{
    All, Field, Raw, Render, Table,
    statements::{
        DefineTableStatement, RemoveTableStatement, Returning, begin_transaction, define_field,
        define_table, remove_table, select,
    },
};
use surrealdb::{
    Connection, Surreal,
    types::{Kind, RecordId, SurrealValue, ToSql},
};

use crate::migrations::*;

/// A read of the migration history. The history table is defined by the first migration, and
/// the revision table by the first runner transaction, so before them SurrealDB reports the
/// table as missing: that reads as no history. Every other error, including a different missing
/// table, propagates.
#[doc(hidden)]
pub fn history_or_empty<T: Default>(result: crate::Result<T>) -> MigrationResult<T> {
    Ok(history_if_defined(result)?.unwrap_or_default())
}

/// `result`, or `None` when it failed because a history table is not defined yet.
fn history_if_defined<T>(result: crate::Result<T>) -> MigrationResult<Option<T>> {
    match result {
        Err(crate::Error::TableNotFound { table, .. })
            if table == Migration::table().as_str()
                || table == Migration::revision_table().as_str() =>
        {
            Ok(None)
        }
        result => Ok(Some(result?)),
    }
}

/// The history's revision record. Snapshot isolation lets two overlapping transactions that
/// write different history records both commit, so every runner transaction that changes the
/// history also writes this one record: overlapping ones conflict at commit, and the later fails.
#[derive(SurrealValue, Clone, Debug)]
struct Revision {
    id: RecordId,
    /// How many runner transactions have changed the history.
    revision: i64,
    /// Who holds the lock, while a prune deletes files.
    lock: Option<String>,
}

/// One reading of the history: the applied migrations, and the revision and lock they were read
/// at. Every runner transaction checks that the revision is still this one.
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct HistoryReading {
    /// Whether the history table is defined.
    pub(crate) exists: bool,
    /// The applied migrations by the up (or only) file name, so in file order.
    pub(crate) applied: BTreeMap<MigrationFilename, Migration>,
    /// The revision read.
    pub(crate) revision: i64,
    /// Who held the lock.
    pub(crate) lock: Option<String>,
}

impl HistoryReading {
    /// Reads the revision, then the history. A runner that commits in between leaves this
    /// reading's revision behind the history it read, so the reading is refused, never trusted.
    ///
    /// # Errors
    ///
    /// The query's error, or [`MigrationError::InvalidMigrationName`] for a recorded name that is
    /// not a migration file name.
    pub async fn read(db: &Surreal<impl Connection>) -> MigrationResult<Self> {
        let revision = history_or_empty(
            select(All)
                .from(Migration::revision_table())
                .decode::<Revision>()
                .execute(db)
                .await,
        )?
        .into_iter()
        .next();
        let history = history_if_defined(
            select(All)
                .from(Migration::table())
                .decode::<Migration>()
                .execute(db)
                .await,
        )?;
        let exists = history.is_some();
        let applied = history
            .unwrap_or_default()
            .into_iter()
            .map(|migration| {
                let name = MigrationFilename::try_from(migration.name.clone())?;
                Ok((name.to_up(), migration))
            })
            .collect::<MigrationResult<_>>()?;
        Ok(Self {
            exists,
            applied,
            revision: revision.as_ref().map_or(0, |revision| revision.revision),
            lock: revision.and_then(|revision| revision.lock),
        })
    }

    /// Fails while a prune holds the lock, before a runner plans any change.
    ///
    /// # Errors
    ///
    /// [`MigrationError::HistoryLocked`] with the holder.
    pub(crate) fn check_unlocked(&self) -> MigrationResult<()> {
        match &self.lock {
            Some(holder) => Err(MigrationError::HistoryLocked {
                holder: holder.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Whether the migration named `name` is applied.
    pub(crate) fn is_applied(&self, name: &MigrationFilename) -> bool {
        self.applied.contains_key(&name.to_up())
    }

    /// The newest applied migration's up (or only) file name.
    pub(crate) fn newest(&self) -> Option<&MigrationFilename> {
        self.applied.keys().next_back()
    }

    /// The recorded names of the applied migrations, in file order.
    #[cfg(any(feature = "cli", feature = "test-support"))]
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.applied
            .values()
            .map(|migration| migration.name.as_str())
    }
}

/// An applied migration, as recorded in the history table. Migrations order by timestamp.
#[derive(SurrealValue, Clone, Debug)]
pub struct Migration {
    /// The record id, keyed by the file name.
    pub id: RecordId,
    /// The migration's file name.
    pub name: String,
    /// The timestamp the file name starts with.
    pub timestamp: Timestamp,
    /// The checksum of the up (or only) file when it was applied.
    pub checksum_up: Checksum,
    /// The checksum of the down file when it was applied; `None` for one-way migrations.
    pub checksum_down: Option<Checksum>,
}

impl Ord for Migration {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl PartialOrd for Migration {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Migration {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}

impl Eq for Migration {}

/// The timestamp a migration file name starts with, read as a number.
#[derive(SurrealValue, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Timestamp(timestamp) = self;
        write!(f, "{timestamp}")
    }
}

impl From<Timestamp> for u64 {
    fn from(timestamp: Timestamp) -> Self {
        timestamp.0
    }
}

impl Timestamp {
    /// The timestamp as a number.
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<u64> for Timestamp {
    fn from(timestamp: u64) -> Self {
        Self(timestamp)
    }
}

/// The history table's fields, for building queries against it.
pub struct MigrationSchema {
    /// The record id.
    pub id: Field,
    /// The file name.
    pub name: Field,
    /// The timestamp.
    pub timestamp: Field,
    /// The up (or only) file's checksum.
    pub checksum_up: Field,
    /// The down file's checksum, defined only for two-way migrations.
    pub checksum_down: Field,
}

impl Migration {
    /// The history record id for a migration file.
    pub fn create_id(filename: &MigrationFilename) -> RecordId {
        RecordId::new(Migration::table(), filename.to_string())
    }

    /// A transaction that recreates the history table with only this migration recorded.
    pub fn create_reinitialize_table_raw_tx(
        filename: &MigrationFilename,
        checksum_up: &Checksum,
        checksum_down: Option<&Checksum>,
        migration_type: MigrationFlag,
    ) -> Raw {
        let mut tx = begin_transaction()
            .query(Self::remove_table())
            .query(Self::define_table());

        for def in Self::define_fields(migration_type) {
            tx = tx.query(def);
        }

        let tx = tx
            .query(Self::create_raw(filename, checksum_up, checksum_down))
            .commit_transaction();
        Raw::inline(&tx)
    }

    /// The statement recording a migration as applied.
    pub fn create_raw(
        filename: &MigrationFilename,
        checksum_up: &Checksum,
        checksum_down: Option<&Checksum>,
    ) -> Raw {
        let MigrationSchema {
            id: _id_field,
            name: name_field,
            timestamp: timestamp_field,
            checksum_up: checksum_up_field,
            checksum_down: checksum_down_field,
        } = Self::schema();

        let record_id = Self::create_id(filename).to_sql();
        let name = string_sql(&filename.to_string());
        let timestamp = filename.timestamp().into_inner().to_sql();
        let checksum_up = string_sql(&checksum_up.to_string());
        let name_field = name_field.to_debug_sql();
        let timestamp_field = timestamp_field.to_debug_sql();
        let checksum_up_field = checksum_up_field.to_debug_sql();

        let mut create = format!(
            "CREATE {record_id} SET {name_field} = {name}, {timestamp_field} = {timestamp}, \
        {checksum_up_field} = {checksum_up}"
        );
        // One-way history has no `checksum_down` field, and a schemafull table rejects
        // undefined fields.
        if let Some(checksum_down) = checksum_down {
            create.push_str(&format!(
                ", {} = {}",
                checksum_down_field.to_debug_sql(),
                string_sql(&checksum_down.to_string())
            ));
        }
        create.push(';');
        Raw::new(create)
    }

    /// The message [`expect_revision_raw`](Self::expect_revision_raw) throws for a stale reading.
    pub(crate) const HISTORY_CHANGED: &str = "umbriel: the migration history changed";

    /// The message [`expect_revision_raw`](Self::expect_revision_raw) throws for a lock held by
    /// another, followed by its holder.
    pub(crate) const HISTORY_LOCKED: &str = "umbriel: the migration history is locked by ";

    /// The message [`expect_size_raw`](Self::expect_size_raw) throws.
    pub(crate) const HISTORY_REWRITTEN: &str = "umbriel: a migration changed the history";

    /// The statements opening a runner transaction: they throw
    /// [`HISTORY_CHANGED`](Self::HISTORY_CHANGED) unless the revision is still `expected`, and
    /// [`HISTORY_LOCKED`](Self::HISTORY_LOCKED) while the lock is held by someone other than
    /// `holder`. With [`next_revision_raw`](Self::next_revision_raw) closing the transaction,
    /// they make a runner whose reading is stale, or that overlaps another, change nothing.
    pub(crate) fn expect_revision_raw(expected: i64, holder: Option<&str>) -> Raw {
        let table = Self::revision_table().to_debug_sql();
        let holder = holder.map_or_else(|| "NONE".to_owned(), string_sql);
        // Reading an undefined table fails, so its existence is checked first.
        Raw::new(format!(
            "LET $umbriel_revision = IF (INFO FOR DB).tables.{table} = NONE {{ NONE }} \
             ELSE {{ (SELECT revision, lock FROM {table}:current)[0] }};\n\
             IF ($umbriel_revision.revision ?? 0) != {expected} {{ THROW '{changed}'; }};\n\
             IF $umbriel_revision.lock != NONE AND $umbriel_revision.lock != {holder} \
             {{ THROW '{locked}' + $umbriel_revision.lock; }};",
            changed = Self::HISTORY_CHANGED,
            locked = Self::HISTORY_LOCKED,
        ))
    }

    /// The statement closing a runner transaction: it writes `revision`, the one after the
    /// reading's, and takes the lock for `holder` when given; otherwise the lock stays as it was.
    pub(crate) fn next_revision_raw(revision: i64, holder: Option<&str>) -> Raw {
        let table = Self::revision_table().to_debug_sql();
        Raw::new(match holder {
            None => format!("UPSERT {table}:current SET revision = {revision};"),
            Some(holder) => format!(
                "UPSERT {table}:current SET revision = {revision}, lock = {};",
                string_sql(holder)
            ),
        })
    }

    /// The statement releasing the lock if `holder` holds it. Releasing changes no history, so
    /// the revision stays.
    #[cfg(any(feature = "cli", feature = "test-support"))]
    pub(crate) fn release_lock_raw(holder: &str) -> Raw {
        let table = Self::revision_table().to_debug_sql();
        Raw::new(format!(
            "UPDATE {table}:current SET lock = NONE WHERE lock = {};",
            string_sql(holder)
        ))
    }

    /// The statement that throws [`HISTORY_REWRITTEN`](Self::HISTORY_REWRITTEN) unless the
    /// history holds `size` records. Run after a migration and its record, it refuses a
    /// migration that changed other records, as a reset's migration clears the history.
    pub(crate) fn expect_size_raw(size: usize) -> Raw {
        let table = Self::table().to_debug_sql();
        Raw::new(format!(
            "IF count(SELECT id FROM {table}) != {size} {{ THROW '{}'; }};",
            Self::HISTORY_REWRITTEN
        ))
    }

    /// The statement removing a migration from the history.
    pub fn delete_raw(migration_id: &RecordId) -> Raw {
        Raw::new(format!("DELETE {};", migration_id.to_sql()))
    }

    /// The history table's fields.
    pub fn schema() -> MigrationSchema {
        MigrationSchema {
            id: "id".into(),
            name: Field::new("name"),
            timestamp: Field::new("timestamp"),
            checksum_up: Field::new("checksum_up"),
            checksum_down: Field::new("checksum_down"),
        }
    }

    /// The history table.
    pub fn table() -> Table {
        Table::new("migration")
    }

    /// The table of the history's revision record, which the runner defines and maintains.
    pub fn revision_table() -> Table {
        Table::new("migration_revision")
    }

    /// The history table's definition.
    pub fn define_table() -> DefineTableStatement {
        define_table(Migration::table()).schemafull()
    }

    /// The statement removing the history table.
    pub fn remove_table() -> RemoveTableStatement {
        remove_table(Self::table())
    }

    /// The statement clearing the history.
    pub fn delete_all() -> Raw {
        Raw::new(format!("DELETE {};", Self::table().to_debug_sql()))
    }

    /// The history table's field definitions; `checksum_down` only for two-way migrations.
    pub fn define_fields(migration_type: MigrationFlag) -> Vec<Raw> {
        let MigrationSchema {
            id,
            name,
            timestamp,
            checksum_up,
            checksum_down,
        } = Migration::schema();
        let define = |field: Field, kind: Kind| {
            Raw::inline(&define_field(field).type_(kind).on_table(Self::table()))
        };

        // `id`'s type is the record key's: the file name.
        let id = define(id, Kind::String);
        let name = define(name, Kind::String);
        let timestamp = define(timestamp, Kind::Int);
        let checksum_up = define(checksum_up, Kind::String);

        let mut fields = vec![id, name, timestamp, checksum_up];

        let checksum_down = define(checksum_down, Kind::String);

        if migration_type == MigrationFlag::TwoWay {
            fields.push(checksum_down);
        }

        fields
    }

    /// The newest applied migration by file name, which is the most recently applied: the
    /// runner applies migrations only in file order.
    pub async fn get_latest(db: &Surreal<impl Connection>) -> MigrationResult<Option<Self>> {
        Ok(HistoryReading::read(db)
            .await?
            .applied
            .into_values()
            .next_back())
    }

    /// The applied migration with this file name (up or down), if any.
    pub async fn get_by_filename(
        db: &Surreal<impl Connection>,
        filename: &MigrationFilename,
    ) -> MigrationResult<Option<Self>> {
        Ok(HistoryReading::read(db)
            .await?
            .applied
            .remove(&filename.to_up()))
    }

    /// Every applied migration, the most recent first, in file name order.
    pub async fn get_all_desc(db: &Surreal<impl Connection>) -> MigrationResult<Vec<Self>> {
        Ok(HistoryReading::read(db)
            .await?
            .applied
            .into_values()
            .rev()
            .collect())
    }
}

/// `text` as a SurrealQL string literal. Strings are written as `Value`s, whose SurrealQL
/// formatting quotes and escapes them; a `String`'s own `to_sql` writes its text as is.
fn string_sql(text: &str) -> String {
    text.to_owned().into_value().to_sql()
}
