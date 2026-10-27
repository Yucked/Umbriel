use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Bound, Deref},
};

use surrealdb::types::{
    self, ConversionError, Kind, RecordIdKey, RecordIdKeyRange, SurrealValue, Table, ToSql, Value,
};

use crate::{Error, Model, Render, Renderer, Result, TableOf, TableSpec};

mod sealed {
    pub trait Sealed {}
}

/// A key identifying a single record: a string, integer, UUID, array or object. Ranges are not
/// keys; use [`RecordRange`] for them.
pub trait RecordKey: sealed::Sealed {
    /// Converts the input into an SDK record key.
    fn into_key(self) -> RecordIdKey;
}

macro_rules! record_keys {
    (integers: $($int:ty),*; others: $($other:ty),* $(,)?) => {
        $(
            impl sealed::Sealed for $int {}
            impl RecordKey for $int {
                fn into_key(self) -> RecordIdKey {
                    RecordIdKey::Number(i64::from(self))
                }
            }
        )*
        $(
            impl sealed::Sealed for $other {}
            impl RecordKey for $other {
                fn into_key(self) -> RecordIdKey {
                    RecordIdKey::from(self)
                }
            }
        )*
    };
}

record_keys!(
    integers: i8, i16, i32, i64, u8, u16, u32;
    others: String, &str, types::Uuid, types::Array, types::Object,
);

impl sealed::Sealed for uuid::Uuid {}
impl RecordKey for uuid::Uuid {
    fn into_key(self) -> RecordIdKey {
        RecordIdKey::Uuid(self.into())
    }
}

impl<T: SurrealValue> sealed::Sealed for Vec<T> {}
impl<T: SurrealValue> RecordKey for Vec<T> {
    fn into_key(self) -> RecordIdKey {
        RecordIdKey::Array(self.into())
    }
}

/// The id of a single record in one of `T`'s tables: its default table or, for a model with a
/// `table_prefix`, a member chosen at run time ([`TableOf::id`]). The id keeps its table.
///
/// Converting or decoding an id of a table outside the family, or a record range, fails.
pub struct RecordId<T> {
    id: types::RecordId,
    model: PhantomData<fn() -> T>,
}

impl<T: Model> RecordId<T> {
    /// The id with the given key in `T`'s table.
    pub fn new(key: impl RecordKey) -> Self {
        Self::from_key(key.into_key())
    }

    /// A new random id in SurrealDB's default format: 20 characters of `[a-z0-9]`, generated
    /// locally.
    pub fn rand() -> Self {
        Self::from_key(RecordIdKey::rand())
    }

    /// A new ULID id, generated locally.
    pub fn ulid() -> Self {
        Self::from_key(RecordIdKey::ulid())
    }

    /// A new UUID v7 id, generated locally.
    pub fn uuid() -> Self {
        Self::from_key(RecordIdKey::uuid())
    }

    fn from_key(key: RecordIdKey) -> Self {
        Self::in_table(T::table(), key)
    }

    /// The id with `key` in `table`, which the caller has checked is a member.
    pub(crate) fn in_table(table: Table, key: RecordIdKey) -> Self {
        Self {
            id: types::RecordId::new(table, key),
            model: PhantomData,
        }
    }

    /// The key within the table.
    pub fn key(&self) -> &RecordIdKey {
        &self.id.key
    }

    /// The member of `T`'s family the record is in.
    pub fn table_of(&self) -> TableOf<T> {
        TableOf::unchecked(self.id.table.clone())
    }

    /// The untyped SDK record id.
    pub fn into_inner(self) -> types::RecordId {
        self.id
    }
}

