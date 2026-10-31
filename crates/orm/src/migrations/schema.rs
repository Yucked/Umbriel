//! Schema inspection.
//!
//! A database's definitions are read from `INFO FOR … STRUCTURE`, keyed by kind, table and name,
//! and compared by their structure. Both sides of a migration are rendered by the same server, so
//! nothing is parsed or trimmed here. The plain `INFO FOR …` output supplies each definition's
//! `DEFINE` statement, which is used only to write migrations.

use std::{collections::BTreeMap, fmt};

use surrealdb::{
    Connection, IndexedResults, Surreal,
    types::{Object, Table, Value},
};

use crate::{
    Chainable, QueryChain, Runnable,
    migrations::{MigrationError, MigrationResult},
    statements::{
        DatabaseInfo, InfoOutput, NamespaceInfo, Statements, Structures, TableInfo, info_for,
    },
};

/// What SurrealDB writes in place of a secret in `INFO` output: a user's password hash (`hash`)
/// and SCRAM verifier (`scram`), and an access's symmetric verification key
/// (`kind.jwt.verify.key`) and issuer key (`kind.jwt.issuer.key`).
const REDACTED: &str = "[REDACTED]";

/// A kind of definition migrations manage, in creation order: each kind may reference only kinds
/// before it (an analyzer its function, an index its analyzer, a field its table), so creation
/// follows this order and removal reverses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    /// `DEFINE FUNCTION`.
    Function,
    /// `DEFINE ANALYZER`.
    Analyzer,
    /// `DEFINE PARAM`.
    Param,
    /// `DEFINE ACCESS … ON DATABASE`.
    Access,
    /// `DEFINE USER … ON DATABASE`.
    User,
    /// `DEFINE TABLE`.
    Table,
    /// `DEFINE FIELD`.
    Field,
    /// `DEFINE INDEX`.
    Index,
    /// `DEFINE EVENT`.
    Event,
}

impl ResourceKind {
    /// The keyword that follows `DEFINE` and `REMOVE`.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Function => "FUNCTION",
            Self::Analyzer => "ANALYZER",
            Self::Param => "PARAM",
            Self::Access => "ACCESS",
            Self::User => "USER",
            Self::Table => "TABLE",
            Self::Field => "FIELD",
            Self::Index => "INDEX",
            Self::Event => "EVENT",
        }
    }

    /// The key listing this kind in `INFO FOR DB` or `INFO FOR TABLE`.
    fn info_key(self) -> &'static str {
        match self {
            Self::Function => "functions",
            Self::Analyzer => "analyzers",
            Self::Param => "params",
            Self::Access => "accesses",
            Self::User => "users",
            Self::Table => "tables",
            Self::Field => "fields",
            Self::Index => "indexes",
            Self::Event => "events",
        }
    }

    /// Whether definitions of this kind belong to a table.
    pub fn on_table(self) -> bool {
        matches!(self, Self::Field | Self::Index | Self::Event)
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.keyword().to_lowercase())
    }
}

/// Identifies a definition: its kind, its table for fields, indexes and events, and its name as
/// `INFO … STRUCTURE` reports it (a field's name is its canonical idiom, e.g. `address.city`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceKey {
    /// The kind of definition.
    pub kind: ResourceKind,
    /// The table a field, index or event belongs to.
    pub table: Option<String>,
    /// The definition's name.
    pub name: String,
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} `{}`", self.kind, self.name)?;
        if let Some(table) = &self.table {
            write!(f, " on table `{table}`")?;
        }
        Ok(())
    }
}

/// One definition: its normalized structure, which comparisons use, and the server's `DEFINE`
/// statement for it, which migrations are written with.
#[derive(Debug, Clone)]
pub struct Definition {
    structure: Value,
    sql: String,
    redacted: bool,
}

impl Definition {
    /// The definition's properties as `INFO … STRUCTURE` reports them, without values that
    /// differ between databases holding the same schema.
    pub fn structure(&self) -> &Value {
        &self.structure
    }

    /// The `DEFINE` statement the server renders for the definition, without a trailing `;`.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Whether the server hid a secret of the definition. Its `DEFINE` statement then holds the
    /// placeholder instead of the secret, so it must never be written to a migration.
    pub fn is_redacted(&self) -> bool {
        self.redacted
    }
}

/// Definitions are equal when their observable structure is: redacted secrets compare as the
/// same placeholder, so a change to a secret alone is not detected.
impl PartialEq for Definition {
    fn eq(&self, other: &Self) -> bool {
        self.structure == other.structure
    }
}

