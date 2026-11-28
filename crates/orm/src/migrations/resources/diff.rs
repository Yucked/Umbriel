//! Migration statements from the difference between two schemas.
//!
//! The changes are computed once; each direction then plans its own statements from them, since
//! a create in one direction is a removal in the other. Within a direction every definition is
//! written first, prerequisites before their dependents (see [`ResourceKind`]), then every removal,
//! dependents before their prerequisites: replacing an analyzer defines the new one, moves the
//! index to it, and only then removes the old one. Between the two, the full-text indexes whose
//! analyzer changed in place are rebuilt.

use std::collections::{BTreeSet, HashMap, HashSet};

use surrealdb::types::{Object, ToSql, Value};

use super::{bucket_backend::credential_in_backend, table_fields::field_changes};
use crate::{
    DbResources, Field, Render, Table, TableIndex,
    migrations::{
        Definition, Direction, MigrationError, MigrationResult, Prompter, QueryType, ResourceKey,
        ResourceKind, Schema,
    },
    render::render_inline,
    statements::{
        ModuleName, alter_bucket, alter_index, alter_sequence, rebuild_index, remove_access,
        remove_analyzer, remove_api, remove_bucket, remove_config_api, remove_config_graphql,
        remove_event, remove_field, remove_function, remove_index, remove_module, remove_param,
        remove_sequence, remove_table, remove_user,
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
    /// `REBUILD INDEX`: a full-text index whose analyzer changed in place.
    Rebuild(&'a ResourceKey),
    Rename {
        table: &'a str,
        from: &'a str,
        to: &'a str,
    },
}

impl Step<'_> {
    /// Where the step sorts: by kind, then table, then renames before other steps, then name.
    /// A rename copies values with an `UPDATE` of every record, which checks each record against
    /// the table's field definitions, so it runs before a new required field is defined
    /// (UMB-036).
    fn order(&self) -> (ResourceKind, Option<&str>, bool, &str) {
        match self {
            Self::Create(key) | Self::Update(key) | Self::Remove(key) | Self::Rebuild(key) => {
                (key.kind, key.table.as_deref(), true, key.name.as_str())
            }
            Self::Rename { table, to, .. } => (ResourceKind::Field, Some(*table), false, *to),
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

    /// The tables and fields going up removes, with their data, as `REMOVE TABLE person` and
    /// `REMOVE FIELD name ON person`; a field of a removed table goes with the table. A rename
    /// keeps its values, so it is not listed.
    pub(crate) fn data_removals(&self) -> Vec<String> {
        let removed = |kind| {
            self.changes.iter().filter_map(move |change| match change {
                Change::Remove(key) if key.kind == kind => Some(key),
                _ => None,
            })
        };
        let tables = removed(ResourceKind::Table)
            .map(|key| key.name.as_str())
            .collect::<HashSet<_>>();
        removed(ResourceKind::Table)
            .map(|key| format!("REMOVE TABLE {}", key.name))
            .chain(removed(ResourceKind::Field).filter_map(|key| {
                let table = key.table.as_deref()?;
                (!tables.contains(table)).then(|| format!("REMOVE FIELD {} ON {table}", key.name))
            }))
            .collect()
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
    /// written with a plain `DEFINE`, a change of its comment, `READONLY` or permissions as
    /// `ALTER BUCKET`, which keeps its files, and a change of its backend fails with
    /// [`MigrationError::UnsafeBucketUpdate`].
    ///
    /// An index whose comment alone changes is written as `ALTER INDEX`, which keeps its data,
    /// rather than redefined, which rebuilds it. Its `PREPARE REMOVE` mark, which a redefinition
    /// clears, decides the rest:
    ///
    /// | Change | Written as |
    /// |---|---|
    /// | marked for removal, nothing else but the comment | `ALTER INDEX … PREPARE REMOVE` |
    /// | marked for removal with another change | [`MigrationError::UnsafeIndexUpdate`] |
    /// | unmarked, with or without another change | `DEFINE INDEX OVERWRITE` |
    /// | a marked index, another change | [`MigrationError::UnsafeIndexUpdate`] |
    /// | created marked | `DEFINE INDEX`, then `ALTER INDEX … PREPARE REMOVE` |
    ///
    /// Unmarking with another change is forward-only: its reverse is rejected, so a two-way
    /// migration with it fails. Every other allowed change is reversible.
    ///
    /// SurrealDB does not rebuild an index when its analyzer is redefined, so after every
    /// definition a direction rebuilds (`REBUILD INDEX`) each full-text index that it keeps,
    /// does not redefine and has not marked for removal, whose analyzer changed other than in its
    /// comment, or whose analyzer's `FUNCTION` names a function (`fn::`) or module (`mod::`)
    /// that changed other than in its comment. A function that function calls, and the
    /// contents of a module's file, are not followed.
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

        // Index updates are planned first: whether one redefines its index decides whether an
        // analyzer change still needs to rebuild it.
        let mut index_updates = HashMap::new();
        for step in &defines {
            if let Step::Update(key) = step
                && key.kind == ResourceKind::Index
            {
                index_updates.insert(*key, index_update(source, target, key)?);
            }
        }
        let redefined = defines
            .iter()
            .filter_map(|step| match step {
                Step::Create(key) if key.kind == ResourceKind::Index => Some(*key),
                Step::Update(key) => index_updates
                    .get(key)
                    .is_some_and(|update| update.redefines)
                    .then_some(*key),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let mut rebuilds = self.rebuilds(source, target, &redefined)?;
        rebuilds.sort_by(|a, b| a.order().cmp(&b.order()));

        let mut statements = Vec::new();
        let mut group = None;
        for step in defines.iter().chain(&rebuilds).chain(&removes) {
            // A blank line between kinds and between tables.
            let (kind, table, _, _) = step.order();
            if group.is_some_and(|group| group != (kind, table)) {
                statements.push(QueryType::Blank);
            }
            group = Some((kind, table));

            match step {
                Step::Create(key) => {
                    statements.push(QueryType::Statement(define(target, key)?));
                    if key.kind == ResourceKind::Index && flagged(target, key)? {
                        let statement = alter_index(TableIndex::new(key.name.clone()))
                            .on_table(index_table(key))
                            .prepare_remove();
                        statements.push(QueryType::Statement(rendered(key, &statement)?));
                    }
                }
                Step::Update(key) if key.kind == ResourceKind::Sequence => statements.push(
                    QueryType::Statement(alter_sequence_timeout(source, target, key)?),
                ),
                Step::Update(key) if key.kind == ResourceKind::Bucket => statements.push(
                    QueryType::Statement(alter_bucket_metadata(source, target, key)?),
                ),
                Step::Update(key) if key.kind == ResourceKind::Index => {
                    if let Some(update) = index_updates.remove(key) {
                        statements.extend(update.statements.into_iter().map(QueryType::Statement));
                    }
                }
                Step::Update(key) => statements.push(QueryType::Statement(define(target, key)?)),
                Step::Rebuild(key) => {
                    let statement =
                        rebuild_index(TableIndex::new(key.name.clone())).on_table(index_table(key));
                    statements.push(QueryType::Statement(rendered(key, &statement)?));
                }
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

impl<'a> SchemaDiff<'a> {
    /// The `REBUILD INDEX` steps of a direction from `source` to `target`: each full-text index
    /// of both schemas, not in `redefined` and not marked for removal in `target`, whose analyzer
    /// changed other than in its comment, or names in its `FUNCTION` a function or module that
    /// did.
    fn rebuilds(
        &self,
        source: &'a Schema,
        target: &'a Schema,
        redefined: &HashSet<&ResourceKey>,
    ) -> MigrationResult<Vec<Step<'a>>> {
        let mut changed = HashMap::<ResourceKind, HashSet<&str>>::new();
        for change in &self.changes {
            if let Change::Update(key) = change
                && matches!(
                    key.kind,
                    ResourceKind::Analyzer | ResourceKind::Function | ResourceKind::Module
                )
                && differs_outside_comment(source, target, key)?
            {
                changed
                    .entry(key.kind)
                    .or_default()
                    .insert(key.name.as_str());
            }
        }
        let is_changed = |kind: ResourceKind, name: &str| {
            changed.get(&kind).is_some_and(|names| names.contains(name))
        };

        let mut analyzers = HashSet::new();
        for (key, definition) in target.iter() {
            if key.kind != ResourceKind::Analyzer || source.get(key).is_none() {
                continue;
            }
            let function = match structure(definition, key)?.get("function") {
                Some(Value::String(path)) => Some(path.as_str()),
                _ => None,
            };
            let function_changed = function.is_some_and(|path| match path.strip_prefix("mod::") {
                // `mod::module::function`: the module is the first segment.
                Some(rest) => {
                    let module = rest.split("::").next().unwrap_or_default();
                    is_changed(ResourceKind::Module, module)
                        || is_changed(ResourceKind::Module, &format!("mod::{module}"))
                }
                None => is_changed(
                    ResourceKind::Function,
                    path.strip_prefix("fn::").unwrap_or(path),
                ),
            });
            if function_changed || is_changed(ResourceKind::Analyzer, &key.name) {
                analyzers.insert(key.name.as_str());
            }
        }
        if analyzers.is_empty() {
            return Ok(Vec::new());
        }

        let mut rebuilds = Vec::new();
        for (key, definition) in target.iter() {
            if key.kind != ResourceKind::Index
                || source.get(key).is_none()
                || redefined.contains(key)
                || flagged(target, key)?
            {
                continue;
            }
            if let Some(Value::String(index)) = structure(definition, key)?.get("index")
                && fulltext_analyzer(key, index)?
                    .is_some_and(|name| analyzers.contains(name.as_str()))
            {
                rebuilds.push(Step::Rebuild(key));
            }
        }
        Ok(rebuilds)
    }
}

/// The analyzer a rendered index kind of `key` names, `FULLTEXT ANALYZER <name> …`, decoded;
/// `None` for any other kind. SurrealDB writes the name bare, or between backticks with the
/// escapes of `common::fmt::escape.rs` (`\0`, `\r`, `\t`, `\n`, `\u{8}`, `\f`, `\\` and
/// `` \` ``). Any other text fails with [`MigrationError::UnexpectedInfoShape`] rather than
/// skipping a rebuild.
fn fulltext_analyzer(key: &ResourceKey, index: &str) -> MigrationResult<Option<String>> {
    let Some(rest) = index.strip_prefix("FULLTEXT ANALYZER ") else {
        return Ok(None);
    };
    let unexpected = || {
        MigrationError::UnexpectedInfoShape(format!(
            "the analyzer name of {key} cannot be read from `{index}`"
        ))
    };
    let Some(quoted) = rest.strip_prefix('`') else {
        return Ok(Some(rest.split(' ').next().unwrap_or_default().to_owned()));
    };
    let mut name = String::new();
    let mut chars = quoted.chars();
    while let Some(c) = chars.next() {
        match c {
            '`' => return Ok(Some(name)),
            '\\' => name.push(match chars.next().ok_or_else(unexpected)? {
                '0' => '\0',
                'r' => '\r',
                't' => '\t',
                'n' => '\n',
                'f' => '\x0C',
                '\\' => '\\',
                '`' => '`',
                'u' => {
                    let hex = chars
                        .by_ref()
                        .skip_while(|&c| c == '{')
                        .take_while(|&c| c != '}')
                        .collect::<String>();
                    u32::from_str_radix(&hex, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or_else(unexpected)?
                }
                _ => return Err(unexpected()),
            }),
            c => name.push(c),
        }
    }
    Err(unexpected())
}

/// Whether the definitions of `key` in `source` and `target` differ other than in their comment.
fn differs_outside_comment(
    source: &Schema,
    target: &Schema,
    key: &ResourceKey,
) -> MigrationResult<bool> {
    let without_comment = |schema: &Schema| -> MigrationResult<Object> {
        let mut structure = structure(definition(schema, key)?, key)?.clone();
        structure.remove("comment");
        Ok(structure)
    };
    Ok(without_comment(source)? != without_comment(target)?)
}

/// Whether the index `key` is marked for removal (`PREPARE REMOVE`) in `schema`.
fn flagged(schema: &Schema, key: &ResourceKey) -> MigrationResult<bool> {
    Ok(structure(definition(schema, key)?, key)?.get("prepare_remove") == Some(&Value::Bool(true)))
}

/// The table of the index `key`.
fn index_table(key: &ResourceKey) -> Table {
    Table::new(key.table.as_deref().unwrap_or_default())
}

/// The statements an index update is written as, and whether they redefine the index.
struct IndexUpdate {
    statements: Vec<String>,
    redefines: bool,
}

/// How the index `key` goes from its definition in `source` to the one in `target`: an
/// `ALTER INDEX` when only its comment changes or it becomes marked for removal, a redefinition
/// when its mark is cleared or something else changes on an unmarked index, and
/// [`MigrationError::UnsafeIndexUpdate`] when a redefinition would clear a mark `target` keeps
/// or sets.
fn index_update(
    source: &Schema,
    target: &Schema,
    key: &ResourceKey,
) -> MigrationResult<IndexUpdate> {
    let parts = |schema: &Schema| -> MigrationResult<(Object, Option<Value>, bool)> {
        let mut structure = structure(definition(schema, key)?, key)?.clone();
        let comment = structure.remove("comment");
        let flagged = structure.remove("prepare_remove") == Some(Value::Bool(true));
        Ok((structure, comment, flagged))
    };
    let (from, from_comment, from_flagged) = parts(source)?;
    let (to, to_comment, to_flagged) = parts(target)?;
    match (from_flagged, to_flagged, from != to) {
        (false, false, true) | (true, false, _) => Ok(IndexUpdate {
            statements: vec![define(target, key)?],
            redefines: true,
        }),
        (_, true, true) => Err(MigrationError::UnsafeIndexUpdate { key: key.clone() }),
        (_, _, false) => {
            let mut statement =
                alter_index(TableIndex::new(key.name.clone())).on_table(index_table(key));
            if to_flagged && !from_flagged {
                statement = statement.prepare_remove();
            }
            if from_comment != to_comment {
                statement = match to_comment {
                    Some(Value::String(comment)) => statement.comment(comment),
                    None => statement.drop_comment(),
                    Some(_) => {
                        return Err(MigrationError::UnexpectedInfoShape(format!(
                            "the comment of {key} is not a string"
                        )));
                    }
                };
            }
            Ok(IndexUpdate {
                statements: vec![rendered(key, &statement)?],
                redefines: false,
            })
        }
    }
}

/// The `ALTER BUCKET` that takes the bucket `key` from its definition in `source` to the one in
/// `target`, which may differ only in their comment, `READONLY` and permissions: `ALTER BUCKET`
/// keeps the bucket's files, but a backend change would leave the open store in place, and
/// redefining the bucket empties a `memory` one, so it fails with
/// [`MigrationError::UnsafeBucketUpdate`].
fn alter_bucket_metadata(
    source: &Schema,
    target: &Schema,
    key: &ResourceKey,
) -> MigrationResult<String> {
    let parts = |schema: &Schema| -> MigrationResult<(Object, [Option<Value>; 3])> {
        let mut structure = structure(definition(schema, key)?, key)?.clone();
        let metadata = [
            structure.remove("comment"),
            structure.remove("readonly"),
            structure.remove("permissions"),
        ];
        Ok((structure, metadata))
    };
    let (from, [from_comment, from_readonly, from_permissions]) = parts(source)?;
    let (to, [to_comment, to_readonly, to_permissions]) = parts(target)?;
    if from != to {
        return Err(MigrationError::UnsafeBucketUpdate { key: key.clone() });
    }
    let unexpected = |what: &str| {
        MigrationError::UnexpectedInfoShape(format!("the {what} of {key} has an unexpected shape"))
    };

    let mut statement = alter_bucket(key.name.clone());
    if from_comment != to_comment {
        statement = match to_comment {
            Some(Value::String(comment)) => statement.comment(comment),
            None => statement.drop_comment(),
            Some(_) => return Err(unexpected("comment")),
        };
    }
    if from_readonly != to_readonly {
        statement = match to_readonly {
            Some(Value::Bool(true)) => statement.readonly(),
            None | Some(Value::Bool(false)) => statement.drop_readonly(),
            Some(_) => return Err(unexpected("readonly flag")),
        };
    }
    if from_permissions != to_permissions {
        statement = match to_permissions {
            Some(Value::Bool(true)) => statement.permissions_full(),
            Some(Value::Bool(false)) | None => statement.permissions_none(),
            // The server's own rendering of the condition, so it is written as it is.
            Some(Value::String(condition)) => statement.permissions_where(Field::raw(condition)),
            Some(_) => return Err(unexpected("permissions")),
        };
    }
    rendered(key, &statement)
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
