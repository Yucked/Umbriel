//! Table families: a model stored in its default table and, with `table_prefix`, in tables chosen
//! at run time. [`TableSpec`] describes the family statically; [`TableOf`] is one checked member.

use std::{fmt, marker::PhantomData};

use surrealdb::{
    Connection, Surreal,
    types::{RecordIdKeyRange, SurrealValue, Table},
};

use crate::{
    __private::ModelField,
    All, Conditional, Error, Field, Filter, Model, RecordId, RecordKey, RecordRange, Result,
    Writable, count,
    execution::classify,
    statements::{
        DeleteStatement, SelectCount, SelectStatement, delete::delete, select, select_value,
    },
};

/// The tables a model may be stored in, from its `TABLE` and `TABLE_PREFIX`: the default table,
/// and with a prefix every longer name that starts with it. Table names are escaped when rendered,
/// so a member may hold any characters.
///
/// Membership is a typing contract, not an authorization boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSpec {
    table: &'static str,
    prefix: Option<&'static str>,
}

impl TableSpec {
    /// The family of `T`.
    pub const fn of<T: Model>() -> Self {
        Self {
            table: T::TABLE,
            prefix: T::TABLE_PREFIX,
        }
    }

    /// The default table.
    pub const fn table(self) -> &'static str {
        self.table
    }

    /// The prefix of the tables chosen at run time, if any.
    pub const fn prefix(self) -> Option<&'static str> {
        self.prefix
    }

    /// Whether `name` is a member.
    pub const fn accepts(self, name: &str) -> bool {
        if str_eq(name, self.table) {
            return true;
        }
        match self.prefix {
            Some(prefix) => under_prefix(name, prefix),
            None => false,
        }
    }

    /// Whether the two families share a table: equal default tables, one's default table under the
    /// other's prefix, or one prefix starting with the other, since both then hold every longer
    /// name.
    pub const fn overlaps(self, other: Self) -> bool {
        if self.accepts(other.table) || other.accepts(self.table) {
            return true;
        }
        match (self.prefix, other.prefix) {
            (Some(a), Some(b)) => starts_with(a, b) || starts_with(b, a),
            _ => false,
        }
    }
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut index = 0;
    while index < a.len() {
        if a[index] != b[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn starts_with(text: &str, prefix: &str) -> bool {
    let (text, prefix) = (text.as_bytes(), prefix.as_bytes());
    if text.len() < prefix.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if text[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether `name` is `prefix` followed by at least one more character.
const fn under_prefix(name: &str, prefix: &str) -> bool {
    starts_with(name, prefix) && name.len() > prefix.len()
}

/// One table of `T`'s family, checked when it is chosen. `TableOf::default()` is the default
/// table; [`Model::table_of`] checks a name chosen at run time.
///
/// The table-based record operations live here, so they work on any member; the
/// [`SurrealRead`](crate::SurrealRead) and [`SurrealCrud`](crate::SurrealCrud) versions use the default
/// table.
///
/// # Examples
///
/// ```
/// use umbriel::*;
/// # #[model(node, table = "log", table_prefix = "log_")]
/// # pub struct Log { pub id: RecordId<Self>, pub message: String }
///
/// let october = Log::table_of("log_2026_10").expect("a member");
/// assert_eq!(october.id(1).to_string(), "log_2026_10:1");
/// assert!(Log::table_of("log_").is_err());
/// assert!(Log::table_of("audit").is_err());
/// ```
pub struct TableOf<T> {
    table: Table,
    model: PhantomData<fn() -> T>,
}

impl<T: Model> TableOf<T> {
    /// The member named `name`.
    ///
    /// # Errors
    ///
    /// [`Error::TableNotInFamily`] when `name` is not a member.
    pub fn new(name: &str) -> Result<Self> {
        if !TableSpec::of::<T>().accepts(name) {
            return Err(Error::TableNotInFamily {
                table: name.to_owned(),
                model: T::TABLE,
            });
        }
        Ok(Self::unchecked(Table::new(name)))
    }

    /// A member known to be valid: the default table or a decoded record's table.
    pub(crate) fn unchecked(table: Table) -> Self {
        Self {
            table,
            model: PhantomData,
        }
    }

    /// The table.
    pub fn table(&self) -> &Table {
        &self.table
    }

    /// The id with `key` in this table.
    pub fn id(&self, key: impl RecordKey) -> RecordId<T> {
        RecordId::in_table(self.table.clone(), key.into_key())
    }

    /// The records of this table whose keys fall in `keys`.
    pub fn range(&self, keys: impl Into<RecordIdKeyRange>) -> RecordRange<T> {
        RecordRange::in_table(self.table.clone(), keys.into())
    }

    /// Inserts several records into this table in one SDK call, returning the stored records.
    ///
    /// # Errors
    ///
    /// [`Error::QueryBuilder`] before anything is sent when a record's id names another table,
    /// and [`Error::RecordExists`] when an id already exists. Views are read-only, so it is not
    /// offered for them.
    pub async fn insert_many(
        &self,
        db: &Surreal<impl Connection>,
        records: Vec<T>,
    ) -> Result<Vec<T>>
    where
        T: Writable,
    {
        if let Some(record) = records
            .iter()
            .find(|record| record.record_id().table != self.table)
        {
            return Err(Error::QueryBuilder(format!(
                "record `{}` is not in the table `{}` the records are inserted into",
                record.record_id(),
                self.table.as_str()
            )));
        }
        db.insert::<Vec<T>>(self.table.as_str())
            .content(records)
            .await
            .map_err(|error| classify(error, None))
    }

    /// Fetches the records of this table whose keys fall in `keys`, in one SDK call.
    ///
    /// # Errors
    ///
    /// The database error, classified.
    pub async fn get_range(
        &self,
        db: &Surreal<impl Connection>,
        keys: impl Into<RecordIdKeyRange>,
    ) -> Result<Vec<T>> {
        select_range(db, &self.table, keys.into()).await
    }

    /// `SELECT * FROM table`: every record of this table, decoded as `T`.
    pub fn select(&self) -> SelectStatement<T> {
        select(All).from(self.table.clone()).decode()
    }

    /// `SELECT VALUE field FROM table`: one value per record, of the field's own type `V`. The
    /// field must be one of `T`'s generated schema fields.
    pub fn select_value<F, V>(&self, field: F) -> SelectStatement<V>
    where
        F: ModelField<T, V>,
        V: SurrealValue,
    {
        select_value((*field).clone())
            .from(self.table.clone())
            .decode()
    }

    /// Finds the records of this table matching `filter`.
    pub fn find_where(&self, filter: impl Conditional + Clone) -> SelectStatement<T> {
        self.select().where_(filter)
    }

    /// Counts the records of this table matching `filter`.
    pub fn count_where(&self, filter: impl Conditional + Clone) -> SelectCount {
        SelectCount::new(
            select_value(Field::new("count")).from(
                select(count!(Filter::new(filter)))
                    .from(self.table.clone())
                    .group_all(),
            ),
        )
    }

    /// Counts every record of this table.
    pub fn count_all(&self) -> SelectCount {
        SelectCount::new(
            select_value(Field::new("count"))
                .from(select(count!()).from(self.table.clone()).group_all()),
        )
    }

    /// Deletes the records of this table matching `filter`. Views are read-only, so it is not
    /// offered for them.
    pub fn delete_where(&self, filter: impl Conditional + Clone) -> DeleteStatement<T>
    where
        T: Writable,
    {
        delete::<T>(self.clone()).where_(filter)
    }
}

/// Fetches the records of `table` whose keys fall in `keys`, through the SDK's range resource.
pub(crate) async fn select_range<T: Model>(
    db: &Surreal<impl Connection>,
    table: &Table,
    keys: RecordIdKeyRange,
) -> Result<Vec<T>> {
    db.select::<Vec<T>>(table.as_str())
        .range(keys)
        .await
        .map_err(|error| classify(error, None))
}

/// The default table.
impl<T: Model> Default for TableOf<T> {
    fn default() -> Self {
        Self::unchecked(T::table())
    }
}

impl<T> Clone for TableOf<T> {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
            model: PhantomData,
        }
    }
}

impl<T> fmt::Debug for TableOf<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TableOf").field(&self.table).finish()
    }
}

impl<T> PartialEq for TableOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table
    }
}

impl<T> Eq for TableOf<T> {}

impl<T> From<TableOf<T>> for Table {
    fn from(table: TableOf<T>) -> Self {
        table.table
    }
}
