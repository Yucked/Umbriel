//! Typed merges (`orm/src/query/merge.rs` and the generated `<Model>Merge`): each `Merge` value
//! renders as its row of the M1 table, nested objects merge field by field, and an empty merge is
//! a `SELECT` through `SurrealCrud::merge` but a real `MERGE {}` through the builder. Typed maps,
//! optional objects, dynamic objects and caller-defined merges cover the values outside generated
//! object merges.

use std::collections::{BTreeMap, HashMap};

use surrealdb::{
    Surreal,
    engine::local::Db,
    types::{Object, SurrealValue, Value},
};
use umbriel::{
    FlatMerge, Merge, MergeFields, MergeObject, Model, Operatable, Raw, RecordId, Runnable,
    SchemaGetter, SurrealCrud, SurrealCrudNode, model,
    statements::{Returning, update, update_only},
};
use umbriel_tests::{assert_sql, db, read};

/// A value that is not a `#[model(object)]`: its stored fields are stated by [`GeoMerge`].
#[derive(Debug, Clone, Default, SurrealValue)]
#[surreal(crate = "umbriel::types")]
pub struct Geo {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Default)]
pub struct GeoMerge {
    pub lat: Merge<f64>,
    pub lon: Merge<f64>,
}

impl MergeFields for GeoMerge {
    fn push_entries(self, object: &mut MergeObject) {
        self.lat.push_entry("lat", object);
        self.lon.push_entry("lon", object);
    }
}

#[model(node, table = "profile")]
#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub id: RecordId<Self>,
    pub scores: HashMap<String, u32>,
    pub labels: BTreeMap<String, String>,
    #[orm(nest_object = "Address")]
    pub shipping: Option<Address>,
    #[orm(ty = "object")]
    pub extra: Object,
    #[orm(ty = "object", merge = "GeoMerge")]
    pub geo: Geo,
}

fn profile() -> Profile {
    Profile {
        id: Profile::id("p"),
        scores: HashMap::from([("a".to_owned(), 1), ("b".to_owned(), 2)]),
        labels: BTreeMap::from([("x".to_owned(), "1".to_owned())]),
        shipping: Some(Address {
            street: "Main".to_owned(),
            city: "Springfield".to_owned(),
        }),
        extra: Object::from(BTreeMap::from([("k1".to_owned(), 1_i64.into_value())])),
        geo: Geo { lat: 1.0, lon: 2.0 },
    }
}

async fn merge_profile(db: &Surreal<Db>, merge: ProfileMerge) {
    update::<Profile>(Profile::id("p"))
        .merge(merge)
        .return_none()
        .execute(db)
        .await
        .expect("merged");
}

/// A partial map through `Set` keeps the stored keys it omits, so key-wise merging needs no
/// variant of its own.
#[tokio::test]
async fn a_partial_map_merges_by_key() {
    let db = db().await;
    profile().create(&db).await.expect("p");
    merge_profile(
        &db,
        ProfileMerge {
            scores: Merge::Set(HashMap::from([("b".to_owned(), 5), ("c".to_owned(), 3)])),
            labels: Merge::Set(BTreeMap::from([("y".to_owned(), "2".to_owned())])),
            ..Default::default()
        },
    )
    .await;
    let scores: BTreeMap<String, u32> = read(&db, "RETURN profile:p.scores;").await;
    assert_eq!(
        scores,
        BTreeMap::from([
            ("a".to_owned(), 1),
            ("b".to_owned(), 5),
            ("c".to_owned(), 3)
        ])
    );
    let labels: BTreeMap<String, String> = read(&db, "RETURN profile:p.labels;").await;
    assert_eq!(labels.len(), 2);
}

/// `Option<Object>` merges field by field into an existing, an absent and a null object, and
/// `Keep`, `None` and `Null` stay distinct.
#[tokio::test]
async fn an_optional_object_merges_by_field_in_every_state() {
    let db = db().await;
    profile().create(&db).await.expect("p");
    let city = |city: &str| {
        Merge::Fields(AddressMerge {
            city: Merge::Set(city.to_owned()),
            ..Default::default()
        })
    };
    let stored = |db: &Surreal<Db>| {
        let db = db.clone();
        async move { read::<Value>(&db, "RETURN profile:p.shipping;").await }
    };

    // Existing: the street survives.
    merge_profile(
        &db,
        ProfileMerge {
            shipping: city("Paris"),
            ..Default::default()
        },
    )
    .await;
    let street: String = read(&db, "RETURN profile:p.shipping.street;").await;
    assert_eq!(street, "Main");

    // Keep leaves it as it is.
    let before = stored(&db).await;
    merge_profile(&db, ProfileMerge::default()).await;
    assert_eq!(stored(&db).await, before);

    // Null stores null, and fields then merge into a new object.
    merge_profile(
        &db,
        ProfileMerge {
            shipping: Merge::Null,
            ..Default::default()
        },
    )
    .await;
    assert_eq!(stored(&db).await, Value::Null);
    merge_profile(
        &db,
        ProfileMerge {
            shipping: city("Rome"),
            ..Default::default()
        },
    )
    .await;
    let rome: String = read(&db, "RETURN profile:p.shipping.city;").await;
    assert_eq!(rome, "Rome");

    // None removes it, and fields then merge into a new object.
    merge_profile(
        &db,
        ProfileMerge {
            shipping: Merge::None,
            ..Default::default()
        },
    )
    .await;
    assert_eq!(stored(&db).await, Value::None);
    merge_profile(
        &db,
        ProfileMerge {
            shipping: city("Oslo"),
            ..Default::default()
        },
    )
    .await;
    let oslo: String = read(&db, "RETURN profile:p.shipping.city;").await;
    assert_eq!(oslo, "Oslo");
}

