//! Objects flattened into a model: their fields are stored as the model's own fields.

use surrealdb::types::Value;

use crate::{Field, NONE, Object, Operator, Setter, ValueLike, render::Checked};

/// The names a model or object stores on its records: its own fields and those of the objects it
/// flattens. Emitted by `#[model]`.
#[doc(hidden)]
#[derive(Debug)]
pub struct StoredFields {
    own: &'static [&'static str],
    flattened: &'static [&'static StoredFields],
}

impl StoredFields {
    pub const fn new(
        own: &'static [&'static str],
        flattened: &'static [&'static StoredFields],
    ) -> Self {
        Self { own, flattened }
    }

    /// Fails compilation, when evaluated in a constant, if a flattened object stores no field —
    /// its patch would assign nothing, so a conversion error could not surface — or if two stored
    /// names are equal: renames and flattened objects must not store two values under one key.
    pub const fn assert_valid(&self) {
        let mut i = 0;
        while i < self.flattened.len() {
            if self.flattened[i].len() == 0 {
                panic!("a flattened object must store at least one field");
            }
            i += 1;
        }
        self.assert_unique();
    }

    const fn assert_unique(&self) {
        let len = self.len();
        let mut i = 0;
        while i < len {
            let mut j = i + 1;
            while j < len {
                if str_eq(self.get(i), self.get(j)) {
                    panic!(
                        "two fields are stored under the same name; check `#[surreal(rename)]`, \
                         `rename_all` and the fields of flattened objects"
                    );
                }
                j += 1;
            }
            i += 1;
        }
    }

    const fn len(&self) -> usize {
        let mut len = self.own.len();
        let mut i = 0;
        while i < self.flattened.len() {
            len += self.flattened[i].len();
            i += 1;
        }
        len
    }

    /// The name at `index`, counting own names first, then each flattened object's in order.
    const fn get(&self, index: usize) -> &'static str {
        if index < self.own.len() {
            return self.own[index];
        }
        let mut index = index - self.own.len();
        let mut i = 0;
        while i < self.flattened.len() {
            let len = self.flattened[i].len();
            if index < len {
                return self.flattened[i].get(index);
            }
            index -= len;
            i += 1;
        }
        panic!("stored field index out of range")
    }

    /// Calls `f` with every stored name, in the same order.
    pub fn for_each(&self, f: &mut impl FnMut(&'static str)) {
        self.own.iter().copied().for_each(&mut *f);
        for flattened in self.flattened {
            flattened.for_each(f);
        }
    }
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// How a partial update treats a flattened object. Its fields are the model's own, so there is no
/// single field to remove or set to `NULL`.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum FlatPatch<T> {
    /// Leave every field of the object unchanged.
    #[default]
    Keep,
    /// Assign every field of the object; a field the value omits is removed (`NONE`).
    Set(T),
}

impl<T: Object> FlatPatch<T> {
    /// One assignment per field the object owns, or none when kept.
    #[doc(hidden)]
    pub fn into_setters(self) -> Vec<Setter> {
        let FlatPatch::Set(object) = self else {
            return vec![];
        };
        // A hand-written `SurrealValue` may convert to something other than an object, which owns
        // no keys: the update then fails rather than remove every field.
        let (mut values, mut error) = match object.into_value() {
            Value::Object(object) => (object.into_inner(), None),
            other => (
                Default::default(),
                Some(format!(
                    "a flattened object must convert into an object, not {:?}",
                    other.kind()
                )),
            ),
        };
        let mut setters = vec![];
        T::STORED_FIELDS.for_each(&mut |name| {
            let mut value = values
                .remove(name)
                .map_or_else(|| ValueLike::from(NONE), ValueLike::value);
            if let Some(error) = error.take() {
                value = ValueLike::new(Checked::new(value, Some(error)));
            }
            setters.push(Setter::new(Field::new(name), Operator::Equal, value));
        });
        setters
    }
}
