use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};
use umbriel::{
    Nullable, RecordId, model,
    types::{self, SurrealValue},
};

use crate::User;

/// One field per SurrealQL value kind. Fields without `ty` have their stored type inferred from
/// the Rust type; the others need an explicit one.
#[model(node, table = "value_kind")]
#[derive(Debug, Clone, PartialEq)]
pub struct ValueKind {
    pub id: RecordId<Self>,
    pub flag: bool,
    pub count: i64,
    pub ratio: f64,
    pub price: types::Decimal,
    pub amount: types::Number,
    pub label: String,
    pub created: DateTime<Utc>,
    pub elapsed: Duration,
    pub token: types::Uuid,
    pub payload: types::Bytes,
    pub members: types::Set,
    pub scores: BTreeMap<String, i64>,
    pub window: types::Range,
    /// A table field names its tables: SurrealDB 3.3.4 refuses table values in a plain `table`.
    #[orm(ty = "table<user>")]
    pub source: types::Table,
    /// `types::Regex` does not implement `SurrealValue`, so a regex is held as a `Value`.
    #[orm(ty = "regex")]
    pub pattern: types::Value,
    pub triple: [i64; 3],
    pub point: geo::Point,
    pub line: geo::LineString,
    pub polygon: geo::Polygon,
    pub points: geo::MultiPoint,
    pub lines: geo::MultiLineString,
    pub polygons: geo::MultiPolygon,
    /// `geo::GeometryCollection` does not implement `SurrealValue`.
    #[orm(ty = "geometry<collection>")]
    pub collection: types::Geometry,
    pub maybe: Option<i64>,
    pub nullable: Nullable<i64>,
    #[orm(ty = "'draft' | 'sent'")]
    pub status: MessageStatus,
    #[orm(ty = "1 | 2 | 3")]
    pub level: i64,
    #[orm(ty = "{ kind: 'a', n: int }")]
    pub shape: types::Value,
    #[orm(ty = "int | string")]
    pub either: types::Value,
    #[orm(ty = "option<int | string>")]
    pub maybe_either: Option<types::Value>,
    pub owner: RecordId<User>,
    pub reviewer: Option<RecordId<User>>,
    pub watchers: Vec<RecordId<User>>,
}

/// A message's status, stored as its name.
#[derive(Debug, Clone, Copy, PartialEq, SurrealValue)]
// Separate attributes: the derive's enum options stop at the first key they don't know.
#[surreal(crate = "umbriel::types")]
#[surreal(untagged)]
pub enum MessageStatus {
    #[surreal(rename = "draft")]
    Draft,
    #[surreal(rename = "sent")]
    Sent,
}