/// Whether the server hid a secret of `entry`. Only the properties SurrealDB redacts are read, so
/// the placeholder text elsewhere, such as in a comment or a value, is not mistaken for a secret.
fn is_redacted(kind: ResourceKind, entry: &Object) -> bool {
    let at = |parents: &[&str], key: &str| {
        parents
            .iter()
            .try_fold(entry, |object, parent| match object.get(*parent) {
                Some(Value::Object(object)) => Some(object),
                _ => None,
            })
            .and_then(|object| object.get(key))
            .is_some_and(|value| matches!(value, Value::String(text) if text == REDACTED))
    };
    match kind {
        ResourceKind::User => at(&[], "hash") || at(&[], "scram"),
        ResourceKind::Access => {
            at(&["kind", "jwt", "verify"], "key") || at(&["kind", "jwt", "issuer"], "key")
        }
        _ => false,
    }
}

/// The definitions of one database that migrations manage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    definitions: BTreeMap<ResourceKey, Definition>,
}

impl Schema {
    /// Reads the definitions of the current database: the database level and every table's,
    /// in two round trips.
    ///
    /// Only the database is inspected, so a database user can run it. Definitions of a kind
    /// migrations do not manage yet (APIs, buckets, configs, models, modules, sequences, or one
    /// the server adds) fail with [`MigrationError::UnsupportedSchemaResource`] instead of being
    /// ignored.
    pub async fn inspect(db: &Surreal<impl Connection>) -> MigrationResult<Self> {
        let mut info = QueryChain::from(vec![
            Chainable::from(info_for().database().structure()),
            Chainable::from(info_for().database()),
        ])
        .run(db.clone())
        .await?;
        let structures = DatabaseInfo::<Structures>::from_info(take_value(&mut info, 0)?)?;
        let statements = DatabaseInfo::<Statements>::from_info(take_value(&mut info, 1)?)?;

        let mut schema = Self::default();
        let DatabaseInfo {
            accesses,
            apis,
            analyzers,
            buckets,
            configs,
            functions,
            models,
            modules,
            params,
            sequences,
            tables,
            users,
            other,
        } = structures;
        schema.collect(
            "database",
            None,
            [
                (ResourceKind::Function, functions, statements.functions),
                (ResourceKind::Analyzer, analyzers, statements.analyzers),
                (ResourceKind::Param, params, statements.params),
                (ResourceKind::Access, accesses, statements.accesses),
                (ResourceKind::User, users, statements.users),
                (ResourceKind::Table, tables, statements.tables),
            ],
            [
                ("apis", apis),
                ("buckets", buckets),
                ("configs", configs),
                ("models", models),
                ("modules", modules),
                ("sequences", sequences),
            ],
            other,
        )?;

        let tables = schema
            .definitions
            .keys()
            .filter(|key| key.kind == ResourceKind::Table)
            .map(|key| key.name.clone())
            .collect::<Vec<_>>();
        if tables.is_empty() {
            return Ok(schema);
        }

        let statements = tables
            .iter()
            .flat_map(|table| {
                [
                    Chainable::from(info_for().table(Table::new(table.as_str())).structure()),
                    Chainable::from(info_for().table(Table::new(table.as_str()))),
                ]
            })
            .collect::<Vec<_>>();
        let mut info = QueryChain::from(statements).run(db.clone()).await?;
        for (index, table) in tables.iter().enumerate() {
            let structures =
                TableInfo::<Structures>::from_info(take_value(&mut info, 2 * index)?)?;
            let statements =
                TableInfo::<Statements>::from_info(take_value(&mut info, 2 * index + 1)?)?;
            // `lives` are running queries, not schema; `tables` are the views fed by this table,
            // which `INFO FOR DB` already lists as tables of their own.
            schema.collect(
                &format!("table `{table}`"),
                Some(table.as_str()),
                [
                    (ResourceKind::Field, structures.fields, statements.fields),
                    (ResourceKind::Index, structures.indexes, statements.indexes),
                    (ResourceKind::Event, structures.events, statements.events),
                ],
                [],
                structures.other,
            )?;
        }
        Ok(schema)
    }

    /// Rejects definitions at the namespace level, which migrations do not manage. Only the
    /// generator's own in-memory databases run it: `INFO FOR NS` needs a root or namespace user,
    /// which [`Schema::inspect`] must not require.
    pub(crate) async fn reject_namespace_definitions(
        db: &Surreal<impl Connection>,
    ) -> MigrationResult<()> {
        let mut info = info_for().namespace().structure().run(db.clone()).await?;
        let info = NamespaceInfo::<Structures>::from_info(take_value(&mut info, 0)?)?;
        // `databases` are the namespace's databases themselves, not definitions inside one.
        Self::default().collect(
            "namespace",
            None,
            [],
            [("accesses", info.accesses), ("users", info.users)],
            info.other,
        )
    }

