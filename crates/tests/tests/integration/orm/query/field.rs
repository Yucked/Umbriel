//! Fields (`orm/src/query/field.rs`): the idiom parts SurrealDB 3.3 adds (`.?`, `...` and
//! destructuring), rendered, parsed and evaluated.

use std::collections::BTreeMap;

use surrealdb::{Surreal, engine::any::Any, types::Value};
use umbriel::{
    All, Destructure, Field, Raw, Render, Runnable, Table,
    statements::{Returning, select, select_value},
};
use umbriel_tests::{support::query::*, *};

async fn seeded() -> (Surreal<Any>, TestDb) {
    let (db, guard) = db().await;
    Raw::new(
        "CREATE profile:alice SET name = 'Alice', address = { city: 'Lisbon', zip: '1000' }, \
             groups = [[1, 2], [3]], \
             items = [{ name: 'pen', qty: 2, colour: 'red' }, { name: 'ink', qty: 1, colour: 'blue' }], \
             opts = { security: 'high', enabled: true, theme: 'dark' }; \
         CREATE profile:bob SET name = 'Bob';",
    )
    .run(&db)
    .await
    .expect("seeded");
    (db, guard)
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
        let (db, _guard) = seeded().await;
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
        let (db, _guard) = seeded().await;
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
        let (db, _guard) = seeded().await;
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
        assert_eq!(
            items[0][0].get("name"),
            Some(&Value::String("pen".to_owned()))
        );
    }

    #[tokio::test]
    async fn omitting_destructured_parts_removes_each() {
        let (db, _guard) = seeded().await;
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
        let (db, _guard) = seeded().await;
        let values: Vec<Value> = select_value(Field::new("address").all())
            .from(umbriel::types::RecordId::new("profile", "alice"))
            .execute(&db)
            .await
            .expect("read");
        let expected: Value = read(&db, "RETURN { city: 'Lisbon', zip: '1000' };").await;
        assert_eq!(values, [expected]);
    }
}

mod fields {
    use super::*;

    #[test]
    fn names_and_paths() {
        assert_eq!(Field::new("name").to_debug_sql(), "name");
        assert_eq!(
            Field::new("address").with_field("city").to_debug_sql(),
            "address.city"
        );
        assert_eq!(Field::new("display-name").to_debug_sql(), "`display-name`");
        assert_eq!(Field::from("name").to_debug_sql(), "name");
        assert_eq!(Field::new("tags").all().to_debug_sql(), "tags.*");
        assert_eq!(
            Field::from(Param::new("user")).all().to_debug_sql(),
            "$user.*"
        );
        assert_eq!(
            Field::from_expr(ValueLike::from(User::id("alice"))).to_debug_sql(),
            "user:alice"
        );
        assert!(Field::new("").to_query().is_err());
    }

    /// T28: a name is always one escaped identifier, whatever it holds, and verbatim idiom text
    /// is the tracked `Field::raw`.
    #[test]
    fn names_are_escaped_and_raw_fields_are_tracked() {
        assert_eq!(Field::new("select").to_debug_sql(), "`select`");
        assert_eq!(Field::new("address.city").to_debug_sql(), "`address.city`");
        assert_eq!(Field::new("time::now()").to_debug_sql(), "`time::now()`");
        assert_parses(&format!(
            "SELECT {} FROM user;",
            Field::new("select").to_debug_sql()
        ));

        assert_eq!(Field::raw("tags[*]").to_debug_sql(), "tags[*]");
        let raw_with_value = umbriel::statements::select(Field::raw("tags[*]"))
            .from(Table::from("user"))
            .where_(Field::new("name").equal("Alice"));
        assert!(raw_with_value.to_query().is_err());
        assert!(
            umbriel::statements::select(Field::raw("tags[*]"))
                .from(Table::from("user"))
                .to_query()
                .is_ok()
        );
    }

    #[test]
    fn aliases() {
        assert_eq!(
            Field::new("name").__as__("moniker").to_debug_sql(),
            "name AS moniker"
        );
        assert_eq!(
            Field::new("age")
                .gt(18)
                .__as__(AliasName::new("adult"))
                .to_debug_sql(),
            "age > 18 AS adult"
        );
        assert_eq!(
            select(All)
                .from(Table::from("user"))
                .__as__("everyone")
                .to_debug_sql(),
            "(SELECT * FROM user) AS everyone"
        );
        assert_eq!(AliasName::new("total").as_str(), "total");
        let alias = Field::new("name").__as__("moniker");
        assert_eq!(alias.get_alias_name().to_string(), "moniker");
    }

    #[test]
    fn array_elements() {
        let alien = Alien::schema();
        assert_eq!(alien.tags(Last).to_debug_sql(), "tags[$]");
        assert_eq!(alien.tags(All).to_debug_sql(), "tags[*]");
        assert_eq!(alien.tags(index(0)).to_debug_sql(), "tags[0]");
        assert_eq!(
            alien.tags(index(1)).to_query().expect("valid").sql(),
            "tags[$_p0]"
        );
        assert_eq!(
            alien.tags(index(Field::new("position"))).to_debug_sql(),
            "tags[position]"
        );
        assert_eq!(
            alien.tags(index(Param::new("position"))).to_debug_sql(),
            "tags[$position]"
        );
        assert_eq!(
            alien.tags(index(1)).greater_than(1).to_debug_sql(),
            "tags[1] > 1"
        );
        assert_eq!(
            alien.tags(where_(this().less_than(18))).to_debug_sql(),
            "tags[WHERE $this < 18]"
        );
        assert_eq!(
            alien
                .tags(where_(this().less_than(18)))
                .all()
                .to_debug_sql(),
            "tags[WHERE $this < 18].*"
        );
    }
}
