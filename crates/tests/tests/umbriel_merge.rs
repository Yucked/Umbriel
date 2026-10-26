//! Typed merges (`orm/src/query/merge.rs` and the generated `<Model>Merge`): each `Merge` value
//! renders as its row of the M1 table, nested objects merge field by field, and an empty merge is
//! a `SELECT` through `SurrealCrud::merge` but a real `MERGE {}` through the builder.

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    FlatMerge, Merge, Model, Operatable, Raw, RecordId, Runnable, SchemaGetter, SurrealCrud,
    SurrealCrudNode, model,
    statements::{Returning, update, update_only},
};
use umbriel_tests::{assert_sql, db, read};

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
