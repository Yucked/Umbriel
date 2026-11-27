//! Field changes of one table, including renames: a field whose model names its previous name
//! (`#[orm(old_name = …)]`), or a single field replaced by another, where the prompter decides
//! between renaming and deleting.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt::Display,
};

use super::diff::Change;
use crate::{
    Field, FieldChangeMeta, FieldMetadata, Render, Table,
    migrations::{MigrationError, MigrationResult, Prompter, ResourceKey, ResourceKind, Schema},
};

/// The answer to a prompt about a single field replaced by another.
#[derive(Debug, Clone)]
pub enum SingleFieldChangeType {
    /// Remove the old field and define the new one, dropping the old field's values.
    Delete(FieldChangeMeta),
    /// Define the new field, copy the old field's values into it, then remove the old field.
    Rename(FieldChangeMeta),
}

impl Display for SingleFieldChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SingleFieldChangeType::Delete(change) => write!(
                f,
                "Delete old field '{}' and create a new one '{}' on table '{}'",
                change.old_name.to_debug_sql(),
                change.new_name.to_debug_sql(),
                change.table
            ),
            SingleFieldChangeType::Rename(change) => write!(
                f,
                "Rename old field '{}' to new field '{}' on table '{}'",
                change.old_name.to_debug_sql(),
                change.new_name.to_debug_sql(),
                change.table
            ),
        }
    }
}

/// Appends the changes to the fields of `table` from `left` to `right`. `models` holds the
/// codebase's field metadata for the table, which names explicit renames.
pub(crate) fn field_changes(
    table: &str,
    left: &Schema,
    right: &Schema,
    models: &[FieldMetadata],
    prompter: &dyn Prompter,
    changes: &mut Vec<Change>,
) -> MigrationResult<()> {
    let left_names = names(left, table);
    let right_names = names(right, table);
    let renames = Renames::new(models);
    let mut fields = left_names
        .union(&right_names)
        .copied()
        .collect::<BTreeSet<_>>();

    // A single field replaced by another without `old_name` may be a rename: ask.
    let mut removed = left_names.difference(&right_names);
    let mut added = right_names.difference(&left_names);
    if let (Some(&old), None, Some(&new), None) =
        (removed.next(), removed.next(), added.next(), added.next())
        && renames.to(new).is_none()
    {
        let meta = FieldChangeMeta {
            table: Table::new(table),
            old_name: Field::raw(old.to_owned()),
            new_name: Field::raw(new.to_owned()),
        };
        let answer = prompter.prompt_single_field_rename_or_delete(
            SingleFieldChangeType::Delete(meta.clone()),
            SingleFieldChangeType::Rename(meta),
        )?;
        match answer {
            SingleFieldChangeType::Rename(_) => changes.push(Change::Rename {
                table: table.to_owned(),
                old: old.to_owned(),
                new: new.to_owned(),
            }),
            SingleFieldChangeType::Delete(_) => {
                changes.push(Change::Remove(key(table, old)));
                changes.push(Change::Create(key(table, new)));
            }
        }
        fields.remove(old);
        fields.remove(new);
    }

    for field in fields {
        match (renames.renames_from(field), renames.to(field)) {
            (true, Some(_)) => {
                return Err(MigrationError::RenamingToSameOldFieldDisallowed {
                    field: Field::raw(field.to_owned()),
                    table: Table::new(table),
                });
            }
            (false, Some(old)) => {
                let old = old.to_owned();
                if right_names.contains(old.as_str()) {
                    return Err(MigrationError::CannotRenameFromOldFieldInUSeInTable {
                        new_field: field.to_owned(),
                        old_field: old,
                        table: Table::new(table),
                        renamables: join(&left_names),
                    });
                }
                if !left_names.contains(old.as_str()) {
                    // Applied by an earlier migration: both schemas hold the new field, so
                    // `old_name` is history and the field compares like any other.
                    if left_names.contains(field) {
                        compare(table, field, left, right, changes);
                        continue;
                    }
                    return Err(MigrationError::InvalidOldFieldName {
                        new_name: Box::new(Field::raw(field.to_owned())),
                        table: Table::new(table),
                        old_name: Box::new(Field::raw(old)),
                        renamables: join(&left_names),
                    });
                }
                changes.push(Change::Rename {
                    table: table.to_owned(),
                    old,
                    new: field.to_owned(),
                });
            }
            // The rename is planned from the new field's side.
            (true, None) => {}
            (false, None) => compare(table, field, left, right, changes),
        }
    }
    Ok(())
}

/// Appends the change of a field that is not being renamed: created, removed or updated.
fn compare(table: &str, field: &str, left: &Schema, right: &Schema, changes: &mut Vec<Change>) {
    let key = key(table, field);
    match (left.get(&key), right.get(&key)) {
        (None, Some(_)) => changes.push(Change::Create(key)),
        (Some(_), None) => changes.push(Change::Remove(key)),
        (Some(left), Some(right)) if left != right => changes.push(Change::Update(key)),
        _ => {}
    }
}

/// The codebase's renamed fields (those with a non-empty `old_name`), each name rendered once,
/// looked up by the new name and by the old one. When several fields share a name, the first in
/// `models` wins, as the earlier linear search did.
struct Renames {
    /// Each new name and its old name.
    by_new: HashMap<String, String>,
    /// The old names.
    old_names: HashSet<String>,
}

impl Renames {
    fn new(models: &[FieldMetadata]) -> Self {
        let mut by_new = HashMap::with_capacity(models.len());
        let mut old_names = HashSet::with_capacity(models.len());
        for meta in models {
            let Some(old) = meta.old_name.as_ref().map(Render::to_debug_sql) else {
                continue;
            };
            if old.is_empty() {
                continue;
            }
            by_new
                .entry(meta.name.to_debug_sql())
                .or_insert_with(|| old.clone());
            old_names.insert(old);
        }
        Self { by_new, old_names }
    }

    /// The old name of the field renamed to `new`.
    fn to(&self, new: &str) -> Option<&str> {
        self.by_new.get(new).map(String::as_str)
    }

    /// Whether a field is renamed from `old`.
    fn renames_from(&self, old: &str) -> bool {
        self.old_names.contains(old)
    }
}

fn names<'a>(schema: &'a Schema, table: &'a str) -> BTreeSet<&'a str> {
    schema
        .fields(table)
        .map(|(key, _)| key.name.as_str())
        .collect()
}

fn key(table: &str, name: &str) -> ResourceKey {
    ResourceKey {
        kind: ResourceKind::Field,
        table: Some(table.to_owned()),
        name: name.to_owned(),
    }
}

fn join(names: &BTreeSet<&str>) -> String {
    names.iter().copied().collect::<Vec<_>>().join(", ")
}
