//! Migration statements from the difference between two schemas.
//!
//! The changes are computed once; each direction then plans its own statements from them, since
//! a create in one direction is a removal in the other. Within a direction every definition is
//! written first, prerequisites before their dependents (see [`ResourceKind`]), then every removal,
//! dependents before their prerequisites: replacing an analyzer defines the new one, moves the
//! index to it, and only then removes the old one.

use std::collections::{BTreeSet, HashSet};

use surrealdb::types::{Object, ToSql, Value};

use super::{bucket_backend::credential_in_backend, table_fields::field_changes};
use crate::{
    DbResources, Field, Render, Table,
    migrations::{
        Definition, Direction, MigrationError, MigrationResult, Prompter, QueryType, ResourceKey,
        ResourceKind, Schema,
    },
    render::render_inline,
    statements::{
        ModuleName, alter_sequence, remove_access, remove_analyzer, remove_api, remove_bucket,
        remove_config_api, remove_config_graphql, remove_event, remove_field, remove_function,
        remove_index, remove_module, remove_param, remove_sequence, remove_table, remove_user,
    },
};

/// A change from the left schema (the migration files') to the right one (the codebase's).
#[derive(Debug, Clone)]
pub(crate) enum Change {
    Create(ResourceKey),
    Remove(ResourceKey),
    Update(ResourceKey),
    /// A field of `table` renamed from `old` to `new`, keeping its values.
    Rename {
        table: String,
        old: String,
        new: String,
    },
}

/// One statement group of a direction.
enum Step<'a> {
    /// A definition the database does not hold yet in this direction.
    Create(&'a ResourceKey),
    /// A definition the database holds in another form.
    Update(&'a ResourceKey),
    Remove(&'a ResourceKey),
    Rename {
        table: &'a str,
        from: &'a str,
        to: &'a str,
    },
}

impl Step<'_> {
    /// Where the step sorts: by kind, then table, then name.
    fn order(&self) -> (ResourceKind, Option<&str>, &str) {
        match self {
            Self::Create(key) | Self::Update(key) | Self::Remove(key) => {
                (key.kind, key.table.as_deref(), key.name.as_str())
            }
            Self::Rename { table, to, .. } => (ResourceKind::Field, Some(*table), *to),
        }
    }
}

/// The changes between the schema the migration files produce and the codebase's.
pub(crate) struct SchemaDiff<'a> {
    left: &'a Schema,
    right: &'a Schema,
    changes: Vec<Change>,
}

impl<'a> SchemaDiff<'a> {
    /// Compares `left` (the migration files' schema) with `right` (the codebase's). Fields are
    /// compared table by table, so renames can be recognised.
    pub(crate) fn new(
        left: &'a Schema,
        right: &'a Schema,
        codebase_resources: &impl DbResources,
        prompter: &dyn Prompter,
    ) -> MigrationResult<Self> {
        let mut changes = Vec::new();
        let keys = left
            .iter()
            .chain(right.iter())
            .map(|(key, _)| key)
            .filter(|key| key.kind != ResourceKind::Field)
            .collect::<BTreeSet<_>>();
        for key in keys {
            match (left.get(key), right.get(key)) {
                (None, Some(_)) => changes.push(Change::Create(key.clone())),
                (Some(_), None) => changes.push(Change::Remove(key.clone())),
                (Some(left), Some(right)) if left != right => {
                    changes.push(Change::Update(key.clone()));
                }
                _ => {}
            }
        }

        let models = codebase_resources.tables_fields_meta();
        let tables = left
            .iter()
            .chain(right.iter())
            .filter(|(key, _)| key.kind == ResourceKind::Field)
            .filter_map(|(key, _)| key.table.as_deref())
            .collect::<BTreeSet<_>>();
        for table in tables {
            let models = models
                .get(&Table::new(table))
                .map(Vec::as_slice)
                .unwrap_or_default();
            field_changes(table, left, right, models, prompter, &mut changes)?;
        }

        Ok(Self {
            left,
            right,
            changes,
        })
    }

