//! What `INFO` statements return, typed by level and form.
//!
//! The shapes are SurrealDB 3.3.4's (`exec/operators/info/*.rs`):
//!
//! - `ROOT`, `NS`, `DB` and `TABLE` list definitions by kind. The plain form gives each kind as a
//!   map from a name to its `DEFINE` statement; `STRUCTURE` gives each as an array of objects.
//!   `ROOT` also reports `defaults`, `config` and `system`, which are not definitions.
//! - `USER` gives one `DEFINE USER` statement, or one object with `STRUCTURE`.
//! - `INDEX` gives the index's build status, with or without `STRUCTURE`.
//!
//! Each definition's own properties stay values: their shapes differ between the documentation
//! and SurrealDB 3.3.4, and migrations compare them as values. Keys a type does not name are kept
//! in `other`, so a caller can reject kinds it does not handle.

use std::collections::BTreeMap;

use surrealdb::types::{ConversionError, Kind, Object, SurrealValue, Value};

use crate::{Error, Result};

mod sealed {
    pub trait Sealed {}
}

/// The plain form of `INFO`: each kind maps a definition's name to its `DEFINE` statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Statements;

/// The `STRUCTURE` form of `INFO`: each kind lists its definitions as objects of their
/// properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Structures;

/// The form an `INFO` result lists definitions in: [`Statements`] or [`Structures`].
pub trait InfoForm: sealed::Sealed {
    /// How one kind's definitions are listed.
    type Definitions: SurrealValue + std::fmt::Debug + Clone;
}

impl sealed::Sealed for Statements {}
impl sealed::Sealed for Structures {}

impl InfoForm for Statements {
    type Definitions = BTreeMap<String, String>;
}

impl InfoForm for Structures {
    type Definitions = Vec<Object>;
}

/// A result an `INFO` statement decodes into. Implemented for the types in this module and for
/// `String` and `Object`, the forms of `INFO FOR USER`.
pub trait InfoOutput: sealed::Sealed + Sized {
    /// Decodes the statement's result.
    #[doc(hidden)]
    fn from_info(value: Value) -> Result<Self>;
}

/// A conversion error for an `INFO` result of an unexpected shape.
fn unexpected(expected: Kind, value: &Value, context: &str) -> Error {
    Error::Conversion(
        ConversionError::from_value(expected, value)
            .with_context(context.to_owned())
            .into(),
    )
}

/// The object `INFO` returned, or an error naming `level`.
fn object(value: Value, level: &str) -> Result<Object> {
    match value {
        Value::Object(object) => Ok(object),
        other => Err(unexpected(
            Kind::Object,
            &other,
            &format!("`INFO FOR {level}` returns an object"),
        )),
    }
}

/// Removes `key` from `info` and decodes it. A missing key is an error: the shape changed.
fn take<T: SurrealValue>(info: &mut Object, key: &str, level: &str) -> Result<T> {
    let value = info.remove(key).ok_or_else(|| {
        unexpected(
            Kind::Object,
            &Value::None,
            &format!("`INFO FOR {level}` has no `{key}`"),
        )
    })?;
    T::from_value(value).map_err(Error::Conversion)
}

/// The keys left after the named ones were taken.
fn rest(info: Object) -> BTreeMap<String, Value> {
    info.into_iter().collect()
}

/// `INFO FOR ROOT`: the namespaces, root users and root access methods, and the server's
/// defaults, configuration and resource use.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RootInfo<F: InfoForm = Statements> {
    /// Root access methods.
    pub accesses: F::Definitions,
    /// Namespaces.
    pub namespaces: F::Definitions,
    /// Cluster nodes, keyed by id. Not schema.
    pub nodes: F::Definitions,
    /// Root users.
    pub users: F::Definitions,
    /// `DEFINE CONFIG DEFAULT`, when set.
    pub defaults: Option<Object>,
    /// The server's dynamic configuration.
    pub config: Object,
    /// The server's resource use: cores, CPU, memory and load.
    pub system: Object,
    /// Keys this type does not name.
    pub other: BTreeMap<String, Value>,
}