/// Checks that `id` is a single record of a table in `T`'s family. Shared by conversion and
/// decoding.
fn check<T: Model>(id: &types::RecordId) -> Result<(), String> {
    if id.key.is_range() {
        return Err(format!(
            "`{}` is a record range, not a single record id",
            id.to_sql()
        ));
    }
    if !TableSpec::of::<T>().accepts(id.table.as_str()) {
        return Err(format!(
            "record `{}` does not belong to a table of `{}`",
            id.to_sql(),
            T::TABLE
        ));
    }
    Ok(())
}

impl<T: Model> TryFrom<types::RecordId> for RecordId<T> {
    type Error = types::Error;

    fn try_from(id: types::RecordId) -> Result<Self, Self::Error> {
        match check::<T>(&id) {
            Ok(()) => Ok(Self {
                id,
                model: PhantomData,
            }),
            Err(context) => Err(
                ConversionError::from_value(Self::kind_of(), &Value::RecordId(id))
                    .with_context(context)
                    .into(),
            ),
        }
    }
}

impl<T: Model> SurrealValue for RecordId<T> {
    /// `record<table>` for a model with one table. A family's prefix cannot be written as a
    /// SurrealQL kind, so its ids are `record`, any table, and Umbriel checks membership when
    /// it decodes.
    fn kind_of() -> Kind {
        match T::TABLE_PREFIX {
            None => Kind::Record(vec![T::table()]),
            Some(_) => Kind::Record(vec![]),
        }
    }

    fn is_value(value: &Value) -> bool {
        matches!(value, Value::RecordId(id) if check::<T>(id).is_ok())
    }

    fn into_value(self) -> Value {
        Value::RecordId(self.id)
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        match value {
            Value::RecordId(id) => Self::try_from(id),
            other => Err(ConversionError::from_value(Self::kind_of(), &other).into()),
        }
    }
}

impl<T> From<RecordId<T>> for types::RecordId {
    fn from(value: RecordId<T>) -> Self {
        value.id
    }
}

impl<T> From<&RecordId<T>> for types::RecordId {
    fn from(value: &RecordId<T>) -> Self {
        value.id.clone()
    }
}

impl<T> Deref for RecordId<T> {
    type Target = types::RecordId;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

/// A random id, as SurrealDB would generate.
impl<T: Model> Default for RecordId<T> {
    fn default() -> Self {
        Self::rand()
    }
}

impl<T> Clone for RecordId<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            model: PhantomData,
        }
    }
}

impl<T> fmt::Debug for RecordId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RecordId").field(&self.id).finish()
    }
}

/// `table:key`, in SurrealQL syntax.
impl<T> fmt::Display for RecordId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id.to_sql())
    }
}

impl<T> PartialEq for RecordId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for RecordId<T> {}

impl<T> PartialOrd for RecordId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for RecordId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<T> Hash for RecordId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// A range of records in one of `T`'s tables, e.g. `user:1..5`: the default table, the table of
/// the ids that bound it, or a member given by [`TableOf::range`].
pub struct RecordRange<T> {
    table: Table,
    range: RecordIdKeyRange,
    /// Set when the bounds name different tables; the range then fails when it is used.
    error: Option<String>,
    model: PhantomData<fn() -> T>,
}

impl<T: Model> RecordRange<T> {
    /// The records of `T`'s default table whose keys fall in `range`, e.g.
    /// `RecordRange::<User>::new(1..5)`.
    pub fn new(range: impl Into<RecordIdKeyRange>) -> Self {
        Self::in_table(T::table(), range.into())
    }

    /// The records of `table`, a checked member, whose keys fall in `range`.
    pub(crate) fn in_table(table: Table, range: RecordIdKeyRange) -> Self {
        Self {
            table,
            range,
            error: None,
            model: PhantomData,
        }
    }