/// A dynamic `Object` merges recursively at run time; its keys are not known when compiling.
#[tokio::test]
async fn a_dynamic_object_merges_at_run_time() {
    let db = db().await;
    profile().create(&db).await.expect("p");
    merge_profile(
        &db,
        ProfileMerge {
            extra: Merge::Set(Object::from(BTreeMap::from([(
                "k2".to_owned(),
                2_i64.into_value(),
            )]))),
            ..Default::default()
        },
    )
    .await;
    let keys: Vec<String> = read(&db, "RETURN object::keys(profile:p.extra);").await;
    assert_eq!(keys, ["k1", "k2"]);
}

/// `#[orm(merge = "GeoMerge")]` merges a non-model struct through the caller's own field list.
#[tokio::test]
async fn a_caller_defined_merge_merges_by_field() {
    let db = db().await;
    profile().create(&db).await.expect("p");
    merge_profile(
        &db,
        ProfileMerge {
            geo: Merge::Fields(GeoMerge {
                lat: Merge::Set(9.0),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await;
    let geo: (f64, f64) = read(&db, "RETURN [profile:p.geo.lat, profile:p.geo.lon];").await;
    assert_eq!(geo, (9.0, 2.0));
}

#[model(object)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Address {
    pub street: String,
    pub city: String,
}

#[model(node, table = "customer")]
#[derive(Debug, Clone, Default)]
pub struct Customer {
    pub id: RecordId<Self>,
    pub name: String,
    #[surreal(flatten)]
    pub address: Address,
    #[orm(nest_object = "Address")]
    pub billing: Address,
    pub tags: Vec<String>,
    pub note: Option<String>,
}

fn ann() -> Customer {
    Customer {
        id: Customer::id("ann"),
        name: "Ann".to_owned(),
        address: Address {
            street: "Main".to_owned(),
            city: "Springfield".to_owned(),
        },
        billing: Address {
            street: "Oak".to_owned(),
            city: "Shelbyville".to_owned(),
        },
        tags: vec!["a".to_owned(), "b".to_owned()],
        note: Some("first".to_owned()),
    }
}

async fn stored(db: &Surreal<Db>) -> Customer {
    read(db, "SELECT * FROM ONLY customer:ann;").await
}

#[test]
fn each_value_renders_as_documented() {
    let id = Customer::id("ann");
    assert_sql(
        &update::<Customer>(&id).merge(CustomerMerge {
            name: Merge::Set("Bob".to_owned()),
            address: FlatMerge::Fields(AddressMerge {
                street: Merge::Set("Elm".to_owned()),
                ..Default::default()
            }),
            billing: Merge::Fields(AddressMerge {
                city: Merge::Set("Paris".to_owned()),
                ..Default::default()
            }),
            tags: Merge::Set(vec!["c".to_owned()]),
            note: Merge::Null,
            ..Default::default()
        }),
        "UPDATE customer:ann MERGE { 'name': 'Bob', 'street': 'Elm', \
         'billing': { 'city': 'Paris' }, 'tags': ['c'], 'note': NULL };",
    );
    assert_sql(
        &update::<Customer>(&id).merge(CustomerMerge {
            note: Merge::None,
            ..Default::default()
        }),
        "UPDATE customer:ann MERGE { 'note': NONE };",
    );
    assert_sql(
        &update::<Customer>(&id).merge(CustomerMerge {
            address: FlatMerge::Set(Address {
                street: "Elm".to_owned(),
                city: "Paris".to_owned(),
            }),
            ..Default::default()
        }),
        "UPDATE customer:ann MERGE { 'street': 'Elm', 'city': 'Paris' };",
    );
    assert_sql(
        &update::<Customer>(&id).merge(CustomerMerge::default()),
        "UPDATE customer:ann MERGE {};",
    );
    assert!(CustomerMerge::default().name.is_keep());
}

#[tokio::test]
async fn nested_fields_merge_and_arrays_replace() {
    let db = db().await;
    ann().create(&db).await.expect("ann");
    let merged = Customer::merge(
        &db,
        &Customer::id("ann"),
        CustomerMerge {
            billing: Merge::Fields(AddressMerge {
                city: Merge::Set("Paris".to_owned()),
                ..Default::default()
            }),
            address: FlatMerge::Fields(AddressMerge {
                street: Merge::Set("Elm".to_owned()),
                ..Default::default()
            }),
            tags: Merge::Set(vec!["c".to_owned()]),
            ..Default::default()
        },
    )
    .await
    .expect("merged")
    .expect("returned");
    assert_eq!(
        merged.billing,
        Address {
            street: "Oak".to_owned(),
            city: "Paris".to_owned(),
        }
    );
    assert_eq!(
        merged.address,
        Address {
            street: "Elm".to_owned(),
            city: "Springfield".to_owned(),
        }
    );
    assert_eq!(merged.tags, ["c"]);
    assert_eq!(merged.name, "Ann");
}

#[tokio::test]
async fn a_set_object_overwrites_its_declared_fields_and_keeps_others() {
    let db = db().await;
    ann().create(&db).await.expect("ann");
    Raw::new("UPDATE customer:ann SET billing.zip = '12345';")
        .run(db.clone())
        .await
        .expect("extra field");
    update::<Customer>(Customer::id("ann"))
        .merge(CustomerMerge {
            billing: Merge::Set(Address {
                street: "Elm".to_owned(),
                city: "Paris".to_owned(),
            }),
            ..Default::default()
        })
        .return_none()
        .execute(&db)
        .await
        .expect("merged");
    let zip: String = read(&db, "RETURN customer:ann.billing.zip;").await;
    assert_eq!(zip, "12345");
    assert_eq!(stored(&db).await.billing.city, "Paris");
}

#[tokio::test]
async fn null_stores_null_and_none_removes_the_field() {
    let db = db().await;
    ann().create(&db).await.expect("ann");
    update::<Customer>(Customer::id("ann"))
        .merge(CustomerMerge {
            note: Merge::Null,
            ..Default::default()
        })
        .return_none()
        .execute(&db)
        .await
        .expect("merged");
    let is_null: bool = read(&db, "RETURN customer:ann.note IS NULL;").await;
    assert!(is_null);

    update::<Customer>(Customer::id("ann"))
        .merge(CustomerMerge {
            note: Merge::None,
            ..Default::default()
        })
        .return_none()
        .execute(&db)
        .await
        .expect("merged");
    let removed: bool = read(&db, "RETURN type::is_none(customer:ann.note);").await;
    assert!(removed, "MERGE with NONE removes the field");
}

/// A field recomputed on every write, to tell a write from a read. An event would not do:
/// SurrealDB 3.3 runs no event for an update that leaves the document unchanged, as an empty
/// merge does.
async fn count_updates(db: &Surreal<Db>) {
    Raw::new("DEFINE FIELD writes ON customer VALUE ($value ?? 0) + 1;")
        .run(db.clone())
        .await
        .expect("field");
}

async fn updates(db: &Surreal<Db>) -> usize {
    read(db, "RETURN customer:ann.writes ?? 0;").await
}

#[tokio::test]
async fn an_empty_merge_through_the_helper_reads_without_writing() {
    let db = db().await;
    ann().create(&db).await.expect("ann");
    count_updates(&db).await;
    let read_back = Customer::merge(&db, &Customer::id("ann"), CustomerMerge::default())
        .await
        .expect("read")
        .expect("returned");
    assert_eq!(read_back.name, "Ann");
    assert_eq!(updates(&db).await, 0);
}

#[tokio::test]
async fn an_empty_merge_through_the_builder_is_a_real_write() {
    let db = db().await;
    ann().create(&db).await.expect("ann");
    count_updates(&db).await;
    let schema = Customer::schema();

    let () = update_only::<Customer>(Customer::id("ann"))
        .merge(CustomerMerge::default())
        .return_none()
        .execute(&db)
        .await
        .expect("merged");
    assert_eq!(updates(&db).await, 1);

    // Its `WHERE` applies as written: an unmatched record is not written.
    let unmatched = update::<Customer>(Customer::id("ann"))
        .merge(CustomerMerge::default())
        .where_(schema.name.equal("Bob"))
        .execute(&db)
        .await
        .expect("ran");
    assert!(unmatched.is_empty());
    assert_eq!(updates(&db).await, 1);
    assert_eq!(stored(&db).await.name, "Ann");
}