/// `INFO FOR NS`: the namespace's databases, users and access methods.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct NamespaceInfo<F: InfoForm = Statements> {
    /// Namespace access methods.
    pub accesses: F::Definitions,
    /// Databases. Each is a definition of the namespace, not one inside a database.
    pub databases: F::Definitions,
    /// Namespace users.
    pub users: F::Definitions,
    /// Keys this type does not name.
    pub other: BTreeMap<String, Value>,
}

/// `INFO FOR DB`: every definition of the database.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DatabaseInfo<F: InfoForm = Statements> {
    /// Database access methods.
    pub accesses: F::Definitions,
    /// APIs, keyed by path.
    pub apis: F::Definitions,
    /// Analyzers.
    pub analyzers: F::Definitions,
    /// Buckets.
    pub buckets: F::Definitions,
    /// Configurations.
    pub configs: F::Definitions,
    /// Functions, keyed without `fn::`.
    pub functions: F::Definitions,
    /// Machine-learning models.
    pub models: F::Definitions,
    /// Modules.
    pub modules: F::Definitions,
    /// Params.
    pub params: F::Definitions,
    /// Sequences.
    pub sequences: F::Definitions,
    /// Tables.
    pub tables: F::Definitions,
    /// Database users.
    pub users: F::Definitions,
    /// Keys this type does not name.
    pub other: BTreeMap<String, Value>,
}

/// `INFO FOR TABLE`: the table's fields, indexes and events, the views fed by it, and its live
/// queries.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TableInfo<F: InfoForm = Statements> {
    /// Events.
    pub events: F::Definitions,
    /// Fields. The plain form keys them by their unescaped name.
    pub fields: F::Definitions,
    /// Indexes.
    pub indexes: F::Definitions,
    /// Live queries, keyed by id. Not schema.
    pub lives: F::Definitions,
    /// The views fed by this table, which `INFO FOR DB` also lists as tables.
    pub tables: F::Definitions,
    /// Keys this type does not name.
    pub other: BTreeMap<String, Value>,
}

impl<F: InfoForm> sealed::Sealed for RootInfo<F> {}
impl<F: InfoForm> sealed::Sealed for NamespaceInfo<F> {}
impl<F: InfoForm> sealed::Sealed for DatabaseInfo<F> {}
impl<F: InfoForm> sealed::Sealed for TableInfo<F> {}

impl<F: InfoForm> InfoOutput for RootInfo<F> {
    fn from_info(value: Value) -> Result<Self> {
        let mut info = object(value, "ROOT")?;
        let defaults = match info.remove("defaults") {
            None | Some(Value::None | Value::Null) => None,
            Some(Value::Object(defaults)) => Some(defaults),
            Some(other) => {
                return Err(unexpected(
                    Kind::Object,
                    &other,
                    "`INFO FOR ROOT` has `defaults` as an object or NONE",
                ));
            }
        };
        Ok(Self {
            accesses: take(&mut info, "accesses", "ROOT")?,
            namespaces: take(&mut info, "namespaces", "ROOT")?,
            nodes: take(&mut info, "nodes", "ROOT")?,
            users: take(&mut info, "users", "ROOT")?,
            config: take(&mut info, "config", "ROOT")?,
            system: take(&mut info, "system", "ROOT")?,
            defaults,
            other: rest(info),
        })
    }
}

impl<F: InfoForm> InfoOutput for NamespaceInfo<F> {
    fn from_info(value: Value) -> Result<Self> {
        let mut info = object(value, "NS")?;
        Ok(Self {
            accesses: take(&mut info, "accesses", "NS")?,
            databases: take(&mut info, "databases", "NS")?,
            users: take(&mut info, "users", "NS")?,
            other: rest(info),
        })
    }
}

