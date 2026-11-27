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
                .greater_than(18)
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

/// Graph steps (`<->`, `?`, lookup `SELECT`s, `<~`) and recursion, rendered and evaluated.
mod graph {
    use super::*;
    use umbriel::{Lookup, Operatable, Param, Recurse, statements::order, types::RecordId};

    fn start(param: &'static str) -> Field {
        Field::from_expr(Param::new(param))
    }

    fn person(key: &str) -> RecordId {
        RecordId::new("person", key)
    }

    #[test]
    fn lookup_selects_render_each_clause() {
        let person = start("person");
        let cases = [
            (person.clone().both_ways(Lookup::any()), "$person<->?"),
            (
                person
                    .clone()
                    .outgoing(Lookup::tables(["likes", "follows"])),
                "$person->(likes, follows)",
            ),
            (
                person.clone().incoming(
                    Lookup::tables(["rated"])
                        // SurrealDB requires the SPLIT field to be selected.
                        .select([Field::new("weight"), Field::new("tags")])
                        .only()
                        .where_(Field::new("weight").greater_than(1))
                        .split(Field::new("tags"))
                        .order_by(order(Field::new("weight")).desc())
                        .limit(2)
                        .start(1)
                        .alias(Field::new("ratings")),
                ),
                "$person<-(SELECT weight, tags FROM ONLY rated WHERE weight > 1 SPLIT tags \
                 ORDER BY weight DESC LIMIT 2 START 1 AS ratings)",
            ),
            (
                person.clone().incoming(
                    Lookup::tables(["rated"])
                        .select([Field::new("weight")])
                        .group_by(Field::new("weight")),
                ),
                "$person<-(SELECT weight FROM rated GROUP BY weight)",
            ),
            (
                person
                    .clone()
                    .outgoing(Lookup::any().where_(Field::new("weight").greater_than(1))),
                "$person->(? WHERE weight > 1)",
            ),
            (
                person.clone().referenced_by(Lookup::tables(["post"])),
                "$person<~post",
            ),
            (
                person
                    .clone()
                    .referenced_by(Lookup::referencing("post", "author")),
                "$person<~(post FIELD author)",
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(path.to_debug_sql(), expected);
            assert_parses(&format!("RETURN {expected};"));
        }
        assert!(
            person
                .clone()
                .outgoing(Lookup::tables(["likes"]).only())
                .to_query()
                .is_err(),
            "ONLY needs a SELECT"
        );
        assert!(
            person
                .clone()
                .outgoing(
                    Lookup::tables(["rated"])
                        .select([Field::new("tags")])
                        .split(Field::new("tags"))
                        .group_by(Field::new("tags")),
                )
                .to_query()
                .is_err(),
            "SPLIT and GROUP BY are exclusive"
        );
        assert!(
            person
                .outgoing(Lookup::referencing("post", "author"))
                .to_query()
                .is_err(),
            "FIELD belongs to a reference step"
        );
    }

    #[test]
    fn recursion_shapes_render() {
        let alice = start("alice");
        let cases = [
            (Recurse::times(3), "$alice.{3}"),
            (Recurse::range(1..=3), "$alice.{1..3}"),
            (Recurse::range(1..4), "$alice.{1..3}"),
            (Recurse::range(2..), "$alice.{2..}"),
            (Recurse::range(..=3), "$alice.{..3}"),
            (Recurse::range(..), "$alice.{..}"),
            (Recurse::range(1..=3).path(), "$alice.{1..3+path}"),
            (
                Recurse::range(1..=3).path().inclusive(),
                "$alice.{1..3+path+inclusive}",
            ),
            (Recurse::range(1..=3).collect(), "$alice.{1..3+collect}"),
            (
                Recurse::range(1..=3).collect().inclusive(),
                "$alice.{1..3+collect+inclusive}",
            ),
            (
                Recurse::range(..).shortest(person("bob")),
                "$alice.{..+shortest=person:bob}",
            ),
            (
                Recurse::range(..)
                    .shortest(Param::new("target"))
                    .inclusive(),
                "$alice.{..+shortest=$target+inclusive}",
            ),
            (
                Recurse::range(1..=3).nested(Field::new("next")),
                "$alice.{1..3}(.next)",
            ),
            (
                Recurse::times(2).nested(Field::default().outgoing(Lookup::tables(["knows"]))),
                "$alice.{2}(->knows)",
            ),
        ];
        for (recursion, expected) in cases {
            assert_eq!(alice.clone().recurse(recursion).to_debug_sql(), expected);
            assert_parses(&format!("RETURN {expected};"));
        }
        for invalid in [
            Recurse::times(0),
            // A minimum above the maximum, built without range syntax: clippy's
            // `reversed_empty_ranges` denies the inverted literal this case is about.
            Recurse::range(std::ops::RangeInclusive::new(3, 1)),
            Recurse::range(1..1),
            Recurse::range(1..=3).inclusive(),
        ] {
            assert!(
                alice.clone().recurse(invalid).to_query().is_err(),
                "an invalid recursion renders"
            );
        }
    }

    /// `person:a` to `person:d` linked one to the next by `next`; `a` `knows` `b`, `b` `knows`
    /// `c`, and `a` likes a post and has rated three.
    async fn people() -> (Surreal<Any>, TestDb) {
        let (db, guard) = db().await;
        Raw::new(
            "CREATE person:d; \
             CREATE person:c SET next = person:d; \
             CREATE person:b SET next = person:c; \
             CREATE person:a SET next = person:b; \
             RELATE person:a->knows->person:b; \
             RELATE person:b->knows->person:c; \
             RELATE person:a->likes->post:x; \
             RELATE person:a->rated->post:one SET weight = 1; \
             RELATE person:a->rated->post:two SET weight = 2; \
             RELATE person:a->rated->post:three SET weight = 3;",
        )
        .run(&db)
        .await
        .expect("the people are created");
        (db, guard)
    }

    /// Whether `path` evaluates to `expected` (SurrealQL).
    async fn evaluates_to(db: &Surreal<Any>, path: &Field, expected: &str) -> bool {
        read(db, &format!("RETURN {} = {expected};", path.to_debug_sql())).await
    }

    fn from(key: &str) -> Field {
        Field::from_expr(umbriel::ValueLike::from(person(key)))
    }

    fn next() -> Field {
        Field::new("next")
    }

    #[tokio::test]
    async fn two_way_traversal_reaches_both_ends() {
        let (db, _guard) = people().await;
        let path = from("b")
            .both_ways(Lookup::tables(["knows"]))
            .both_ways(Lookup::tables(["person"]));
        let call = path.to_debug_sql();
        let both: bool = read(
            &db,
            &format!("RETURN {call} CONTAINSALL [person:a, person:c];"),
        )
        .await;
        assert!(both, "`{call}` reaches a and c");
    }

    #[tokio::test]
    async fn any_edge_table_is_traversed_with_a_wildcard() {
        let (db, _guard) = people().await;
        let path = from("a").outgoing(Lookup::any()).outgoing(Lookup::any());
        let call = path.to_debug_sql();
        let reached: bool = read(
            &db,
            &format!("RETURN {call} CONTAINSALL [person:b, post:x, post:one];"),
        )
        .await;
        assert!(reached, "`{call}` follows every edge table");
    }

    #[tokio::test]
    async fn a_lookup_select_filters_orders_and_limits_edges() {
        let (db, _guard) = people().await;
        let path = from("a").outgoing(
            Lookup::tables(["rated"])
                .select([Field::new("weight")])
                .where_(Field::new("weight").greater_than(1))
                .order_by(order(Field::new("weight")).desc())
                .limit(1),
        );
        assert!(evaluates_to(&db, &path, "[{ weight: 3 }]").await);
    }

    #[tokio::test]
    async fn fixed_depth_stops_at_exactly_n() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::times(2).nested(next()));
        assert!(evaluates_to(&db, &path, "person:c").await);
    }

    #[tokio::test]
    async fn fixed_depth_past_a_dead_end_is_the_dead_end_value() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::times(5).nested(next()));
        assert!(evaluates_to(&db, &path, "NONE").await);
    }

    #[tokio::test]
    async fn range_maximum_is_inclusive() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::range(1..=2).nested(next()));
        assert!(evaluates_to(&db, &path, "person:c").await);
        let unbounded = from("a").recurse(Recurse::range(1..).nested(next()));
        assert!(evaluates_to(&db, &unbounded, "person:d").await);
    }

    #[tokio::test]
    async fn ending_below_the_range_minimum_is_none() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::range(5..).nested(next()));
        assert!(evaluates_to(&db, &path, "NONE").await);
    }

    #[tokio::test]
    async fn path_returns_each_walk_and_inclusive_adds_the_start() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::range(1..).path().nested(next()));
        assert!(evaluates_to(&db, &path, "[[person:b, person:c, person:d]]").await);
        let inclusive = from("a").recurse(Recurse::range(1..).path().inclusive().nested(next()));
        assert!(
            evaluates_to(
                &db,
                &inclusive,
                "[[person:a, person:b, person:c, person:d]]"
            )
            .await
        );
    }

    #[tokio::test]
    async fn collect_returns_each_node_once_and_skips_nodes_below_the_minimum() {
        let (db, _guard) = people().await;
        let all = from("a").recurse(Recurse::range(1..).collect().nested(next()));
        assert!(evaluates_to(&db, &all, "[person:b, person:c, person:d]").await);
        let from_two = from("a").recurse(Recurse::range(2..).collect().nested(next()));
        assert!(evaluates_to(&db, &from_two, "[person:c, person:d]").await);
    }

    #[tokio::test]
    async fn shortest_reaches_a_record_target() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::range(..).shortest(person("c")).nested(next()));
        assert!(evaluates_to(&db, &path, "[person:b, person:c]").await);
    }

    #[tokio::test]
    async fn shortest_takes_a_parameter_target() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(
            Recurse::range(..)
                .shortest(Param::new("target"))
                .inclusive()
                .nested(next()),
        );
        let mut response = db
            .query(format!("RETURN {};", path.to_debug_sql()))
            .bind(("target", person("c")))
            .await
            .and_then(surrealdb::IndexedResults::check)
            .expect("the path evaluates");
        let walk: Vec<RecordId> = response.take(0).expect("a walk");
        assert_eq!(walk, [person("a"), person("b"), person("c")]);
    }

    #[tokio::test]
    async fn shortest_to_an_unreachable_target_is_none() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::range(..).shortest(person("z")).nested(next()));
        assert!(evaluates_to(&db, &path, "NONE").await);
    }

    #[tokio::test]
    async fn a_minimum_of_zero_is_rejected_by_the_server() {
        let (db, _guard) = people().await;
        assert!(
            from("a")
                .recurse(Recurse::range(0..=2).nested(next()))
                .to_query()
                .is_err()
        );
        let message = failure(&db, "RETURN person:a.{0..2}(.next);").await;
        assert!(message.contains("at least 1"), "{message}");
    }

    #[tokio::test]
    async fn a_maximum_above_the_limit_is_rejected() {
        let (db, _guard) = people().await;
        let path = from("a").recurse(Recurse::range(1..=300).nested(next()));
        let message = failure(&db, &format!("RETURN {};", path.to_debug_sql())).await;
        assert!(message.contains("at most"), "{message}");
    }

    #[tokio::test]
    async fn an_unbounded_walk_on_a_cycle_terminates_for_collect() {
        let (db, _guard) = db().await;
        assert!(
            succeeds(
                &db,
                "CREATE person:x SET next = person:y; CREATE person:y SET next = person:x;"
            )
            .await
        );
        let path = from("x").recurse(Recurse::range(..).collect().nested(next()));
        let call = path.to_debug_sql();
        let collected: bool = read(
            &db,
            &format!("RETURN {call} CONTAINS person:y AND array::len({call}) <= 2;"),
        )
        .await;
        assert!(collected, "`{call}` collects each record once");
    }
}
