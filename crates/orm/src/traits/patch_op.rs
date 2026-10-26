use std::ops::Deref;

use surrealdb::types::{SurrealValue, Value};

use crate::{Field, Render, Renderer};

const INVALID_PATH: &str = "The path you have provided is invalid. Make sure that there are no clauses or conditions included. Valid path include e.g name, name(E).first, name(E).first(E).second, etc.";

/// A JSON patch operation, e.g. `{ op: 'add', path: '/name', value: 'Alice' }`.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchOp {
    op: &'static str,
    /// The JSON pointer to the field, or `None` when the field path has no pointer.
    path: Option<Value>,
    /// For `copy` and `move`, the JSON pointer to the source field, `None` when it has none.
    from: Option<Option<Value>>,
    value: Option<Value>,
}

fn pointer(field: &Field) -> Option<Value> {
    field.json_pointer().map(Value::String)
}

fn push_pointer(out: &mut Renderer, pointer: Option<&Value>) {
    match pointer {
        Some(pointer) => out.push_value(pointer),
        None => out.push_error(INVALID_PATH).push_value(&Value::None),
    };
}

impl PatchOp {
    fn new(path: &Field, op: &'static str, value: Option<Value>) -> Self {
        Self {
            op,
            path: pointer(path),
            from: None,
            value,
        }
    }

    fn from_source(path: &Field, op: &'static str, source: &Field) -> Self {
        Self {
            op,
            path: pointer(path),
            from: Some(pointer(source)),
            value: None,
        }
    }
}

impl Render for PatchOp {
    fn render(&self, out: &mut Renderer) {
        out.push_str("{ op: '")
            .push_str(self.op)
            .push_str("', path: ");
        push_pointer(out, self.path.as_ref());
        if let Some(from) = &self.from {
            out.push_str(", from: ");
            push_pointer(out, from.as_ref());
        }
        if let Some(value) = &self.value {
            out.push_str(", value: ").push_value(value);
        }
        out.push_str(" }");
    }
}

/// Why a [`PatchOp`] has no SDK [`surrealdb::opt::PatchOp`] equivalent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PatchOpConversionError {
    /// The SDK offers only `add`, `remove`, `replace` and `change`, not this operation.
    #[error("the JSON Patch `{0}` operation has no SDK equivalent")]
    Unsupported(&'static str),
    /// The operation's field path has no JSON pointer.
    #[error("the JSON Patch operation's field path has no JSON pointer")]
    InvalidPath,
}

/// Converts to the SDK's operation, for its `patch` method. Fails for `copy`, `move` and `test`,
/// which the SDK does not offer, and for a field path without a JSON pointer.
impl TryFrom<PatchOp> for surrealdb::opt::PatchOp {
    type Error = PatchOpConversionError;

    fn try_from(op: PatchOp) -> Result<Self, Self::Error> {
        let Some(Value::String(path)) = op.path else {
            return Err(PatchOpConversionError::InvalidPath);
        };
        let value = op.value.unwrap_or(Value::None);
        match op.op {
            "add" => Ok(Self::Add { path, value }),
            "remove" => Ok(Self::Remove { path }),
            "replace" => Ok(Self::Replace { path, value }),
            "change" => match value {
                Value::String(value) => Ok(Self::Change { path, value }),
                _ => Err(PatchOpConversionError::Unsupported("change")),
            },
            other => Err(PatchOpConversionError::Unsupported(other)),
        }
    }
}

/// Converts the SDK's operation, so it can be used in an `UPDATE … PATCH` statement.
impl From<surrealdb::opt::PatchOp> for PatchOp {
    fn from(op: surrealdb::opt::PatchOp) -> Self {
        let (op, path, value) = match op {
            surrealdb::opt::PatchOp::Add { path, value } => ("add", path, value),
            surrealdb::opt::PatchOp::Remove { path } => ("remove", path, Value::None),
            surrealdb::opt::PatchOp::Replace { path, value } => ("replace", path, value),
            surrealdb::opt::PatchOp::Change { path, value } => {
                ("change", path, Value::String(value))
            }
        };
        Self {
            op,
            path: Some(Value::String(path)),
            from: None,
            value: (op != "remove").then_some(value),
        }
    }
}

impl From<PatchOp> for Vec<PatchOp> {
    fn from(patch_op: PatchOp) -> Self {
        vec![patch_op]
    }
}

/// JSON Patch operations on a field, for `UPDATE … PATCH`. The field is the operation's path,
/// written as a JSON pointer: `name` becomes `/name` and `name.first` becomes `/name/first`. A
/// path with an index or filter has no pointer and fails when the query is rendered.
///
/// # Examples
///
/// ```
/// # use umbriel::*;
/// # struct Name(Field);
/// # impl std::ops::Deref for Name { type Target = Field; fn deref(&self) -> &Field { &self.0 } }
/// # impl Patchable<String> for Name {}
/// # let name = Name(Field::new("name"));
/// assert_eq!(
///     name.patch_add("Charlie").to_debug_sql(),
///     "{ op: 'add', path: '/name', value: 'Charlie' }"
/// );
/// assert_eq!(name.patch_remove().to_debug_sql(), "{ op: 'remove', path: '/name' }");
/// assert_eq!(
///     name.patch_copy(Field::new("nickname")).to_debug_sql(),
///     "{ op: 'copy', path: '/name', from: '/nickname' }"
/// );
/// assert_eq!(
///     name.patch_test("Alice").to_debug_sql(),
///     "{ op: 'test', path: '/name', value: 'Alice' }"
/// );
/// ```
pub trait Patchable<T: SurrealValue>
where
    Self: Deref<Target = Field>,
{
    /// `add`: sets the field, or inserts into an array at the path.
    fn patch_add(&self, value: impl Into<T>) -> PatchOp {
        PatchOp::new(self.deref(), "add", Some(value.into().into_value()))
    }

    /// `remove`: removes the field.
    fn patch_remove(&self) -> PatchOp {
        PatchOp::new(self.deref(), "remove", None)
    }

    /// `replace`: replaces the field's value, which must exist.
    fn patch_replace(&self, value: impl Into<T>) -> PatchOp {
        PatchOp::new(self.deref(), "replace", Some(value.into().into_value()))
    }

    /// `change`: edits a string field with a text diff in the `diff-match-patch` patch format;
    /// `"@@ -1,4 +1,4 @@\n te\n-s\n+x\n t\n"` changes "test" to "text".
    fn patch_change(&self, diff: &str) -> PatchOp {
        PatchOp::new(self.deref(), "change", Some(Value::String(diff.to_owned())))
    }

    /// `copy`: copies the value at `source` to this field.
    fn patch_copy(&self, source: impl Into<Field>) -> PatchOp {
        PatchOp::from_source(self.deref(), "copy", &source.into())
    }

    /// `move`: moves the value at `source` to this field, removing it from `source`.
    fn patch_move(&self, source: impl Into<Field>) -> PatchOp {
        PatchOp::from_source(self.deref(), "move", &source.into())
    }

    /// `test`: the patch, and so the statement, fails unless this field holds `value`. SurrealDB
    /// reports the failure as "The JSON Patch contains invalid operations".
    fn patch_test(&self, value: impl Into<T>) -> PatchOp {
        PatchOp::new(self.deref(), "test", Some(value.into().into_value()))
    }

    /// Derefs to field
    fn to_field(&self) -> Field {
        self.deref().clone()
    }
}