    /// Whether the schemas are the same.
    pub(crate) fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The statements that take the database from one schema to the other: `Up` from the
    /// migration files' schema to the codebase's, `Down` back.
    ///
    /// Every definition is written with `OVERWRITE`: since SurrealDB 2, a plain `DEFINE` fails
    /// when the resource exists, which an update always and a reset's first migration usually
    /// meets. Sequences are the exception, since redefining one resets the values it has issued:
    /// a new sequence is written with a plain `DEFINE`, so it fails rather than resets one that
    /// exists, and only a change of its timeout is written, as `ALTER SEQUENCE`; any other change
    /// fails with [`MigrationError::UnsafeSequenceUpdate`]. Buckets are the other, since
    /// redefining one connects a new store, which empties a `memory` bucket: a new bucket is
    /// written with a plain `DEFINE`, and any change fails with
    /// [`MigrationError::UnsafeBucketUpdate`].
    ///
    /// A definition whose secrets the server redacted fails with
    /// [`MigrationError::RedactedDefinition`] rather than being written with the placeholder,
    /// and a bucket whose backend carries credentials fails with
    /// [`MigrationError::CredentialInBucketBackend`].
    pub(crate) fn statements(&self, direction: &Direction) -> MigrationResult<Vec<QueryType>> {
        let (source, target) = match direction {
            Direction::Up => (self.left, self.right),
            Direction::Down => (self.right, self.left),
        };
        let up = matches!(direction, Direction::Up);

        let mut defines = Vec::new();
        let mut removes = Vec::new();
        for change in &self.changes {
            match change {
                Change::Create(key) if up => defines.push(Step::Create(key)),
                Change::Create(key) => removes.push(Step::Remove(key)),
                Change::Remove(key) if up => removes.push(Step::Remove(key)),
                Change::Remove(key) => defines.push(Step::Create(key)),
                Change::Update(key) => defines.push(Step::Update(key)),
                Change::Rename { table, old, new } => defines.push(Step::Rename {
                    table,
                    from: if up { old } else { new },
                    to: if up { new } else { old },
                }),
            }
        }
        defines.sort_by(|a, b| a.order().cmp(&b.order()));
        removes.sort_by(|a, b| b.order().cmp(&a.order()));

        // Removing a table removes its fields, indexes and events with it.
        let removed_tables = removes
            .iter()
            .filter_map(|step| match step {
                Step::Remove(key) if key.kind == ResourceKind::Table => Some(key.name.as_str()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        removes.retain(|step| match step {
            Step::Remove(key) => key
                .table
                .as_deref()
                .is_none_or(|table| !removed_tables.contains(table)),
            _ => true,
        });

        let mut statements = Vec::new();
        let mut group = None;
        for step in defines.iter().chain(&removes) {
            // A blank line between kinds and between tables.
            let (kind, table, _) = step.order();
            if group.is_some_and(|group| group != (kind, table)) {
                statements.push(QueryType::Blank);
            }
            group = Some((kind, table));

            match step {
                Step::Create(key) => statements.push(QueryType::Statement(define(target, key)?)),
                Step::Update(key) if key.kind == ResourceKind::Sequence => statements.push(
                    QueryType::Statement(alter_sequence_timeout(source, target, key)?),
                ),
                Step::Update(key) if key.kind == ResourceKind::Bucket => {
                    return Err(MigrationError::UnsafeBucketUpdate {
                        key: (*key).clone(),
                    });
                }
                Step::Update(key) => statements.push(QueryType::Statement(define(target, key)?)),
                Step::Remove(key) => statements.push(QueryType::Statement(remove(key)?)),
                Step::Rename { table, from, to } => {
                    let new = field_key(table, to);
                    statements.extend([
                        QueryType::Comment(format!("Rename field {from} to {to} on table {table}")),
                        QueryType::Statement(define(target, &new)?),
                        QueryType::Statement(format!(
                            "UPDATE {} SET {to} = {from}",
                            Table::new(*table).to_sql()
                        )),
                        QueryType::Statement(remove(&field_key(table, from))?),
                        QueryType::Comment("Rename field ending".to_owned()),
                    ]);
                }
            }
        }
        Ok(statements)
    }
}

/// The definition of `key` in `schema`.
fn definition<'a>(schema: &'a Schema, key: &ResourceKey) -> MigrationResult<&'a Definition> {
    schema
        .get(key)
        .ok_or_else(|| MigrationError::UnexpectedInfoShape(format!("{key} is not defined")))
}

/// The definition of `key` in `schema`, written with `OVERWRITE`, except a sequence's or a
/// bucket's: redefining a sequence resets the values it has issued, and redefining a bucket
/// connects a new store, so they are written with a plain `DEFINE`, which fails where the
/// resource exists.
fn define(schema: &Schema, key: &ResourceKey) -> MigrationResult<String> {
    let definition = definition(schema, key)?;
    if definition.is_redacted() {
        return Err(MigrationError::RedactedDefinition { key: key.clone() });
    }
    if key.kind == ResourceKind::Bucket
        && let Some(Value::String(backend)) = structure(definition, key)?.get("backend")
        && let Some(reason) = credential_in_backend(backend)
    {
        return Err(MigrationError::CredentialInBucketBackend {
            key: key.clone(),
            reason,
        });
    }
    if matches!(key.kind, ResourceKind::Sequence | ResourceKind::Bucket) {
        return Ok(definition.sql().to_owned());
    }
    // The server renders `DEFINE <KEYWORD> <rest>`; `OVERWRITE` goes between the two.
    let keyword = key.kind.keyword();
    let rest = definition
        .sql()
        .strip_prefix("DEFINE ")
        .and_then(|sql| sql.strip_prefix(keyword))
        .and_then(|sql| sql.strip_prefix(' '))
        .ok_or_else(|| MigrationError::UnexpectedDefinitionText { key: key.clone() })?;
    Ok(format!("DEFINE {keyword} OVERWRITE {rest}"))
}

/// The definition of `key` as the object `INFO … STRUCTURE` lists.
fn structure<'a>(definition: &'a Definition, key: &ResourceKey) -> MigrationResult<&'a Object> {
    match definition.structure() {
        Value::Object(object) => Ok(object),
        _ => Err(MigrationError::UnexpectedInfoShape(format!(
            "{key} is not an object"
        ))),
    }
}