    /// A range bounded by ids: their table, which both bounds must share.
    fn from_ids(start: Bound<&RecordId<T>>, end: Bound<&RecordId<T>>) -> Self {
        let table_of = |bound: &Bound<&RecordId<T>>| match bound {
            Bound::Included(id) | Bound::Excluded(id) => Some(id.id.table.clone()),
            Bound::Unbounded => None,
        };
        let (start_table, end_table) = (table_of(&start), table_of(&end));
        let error = match (&start_table, &end_table) {
            (Some(start), Some(end)) if start != end => Some(format!(
                "the bounds of a record range are in different tables, `{}` and `{}`",
                start.as_str(),
                end.as_str()
            )),
            _ => None,
        };
        let key = |bound: Bound<&RecordId<T>>| bound.map(|id| id.id.key.clone());
        Self {
            table: start_table.or(end_table).unwrap_or_else(T::table),
            range: RecordIdKeyRange {
                start: key(start),
                end: key(end),
            },
            error,
            model: PhantomData,
        }
    }

    /// The table and the key range.
    ///
    /// # Errors
    ///
    /// [`Error::QueryBuilder`] when the bounds name different tables.
    pub(crate) fn into_parts(self) -> Result<(Table, RecordIdKeyRange)> {
        match self.error {
            Some(error) => Err(Error::QueryBuilder(error)),
            None => Ok((self.table, self.range)),
        }
    }
}

impl<T: Model> Render for RecordRange<T> {
    fn render(&self, out: &mut Renderer) {
        if let Some(error) = &self.error {
            out.push_error(error.as_str());
        }
        let is_range = |bound: &Bound<RecordIdKey>| matches!(bound, Bound::Included(key) | Bound::Excluded(key) if key.is_range());
        if is_range(&self.range.start) || is_range(&self.range.end) {
            out.push_error("record range bounds must be single keys");
        }
        out.push_value(&Value::RecordId(types::RecordId::new(
            self.table.clone(),
            RecordIdKey::Range(Box::new(self.range.clone())),
        )));
    }
}

impl<T> Clone for RecordRange<T> {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
            range: self.range.clone(),
            error: self.error.clone(),
            model: PhantomData,
        }
    }
}

impl<T> fmt::Debug for RecordRange<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RecordRange")
            .field(&self.table)
            .field(&self.range)
            .finish()
    }
}

fn id_of<T>(id: &impl Borrow<RecordId<T>>) -> &RecordId<T> {
    id.borrow()
}

macro_rules! record_range_from_ids {
    ($($id:ty),*) => {
        $(
            impl<T: Model> From<std::ops::Range<$id>> for RecordRange<T> {
                fn from(range: std::ops::Range<$id>) -> Self {
                    Self::from_ids(
                        Bound::Included(id_of::<T>(&range.start)),
                        Bound::Excluded(id_of::<T>(&range.end)),
                    )
                }
            }

            impl<T: Model> From<std::ops::RangeInclusive<$id>> for RecordRange<T> {
                fn from(range: std::ops::RangeInclusive<$id>) -> Self {
                    Self::from_ids(
                        Bound::Included(id_of::<T>(range.start())),
                        Bound::Included(id_of::<T>(range.end())),
                    )
                }
            }

            impl<T: Model> From<std::ops::RangeFrom<$id>> for RecordRange<T> {
                fn from(range: std::ops::RangeFrom<$id>) -> Self {
                    Self::from_ids(Bound::Included(id_of::<T>(&range.start)), Bound::Unbounded)
                }
            }

            impl<T: Model> From<std::ops::RangeTo<$id>> for RecordRange<T> {
                fn from(range: std::ops::RangeTo<$id>) -> Self {
                    Self::from_ids(Bound::Unbounded, Bound::Excluded(id_of::<T>(&range.end)))
                }
            }

            impl<T: Model> From<std::ops::RangeToInclusive<$id>> for RecordRange<T> {
                fn from(range: std::ops::RangeToInclusive<$id>) -> Self {
                    Self::from_ids(Bound::Unbounded, Bound::Included(id_of::<T>(&range.end)))
                }
            }
        )*
    };
}

record_range_from_ids!(RecordId<T>, &RecordId<T>);
