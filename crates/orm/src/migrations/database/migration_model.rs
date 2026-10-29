use std::fmt::Display;

use crate::{
    All, Field, Operatable, Raw, Render, Table,
    statements::{
        DefineTableStatement, RemoveTableStatement, Returning, begin_transaction, define_field,
        define_table, order, remove_table, select,
    },
};
use surrealdb::{
    Connection, Surreal,
    types::{Kind, RecordId, SurrealValue, ToSql},
};

use crate::migrations::*;

/// A read of the migration history. The history table is defined by the first migration, so
/// before it runs SurrealDB reports the table as missing: that reads as no history. Every other
/// error, including a different missing table, propagates.
#[doc(hidden)]
pub fn history_or_empty<T: Default>(result: crate::Result<T>) -> MigrationResult<T> {
    match result {
        Err(crate::Error::TableNotFound { table, .. }) if table == Migration::table().as_str() => {
            Ok(T::default())
        }
        result => Ok(result?),
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

        // Strings are written as `Value`s, whose SurrealQL formatting quotes and escapes them;
        // a `String`'s own `to_sql` writes its text as is.
        let string = |text: String| text.into_value().to_sql();
        let record_id = Self::create_id(filename).to_sql();
        let name = string(filename.to_string());
        let timestamp = filename.timestamp().into_inner().to_sql();
        let checksum_up = string(checksum_up.to_string());
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
                string(checksum_down.to_string())
            ));
        }
        create.push(';');
        Raw::new(create)
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

        if migration_type.is_twoway() {
            fields.push(checksum_down);
        }

        fields
    }

    /// The most recently applied migration, if any.
    pub async fn get_latest(db: Surreal<impl Connection>) -> MigrationResult<Option<Self>> {
        let MigrationSchema { timestamp, .. } = Self::schema();

        history_or_empty(
            select(All)
                .from(Self::table())
                .order_by(order(timestamp).desc())
                .limit(1)
                .decode::<Self>()
                .execute(&db)
                .await
                .map(|rows| rows.into_iter().next()),
        )
    }

    /// The applied migration with this file name, if any.
    pub async fn get_by_filename(
        db: Surreal<impl Connection>,
        filename: &MigrationFilename,
    ) -> MigrationResult<Option<Self>> {
        let MigrationSchema { name, .. } = Self::schema();

        history_or_empty(
            select(All)
                .from(Self::table())
                .where_(name.equal(filename.to_string()))
                .decode::<Self>()
                .execute(&db)
                .await
                .map(|rows| rows.into_iter().next()),
        )
    }

    /// Every applied migration, the most recent first.
    pub async fn get_all_desc(db: Surreal<impl Connection>) -> MigrationResult<Vec<Self>> {
        let MigrationSchema { timestamp, .. } = Self::schema();

        history_or_empty(
            select(All)
                .from(Self::table())
                .order_by(order(timestamp).desc())
                .decode::<Self>()
                .execute(&db)
                .await,
        )
    }
}