/// The `ALTER SEQUENCE` that takes the sequence `key` from its definition in `source` to the one
/// in `target`, which may differ only in their timeout: changing the batch or start means
/// redefining the sequence, which resets the values it has issued, so it fails with
/// [`MigrationError::UnsafeSequenceUpdate`].
fn alter_sequence_timeout(
    source: &Schema,
    target: &Schema,
    key: &ResourceKey,
) -> MigrationResult<String> {
    let without_timeout = |schema: &Schema| -> MigrationResult<(Object, Option<Value>)> {
        let mut structure = structure(definition(schema, key)?, key)?.clone();
        let timeout = structure.remove("timeout");
        Ok((structure, timeout))
    };
    let (from, _) = without_timeout(source)?;
    let (to, timeout) = without_timeout(target)?;
    if from != to {
        return Err(MigrationError::UnsafeSequenceUpdate { key: key.clone() });
    }
    let statement = alter_sequence(key.name.clone());
    let statement = match timeout {
        Some(Value::Duration(timeout)) => statement.timeout(timeout),
        _ => statement.drop_timeout(),
    };
    rendered(key, &statement)
}

/// `statement` rendered, or [`MigrationError::InvalidStatement`] with the errors rendering it
/// found.
fn rendered(key: &ResourceKey, statement: &impl Render) -> MigrationResult<String> {
    let (sql, errors) = render_inline(statement);
    if !errors.is_empty() {
        return Err(MigrationError::InvalidStatement {
            key: key.clone(),
            errors,
        });
    }
    Ok(sql)
}

/// The `REMOVE` statement of `key`, built from its kind, table and name. A statement that cannot
/// be rendered, such as for a function whose name is not a path of identifiers, fails with
/// [`MigrationError::InvalidStatement`] rather than being written.
fn remove(key: &ResourceKey) -> MigrationResult<String> {
    let name = key.name.clone();
    let table = || Table::new(key.table.as_deref().unwrap_or_default());
    match key.kind {
        ResourceKind::Bucket => rendered(key, &remove_bucket(name)),
        ResourceKind::Module => match name.parse::<ModuleName>() {
            Ok(module) => rendered(key, &remove_module(module)),
            Err(error) => Err(MigrationError::InvalidStatement {
                key: key.clone(),
                errors: vec![error.to_string()],
            }),
        },
        ResourceKind::Function => rendered(key, &remove_function(name)),
        ResourceKind::Analyzer => rendered(key, &remove_analyzer(name)),
        ResourceKind::Param => rendered(key, &remove_param(name)),
        ResourceKind::Sequence => rendered(key, &remove_sequence(name)),
        ResourceKind::Access => rendered(key, &remove_access(name).on_database()),
        ResourceKind::User => rendered(key, &remove_user(name).on_database()),
        ResourceKind::Table => rendered(key, &remove_table(Table::new(name))),
        // A field key is SurrealDB's own rendering of the path, such as `address.city` or
        // `tags[*]`, so it is written as it is; the statement binds no values.
        ResourceKind::Field => rendered(key, &remove_field(Field::raw(name)).on_table(table())),
        ResourceKind::Index => rendered(key, &remove_index(name).on_table(table())),
        ResourceKind::Event => rendered(key, &remove_event(name).on_table(table())),
        ResourceKind::Api => rendered(key, &remove_api(name)),
        ResourceKind::Config => match name.as_str() {
            "API" => rendered(key, &remove_config_api()),
            "GraphQL" => rendered(key, &remove_config_graphql()),
            _ => Err(MigrationError::UnexpectedInfoShape(format!(
                "{key} is not an API or GraphQL config"
            ))),
        },
    }
}

fn field_key(table: &str, name: &str) -> ResourceKey {
    ResourceKey {
        kind: ResourceKind::Field,
        table: Some(table.to_owned()),
        name: name.to_owned(),
    }
}
