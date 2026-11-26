use std::marker::PhantomData;

use surrealdb::types::{self, Kind, SurrealValue, Value};

use crate::{Field, Model, Node, RecordId, Render, Renderer};

/// A typed path through record links, from a `From` record to the `To` record its last link
/// reaches: one link field per hop.
///
/// Paths come from [`Links::links`](crate::Links::links), which `#[model]` generates with one
/// path per `LinkOne`, `LinkSelf` and `LinkMany` field, under the field's stored name.
/// [`then`](Self::then) adds a hop, and
/// [`SelectStatement::fetch_link`](crate::statements::SelectStatement::fetch_link) fetches every
/// record along the path while keeping the statement's model type.
///
/// A path holds only link fields, so it never holds a graph step, which `FETCH` skips.
pub struct LinkPath<From, To> {
    field: Field,
    hops: PhantomData<fn() -> (From, To)>,
}

impl<From, To> LinkPath<From, To> {
    /// The one-hop path over the link stored as `name`. Only `#[model]` builds paths, through
    /// `__private::link_path`, so a path always follows a real link field.
    pub(crate) fn new(name: &'static str) -> Self {
        Self {
            field: Field::new(name),
            hops: PhantomData,
        }
    }

    /// This path followed by `next`, which must start at the record this path reaches:
    /// `Member::links().wallet.then(Wallet::links().bank)` is `wallet.bank`.
    #[must_use]
    pub fn then<W>(mut self, next: LinkPath<To, W>) -> LinkPath<From, W> {
        self.field.extend(next.field);
        LinkPath {
            field: self.field,
            hops: PhantomData,
        }
    }

    pub(crate) fn into_field(self) -> Field {
        self.field
    }
}

// Manual impls avoid derive's bounds on `From` and `To`, which the path never holds.
impl<From, To> std::fmt::Debug for LinkPath<From, To> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LinkPath").field(&self.field).finish()
    }
}

impl<From, To> Clone for LinkPath<From, To> {
    fn clone(&self) -> Self {
        Self {
            field: self.field.clone(),
            hops: PhantomData,
        }
    }
}

impl<From, To> Render for LinkPath<From, To> {
    fn render(&self, out: &mut Renderer) {
        self.field.render(out);
    }
}

/// A reference to a foreign node: its id, the fetched record, or nothing.
#[derive(Debug, Clone, Default)]
pub enum Reference<V: Model> {
    /// The id of the foreign node, when the link is not fetched.
    Id(RecordId<V>),
    /// The fetched foreign node.
    FetchedValue(V),
    /// No linked node; the default.
    #[default]
    Null,
}

impl<V: Node> Reference<V> {
    /// A reference to `model` by its id.
    pub fn from_model(model: &V) -> Self {
        Self::Id(model.record_id().clone())
    }

    /// returns the id of the foreign node if it is not fetched
    pub fn get_id(&self) -> Option<&RecordId<V>> {
        match self {
            Self::Id(id) => Some(id),
            _ => None,
        }
    }

    /// The id of the foreign node, fetched or not.
    pub fn key(&self) -> Option<&RecordId<V>> {
        match self {
            Self::Id(id) => Some(id),
            Self::FetchedValue(value) => Some(value.record_id()),
            Self::Null => None,
        }
    }

    /// returns a referenced value of the foreign node if it exists and has been fetched
    pub fn value(&self) -> Option<&V> {
        match self {
            Self::FetchedValue(v) => Some(v),
            _ => None,
        }
    }

    /// returns an owned value of the foreign node if it exists and has been fetched
    pub fn value_owned(self) -> Option<V> {
        match self {
            Self::FetchedValue(v) => Some(v),
            _ => None,
        }
    }
}

/// Stored as the record id; read back as an id, a fetched record, or nothing.
impl<V: Node> SurrealValue for Reference<V> {
    fn kind_of() -> Kind {
        Kind::option(RecordId::<V>::kind_of())
    }

    fn is_value(value: &Value) -> bool {
        matches!(value, Value::None | Value::Null)
            || RecordId::<V>::is_value(value)
            || V::is_value(value)
    }

