//! Fields (`orm/src/query/field.rs`): the idiom parts SurrealDB 3.3 adds (`.?`, `...` and
//! destructuring), rendered, parsed and evaluated.

use std::collections::BTreeMap;

use surrealdb::{Surreal, engine::local::Db, types::Value};
use umbriel::{
    All, Destructure, Field, Raw, Render, Runnable, Table,
    statements::{Returning, select, select_value},
};
use umbriel_tests::*;

async fn seeded() -> Surreal<Db> {
    let db = db().await;
    Raw::new(
        "CREATE profile:alice SET name = 'Alice', address = { city: 'Lisbon', zip: '1000' }, \
             groups = [[1, 2], [3]], \
             items = [{ name: 'pen', qty: 2, colour: 'red' }, { name: 'ink', qty: 1, colour: 'blue' }], \
             opts = { security: 'high', enabled: true, theme: 'dark' }; \
         CREATE profile:bob SET name = 'Bob';",
    )
    .run(db.clone())
    .await
    .expect("seeded");
    db
}

mod rendering {
    use super::*;

    #[test]
    fn optional_and_flatten() {
        let city = Field::new("address").optional().with_field("city");
        assert_eq!(city.to_debug_sql(), "address.?.city");
        assert_parses(&format!("SELECT {} FROM profile;", city.to_debug_sql()));

        let flat = Field::new("groups").flatten();
        assert_eq!(flat.to_debug_sql(), "groups...");
        assert_parses(&format!("SELECT {} FROM profile;", flat.to_debug_sql()));
    }

    #[test]
    fn destructuring_with_every_part_kind() {
        let shape = Field::new("address").destructure([
            Destructure::from("city"),
            Destructure::aliased("postcode", Field::new("zip")),
            Destructure::all("geo"),
            Destructure::nested("meta", ["created", "updated"]),
        ]);
        assert_sql(
            &select(shape).from(Table::from("profile")),
            "SELECT address.{ city, postcode: zip, geo.*, meta.{ created, updated } } FROM profile;",
        );
    }

    #[test]
    fn destructured_omit() {
        assert_sql(
            &select(All)
                .omit(Field::new("opts").destructure(["security", "enabled"]))
                .from(Table::from("profile")),
            "SELECT * OMIT opts.{ security, enabled } FROM profile;",
        );
    }
}

mod evaluation {
    use super::*;

    #[tokio::test]
    async fn optional_reads_none_where_the_value_is_missing() {
        let db = seeded().await;
        let cities: Vec<Option<String>> =
            select_value(Field::new("address").optional().with_field("city"))
                .from(Table::from("profile"))
                .decode()
                .execute(&db)
                .await
                .expect("read");
        assert_eq!(cities, [Some("Lisbon".to_owned()), None]);
    }

    #[tokio::test]
    async fn flatten_joins_nested_arrays() {
        let db = seeded().await;
        let flat: Vec<Vec<i64>> = select_value(Field::new("groups").flatten())
            .from(umbriel::types::RecordId::new("profile", "alice"))
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(flat, [vec![1, 2, 3]]);
    }

    #[tokio::test]
    async fn destructuring_keeps_each_item_with_its_parts() {
        let db = seeded().await;
        let items: Vec<Vec<BTreeMap<String, Value>>> =
            select_value(Field::new("items").destructure(["name", "qty"]))
                .from(umbriel::types::RecordId::new("profile", "alice"))
                .decode()
                .execute(&db)
                .await
                .expect("read");
        let names = items[0]
            .iter()
            .map(|item| item.keys().cloned().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(names, [["name", "qty"], ["name", "qty"]]);
        assert_eq!(items[0][0].get("name"), Some(&Value::String("pen".to_owned())));
    }

    #[tokio::test]
    async fn omitting_destructured_parts_removes_each() {
        let db = seeded().await;
        let rows: Vec<BTreeMap<String, Value>> = select(All)
            .omit(Field::new("opts").destructure(["security", "enabled"]))
            .from(umbriel::types::RecordId::new("profile", "alice"))
            .decode()
            .execute(&db)
            .await
            .expect("read");
        let theme_only: Value = read(&db, "RETURN { theme: 'dark' };").await;
        assert_eq!(rows[0].get("opts"), Some(&theme_only));
    }

    /// T21 verification: `.*` on a plain object gives the object itself, unchanged; only record
    /// ids are fetched (3.3.4 `exec/parts/array_ops.rs` `evaluate_all`, and the legacy
    /// `value/get.rs`).
    #[tokio::test]
    async fn all_fields_of_an_object() {
        let db = seeded().await;
        let values: Vec<Value> = select_value(Field::new("address").all())
            .from(umbriel::types::RecordId::new("profile", "alice"))
            .execute(&db)
            .await
            .expect("read");
        let expected: Value = read(&db, "RETURN { city: 'Lisbon', zip: '1000' };").await;
        assert_eq!(values, [expected]);
    }
}
