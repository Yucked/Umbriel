//! Migration statements from the difference between two schemas.
//!
//! The changes are computed once; each direction then plans its own statements from them, since
//! a create in one direction is a removal in the other. Within a direction every definition is
//! written first, prerequisites before their dependents (see [`ResourceKind`]), then every removal,
//! dependents before their prerequisites: replacing an analyzer defines the new one, moves the
//! index to it, and only then removes the old one.

use std::collections::{BTreeSet, HashSet};

use surrealdb::types::ToSql;

use super::table_fields::field_changes;
use crate::{
    DbResources, Field, Render, Table,
    migrations::{
        Direction, MigrationError, MigrationResult, Prompter, QueryType, ResourceKey, ResourceKind,
        Schema,
    },
    render::write_ident,
    statements::{
        remove_access, remove_analyzer, remove_event, remove_field, remove_function, remove_index,
        remove_param, remove_table, remove_user,
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
    Define(&'a ResourceKey),
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
            Self::Define(key) | Self::Remove(key) => {
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
    /// meets. A definition whose secrets the server redacted fails with
    /// [`MigrationError::RedactedDefinition`] rather than being written with the placeholder.
    pub(crate) fn statements(&self, direction: &Direction) -> MigrationResult<Vec<QueryType>> {
        let target = match direction {
            Direction::Up => self.right,
            Direction::Down => self.left,
        };
        let up = matches!(direction, Direction::Up);

        let mut defines = Vec::new();
        let mut removes = Vec::new();
        for change in &self.changes {
            match change {
                Change::Create(key) if up => defines.push(Step::Define(key)),
                Change::Create(key) => removes.push(Step::Remove(key)),
                Change::Remove(key) if up => removes.push(Step::Remove(key)),
                Change::Remove(key) => defines.push(Step::Define(key)),
                Change::Update(key) => defines.push(Step::Define(key)),
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
                Step::Define(key) => statements.push(QueryType::Statement(define(target, key)?)),
                Step::Remove(key) => statements.push(QueryType::Statement(remove(key))),
                Step::Rename { table, from, to } => {
                    let new = field_key(table, to);
                    statements.extend([
                        QueryType::Comment(format!("Rename field {from} to {to} on table {table}")),
                        QueryType::Statement(define(target, &new)?),
                        QueryType::Statement(format!(
                            "UPDATE {} SET {to} = {from}",
                            Table::new(*table).to_sql()
                        )),
                        QueryType::Statement(remove(&field_key(table, from))),
                        QueryType::Comment("Rename field ending".to_owned()),
                    ]);
                }
            }
        }
        Ok(statements)
    }
}

/// The definition of `key` in `schema`, written with `OVERWRITE`.
fn define(schema: &Schema, key: &ResourceKey) -> MigrationResult<String> {
    let definition = schema
        .get(key)
        .ok_or_else(|| MigrationError::UnexpectedInfoShape(format!("{key} is not defined")))?;
    if definition.is_redacted() {
        return Err(MigrationError::RedactedDefinition { key: key.clone() });
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

/// The `REMOVE` statement of `key`, built from its kind, table and name.
fn remove(key: &ResourceKey) -> String {
    let name = key.name.clone();
    let table = || Table::new(key.table.as_deref().unwrap_or_default());
    match key.kind {
        ResourceKind::Function => remove_function(function_name(&key.name)).to_debug_sql(),
        ResourceKind::Analyzer => remove_analyzer(name).to_debug_sql(),
        ResourceKind::Param => remove_param(name).to_debug_sql(),
        ResourceKind::Access => remove_access(name).on_database().to_debug_sql(),
        ResourceKind::User => remove_user(name).on_database().to_debug_sql(),
        ResourceKind::Table => remove_table(Table::new(name)).to_debug_sql(),
        // A field key is SurrealDB's own rendering of the path, such as `address.city` or
        // `tags[*]`, so it is written as it is; the statement binds no values.
        ResourceKind::Field => remove_field(Field::raw(name))
            .on_table(table())
            .to_debug_sql(),
        ResourceKind::Index => remove_index(name).on_table(table()).to_debug_sql(),
        ResourceKind::Event => remove_event(name).on_table(table()).to_debug_sql(),
    }
}

/// `fn::` and the function's name, whose `::`-separated parts are identifiers.
fn function_name(name: &str) -> String {
    let mut out = String::from("fn");
    for part in name.split("::") {
        out.push_str("::");
        write_ident(&mut out, part);
    }
    out
}

fn field_key(table: &str, name: &str) -> ResourceKey {
    ResourceKey {
        kind: ResourceKind::Field,
        table: Some(table.to_owned()),
        name: name.to_owned(),
    }
}
