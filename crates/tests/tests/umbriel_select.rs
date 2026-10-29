//! SELECT (`orm/src/statements/select.rs`): typed results, the clauses SurrealDB 3.3.4 adds
//! (`VERSION`, `FOR UPDATE`, `TEMPFILES`), the combinations it refuses, and ordering, grouping and
//! omitting as the database applies them.

use std::{collections::BTreeMap, time::Duration};

use chrono::{TimeZone, Utc};
use surrealdb::{
    Surreal,
    engine::local::{Db, Mem},
    opt::Config,
    types::{Object, Value},
};
use umbriel::{
    All, Error, Field, Model, Operatable, Param, Raw, Render, Runnable, SurrealCrud, Table,
    Aliasable, arr, count,
    statements::{Returning, explain, order, select, select_value, update},
};
use umbriel_tests::*;

fn organization(key: &str, name: &str, age: u8) -> Organization {
    Organization {
        id: Organization::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

/// Acme Corp (30), Globex (20) and Initech (40).
async fn organizations() -> Surreal<Db> {
    let db = db().await;
    for (key, name, age) in [("acme", "Acme Corp", 30), ("globex", "Globex", 20), ("initech", "Initech", 40)] {
        organization(key, name, age)
            .save(&db)
            .await
            .expect("saved");
    }
    db
}

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
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
        let db = organizations().await;
        let found: Vec<Organization> = Organization::select()
            .order_by(order(Field::new("age")))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(
            found.iter().map(|org| org.name.as_str()).collect::<Vec<_>>(),
            ["Globex", "Acme Corp", "Initech"]
        );
    }

    #[tokio::test]
    async fn select_value_decodes_the_field_type() {
        let db = organizations().await;
        let ages: Vec<u8> = Organization::select_value(Organization::schema().age)
            .order_by(order(Field::new("age")).desc())
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(ages, [40, 30, 20]);
    }

    #[tokio::test]
    async fn only_gives_one_record_or_none() {
        let db = organizations().await;
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
        let db = organizations().await;
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
        let db = organizations().await;
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
        let db = organizations().await;
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
        let db = organizations().await;
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
        let db = db().await;
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
        let db = db().await;
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

    #[tokio::test]
    async fn a_non_aggregate_field_outside_the_group_is_refused() {
        let db = organizations().await;
        let refused = select(&[Field::new("name"), Field::new("age")])
            .from(Organization::table())
            .group_by(Field::new("age"))
            .execute(&db)
            .await;
        assert!(refused.is_err(), "{refused:?}");
    }

    #[tokio::test]
    async fn an_empty_group_all_is_no_group() {
        let db = db().await;
        run(&db, Raw::new("DEFINE TABLE organization;")).await;
        let groups: Vec<Value> = select(count!())
            .from(Organization::table())
            .group_all()
            .execute(&db)
            .await
            .expect("read");
        assert!(groups.is_empty(), "{groups:?}");
        assert_eq!(Organization::count_all().execute(&db).await.expect("count"), 0);
    }
}

mod index_hints {
    use super::*;

    /// The organizations, with an index on `age`.
    async fn indexed() -> Surreal<Db> {
        let db = organizations().await;
        run(
            &db,
            Raw::new("DEFINE INDEX organization_age ON organization FIELDS age;"),
        )
        .await;
        db
    }

    fn uses_the_index(plan: &[Value]) -> bool {
        format!("{plan:?}").contains("organization_age")
    }

    #[tokio::test]
    async fn select_follows_its_hint() {
        let db = indexed().await;
        let age = Field::new("age");
        let base = || select(All).from(Organization::table()).where_(age.equal(30));
        let indexed = base()
            .with_index(Field::new("organization_age"))
            .explain()
            .execute(&db)
            .await
            .expect("plan");
        assert!(uses_the_index(&indexed), "{indexed:?}");
        let scanned = base()
            .with_no_index()
            .explain()
            .execute(&db)
            .await
            .expect("plan");
        assert!(!uses_the_index(&scanned), "{scanned:?}");
    }

    /// The #11 transfer's acceptance test: `WITH` on a write statement is kept only if it changes
    /// that statement's plan.
    #[tokio::test]
    async fn write_statements_follow_their_hint() {
        let db = indexed().await;
        let org = Organization::schema();
        let plans = [
            (
                "UPDATE",
                update::<Organization>(Organization::table())
                    .with_index(Field::new("organization_age"))
                    .set(org.name.equal_to("Renamed"))
                    .where_(Field::new("age").equal(30))
                    .explain(),
                update::<Organization>(Organization::table())
                    .with_no_index()
                    .set(org.name.equal_to("Renamed"))
                    .where_(Field::new("age").equal(30))
                    .explain(),
            ),
            (
                "UPSERT",
                umbriel::statements::upsert::<Organization>(Organization::table())
                    .with_index(Field::new("organization_age"))
                    .set(org.name.equal_to("Renamed"))
                    .where_(Field::new("age").equal(30))
                    .explain(),
                umbriel::statements::upsert::<Organization>(Organization::table())
                    .with_no_index()
                    .set(org.name.equal_to("Renamed"))
                    .where_(Field::new("age").equal(30))
                    .explain(),
            ),
            (
                "DELETE",
                umbriel::statements::delete::<Organization>(Organization::table())
                    .with_index(Field::new("organization_age"))
                    .where_(Field::new("age").equal(30))
                    .explain(),
                umbriel::statements::delete::<Organization>(Organization::table())
                    .with_no_index()
                    .where_(Field::new("age").equal(30))
                    .explain(),
            ),
        ];
        for (statement, indexed, scanned) in plans {
            assert_sql_parses(&indexed);
            let indexed = indexed.execute(&db).await.expect("plan");
            let scanned = scanned.execute(&db).await.expect("plan");
            assert!(uses_the_index(&indexed), "{statement}: {indexed:?}");
            assert!(!uses_the_index(&scanned), "{statement}: {scanned:?}");
        }
        // Explaining wrote nothing.
        let names: Vec<String> =
            read(&db, "SELECT VALUE name FROM organization ORDER BY name;").await;
        assert_eq!(names, ["Acme Corp", "Globex", "Initech"]);
    }

    fn assert_sql_parses(statement: &impl Render) {
        assert_parses(&statement.to_debug_sql());
    }
}

mod explain_prefix {
    use super::*;

    #[tokio::test]
    async fn text_and_json_plans() {
        let db = organizations().await;
        let base = || select(All).from(Organization::table());

        let text: String = explain(base()).execute(&db).await.expect("plan");
        assert!(!text.is_empty());

        let json: Value = explain(base()).json().execute(&db).await.expect("plan");
        assert!(matches!(json, Value::Object(_)), "{json:?}");

        let analyzed: String = explain(base()).analyze().execute(&db).await.expect("plan");
        assert!(analyzed.contains("rows"), "{analyzed}");
    }

    /// The verification task for Q10: whether `EXPLAIN FULL` or `EXPLAIN ANALYZE` executes a
    /// write. The docs describe `EXPLAIN` for statements that do not modify data.
    #[tokio::test]
    async fn explaining_a_write_does_not_write() {
        let db = organizations().await;
        let org = Organization::schema();
        update::<Organization>(Organization::id("acme"))
            .set(org.age.equal_to(99u8))
            .explain_full()
            .execute(&db)
            .await
            .expect("plan");
        run(
            &db,
            Raw::new("EXPLAIN ANALYZE UPDATE organization:acme SET age = 98;"),
        )
        .await;
        let age: u8 = read(&db, "RETURN organization:acme.age;").await;
        assert_eq!(age, 30);
    }
}

mod for_update {
    use super::*;

    #[tokio::test]
    async fn it_runs_inside_a_query_transaction() {
        let db = organizations().await;
        let mut results = umbriel::statements::begin_transaction()
            .query(select(All).from(Organization::id("acme")).for_update())
            .commit_transaction()
            .run(db.clone())
            .await
            .expect("committed");
        let read: Vec<Organization> = results.take(0).expect("records");
        assert_eq!(read.len(), 1);
    }

    #[tokio::test]
    async fn a_conflicting_write_fails_the_commit() {
        let db = organizations().await;
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
        let db = organizations().await;
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

mod storage {
    use super::*;

    #[tokio::test]
    async fn the_memory_engine_refuses_version() {
        let db = organizations().await;
        let at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().expect("a time");
        let refused = select(All)
            .from(Organization::table())
            .version(at)
            .decode::<Organization>()
            .execute(&db)
            .await;
        assert!(refused.is_err(), "{refused:?}");
    }

    /// The 3.3.4 parser reads and discards `VERSION` on `CREATE` and `INSERT`, so even the memory
    /// engine accepts it, and the record is written now.
    #[tokio::test]
    async fn version_on_create_and_insert_is_ignored() {
        let db = db().await;
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

    /// `TEMPFILES` with the SDK's temporary directory: the same rows, and a plan that shows the
    /// clause took effect.
    #[tokio::test]
    async fn tempfiles_sorts_through_the_temporary_directory() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let db = Surreal::new::<Mem>(
            Config::new().temporary_directory(Some(directory.path().to_path_buf())),
        )
        .await
        .expect("in-memory engine");
        db.use_ns("test")
            .use_db("test")
            .await
            .expect("namespace and database");
        for (key, name, age) in [("acme", "Acme Corp", 30), ("globex", "Globex", 20)] {
            organization(key, name, age)
                .save(&db)
                .await
                .expect("saved");
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

        let plan: String = explain(sorted().limit(1).tempfiles())
            .execute(&db)
            .await
            .expect("plan");
        assert!(plan.contains("tempfiles"), "{plan}");
    }
}

mod timeout {
    use super::*;

    /// Q11a: a duration held by a parameter, as a bound value is, is evaluated as the statement's
    /// timeout.
    #[tokio::test]
    async fn a_duration_parameter_is_accepted() {
        let db = organizations().await;
        let mut results = umbriel::chain(
            umbriel::statements::let_("limit").equal_to(Duration::from_secs(5)),
        )
        .chain(
            select(All)
                .from(Organization::table())
                .timeout(Param::new("limit")),
        )
        .run(db.clone())
        .await
        .expect("ran");
        let found: Vec<Organization> = results.take(1).expect("records");
        assert_eq!(found.len(), 3);
    }
}