impl<F: InfoForm> InfoOutput for DatabaseInfo<F> {
    fn from_info(value: Value) -> Result<Self> {
        let mut info = object(value, "DB")?;
        Ok(Self {
            accesses: take(&mut info, "accesses", "DB")?,
            apis: take(&mut info, "apis", "DB")?,
            analyzers: take(&mut info, "analyzers", "DB")?,
            buckets: take(&mut info, "buckets", "DB")?,
            configs: take(&mut info, "configs", "DB")?,
            functions: take(&mut info, "functions", "DB")?,
            models: take(&mut info, "models", "DB")?,
            modules: take(&mut info, "modules", "DB")?,
            params: take(&mut info, "params", "DB")?,
            sequences: take(&mut info, "sequences", "DB")?,
            tables: take(&mut info, "tables", "DB")?,
            users: take(&mut info, "users", "DB")?,
            other: rest(info),
        })
    }
}

impl<F: InfoForm> InfoOutput for TableInfo<F> {
    fn from_info(value: Value) -> Result<Self> {
        let mut info = object(value, "TABLE")?;
        Ok(Self {
            events: take(&mut info, "events", "TABLE")?,
            fields: take(&mut info, "fields", "TABLE")?,
            indexes: take(&mut info, "indexes", "TABLE")?,
            lives: take(&mut info, "lives", "TABLE")?,
            tables: take(&mut info, "tables", "TABLE")?,
            other: rest(info),
        })
    }
}

impl sealed::Sealed for String {}
impl sealed::Sealed for Object {}

/// `INFO FOR USER`: the user's `DEFINE USER` statement, with its secrets redacted.
impl InfoOutput for String {
    fn from_info(value: Value) -> Result<Self> {
        String::from_value(value).map_err(Error::Conversion)
    }
}

/// `INFO FOR USER … STRUCTURE`: the user's properties, with its secrets redacted.
impl InfoOutput for Object {
    fn from_info(value: Value) -> Result<Self> {
        object(value, "USER")
    }
}

/// Where an index build is, as `INFO FOR INDEX` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexBuildStatus {
    /// `started`.
    Started,
    /// `cleaning`.
    Cleaning,
    /// `indexing`.
    Indexing,
    /// `ready`: the index serves queries.
    Ready,
    /// `aborted`.
    Aborted,
    /// `error`: the build failed; [`IndexStatus::error`] says why.
    Error,
}

impl IndexBuildStatus {
    fn parse(status: &str) -> Option<Self> {
        Some(match status {
            "started" => Self::Started,
            "cleaning" => Self::Cleaning,
            "indexing" => Self::Indexing,
            "ready" => Self::Ready,
            "aborted" => Self::Aborted,
            "error" => Self::Error,
            _ => return None,
        })
    }
}

/// `INFO FOR INDEX`: the index's build status, under `building`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IndexStatus {
    /// Where the build is.
    pub status: IndexBuildStatus,
    /// Whether updates written during the build are still being folded in.
    pub compacting: bool,
    /// Records indexed by the initial build, while it runs.
    pub initial: Option<u64>,
    /// Updates waiting to be indexed, while the build runs.
    pub pending: Option<u64>,
    /// Updates indexed since the build started.
    pub updated: Option<u64>,
    /// Why the build failed, with [`IndexBuildStatus::Error`].
    pub error: Option<String>,
}

impl sealed::Sealed for IndexStatus {}

impl InfoOutput for IndexStatus {
    fn from_info(value: Value) -> Result<Self> {
        let mut info = object(value, "INDEX")?;
        let mut building: Object = take(&mut info, "building", "INDEX")?;
        let status: String = take(&mut building, "status", "INDEX")?;
        let status = IndexBuildStatus::parse(&status).ok_or_else(|| {
            unexpected(
                Kind::String,
                &Value::String(status.clone()),
                "`INFO FOR INDEX` has a known build `status`",
            )
        })?;
        let optional = |building: &mut Object, key: &str| -> Result<Option<u64>> {
            match building.remove(key) {
                None => Ok(None),
                Some(value) => u64::from_value(value).map(Some).map_err(Error::Conversion),
            }
        };
        Ok(Self {
            status,
            compacting: take(&mut building, "compacting", "INDEX")?,
            initial: optional(&mut building, "initial")?,
            pending: optional(&mut building, "pending")?,
            updated: optional(&mut building, "updated")?,
            error: match building.remove("error") {
                None => None,
                Some(value) => Some(String::from_value(value).map_err(Error::Conversion)?),
            },
        })
    }
}