    fn into_value(self) -> Value {
        match self {
            Self::Id(id) => id.into_value(),
            Self::FetchedValue(value) => value.record_id().clone().into_value(),
            Self::Null => Value::None,
        }
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        match value {
            Value::None | Value::Null => Ok(Self::Null),
            Value::RecordId(id) => RecordId::try_from(id).map(Self::Id),
            value => V::from_value(value).map(Self::FetchedValue),
        }
    }
}

macro_rules! single_link {
    ($(#[$attr:meta])* $name:ident($inner:ty), $wrap:expr, $unwrap:expr) => {
        $(#[$attr])*
        #[derive(Debug, Clone)]
        pub struct $name<V: Node>($inner);

        impl<V: Node> $name<V> {
            /// No linked node.
            pub fn null() -> Self {
                Self($wrap(Reference::Null))
            }

            /// The linked node's id, fetched or not.
            pub fn key(&self) -> Option<&RecordId<V>> {
                self.0.key()
            }
        }

        // A manual impl avoids derive's `V: Default` bound.
        impl<V: Node> Default for $name<V> {
            fn default() -> Self {
                Self::null()
            }
        }

        impl<V: Node> ::std::ops::Deref for $name<V> {
            type Target = Reference<V>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<V: Node> ::std::ops::DerefMut for $name<V> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl<V: Node> From<Reference<V>> for $name<V> {
            fn from(reference: Reference<V>) -> Self {
                Self($wrap(reference))
            }
        }

        impl<V: Node> From<$name<V>> for Reference<V> {
            fn from(link: $name<V>) -> Self {
                $unwrap(link.0)
            }
        }

        impl<V: Node> From<RecordId<V>> for $name<V> {
            fn from(id: RecordId<V>) -> Self {
                Self($wrap(Reference::Id(id)))
            }
        }

        impl<V: Node> From<V> for $name<V> {
            fn from(model: V) -> Self {
                Self($wrap(Reference::from_model(&model)))
            }
        }

        impl<V: Node> From<&V> for $name<V> {
            fn from(model: &V) -> Self {
                Self($wrap(Reference::from_model(model)))
            }
        }

        impl<V: Node> From<$name<V>> for Option<RecordId<V>> {
            fn from(link: $name<V>) -> Self {
                link.key().cloned()
            }
        }

        impl<V: Node> SurrealValue for $name<V> {
            fn kind_of() -> Kind {
                Reference::<V>::kind_of()
            }

            fn is_value(value: &Value) -> bool {
                Reference::<V>::is_value(value)
            }

            fn into_value(self) -> Value {
                $unwrap(self.0).into_value()
            }

            fn from_value(value: Value) -> Result<Self, types::Error> {
                Reference::from_value(value).map(|reference| Self($wrap(reference)))
            }
        }
    };
}

single_link!(
    /// A reference to a foreign node which can either be an ID or a fetched value itself or null.
    LinkOne(Reference<V>),
    std::convert::identity,
    std::convert::identity
);

single_link!(
    /// A reference to the current struct as a foreign node in a one-to-one relationship.
    /// Like `LinkOne`, but boxed to allow recursive models.
    LinkSelf(Box<Reference<V>>),
    Box::new,
    |reference: Box<Reference<V>>| *reference
);

impl<V: Node> From<Box<V>> for LinkSelf<V> {
    fn from(model: Box<V>) -> Self {
        Self(Box::new(Reference::from_model(&model)))
    }
}

macro_rules! many_links {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone)]
        pub struct $name<V: Node>(Vec<Reference<V>>);

        impl<V: Node> $name<V> {
            /// Returns an empty list
            pub fn null() -> Self {
                Self(vec![])
            }

            /// Returns the number of values that are fetched and available and not null
            pub fn values_truthy_count(&self) -> usize {
                self.0
                    .iter()
                    .filter(|m| matches!(m, Reference::FetchedValue(_)))
                    .count()
            }

            /// Returns the values that are fetched and available and not null
            pub fn values_truthy(&self) -> Vec<&V> {
                self.0.iter().filter_map(Reference::value).collect()
            }

            /// Returns just the fully fetched values if fetched and available, otherwise, None
            pub fn values(&self) -> Vec<Option<&V>> {
                self.0.iter().map(Reference::value).collect()
            }

            /// Returns just the keys of the foreign field. Some links may not exist
            pub fn keys(&self) -> Vec<Option<&RecordId<V>>> {
                self.0.iter().map(Reference::key).collect()
            }

            /// Returns only the keys that are non-None ids.
            /// It does not check if the ids actually exist in the
            /// foreign table.
            pub fn keys_truthy(&self) -> Vec<&RecordId<V>> {
                self.0.iter().filter_map(Reference::key).collect()
            }

            /// Returns only the keys that exist if foreign links are fetched
            /// and available.
            pub fn keys_checked(&self) -> Vec<&RecordId<V>> {
                self.0
                    .iter()
                    .filter_map(|m| m.value().map(Model::record_id))
                    .collect()
            }
        }

        // A manual impl avoids derive's `V: Default` bound.
        impl<V: Node> Default for $name<V> {
            fn default() -> Self {
                Self::null()
            }
        }

        impl<V: Node> ::std::ops::Deref for $name<V> {
            type Target = Vec<Reference<V>>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<V: Node> ::std::ops::DerefMut for $name<V> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl<V: Node> From<Vec<Reference<V>>> for $name<V> {
            fn from(references: Vec<Reference<V>>) -> Self {
                Self(references)
            }
        }

        impl<V: Node> From<$name<V>> for Vec<Reference<V>> {
            fn from(links: $name<V>) -> Self {
                links.0
            }
        }

        impl<V: Node> IntoIterator for $name<V> {
            type Item = Reference<V>;
            type IntoIter = std::vec::IntoIter<Self::Item>;

            fn into_iter(self) -> Self::IntoIter {
                self.0.into_iter()
            }
        }
    };
}

many_links!(
    /// Reference to a foreign node in a simple direct one-to-many relationship
    /// Returns either the foreign values if fetched, id keys of the foreign Field if not fetched,
    /// empty Vec if not available
    LinkMany
);

many_links!(
    /// reference to a foreign node in a many-to-many relationship via an edge
    /// This is not stored in the database and is merely a readonly field
    /// derived from the edge referencing the foreign table.
    Relate
);

impl<V: Node> From<Vec<V>> for LinkMany<V> {
    fn from(models: Vec<V>) -> Self {
        Self(models.iter().map(Reference::from_model).collect())
    }
}

impl<V: Node> From<Vec<RecordId<V>>> for LinkMany<V> {
    fn from(ids: Vec<RecordId<V>>) -> Self {
        Self(ids.into_iter().map(Reference::Id).collect())
    }
}

impl<V: Node> SurrealValue for LinkMany<V> {
    fn kind_of() -> Kind {
        Kind::Array(Box::new(Reference::<V>::kind_of()), None)
    }

    fn is_value(value: &Value) -> bool {
        Vec::<Reference<V>>::is_value(value)
    }

    fn into_value(self) -> Value {
        self.0.into_value()
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        match value {
            Value::None | Value::Null => Ok(Self::null()),
            value => Vec::from_value(value).map(Self),
        }
    }
}

/// A computed traversal, never stored: it writes `NONE`, which SurrealDB drops, and reads the
/// traversal's result.
impl<V: Node> SurrealValue for Relate<V> {
    fn kind_of() -> Kind {
        Kind::option(Kind::Array(Box::new(Reference::<V>::kind_of()), None))
    }

    fn is_value(value: &Value) -> bool {
        matches!(value, Value::None | Value::Null) || Vec::<Reference<V>>::is_value(value)
    }

    fn into_value(self) -> Value {
        Value::None
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        match value {
            Value::None | Value::Null => Ok(Self::null()),
            value => Vec::from_value(value).map(Self),
        }
    }
}