    /// Adds the definitions one `INFO` level lists. Each `managed` kind comes with its entries
    /// from `INFO … STRUCTURE` and its `DEFINE` statements from the plain `INFO`. An `unmanaged`
    /// kind, or a key of `other`, that is not empty is rejected.
    fn collect<const M: usize, const U: usize>(
        &mut self,
        level: &str,
        table: Option<&str>,
        managed: [(ResourceKind, Vec<Object>, BTreeMap<String, String>); M],
        unmanaged: [(&str, Vec<Object>); U],
        other: BTreeMap<String, Value>,
    ) -> MigrationResult<()> {
        for (key, entries) in unmanaged {
            if !entries.is_empty() {
                return Err(unsupported(level, key, entries.iter().map(name_of)));
            }
        }
        for (key, value) in other {
            let Value::Array(entries) = value else {
                return Err(unexpected_shape(level, &key));
            };
            if !entries.is_empty() {
                let names = entries.iter().map(|entry| match entry {
                    Value::Object(entry) => name_of(entry),
                    _ => None,
                });
                return Err(unsupported(level, &key, names));
            }
        }

        for (kind, entries, statements) in managed {
            for mut entry in entries {
                let name = name_of(&entry)
                    .ok_or_else(|| unexpected_shape(level, kind.info_key()))?
                    .to_owned();
                let sql = match kind {
                    ResourceKind::Field => {
                        field_definition(&statements, table.unwrap_or_default(), &name)?
                    }
                    _ => statements
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| unexpected_shape(level, kind.info_key()))?,
                };
                if kind == ResourceKind::Table {
                    // The table's internal id depends on the order tables were created in.
                    entry.remove("id");
                }
                self.definitions.insert(
                    ResourceKey {
                        kind,
                        table: table.map(str::to_owned),
                        name,
                    },
                    Definition {
                        redacted: is_redacted(kind, &entry),
                        structure: Value::Object(entry),
                        sql,
                    },
                );
            }
        }
        Ok(())
    }

    /// The definition with this key.
    pub fn get(&self, key: &ResourceKey) -> Option<&Definition> {
        self.definitions.get(key)
    }

    /// Every definition, in creation order: by kind, then table, then name.
    pub fn iter(&self) -> impl Iterator<Item = (&ResourceKey, &Definition)> {
        self.definitions.iter()
    }

    /// The fields of `table`, parents before their nested fields.
    pub fn fields<'a>(
        &'a self,
        table: &'a str,
    ) -> impl Iterator<Item = (&'a ResourceKey, &'a Definition)> {
        self.definitions.iter().filter(move |(key, _)| {
            key.kind == ResourceKind::Field && key.table.as_deref() == Some(table)
        })
    }

    /// Whether the database defines nothing migrations manage.
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// The `DEFINE FIELD` statement, among a table's `statements`, of the field `name`: its canonical
/// idiom as `INFO … STRUCTURE` reports it.
///
/// The plain output keys fields by an unescaped name, which differs from the canonical one for
/// names such as `` `value` ``, so the statement is matched by its text: it must start with
/// exactly `DEFINE FIELD <name> ON `, and exactly one statement may. The delimiter keeps `foo` from
/// matching `foo.bar` or `foo[*]`.
#[doc(hidden)]
pub fn field_definition(
    statements: &BTreeMap<String, String>,
    table: &str,
    name: &str,
) -> MigrationResult<String> {
    let mut found = statements.values().filter(|sql| {
        sql.strip_prefix("DEFINE FIELD ")
            .and_then(|rest| rest.strip_prefix(name))
            .is_some_and(|rest| rest.starts_with(" ON "))
    });
    match (found.next(), found.count()) {
        (Some(sql), 0) => Ok(sql.clone()),
        (first, others) => Err(MigrationError::FieldDefinitionMismatch {
            table: table.to_owned(),
            field: name.to_owned(),
            matches: usize::from(first.is_some()) + others,
        }),
    }
}

fn name_of(entry: &Object) -> Option<&str> {
    match entry.get("name") {
        Some(Value::String(name)) => Some(name),
        _ => None,
    }
}

/// Rejects a kind migrations do not manage, naming its definitions.
fn unsupported<'a>(
    level: &str,
    kind: &str,
    names: impl Iterator<Item = Option<&'a str>>,
) -> MigrationError {
    MigrationError::UnsupportedSchemaResource {
        level: level.to_owned(),
        kind: kind.to_owned(),
        names: names.map(|name| name.unwrap_or("?").to_owned()).collect(),
    }
}

fn take_value(results: &mut IndexedResults, index: usize) -> MigrationResult<Value> {
    Ok(results.take::<Value>(index)?)
}

fn unexpected_shape(level: &str, key: &str) -> MigrationError {
    MigrationError::UnexpectedInfoShape(format!("`{key}` of the {level}"))
}
