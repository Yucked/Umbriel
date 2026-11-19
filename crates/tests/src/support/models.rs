//! What the model test files share: the names they import, and the fixture models and records
//! they use. They import it with `use umbriel_tests::support::models::*;`, so each file uses only
//! what it needs.

pub use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

pub use chrono::{DateTime, Utc};
pub use surrealdb::{Surreal, engine::any::Any};
pub use umbriel::{
    CrudType::*,
    E, Edge, Field, FlatPatch, LinkMany, LinkOne, LinkSelf, Model, ModelPatch, NONE, Node,
    Nullable, Operatable, Patch, Patchable, Raw, RecordId, RecordRange, Reference, Relate, Render,
    Runnable, SchemaGetter, SetterArray, SetterAssignable, SetterNumeric, SurrealCrud,
    SurrealCrudNode, SurrealRead, cond, model,
    statements::{Returning, define_field, define_table, for_permission, relate},
    types::{self, Kind, RecordIdKey, SurrealValue, Value},
    value,
};
use crate::*;

/// The definitions of `field`, in order.
pub fn definitions_of(definitions: &[Raw], field: &str) -> Vec<String> {
    let prefix = format!("DEFINE FIELD {field} ");
    let item_prefix = format!("DEFINE FIELD {field}.* ");
    definitions
        .iter()
        .map(Raw::as_str)
        .filter(|definition| {
            definition.starts_with(&prefix) || definition.starts_with(&item_prefix)
        })
        .map(str::to_owned)
        .collect()
}

pub fn sorted(fields: Vec<Field>) -> Vec<String> {
    let mut fields = fields.iter().map(Render::to_debug_sql).collect::<Vec<_>>();
    fields.sort();
    fields
}

pub fn line_string() -> geo::LineString {
    geo::LineString::from(vec![(0.0, 0.0), (1.0, 1.0)])
}

pub fn polygon() -> geo::Polygon {
    geo::Polygon::new(
        geo::LineString::from(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]),
        vec![],
    )
}

pub fn alien(key: &str, name: &str) -> Alien {
    Alien {
        id: Alien::id(key),
        name: name.to_owned(),
        age: 30,
        created: DateTime::<Utc>::UNIX_EPOCH,
        life_expectancy: Duration::from_secs(60),
        line_string: line_string(),
        territory_area: polygon(),
        home: geo::Point::new(1.0, 2.0),
        tags: vec!["friendly".to_owned()],
        ally: LinkSelf::null(),
        weapon: LinkOne::null(),
        space_ships: LinkMany::null(),
        planets_to_visit: Relate::null(),
    }
}

pub fn space_ship(key: &str, name: &str) -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id(key),
        name: name.to_owned(),
        created: DateTime::<Utc>::UNIX_EPOCH,
    }
}

pub fn weapon(key: i64, strength: f64) -> Weapon {
    Weapon {
        id: Weapon::id(key),
        name: format!("Weapon {key}"),
        strength,
        ..Default::default()
    }
}

/// A node with optional, nullable, set and map fields.
#[model(node, table = "profile")]
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: RecordId<Self>,
    pub name: String,
    pub nickname: Option<String>,
    pub bio: Option<Nullable<String>>,
    pub scores: HashSet<i64>,
    pub settings: std::collections::BTreeMap<String, i64>,
}

pub fn profile(key: &str, nickname: Option<&str>, bio: Option<Nullable<String>>) -> Profile {
    Profile {
        id: Profile::id(key),
        name: "Alice".to_owned(),
        nickname: nickname.map(str::to_owned),
        bio,
        scores: HashSet::from([1, 2]),
        settings: Default::default(),
    }
}

/// An object stored as fields of the model that flattens it.
#[model(object)]
#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    pub street: String,
    pub city: String,
}

/// A generic model whose parameter has the name the field generator would otherwise give its
/// arithmetic operand: the generated name must avoid it.
#[model(node, table = "counter")]
#[derive(Debug, Clone)]
pub struct Counter<__UmbrielOperand: SurrealValue + umbriel::validators::Int + 'static> {
    pub id: RecordId<Self>,
    #[orm(ty = "int")]
    pub step: __UmbrielOperand,
    pub total: u32,
}

#[model(node, table = "customer")]
#[derive(Debug, Clone)]
pub struct Customer {
    pub id: RecordId<Self>,
    pub name: String,
    #[surreal(flatten)]
    pub address: Address,
}

/// A node whose `id` has an assertion and a default but no key type.
#[model(node, table = "ticket")]
#[derive(Debug, Clone)]
pub struct Ticket {
    #[orm(assert = value().not_equal(Ticket::id("blocked")), value = "first")]
    pub id: RecordId<Self>,
    pub title: String,
}

/// An embedded object whose fields may be absent.
#[model(object)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Offsets {
    pub x: Option<i64>,
    pub y: Option<i64>,
}

/// A node holding an embedded object, for whole-object patches.
#[model(node, table = "marker")]
#[derive(Debug, Clone)]
pub struct Marker {
    pub id: RecordId<Self>,
    #[orm(nest_object = "Offsets")]
    pub offsets: Offsets,
}

/// A plan's tier, stored as its name.
#[derive(Debug, Clone, Copy, Default, PartialEq, SurrealValue)]
// Separate attributes: the derive's enum options stop at the first key they don't know.
#[surreal(crate = "umbriel::types")]
#[surreal(untagged)]
pub enum Tier {
    // `rename` rather than `value`: the derive writes `value` variants with a hardcoded
    // `surrealdb_types::` path that ignores `crate`.
    #[default]
    #[surreal(rename = "free")]
    Free,
    #[surreal(rename = "pro")]
    Pro,
}

/// A node with a stored enum.
#[model(node, table = "plan")]
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub id: RecordId<Self>,
    #[orm(ty = "'free' | 'pro'")]
    pub tier: Tier,
}

/// A node whose conversion is written by hand (`value = manual`) instead of derived.
#[model(node, table = "thermostat", value = manual)]
#[derive(Debug, Clone, PartialEq)]
pub struct Thermostat {
    pub id: RecordId<Self>,
    pub tenths: i64,
}

impl SurrealValue for Thermostat {
    fn kind_of() -> Kind {
        Kind::Object
    }

    fn into_value(self) -> Value {
        let mut object = types::Object::new();
        object.insert("id", self.id);
        object.insert("tenths", self.tenths);
        Value::Object(object)
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        let Value::Object(mut object) = value else {
            return Err(types::Error::internal(format!(
                "expected a thermostat object, got {value:?}"
            )));
        };
        let mut field = |name: &str| object.remove(name).unwrap_or(Value::None);
        Ok(Self {
            id: RecordId::from_value(field("id"))?,
            tenths: i64::from_value(field("tenths"))?,
        })
    }
}

/// A node keyed by arrays: `ty` on `id` is the key's type, not the field's.
#[model(node, table = "reading")]
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    #[orm(ty = "array")]
    pub id: RecordId<Self>,
    pub celsius: f64,
}

