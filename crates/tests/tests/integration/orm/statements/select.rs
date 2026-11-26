//! SELECT (`orm/src/statements/select.rs`): typed results, the clauses SurrealDB 3.3.4 adds
//! (`VERSION`, `FOR UPDATE`, `TEMPFILES`), the combinations it refuses, and ordering, grouping and
//! omitting as the database applies them.

use std::{collections::BTreeMap, time::Duration};

use surrealdb::{
    Surreal,
    engine::any::Any,
    types::{Object, Value},
};
use umbriel::{
    Aliasable, All, Error, Field, Model, Node, Operatable, Param, Raw, Render, Runnable,
    SchemaGetter, SetterAssignable, SurrealCrud, SurrealRead, Table, arr, count,
    statements::{Returning, explain, order, select, select_value, update},
};
use umbriel_tests::{support::statements::*, *};

fn organization(key: &str, name: &str, age: u8) -> Organization {
    Organization {
        id: Organization::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

/// Acme Corp (30), Globex (20) and Initech (40).
async fn organizations() -> (Surreal<Any>, TestDb) {
    let (db, guard) = db().await;
    for (key, name, age) in [
        ("acme", "Acme Corp", 30),
        ("globex", "Globex", 20),
        ("initech", "Initech", 40),
    ] {
        organization(key, name, age).save(&db).await.expect("saved");
    }
    (db, guard)
}

async fn run(db: &Surreal<Any>, statement: impl Runnable + Sync) {
    statement.run(db).await.expect("ran");
}

mod rendering {
    use super::*;

    #[test]
    fn the_read_path_renders_every_clause_in_order() {
        let age = Field::new("age");
        assert_sql(
            &select(All)
                .from(Organization::table())
                .where_(age.gte(18))
                .order_by(order(&age).desc())
                .limit(10)
                .start(5)
                .fetch(Field::new("users"))
                .version(Param::new("at"))
                .timeout(Duration::from_secs(5))
                .tempfiles()
                .explain(),
            "SELECT * FROM organization WHERE age >= 18 ORDER BY age DESC LIMIT 10 START AT 5 \
             FETCH users VERSION $at TIMEOUT 5s TEMPFILES EXPLAIN;",
        );
    }

    #[test]
    fn the_locking_path_renders_every_clause_in_order() {
        assert_sql(
            &select(All)
                .from(Organization::id("acme"))
                .where_(Field::new("age").gte(18))
                .fetch(Field::new("users"))
                .timeout(Duration::from_secs(5))
                .for_update()
                .tempfiles()
                .explain(),
            "SELECT * FROM organization:acme WHERE age >= 18 FETCH users TIMEOUT 5s FOR UPDATE \
             TEMPFILES EXPLAIN;",
        );
        assert_sql(
            &select(All)
                .from(vec![Organization::id("acme"), Organization::id("globex")])
                .for_update(),
            "SELECT * FROM organization:acme, organization:globex FOR UPDATE;",
        );
    }

    #[test]
    fn the_grouping_path_renders_every_clause_in_order() {
        let age = Field::new("age");
        assert_sql(
            &select(arr![count!().__as__("total"), &age])
                .from(Organization::table())
                .group_by(&age)
                .order_by(order(&age).asc())
                .limit(3)
                .timeout(Duration::from_secs(5))
                .tempfiles(),
            "SELECT count() AS total, age FROM organization GROUP BY age ORDER BY age ASC \
             LIMIT 3 TIMEOUT 5s TEMPFILES;",
        );
    }

    #[test]
    fn for_update_rejects_what_surrealdb_refuses() {
        let acme = || select(All).from(Organization::id("acme"));
        assert_render_error(
            &select(All).from(Organization::table()).for_update(),
            "FOR UPDATE needs record id targets",
        );
        assert_render_error(
            &select(All)
                .from(Organization::id(1)..Organization::id(5))
                .for_update(),
            "FOR UPDATE needs record id targets",
        );
        assert_render_error(
            &select(All)
                .from(select(All).from(Organization::table()))
                .for_update(),
            "FOR UPDATE needs record id targets",
        );
        assert_render_error(
            &acme().version(Param::new("at")).for_update(),
            "FOR UPDATE cannot be combined with VERSION",
        );
        assert_render_error(
            &acme().group_all().for_update(),
            "FOR UPDATE cannot be combined with GROUP",
        );
        assert_render_error(
            &acme().split(Field::new("users")).for_update(),
            "FOR UPDATE cannot be combined with SPLIT",
        );
        assert_render_error(
            &select(All)
                .from(vec![Organization::id("acme"), Organization::id("globex")])
                .limit(1)
                .for_update(),
            "FOR UPDATE cannot be combined with LIMIT over several targets",
        );
        // One target may be limited.
        assert_sql(
            &acme().limit(1).for_update(),
            "SELECT * FROM organization:acme LIMIT 1 FOR UPDATE;",
        );
    }

    /// Q02a: `type::table` and `type::record` are untyped functions (#16), taken as `Function`
    /// targets.
    #[test]
    fn computed_targets() {
        assert_sql(
            &select(All).from(umbriel::functions::type_::table!("organization")),
            "SELECT * FROM type::table('organization');",
        );
        assert_sql(
            &select(All).from(umbriel::functions::type_::record!("organization", "acme")),
            "SELECT * FROM type::record('organization', 'acme');",
        );
    }

    #[test]
    fn model_entry_points_select_the_default_table() {
        assert_sql(&Organization::select(), "SELECT * FROM organization;");
        assert_sql(
            &Organization::select_value(Organization::schema().age),
            "SELECT VALUE age FROM organization;",
        );
        assert_sql(
            &Organization::select()
                .load_links(Organization::get_link_many_fields())
                .expect("links"),
            "SELECT *, users.* FROM organization;",
        );
    }

    #[test]
    fn the_explain_prefix_renders_its_options() {
        let base = || select(All).from(Organization::table());
        assert_sql(&explain(base()), "EXPLAIN SELECT * FROM organization;");
        assert_sql(
            &explain(base()).analyze(),
            "EXPLAIN ANALYZE SELECT * FROM organization;",
        );
        assert_sql(
            &explain(base()).json(),
            "EXPLAIN FORMAT JSON SELECT * FROM organization;",
        );
        assert_sql(
            &explain(base()).json().analyze(),
            "EXPLAIN ANALYZE FORMAT JSON SELECT * FROM organization;",
        );
    }
}

mod typed_results {
    use super::*;

    #[tokio::test]
    async fn a_model_select_decodes_the_model() {
        let (db, _guard) = organizations().await;
        let found: Vec<Organization> = Organization::select()
            .order_by(order(Field::new("age")))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(
            found
                .iter()
                .map(|org| org.name.as_str())
                .collect::<Vec<_>>(),
            ["Globex", "Acme Corp", "Initech"]
        );
    }

    #[tokio::test]
    async fn select_value_decodes_the_field_type() {
        let (db, _guard) = organizations().await;
        let ages: Vec<u8> = Organization::select_value(Organization::schema().age)
            .order_by(order(Field::new("age")).desc())
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(ages, [40, 30, 20]);
    }

    #[tokio::test]
    async fn only_gives_one_record_or_none() {
        let (db, _guard) = organizations().await;
        let acme: Option<Organization> = select(All)
            .from_only(Organization::id("acme"))
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(acme.map(|org| org.age), Some(30));

        let missing: Option<Organization> = select(All)
            .from_only(Organization::id("umbrella"))
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn only_over_a_table_needs_limit_one() {
        let (db, _guard) = organizations().await;
        let unlimited = select(All)
            .from_only(Organization::table())
            .decode::<Organization>()
            .execute(&db)
            .await;
        assert!(
            matches!(unlimited, Err(Error::Statement { .. })),
            "{unlimited:?}"
        );

        let limited = select(All)
            .from_only(Organization::table())
            .limit(1)
            .decode::<Organization>()
            .execute(&db)
            .await
            .expect("read");
        assert!(limited.is_some());
    }

    #[tokio::test]
    async fn a_type_that_does_not_match_the_projection_fails_to_decode() {
        let (db, _guard) = organizations().await;
        let mismatched = select(Field::new("name"))
            .from(Organization::table())
            .decode::<Organization>()
            .execute(&db)
            .await;
        assert!(
            matches!(mismatched, Err(Error::Conversion(_))),
            "{mismatched:?}"
        );
    }

    #[tokio::test]
    async fn fetch_and_omit_are_decoded_into_the_named_type() {
        let (db, _guard) = organizations().await;
        run(
            &db,
            Raw::new("UPDATE organization:acme SET users = [user:alice]; CREATE user:alice SET name = 'Alice';"),
        )
        .await;

        let fetched: Vec<BTreeMap<String, Vec<BTreeMap<String, Value>>>> =
            select(Field::new("users"))
                .from(Organization::id("acme"))
                .fetch(Field::new("users"))
                .decode()
                .execute(&db)
                .await
                .expect("read");
        assert_eq!(
            fetched[0]["users"][0].get("name"),
            Some(&Value::String("Alice".to_owned()))
        );

        let omitted: Vec<BTreeMap<String, Value>> = select(All)
            .omit(Field::new("time").with_field("connected"))
            .from(Organization::id("acme"))
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(omitted[0].get("time"), Some(&Value::Object(Object::new())));
    }
}

mod ordering_and_grouping {
    use super::*;

    #[tokio::test]
    async fn order_by_a_field_that_is_not_projected() {
        let (db, _guard) = organizations().await;
        let names: Vec<String> = select_value(Field::new("name"))
            .from(Organization::table())
            .order_by(order(Field::new("age")).desc())
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(names, ["Initech", "Acme Corp", "Globex"]);
    }

    #[tokio::test]
    async fn ties_keep_their_order() {
        let (db, _guard) = db().await;
        run(
            &db,
            Raw::new(
                "CREATE item:a SET name = 'a', rank = 1; CREATE item:b SET name = 'b', rank = 1; \
                 CREATE item:c SET name = 'c', rank = 0;",
            ),
        )
        .await;
        let names: Vec<String> = select_value(Field::new("name"))
            .from(Table::from("item"))
            .order_by(order(Field::new("rank")).desc())
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[tokio::test]
    async fn numeric_and_collate_orders() {
        let (db, _guard) = db().await;
        run(
            &db,
            Raw::new(
                "CREATE label:1 SET text = 'file10'; CREATE label:2 SET text = 'file2'; \
                 CREATE label:3 SET text = 'file1'; CREATE word:1 SET text = 'b'; \
                 CREATE word:2 SET text = 'A'; CREATE word:3 SET text = 'a'; \
                 CREATE word:4 SET text = 'B';",
            ),
        )
        .await;
        let text = Field::new("text");

        let numeric: Vec<String> = select_value(text.clone())
            .from(Table::from("label"))
            .order_by(order(&text).numeric().asc())
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(numeric, ["file1", "file2", "file10"]);

        // Unicode collation orders by letter first, so each letter's cases sit together.
        let collated: Vec<String> = select_value(text.clone())
            .from(Table::from("word"))
            .order_by(order(&text).collate().asc())
            .decode()
            .execute(&db)
            .await
            .expect("read");
        let letters = collated
            .iter()
            .map(|word| word.to_lowercase())
            .collect::<Vec<_>>();
        assert_eq!(letters, ["a", "a", "b", "b"]);
    }

    /// Pinned on 3.3.4: a non-aggregate field outside the group is not refused; each group
    /// collects that field's values into an array.
    #[tokio::test]
    async fn a_non_aggregate_field_outside_the_group_is_collected() {
        let (db, _guard) = organizations().await;
        let groups: Vec<Value> = select(&[Field::new("name"), Field::new("age")])
            .from(Organization::table())
            .group_by(Field::new("age"))
            .order_by(order(Field::new("age")))
            .execute(&db)
            .await
            .expect("grouped");
        let expected: Vec<Value> = read(
            &db,
            "RETURN [{ age: 20, name: ['Globex'] }, { age: 30, name: ['Acme Corp'] }, \
             { age: 40, name: ['Initech'] }];",
        )
        .await;
        assert_eq!(groups, expected);
    }

    /// Pinned on 3.3.4: `GROUP ALL` over no records still gives one group, with a zero count.
    #[tokio::test]
    async fn an_empty_group_all_is_one_zero_group() {
        let (db, _guard) = db().await;
        run(&db, Raw::new("DEFINE TABLE organization;")).await;
        let groups: Vec<Value> = select(count!())
            .from(Organization::table())
            .group_all()
            .execute(&db)
            .await
            .expect("read");
        let expected: Vec<Value> = read(&db, "RETURN [{ count: 0 }];").await;
        assert_eq!(groups, expected);
        assert_eq!(
            Organization::count_all().execute(&db).await.expect("count"),
            0
        );
    }
}

mod index_hints {
    use super::*;

    /// The organizations, with an index on `age`.
    async fn indexed() -> (Surreal<Any>, TestDb) {
        let (db, guard) = organizations().await;
        run(
            &db,
            Raw::new("DEFINE INDEX organization_age ON organization FIELDS age;"),
        )
        .await;
        (db, guard)
    }

    /// The operator names of a SELECT plan: the newer executor's tree of `operator` and
    /// `children` (`exec/operators/explain.rs`), such as `IndexScan` and `TableScan`.
    fn operators(plan: &Value) -> Vec<String> {
        let mut names = vec![];
        let mut pending = vec![plan];
        while let Some(Value::Object(node)) = pending.pop() {
            if let Some(Value::String(name)) = node.get("operator") {
                names.push(name.clone());
            }
            if let Some(Value::Array(children)) = node.get("children") {
                pending.extend(children.iter());
            }
        }
        names
    }

    /// The step names of a write statement's plan: the older executor's list of
    /// `{ operation, detail }` (`dbs/plan.rs`), such as `Iterate Index` and `Iterate Table`.
    fn operations(plan: &Value) -> Vec<String> {
        let Value::Array(steps) = plan else {
            panic!("a write plan is a list of steps: {plan:?}");
        };
        steps
            .iter()
            .filter_map(|step| match step {
                Value::Object(step) => match step.get("operation") {
                    Some(Value::String(name)) => Some(name.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Asserts that a write statement reads through the index under `WITH INDEX` and through the
    /// table under `WITH NOINDEX`, and that explaining it wrote nothing.
    async fn assert_write_follows_its_hint(
        db: &Surreal<Any>,
        indexed: umbriel::statements::Explained,
        scanned: umbriel::statements::Explained,
    ) {
        assert_parses(&indexed.to_debug_sql());
        assert_parses(&scanned.to_debug_sql());
        let indexed = operations(&indexed.execute(db).await.expect("plan"));
        assert!(
            indexed.iter().any(|step| step == "Iterate Index"),
            "{indexed:?}"
        );
        assert!(
            !indexed.iter().any(|step| step == "Iterate Table"),
            "{indexed:?}"
        );
        let scanned = operations(&scanned.execute(db).await.expect("plan"));
        assert!(
            scanned.iter().any(|step| step == "Iterate Table"),
            "{scanned:?}"
        );
        assert!(
            !scanned.iter().any(|step| step.starts_with("Iterate Index")),
            "{scanned:?}"
        );
        let names: Vec<String> =
            read(db, "SELECT VALUE name FROM organization ORDER BY name;").await;
        assert_eq!(names, ["Acme Corp", "Globex", "Initech"]);
    }

    #[tokio::test]
    async fn select_follows_its_hint() {
        let (db, _guard) = indexed().await;
        let age = Field::new("age");
        let base = || {
            select(All)
                .from(Organization::table())
                .where_(age.equal(30))
        };
        let indexed = operators(
            &base()
                .with_index(Field::new("organization_age"))
                .explain()
                .execute(&db)
                .await
                .expect("plan"),
        );
        assert!(indexed.iter().any(|op| op == "IndexScan"), "{indexed:?}");
        assert!(!indexed.iter().any(|op| op == "TableScan"), "{indexed:?}");
        let scanned = operators(
            &base()
                .with_no_index()
                .explain()
                .execute(&db)
                .await
                .expect("plan"),
        );
        assert!(scanned.iter().any(|op| op == "TableScan"), "{scanned:?}");
        assert!(!scanned.iter().any(|op| op == "IndexScan"), "{scanned:?}");
    }

    // The #11 transfer's acceptance tests, one per statement: each statement keeps its hint
    // only if its own test passes.

    #[tokio::test]
    async fn update_follows_its_hint() {
        let (db, _guard) = indexed().await;
        let org = Organization::schema();
        let base = || {
            update::<Organization>(Organization::table())
                .set(org.name.equal_to("Renamed"))
                .where_(Field::new("age").equal(30))
        };
        assert_write_follows_its_hint(
            &db,
            base().with_index(Field::new("organization_age")).explain(),
            base().with_no_index().explain(),
        )
        .await;
    }

    #[tokio::test]
    async fn upsert_follows_its_hint() {
        let (db, _guard) = indexed().await;
        let org = Organization::schema();
        let base = || {
            umbriel::statements::upsert::<Organization>(Organization::table())
                .set(org.name.equal_to("Renamed"))
                .where_(Field::new("age").equal(30))
        };
        assert_write_follows_its_hint(
            &db,
            base().with_index(Field::new("organization_age")).explain(),
            base().with_no_index().explain(),
        )
        .await;
    }

    #[tokio::test]
    async fn delete_follows_its_hint() {
        let (db, _guard) = indexed().await;
        let base = || {
            umbriel::statements::delete::<Organization>(Organization::table())
                .where_(Field::new("age").equal(30))
        };
        assert_write_follows_its_hint(
            &db,
            base().with_index(Field::new("organization_age")).explain(),
            base().with_no_index().explain(),
        )
        .await;
    }
}

mod explain_prefix {
    use super::*;

    #[tokio::test]
    async fn text_and_json_plans() {
        let (db, _guard) = organizations().await;
        let base = || select(All).from(Organization::table());

        let text: String = explain(base()).execute(&db).await.expect("plan");
        assert!(text.contains("TableScan"), "{text}");

        let json: Value = explain(base()).json().execute(&db).await.expect("plan");
        assert!(
            matches!(&json, Value::Object(plan) if plan.get("operator").is_some()),
            "{json:?}"
        );

        // `exec/operators/explain.rs` ends an analyzed text plan with its row count.
        let analyzed: String = explain(base()).analyze().execute(&db).await.expect("plan");
        assert!(analyzed.contains("Total rows: 3"), "{analyzed}");
    }
}

/// The verification task for Q10, one form per test so each state change is attributed: 3.3.4
/// runs a write under `EXPLAIN FULL` (`dbs/iterator.rs`), skips it under `EXPLAIN`, and refuses
/// the prefix over a write (`exec/planner.rs`).
mod explaining_writes {
    use super::*;

    async fn acme_age(db: &Surreal<Any>) -> u8 {
        read(db, "RETURN organization:acme.age;").await
    }

    #[tokio::test]
    async fn explain_does_not_write() {
        let (db, _guard) = organizations().await;
        update::<Organization>(Organization::id("acme"))
            .set(Organization::schema().age.equal_to(99u8))
            .explain()
            .execute(&db)
            .await
            .expect("plan");
        assert_eq!(acme_age(&db).await, 30);
    }

    #[tokio::test]
    async fn explain_full_performs_the_write() {
        let (db, _guard) = organizations().await;
        update::<Organization>(Organization::id("acme"))
            .set(Organization::schema().age.equal_to(99u8))
            .explain_full()
            .execute(&db)
            .await
            .expect("plan");
        assert_eq!(acme_age(&db).await, 99);
    }

    #[tokio::test]
    async fn the_prefix_over_a_write_is_refused() {
        let (db, _guard) = organizations().await;
        let refused = Raw::new("EXPLAIN ANALYZE UPDATE organization:acme SET age = 98;")
            .run(&db)
            .await;
        assert!(refused.is_err(), "EXPLAIN ANALYZE UPDATE ran");
        assert_eq!(acme_age(&db).await, 30);
    }
}

mod for_update {
    use super::*;

    #[tokio::test]
    async fn it_runs_inside_a_query_transaction() {
        let (db, _guard) = organizations().await;
        let mut results = umbriel::statements::begin_transaction()
            .query(select(All).from(Organization::id("acme")).for_update())
            .commit_transaction()
            .run(&db)
            .await
            .expect("committed");
        // 3.3.4 gives `BEGIN` its own `NONE` result (`dbs/executor.rs`), so the SELECT is second.
        let read: Vec<Organization> = results.take(1).expect("records");
        assert_eq!(read.len(), 1);
    }

    #[tokio::test]
    async fn a_conflicting_write_fails_the_commit() {
        let (db, _guard) = organizations().await;
        let (sql, vars) = select(All)
            .from(Organization::id("acme"))
            .for_update()
            .to_query()
            .expect("renders")
            .into_parts();

        let reader = db.clone().begin().await.expect("reader begins");
        reader
            .query(sql)
            .bind(vars)
            .await
            .and_then(surrealdb::IndexedResults::check)
            .expect("selected for update");

        let writer = db.clone().begin().await.expect("writer begins");
        writer
            .query("UPDATE organization:acme SET age = 31;")
            .await
            .and_then(surrealdb::IndexedResults::check)
            .expect("updated");
        writer.commit().await.expect("the writer commits first");

        assert!(
            reader.commit().await.is_err(),
            "the reader's commit conflicts"
        );
    }

    /// Recorded behaviour: outside a transaction, `FOR UPDATE` reads like a plain `SELECT`.
    #[tokio::test]
    async fn outside_a_transaction_it_reads() {
        let (db, _guard) = organizations().await;
        let read: Vec<Organization> = select(All)
            .from(Organization::id("acme"))
            .for_update()
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(read.len(), 1);
    }
}

/// Storage clauses. Reading with `VERSION` is covered in `server.rs` (`versioned_reads`).
mod storage {
    use super::*;

    /// The 3.3.4 parser reads and discards `VERSION` on `CREATE` and `INSERT`, so even the memory
    /// engine accepts it, and the record is written now.
    #[tokio::test]
    async fn version_on_create_and_insert_is_ignored() {
        let (db, _guard) = db().await;
        run(
            &db,
            Raw::new(
                "CREATE organization:acme SET name = 'Acme Corp' VERSION d'2000-01-01T00:00:00Z'; \
                 INSERT INTO organization { id: organization:globex, name: 'Globex' } \
                 VERSION d'2000-01-01T00:00:00Z';",
            ),
        )
        .await;
        let names: Vec<String> =
            read(&db, "SELECT VALUE name FROM organization ORDER BY name;").await;
        assert_eq!(names, ["Acme Corp", "Globex"]);
    }

    /// `TEMPFILES` with the server's temporary directory: the same rows, and a plan that sorts on
    /// disk. 3.3.4 plans `ExternalSortByKey` only when the statement has `TEMPFILES` and the
    /// server has a temporary directory (`exec/planner/select/pipeline.rs`).
    #[tokio::test]
    async fn tempfiles_sorts_through_the_temporary_directory() {
        let (db, _guard) = db().await;
        for (key, name, age) in [("acme", "Acme Corp", 30), ("globex", "Globex", 20)] {
            organization(key, name, age).save(&db).await.expect("saved");
        }
        let sorted = || {
            select_value(Field::new("name"))
                .from(Organization::table())
                .order_by(order(Field::new("age")))
                .decode::<String>()
        };
        let in_memory = sorted().execute(&db).await.expect("read");
        let in_files = sorted().tempfiles().execute(&db).await.expect("read");
        assert_eq!(in_memory, in_files);

        let plan: String = explain(sorted().tempfiles())
            .execute(&db)
            .await
            .expect("plan");
        assert!(plan.contains("ExternalSortByKey"), "{plan}");
    }
}

mod timeout {
    use super::*;

    /// Q11a: a duration held by a parameter, as a bound value is, is evaluated as the statement's
    /// timeout.
    #[tokio::test]
    async fn a_duration_parameter_is_accepted() {
        let (db, _guard) = organizations().await;
        let mut results =
            umbriel::chain(umbriel::statements::let_("limit").equal_to(Duration::from_secs(5)))
                .chain(
                    select(All)
                        .from(Organization::table())
                        .timeout(Param::new("limit")),
                )
                .run(&db)
                .await
                .expect("ran");
        let found: Vec<Organization> = results.take(1).expect("records");
        assert_eq!(found.len(), 3);
    }

    /// D6: since a duration parameter works, every statement binds its `TIMEOUT` instead of
    /// writing it inline. Debug SQL still shows the literal.
    #[test]
    fn every_statement_binds_its_timeout() {
        let five = Duration::from_secs(5);
        let org = Organization::schema();
        let queries = [
            select(All)
                .from(Organization::table())
                .timeout(five)
                .to_query(),
            umbriel::statements::create::<Organization>(Organization::table())
                .set(org.name.equal_to("Acme Corp"))
                .timeout(five)
                .to_query(),
            update::<Organization>(Organization::table())
                .set(org.age.equal_to(1u8))
                .timeout(five)
                .to_query(),
            umbriel::statements::upsert::<Organization>(Organization::table())
                .set(org.age.equal_to(1u8))
                .timeout(five)
                .to_query(),
            umbriel::statements::delete::<Organization>(Organization::table())
                .timeout(five)
                .to_query(),
            umbriel::statements::insert(organization("acme", "Acme Corp", 30))
                .timeout(five)
                .to_query(),
            umbriel::statements::relate::<CompanyLikeUser>(
                Company::with(Company::id("acme"))
                    .like__(umbriel::E)
                    .user(User::id("alice")),
            )
            .timeout(five)
            .to_query(),
        ];
        for query in queries {
            let query = query.expect("renders");
            assert!(query.sql().contains(" TIMEOUT $_p"), "{}", query.sql());
            assert_parses(query.sql());
        }
        assert_eq!(
            select(All)
                .from(Organization::table())
                .timeout(five)
                .to_debug_sql(),
            "SELECT * FROM organization TIMEOUT 5s;"
        );
    }

    /// A bound `TIMEOUT` is enforced: the statement fails with `TimedOut`.
    #[tokio::test]
    async fn a_bound_timeout_cuts_a_slow_select_short() {
        let (db, _guard) = db().await;
        let failed = select(All)
            .from(umbriel::functions::sleep::sleep!(Duration::from_millis(
                500
            )))
            .timeout(Duration::from_millis(10))
            .execute(&db)
            .await;
        match failed {
            Err(Error::Statement { source, .. }) => assert!(
                matches!(
                    source.query_details(),
                    Some(surrealdb::types::QueryError::TimedOut { .. })
                ),
                "expected a timeout, got {source}"
            ),
            other => panic!("expected a statement error, got {other:?}"),
        }
    }
}

/// Every clause rendered in order, projections and targets, the refused combinations, and
/// filtering, grouping, ranges and fetching on the engine.
mod clauses_and_results {
    use super::*;

    #[test]
    fn every_clause_in_order() {
        let user = Table::from("user");
        let age = Field::new("age");
        assert_sql(
            &select(All)
                .omit(Field::new("password"))
                .from(&user)
                .with_index(Field::new("user_age"))
                .where_(age.gte(18))
                .order_by(order(&age).numeric().desc())
                .order_by(order(Field::new("name")).collate().asc())
                .limit(10)
                .start(5)
                .fetch(Field::new("account"))
                .timeout(Duration::from_secs(5))
                .explain_full(),
            "SELECT * OMIT password FROM user WITH INDEX user_age WHERE age >= 18 \
             ORDER BY age NUMERIC DESC, name COLLATE ASC LIMIT 10 START AT 5 FETCH account \
             TIMEOUT 5s EXPLAIN FULL;",
        );
        assert_sql(
            &select(All).from(&user).with_no_index().explain(),
            "SELECT * FROM user WITH NOINDEX EXPLAIN;",
        );
    }

    #[test]
    fn projections_and_targets() {
        let user = Table::from("user");
        let country = Field::new("country");
        assert_sql(
            &select(arr![count!().__as__(AliasName::new("total")), &country])
                .from(&user)
                .group_by(&country),
            "SELECT count() AS total, country FROM user GROUP BY country;",
        );
        assert_sql(
            &select(math::sum!(Field::new("age")))
                .from(&user)
                .group_all(),
            "SELECT math::sum(age) FROM user GROUP ALL;",
        );
        assert_sql(
            &select(&[Field::new("name"), Field::new("age")]).from_only(User::id("alice")),
            "SELECT name, age FROM ONLY user:alice;",
        );
        assert_sql(
            &select_value(Field::new("name")).from(vec![User::id("alice"), User::id("bob")]),
            "SELECT VALUE name FROM user:alice, user:bob;",
        );
        assert_sql(
            &select(All).from(&[Table::from("user"), Table::from("company")]),
            "SELECT * FROM user, company;",
        );
        assert_sql(
            &select(All).from(select(All).from(&user).limit(1)),
            "SELECT * FROM (SELECT * FROM user LIMIT 1);",
        );
        assert_sql(
            &select(All).from(User::id(1)..User::id(5)),
            "SELECT * FROM user:1..5;",
        );
        assert_sql(
            &select(All).from(&user).split(Field::new("emails")),
            "SELECT * FROM user SPLIT emails;",
        );
    }

    #[test]
    fn rejected_combinations() {
        let user = Table::from("user");
        assert_render_error(
            &select(All)
                .from(&user)
                .split(Field::new("emails"))
                .group_all(),
            "SPLIT and GROUP cannot be combined",
        );
        assert_render_error(
            &select(All)
                .from(&user)
                .split(Field::new("emails"))
                .group_by(Field::new("country")),
            "SPLIT and GROUP cannot be combined",
        );
        assert_render_error(
            &select(All)
                .from(&user)
                .order_rand()
                .order_by(order(Field::new("age"))),
            "ORDER BY RAND()",
        );
        assert_sql(
            &select(All).from(&user).order_rand().limit(3),
            "SELECT * FROM user ORDER BY RAND() LIMIT 3;",
        );
    }

    #[test]
    fn values_are_bound_and_rendering_is_stable() {
        let statement = select(All)
            .from(Table::from("user"))
            .where_(cond(Field::new("name").equal("Alice")).or(Field::new("age").gt(30)))
            .limit(5);
        let query = statement.to_query().expect("valid");
        assert_eq!(
            query.sql(),
            "SELECT * FROM user WHERE (name = $_p0) OR (age > $_p1) LIMIT $_p2;"
        );
        assert_eq!(query.vars().len(), 3);
        assert_eq!(statement.to_query().expect("valid"), query);
    }

    async fn seeded() -> (Surreal<Any>, TestDb) {
        let (db, guard) = db().await;
        run(
            &db,
            Raw::new(
                "CREATE user:alice SET name = 'Alice', age = 30, country = 'CA', emails = ['a@example.com', 'alice@example.com'];
                 CREATE user:bob SET name = 'Bob', age = 20, country = 'CA', emails = ['bob@example.com'];
                 CREATE user:carol SET name = 'Carol', age = 40, country = 'NO', emails = ['carol@example.com'];",
            ),
        )
        .await;
        (db, guard)
    }

    #[tokio::test]
    async fn filters_orders_and_pages() {
        let (db, _guard) = seeded().await;
        let user = Table::from("user");
        let age = Field::new("age");
        let names: Vec<String> = select_value(Field::new("name"))
            .from(&user)
            .where_(age.gte(20))
            .order_by(order(&age).desc())
            .limit(2)
            .start(1)
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(names, ["Alice", "Bob"]);
    }

    #[tokio::test]
    async fn groups_splits_and_omits() {
        let (db, _guard) = seeded().await;
        let user = Table::from("user");
        let country = Field::new("country");

        let groups: Vec<BTreeMap<String, Value>> = select(arr![count!().__as__("total"), &country])
            .from(&user)
            .group_by(&country)
            .decode()
            .execute(&db)
            .await
            .expect("grouped");
        assert_eq!(groups.len(), 2);

        let split: Vec<Value> = select(All)
            .from(&user)
            .split(Field::new("emails"))
            .execute(&db)
            .await
            .expect("split");
        assert_eq!(split.len(), 4);

        let omitted: Vec<BTreeMap<String, Value>> = select(All)
            .omit(Field::new("emails"))
            .from(&user)
            .decode()
            .execute(&db)
            .await
            .expect("omitted");
        assert!(omitted.iter().all(|row| !row.contains_key("emails")));
    }

    #[tokio::test]
    async fn record_ranges_select_by_key() {
        let (db, _guard) = db().await;
        for strength in 1..=6 {
            Weapon {
                id: Weapon::id(strength),
                ..Default::default()
            }
            .save(&db)
            .await
            .expect("saved");
        }
        let in_range: Vec<Value> = select(All)
            .from(Weapon::id(2)..Weapon::id(5))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(in_range.len(), 3);
        let inclusive: Vec<Value> = select(All)
            .from(Weapon::id(2)..=Weapon::id(5))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(inclusive.len(), 4);
    }

    #[tokio::test]
    async fn fetch_replaces_links_with_records() {
        let (db, _guard) = db().await;
        run(
            &db,
            Raw::new(
                "CREATE account:one SET balance = 5; CREATE user:alice SET account = account:one;",
            ),
        )
        .await;
        let balance: Vec<i64> = select_value(Field::new("account").with_field("balance"))
            .from(User::id("alice"))
            .decode()
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(balance, [5]);
        let fetched: BTreeMap<String, Value> = select(All)
            .from_only(User::id("alice"))
            .fetch(Field::new("account"))
            .decode()
            .execute(&db)
            .await
            .expect("read")
            .expect("alice");
        assert!(matches!(fetched.get("account"), Some(Value::Object(_))));
    }
}

/// Every input `fetch` took before typed link paths still compiles, renders the same `FETCH`
/// clause, and still gives untyped values.
mod compiles_fetch_inputs {
    use umbriel::{ValueLike, statements::SelectStatement};

    use super::*;

    /// A caller's own conversion into a field.
    struct Handle;

    impl From<Handle> for Field {
        fn from(_: Handle) -> Self {
            Field::new("wallet")
        }
    }

    fn values(statement: SelectStatement<Value>) -> SelectStatement<Value> {
        statement
    }

    #[test]
    fn every_fetch_input_renders_as_before() {
        let wallet = Field::new("wallet");
        let employer = Field::new("worksFor");
        let one = "SELECT * FROM member FETCH wallet;";
        let two = "SELECT * FROM member FETCH wallet, worksFor;";

        assert_sql(&values(Member::select().fetch(wallet.clone())), one);
        assert_sql(&values(Member::select().fetch(&wallet)), one);
        assert_sql(&values(Member::select().fetch(Handle)), one);
        assert_sql(
            &values(Member::select().fetch([wallet.clone(), employer.clone()])),
            two,
        );
        assert_sql(&values(Member::select().fetch([&wallet, &employer])), two);
        assert_sql(
            &values(Member::select().fetch(&[wallet.clone(), employer.clone()])),
            two,
        );
        // The borrowed array is the input form under test: `From<&[&Field; N]>`.
        #[expect(
            clippy::needless_borrows_for_generic_args,
            reason = "checks that the `&[&Field; N]` input still compiles"
        )]
        let borrowed = Member::select().fetch(&[&wallet, &employer]);
        assert_sql(&values(borrowed), two);
        assert_sql(
            &values(Member::select().fetch(vec![wallet.clone(), employer.clone()])),
            two,
        );
        assert_sql(
            &values(Member::select().fetch(vec![&wallet, &employer])),
            two,
        );
        assert_sql(
            &values(Member::select().fetch(vec![
                ValueLike::from(wallet.clone()),
                ValueLike::from(employer.clone()),
            ])),
            two,
        );
        assert_sql(
            &values(select(All).from(Member::table()).fetch(wallet)),
            one,
        );
    }
}

/// `fetch_link` keeps the model type: the result decodes into the model with no `decode`.
mod compiles_fetch_link {
    use umbriel::{Links, statements::SelectStatement};

    use super::*;

    fn members(statement: SelectStatement<Member>) -> SelectStatement<Member> {
        statement
    }

    #[test]
    fn fetch_link_keeps_the_model_type() {
        let statement = members(
            Member::find_where(Member::schema().name.equal("Alice"))
                .fetch_link(Member::links().wallet),
        );
        assert_sql(
            &statement,
            "SELECT * FROM member WHERE name = 'Alice' FETCH wallet;",
        );
    }
}

/// `fetch_link` against the server: nested and multi-hop paths decode into the model's links.
mod fetch_links {
    use umbriel::{Links, Reference};

    use super::*;

    /// Alice (wallet at Acme Corp, works for Globex, mentored by Bob), Bob (wallet at Acme Corp),
    /// Carol (wallet at a bank that doesn't exist), the Chess club (Alice and Bob) and the empty
    /// Quiet club.
    pub(super) async fn membership() -> (Surreal<Any>, TestDb) {
        let (db, guard) = db().await;
        run(
            &db,
            Raw::new(
                "CREATE bank:acme SET name = 'Acme Corp'; \
                 CREATE bank:globex SET name = 'Globex'; \
                 CREATE wallet:alice SET balance = 5, bank = bank:acme; \
                 CREATE wallet:bob SET balance = 7, bank = bank:acme; \
                 CREATE wallet:carol SET balance = 1, bank = bank:missing; \
                 CREATE member:bob SET name = 'Bob', wallet = wallet:bob; \
                 CREATE member:alice SET name = 'Alice', wallet = wallet:alice, \
                     worksFor = bank:globex, mentor = member:bob; \
                 CREATE member:carol SET name = 'Carol', wallet = wallet:carol; \
                 CREATE club:chess SET name = 'Chess', members = [member:alice, member:bob]; \
                 CREATE club:quiet SET name = 'Quiet', members = [];",
            ),
        )
        .await;
        (db, guard)
    }

    async fn member(
        db: &Surreal<Any>,
        name: &str,
        statement: impl FnOnce(
            umbriel::statements::SelectStatement<Member>,
        ) -> umbriel::statements::SelectStatement<Member>,
    ) -> Member {
        let mut found: Vec<Member> =
            statement(Member::find_where(Member::schema().name.equal(name)))
                .execute(db)
                .await
                .expect("read");
        assert_eq!(found.len(), 1, "one member named {name}");
        found.remove(0)
    }

    async fn club(db: &Surreal<Any>, name: &str) -> Club {
        let mut found: Vec<Club> = Club::find_where(Club::schema().name.equal(name))
            .fetch_link(Club::links().members.then(Member::links().wallet))
            .execute(db)
            .await
            .expect("read");
        assert_eq!(found.len(), 1, "one club named {name}");
        found.remove(0)
    }

    #[tokio::test]
    async fn fetch_link_through_two_links_decodes_both_hops() {
        let (db, _guard) = membership().await;
        let alice = member(&db, "Alice", |statement| {
            statement.fetch_link(Member::links().wallet.then(Wallet::links().bank))
        })
        .await;
        let wallet = alice.wallet.value().expect("wallet fetched");
        assert_eq!(wallet.balance, 5);
        assert_eq!(
            wallet.bank.value().map(|bank| bank.name.as_str()),
            Some("Acme Corp")
        );
    }

    #[tokio::test]
    async fn fetch_link_through_a_link_list_fans_out() {
        let (db, _guard) = membership().await;
        let chess = club(&db, "Chess").await;
        let mut balances = chess
            .members
            .values_truthy()
            .into_iter()
            .map(|member| member.wallet.value().expect("wallet fetched").balance)
            .collect::<Vec<_>>();
        balances.sort_unstable();
        assert_eq!(balances, [5, 7]);
    }

    #[tokio::test]
    async fn fetch_link_through_a_self_link_recurses() {
        let (db, _guard) = membership().await;
        let alice = member(&db, "Alice", |statement| {
            statement.fetch_link(Member::links().mentor.then(Member::links().wallet))
        })
        .await;
        let bob = alice.mentor.value().expect("mentor fetched");
        assert_eq!(bob.name, "Bob");
        assert_eq!(bob.wallet.value().map(|wallet| wallet.balance), Some(7));
        assert!(
            alice.wallet.value().is_none(),
            "Alice's own wallet is not fetched"
        );
    }

    #[tokio::test]
    async fn fetch_links_to_two_target_models() {
        let (db, _guard) = membership().await;
        let alice = member(&db, "Alice", |statement| {
            statement
                .fetch_link(Member::links().wallet)
                .fetch_link(Member::links().employer)
        })
        .await;
        assert_eq!(alice.wallet.value().map(|wallet| wallet.balance), Some(5));
        assert_eq!(
            alice.employer.value().map(|bank| bank.name.as_str()),
            Some("Globex")
        );
    }

    #[tokio::test]
    async fn fetch_link_through_a_renamed_link() {
        let (db, _guard) = membership().await;
        let alice = member(&db, "Alice", |statement| {
            statement.fetch_link(Member::links().employer)
        })
        .await;
        assert_eq!(
            alice.employer.value().map(|bank| bank.name.as_str()),
            Some("Globex")
        );
        assert!(alice.wallet.value().is_none());
    }

    #[tokio::test]
    async fn a_missing_record_on_the_second_hop_decodes_as_null() {
        let (db, _guard) = membership().await;
        let carol = member(&db, "Carol", |statement| {
            statement.fetch_link(Member::links().wallet.then(Wallet::links().bank))
        })
        .await;
        let wallet = carol.wallet.value().expect("wallet fetched");
        assert!(matches!(*wallet.bank, Reference::Null));
    }

    #[tokio::test]
    async fn an_empty_link_list_fetches_nothing() {
        let (db, _guard) = membership().await;
        let quiet = club(&db, "Quiet").await;
        assert!(quiet.members.keys().is_empty());
    }

    #[tokio::test]
    async fn fetch_with_a_field_still_returns_values() {
        let (db, _guard) = membership().await;
        let rows: Vec<Value> = Member::find_where(Member::schema().name.equal("Alice"))
            .fetch(Field::new("wallet"))
            .execute(&db)
            .await
            .expect("read");
        let [Value::Object(alice)] = rows.as_slice() else {
            panic!("one object: {rows:?}");
        };
        assert!(matches!(alice.get("wallet"), Some(Value::Object(_))));
    }
}

/// `select_fields` against the server: each record's fields decode into the tuple, in order.
mod select_fields {
    use umbriel::LinkOne;

    use super::{fetch_links::membership, *};

    #[tokio::test]
    async fn select_fields_decodes_each_field_in_order() {
        let (db, _guard) = membership().await;
        let rows: Vec<(String, LinkOne<Wallet>)> =
            Member::select_fields((Member::schema().name, Member::schema().wallet))
                .where_(Member::schema().name.equal("Alice"))
                .execute(&db)
                .await
                .expect("read");
        let [(name, wallet)] = rows.as_slice() else {
            panic!("one row: {rows:?}");
        };
        assert_eq!(name, "Alice");
        assert_eq!(wallet.key(), Some(&Wallet::id("alice")));
        assert!(wallet.value().is_none(), "a link stays its id");
    }

    #[tokio::test]
    async fn a_renamed_field_is_read_by_its_stored_name() {
        let (db, _guard) = membership().await;
        let rows: Vec<(LinkOne<Bank>,)> = Member::select_fields((Member::schema().worksFor,))
            .where_(Member::schema().name.equal("Alice"))
            .execute(&db)
            .await
            .expect("read");
        let [(employer,)] = rows.as_slice() else {
            panic!("one row: {rows:?}");
        };
        assert_eq!(employer.key(), Some(&Bank::id("globex")));
    }

    #[tokio::test]
    async fn a_missing_field_decodes_as_its_empty_value() {
        let (db, _guard) = membership().await;
        let rows: Vec<(String, LinkOne<Bank>)> =
            Member::select_fields((Member::schema().name, Member::schema().worksFor))
                .where_(Member::schema().name.equal("Bob"))
                .execute(&db)
                .await
                .expect("read");
        let [(name, employer)] = rows.as_slice() else {
            panic!("one row: {rows:?}");
        };
        assert_eq!(name, "Bob");
        assert!(employer.key().is_none(), "Bob works for no one");
    }

    #[tokio::test]
    async fn select_fields_orders_the_records() {
        let (db, _guard) = membership().await;
        let rows: Vec<(String,)> = Member::select_fields((Member::schema().name,))
            .order_by(order(Member::schema().name).desc())
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(
            rows,
            [
                ("Carol".to_owned(),),
                ("Bob".to_owned(),),
                ("Alice".to_owned(),)
            ]
        );
    }
}
