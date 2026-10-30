//! Statements: every builder's rendering, checked against SurrealDB's parser, and what the
//! statements do against the in-memory engine.

use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};
use surrealdb::{
    Surreal,
    engine::local::Db,
    types::{Kind, SurrealValue, Value},
};
use umbriel::{
    AliasName, Aliasable, All,
    CrudType::*,
    E, Error, Field, JwtAlgorithm, LinkOne, Merge, Model, NONE, Namespace, Node, Operatable, Param,
    Patchable, Raw, Render, Runnable, SchemaGetter, SetterArray, SetterAssignable, SetterNumeric,
    StatementContext, SurrealCrud, SurrealCrudNode, Table, ValueLike, arr, block, chain, cond,
    count,
    functions::{math, search, string},
    statements::{
        AnalyzerFilter, CreateTarget, DiffOp, Returning, SnowballLanguage, Tokenizer, UserRole,
        begin_transaction, break_, continue_, create, create_only, define_access, define_analyzer,
        define_database, define_event, define_field, define_function, define_index,
        define_namespace, define_param, define_table, define_user, delete, delete_edges,
        delete_only, for_, for_permission, fulltext_analyzer, if_, info_for, input_of, insert,
        insert_relation, jwt_access, let_, live, order, record_access, relate, relate_only,
        remove_access, remove_analyzer, remove_database, remove_event, remove_field,
        remove_function, remove_index, remove_namespace, remove_param, remove_table, remove_user,
        return_, select, select_value, show_changes_for_table, sleep, throw, update, update_only,
        use_,
    },
    value,
};
use umbriel_tests::*;

fn space_ship(key: &str, name: &str) -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id(key),
        name: name.to_owned(),
        created: DateTime::<Utc>::UNIX_EPOCH,
    }
}

fn organization(key: &str, name: &str, age: u8) -> Organization {
    Organization {
        id: Organization::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("statement succeeds");
}

async fn ships(db: &Surreal<Db>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
}

mod create {
    use super::*;

    #[test]
    fn content_binds_the_whole_record() {
        let ship = space_ship("apollo", "Apollo");
        let query = create::<SpaceShip>(SpaceShip::table())
            .content(ship.clone())
            .to_query()
            .expect("valid");
        assert_eq!(query.sql(), "CREATE space_ship CONTENT $_p0;");
        assert_eq!(query.vars().get("_p0"), Some(&ship.into_value()));
        assert_parses(query.sql());
    }

    #[test]
    fn set_assigns_fields() {
        let schema = SpaceShip::schema();
        assert_sql(
            &create::<SpaceShip>(SpaceShip::table()).set([
                schema.id.equal_to(SpaceShip::id("apollo")),
                schema.name.equal_to("Apollo"),
            ]),
            "CREATE space_ship SET id = space_ship:apollo, name = 'Apollo';",
        );
        assert_sql(
            &create::<SpaceShip>(SpaceShip::table()).set(schema.name.equal_to("Apollo")),
            "CREATE space_ship SET name = 'Apollo';",
        );
    }

    #[test]
    fn targets_mix_the_table_records_and_mocks() {
        assert_sql(
            &create::<SpaceShip>(SpaceShip::id("apollo")),
            "CREATE space_ship:apollo;",
        );
        assert_sql(
            &create::<SpaceShip>([
                CreateTarget::table(),
                CreateTarget::from(SpaceShip::id("apollo")),
                CreateTarget::count(3),
                CreateTarget::range(1..4),
            ]),
            "CREATE space_ship, space_ship:apollo, |space_ship:3|, |space_ship:1..4|;",
        );
        assert_render_error(
            &create::<SpaceShip>(CreateTarget::range(4..4)),
            "the mock range `4..4` is empty",
        );
        assert_render_error(
            &create::<SpaceShip>(Table::from("planet")),
            "is not a table of the model `space_ship`",
        );
    }

    #[test]
    fn only_return_modes_and_timeout() {
        let name = SpaceShip::schema().name;
        let base = || create_only::<SpaceShip>(CreateTarget::table()).set(name.equal_to("Apollo"));
        assert_sql(
            &base().return_none().timeout(Duration::from_secs(5)),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN NONE TIMEOUT 5s;",
        );
        assert_sql(
            &base().return_after(),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN AFTER;",
        );
        assert_sql(
            &base().return_diff(),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN DIFF;",
        );
        assert_sql(
            &base().return_value(SpaceShip::schema().name),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN VALUE name;",
        );
        assert_sql(
            &base().return_fields::<Value>(Field::new("name")),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN name;",
        );
    }

    #[tokio::test]
    async fn content_creates_the_record_once() {
        let db = db().await;
        let ship = space_ship("apollo", "Apollo");

        let created = create_only::<SpaceShip>(CreateTarget::table())
            .content(ship.clone())
            .execute(&db)
            .await
            .expect("created")
            .expect("returned");
        assert_eq!(created.id, ship.id);
        let duplicate = create::<SpaceShip>(SpaceShip::table())
            .content(ship)
            .execute(&db)
            .await;
        assert!(
            matches!(
                &duplicate,
                Err(Error::RecordExists { id, statement: Some(StatementContext { index: 0, .. }), .. })
                    if id == "space_ship:apollo"
            ),
            "{duplicate:?}"
        );
    }

    #[tokio::test]
    async fn the_database_generates_a_missing_id() {
        let db = db().await;
        let schema = SpaceShip::schema();
        let created = create::<SpaceShip>(SpaceShip::table())
            .set([
                schema.name.equal_to("Apollo"),
                schema.created.equal_to(DateTime::<Utc>::UNIX_EPOCH),
            ])
            .execute(&db)
            .await
            .expect("created");
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].id.table.as_str(), "space_ship");
        assert_eq!(created[0].name, "Apollo");
    }

    #[tokio::test]
    async fn without_a_data_clause_a_record_holds_only_its_id() {
        let db = db().await;
        run(
            &db,
            create::<SpaceShip>(SpaceShip::id("apollo")).return_none(),
        )
        .await;
        let stored: Vec<BTreeMap<String, Value>> = read(&db, "SELECT * FROM space_ship;").await;
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].keys().collect::<Vec<_>>(), ["id"]);
    }

    #[tokio::test]
    async fn mock_targets_create_counted_and_ranged_records() {
        let db = db().await;
        let schema = SpaceShip::schema();
        let counted = create::<SpaceShip>(CreateTarget::count(3))
            .set([
                schema.name.equal_to("Mock"),
                schema.created.equal_to(DateTime::<Utc>::UNIX_EPOCH),
            ])
            .return_none()
            .execute(&db)
            .await;
        assert!(counted.is_ok(), "{counted:?}");
        let count: usize = read(&db, "RETURN array::len(SELECT VALUE id FROM space_ship);").await;
        assert_eq!(count, 3);

        // Since SurrealDB 3.0.0 a mock range excludes its end: `1..4` creates ids 1 to 3.
        let ranged = create::<SpaceShip>(CreateTarget::range(1..4))
            .set([
                schema.name.equal_to("Ranged"),
                schema.created.equal_to(DateTime::<Utc>::UNIX_EPOCH),
            ])
            .return_value(SpaceShip::schema().id)
            .execute(&db)
            .await
            .expect("created");
        assert_eq!(
            ranged,
            [SpaceShip::id(1), SpaceShip::id(2), SpaceShip::id(3)]
        );
    }

    #[tokio::test]
    async fn return_modes_decode_into_their_types() {
        let db = db().await;
        let names = create::<SpaceShip>(SpaceShip::table())
            .content(space_ship("apollo", "Apollo"))
            .return_value(SpaceShip::schema().name)
            .execute(&db)
            .await
            .expect("created");
        assert_eq!(names, ["Apollo"]);

        let fields = create::<SpaceShip>(SpaceShip::table())
            .content(space_ship("gemini", "Gemini"))
            .return_fields::<BTreeMap<String, String>>(Field::new("name"))
            .execute(&db)
            .await
            .expect("created");
        assert_eq!(
            fields,
            [BTreeMap::from([("name".to_owned(), "Gemini".to_owned())])]
        );

        let () = create::<SpaceShip>(SpaceShip::table())
            .content(space_ship("mercury", "Mercury"))
            .return_none()
            .execute(&db)
            .await
            .expect("created");

        let diff = create_only::<SpaceShip>(CreateTarget::table())
            .content(space_ship("voyager", "Voyager"))
            .return_diff()
            .execute(&db)
            .await
            .expect("created")
            .expect("returned");
        // A created record's diff replaces the whole document: its path is the root pointer `""`.
        assert!(
            matches!(diff.as_slice(), [DiffOp::Replace { path, .. }] if path.is_empty()),
            "{diff:?}"
        );
        assert_eq!(ships(&db).await, ["Apollo", "Gemini", "Mercury", "Voyager"]);
    }

    #[tokio::test]
    async fn created_records_return_their_links_loaded() {
        let db = db().await;
        User {
            id: User::id("alice"),
            name: "Alice".to_owned(),
            ..Default::default()
        }
        .create(&db)
        .await
        .expect("alice");

        let acme = Organization {
            users: vec![User::id("alice")].into(),
            ..organization("acme", "Acme Corp", 10)
        };
        let created = create_only::<Organization>(CreateTarget::table())
            .content(acme)
            .load_links::<Organization>(Organization::get_link_many_fields())
            .expect("links")
            .execute(&db)
            .await
            .expect("created")
            .expect("returned");
        let names = created
            .users
            .values_truthy()
            .into_iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Alice"]);

        let explicit = create_only::<SpaceShip>(CreateTarget::table())
            .content(space_ship("apollo", "Apollo"))
            .return_fields::<BTreeMap<String, Value>>(vec![ValueLike::from(All)])
            .execute(&db)
            .await
            .expect("created")
            .expect("returned");
        assert!(explicit.contains_key("id"));
    }
}

mod insert {
    use super::*;

    #[test]
    fn records_become_rows_over_their_sorted_fields() {
        let query = insert(space_ship("apollo", "Apollo"))
            .to_query()
            .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO space_ship (created, id, name) VALUES ($_p0, $_p1, $_p2);"
        );
        assert_parses(query.sql());

        let query = insert(vec![
            space_ship("apollo", "Apollo"),
            space_ship("gemini", "Gemini"),
        ])
        .to_query()
        .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO space_ship (created, id, name) VALUES ($_p0, $_p1, $_p2), ($_p3, $_p4, $_p5);"
        );
        assert_eq!(
            query.vars().get("_p4"),
            Some(&SpaceShip::id("gemini").into_value())
        );
    }

    #[test]
    fn duplicate_keys_update_and_return_modes() {
        let name = SpaceShip::schema().name;
        let query = insert(space_ship("apollo", "Apollo"))
            .on_duplicate_key_update(name.equal_to("Apollo 2"))
            .return_none()
            .to_query()
            .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO space_ship (created, id, name) VALUES ($_p0, $_p1, $_p2) \
             ON DUPLICATE KEY UPDATE name = $_p3 RETURN NONE;"
        );
        assert_parses(query.sql());

        let org = Organization::schema();
        let input = input_of::<Organization>();
        let query = insert(organization("acme", "Acme Corp", 10))
            .on_duplicate_key_update([
                org.age.increment_by(input.age),
                org.name.equal_to(input.name),
            ])
            .on_duplicate_key_update(org.age.increment_by(1u8))
            .to_query()
            .expect("valid");
        assert!(
            query.sql().ends_with(
                "ON DUPLICATE KEY UPDATE age += $input.age, name = $input.name, age += $_p5;"
            ),
            "{}",
            query.sql()
        );
        assert_parses(query.sql());

        let base = || insert(space_ship("apollo", "Apollo"));
        for (statement, clause) in [
            (base().return_after().to_debug_sql(), " RETURN AFTER;"),
            (base().return_diff().to_debug_sql(), " RETURN DIFF;"),
            (
                base().return_value(SpaceShip::schema().name).to_debug_sql(),
                " RETURN VALUE name;",
            ),
            (
                base()
                    .return_fields::<Value>(Field::new("name"))
                    .to_debug_sql(),
                " RETURN name;",
            ),
        ] {
            assert!(statement.ends_with(clause), "{statement}");
            assert_parses(&statement);
        }
    }

    #[test]
    fn ignore_and_relations() {
        let statement = insert(space_ship("apollo", "Apollo")).ignore();
        assert!(
            statement
                .to_debug_sql()
                .starts_with("INSERT IGNORE INTO space_ship (")
        );
        assert_parses(&statement.to_debug_sql());

        let like = CompanyLikeUser {
            id: CompanyLikeUser::id("acme_alice"),
            r#in: Company::id("acme").into(),
            out: User::id("alice").into(),
            time: Time::default(),
        };
        let query = insert_relation(like.clone())
            .ignore()
            .to_query()
            .expect("valid");
        assert_eq!(query.sql(), "INSERT RELATION IGNORE INTO like $_p0;");
        assert_parses(query.sql());
        let schema = CompanyLikeUser::schema();
        let query = insert_relation(vec![like])
            .on_duplicate_key_update(schema.time.equal_to(Time::default()))
            .return_none()
            .to_query()
            .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT RELATION INTO like $_p0 ON DUPLICATE KEY UPDATE time = $_p1 RETURN NONE;"
        );
        assert_parses(query.sql());
    }

    #[test]
    fn a_select_copies_records() {
        assert_sql(
            &insert::<SpaceShip>(select(All).from(Table::from("old_ship"))),
            "INSERT INTO space_ship (SELECT * FROM old_ship);",
        );
    }

    #[tokio::test]
    async fn inserts_one_or_many_records() {
        let db = db().await;
        let inserted = insert(vec![
            space_ship("apollo", "Apollo"),
            space_ship("gemini", "Gemini"),
        ])
        .execute(&db)
        .await
        .expect("inserted");
        assert_eq!(inserted.len(), 2);

        let one = insert(space_ship("mercury", "Mercury"))
            .return_value(SpaceShip::schema().name)
            .execute(&db)
            .await
            .expect("inserted");
        assert_eq!(one, ["Mercury"]);
        assert_eq!(ships(&db).await, ["Apollo", "Gemini", "Mercury"]);

        let () = insert(space_ship("voyager", "Voyager"))
            .return_none()
            .execute(&db)
            .await
            .expect("inserted");
        let diffs = insert(space_ship("pioneer", "Pioneer"))
            .return_diff()
            .execute(&db)
            .await
            .expect("inserted");
        assert_eq!(diffs.len(), 1);
    }

    /// Every model value holds its full id, so `insert::<T>` stores exactly the records the
    /// targetless `INSERT [ … ]` would.
    #[tokio::test]
    async fn insert_into_stores_what_a_targetless_insert_does() {
        let db = db().await;
        run(
            &db,
            insert(vec![
                space_ship("apollo", "Apollo"),
                space_ship("gemini", "Gemini"),
            ]),
        )
        .await;
        let typed: Vec<BTreeMap<String, Value>> =
            read(&db, "SELECT * FROM space_ship ORDER BY id;").await;

        let other = umbriel_tests::db().await;
        run(
            &other,
            Raw::new(
                "INSERT [{ id: space_ship:apollo, name: 'Apollo', created: d'1970-01-01T00:00:00Z' }, \
                 { id: space_ship:gemini, name: 'Gemini', created: d'1970-01-01T00:00:00Z' }];",
            ),
        )
        .await;
        let targetless: Vec<BTreeMap<String, Value>> =
            read(&other, "SELECT * FROM space_ship ORDER BY id;").await;
        assert_eq!(typed, targetless);
    }

    #[test]
    fn a_table_parameter_target_binds_the_models_table() {
        let query = insert(space_ship("apollo", "Apollo"))
            .into_table_param()
            .return_none()
            .to_query()
            .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO $_p0 (created, id, name) VALUES ($_p1, $_p2, $_p3) RETURN NONE;"
        );
        assert_eq!(
            query.vars().get("_p0"),
            Some(&Value::Table(SpaceShip::table()))
        );
        assert_parses(query.sql());
    }

    #[tokio::test]
    async fn a_table_parameter_target_stores_the_records_in_the_models_table() {
        let db = db().await;
        let inserted = insert(vec![
            space_ship("apollo", "Apollo"),
            space_ship("gemini", "Gemini"),
        ])
        .into_table_param()
        .execute(&db)
        .await
        .expect("inserted");
        assert_eq!(inserted.len(), 2);
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn duplicate_keys_update_the_existing_record() {
        let db = db().await;
        let name = SpaceShip::schema().name;
        run(&db, insert(space_ship("apollo", "Apollo"))).await;
        run(
            &db,
            insert(space_ship("apollo", "Apollo"))
                .on_duplicate_key_update(name.equal_to("Apollo 2")),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo 2"]);
    }

    #[tokio::test]
    async fn duplicate_keys_read_the_rejected_record_through_input() {
        let db = db().await;
        run(&db, insert(organization("acme", "Acme Corp", 10))).await;
        let age = Organization::schema().age;
        let updated = insert(organization("acme", "Acme Corp", 5))
            .on_duplicate_key_update(age.increment_by(input_of::<Organization>().age))
            .return_value(Organization::schema().age)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [15]);
    }

    #[tokio::test]
    async fn ignore_skips_duplicate_ids_and_unique_index_conflicts() {
        let db = db().await;
        run(
            &db,
            Raw::new("DEFINE INDEX ship_name ON space_ship FIELDS name UNIQUE;"),
        )
        .await;
        run(&db, insert(space_ship("apollo", "Apollo"))).await;

        let skipped = insert(vec![
            space_ship("apollo", "Apollo again"),
            space_ship("gemini", "Apollo"),
            space_ship("mercury", "Mercury"),
        ])
        .ignore()
        .execute(&db)
        .await
        .expect("conflicts are skipped");
        assert_eq!(
            skipped
                .iter()
                .map(|ship| ship.name.as_str())
                .collect::<Vec<_>>(),
            ["Mercury"]
        );
        assert_eq!(ships(&db).await, ["Apollo", "Mercury"]);
    }

    /// Since SurrealDB 3.3.4, `IGNORE` skips only id and unique index conflicts.
    #[tokio::test]
    async fn ignore_still_fails_on_other_errors() {
        let db = db().await;
        run(
            &db,
            Raw::new("DEFINE FIELD name ON space_ship ASSERT $value != 'Rejected';"),
        )
        .await;
        let result = insert(space_ship("apollo", "Rejected"))
            .ignore()
            .execute(&db)
            .await;
        assert!(matches!(result, Err(Error::Statement { .. })), "{result:?}");
        assert!(ships(&db).await.is_empty());
    }

    /// `TIMEOUT` is applied to `INSERT`, though SurrealDB documents it only for the other writes:
    /// a record whose field takes longer than the timeout fails the statement and is not stored.
    #[tokio::test]
    async fn a_timeout_fails_a_slow_insert_and_stores_nothing() {
        let db = db().await;
        run(
            &db,
            Raw::new("DEFINE FIELD name ON space_ship VALUE { sleep(500ms); RETURN $value; };"),
        )
        .await;
        let result = insert(space_ship("apollo", "Apollo"))
            .timeout(Duration::from_millis(10))
            .execute(&db)
            .await;
        match &result {
            Err(Error::Statement { source, .. }) => assert!(
                matches!(
                    source.query_details(),
                    Some(surrealdb::types::QueryError::TimedOut { .. })
                ),
                "expected a timeout, got {source}"
            ),
            other => panic!("expected a statement error, got {other:?}"),
        }
        assert!(ships(&db).await.is_empty());
    }

    #[tokio::test]
    async fn relations_insert_and_handle_existing_edge_ids() {
        let db = db().await;
        let like = |note: u32| CompanyLikeUser {
            id: CompanyLikeUser::id("acme_alice"),
            r#in: Company::id("acme").into(),
            out: User::id("alice").into(),
            time: Time {
                connected: DateTime::<Utc>::from_timestamp(i64::from(note), 0).expect("time"),
            },
        };
        let created = insert_relation(like(1))
            .execute(&db)
            .await
            .expect("inserted");
        assert_eq!(created.len(), 1);

        // Since SurrealDB 3.1.5 an existing edge id is an error rather than an update.
        let duplicate = insert_relation(like(2)).execute(&db).await;
        assert!(
            matches!(&duplicate, Err(Error::RecordExists { .. })),
            "{duplicate:?}"
        );

        let ignored = insert_relation(like(2))
            .ignore()
            .execute(&db)
            .await
            .expect("ignored");
        assert!(ignored.is_empty());

        let time = CompanyLikeUser::schema().time;
        let updated = insert_relation(like(2))
            .on_duplicate_key_update(time.equal_to(like(3).time))
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated[0].time.connected.timestamp(), 3);
    }

    #[tokio::test]
    async fn relations_are_rejected_on_lightweight_tables() {
        let db = db().await;
        run(
            &db,
            Raw::new("DEFINE TABLE like TYPE RELATION IN company OUT user LIGHTWEIGHT;"),
        )
        .await;
        let like = CompanyLikeUser {
            id: CompanyLikeUser::id("acme_alice"),
            r#in: Company::id("acme").into(),
            out: User::id("alice").into(),
            time: Time::default(),
        };
        assert!(insert_relation(like).execute(&db).await.is_err());
    }

    #[tokio::test]
    async fn inserting_from_a_select_copies_records() {
        let db = db().await;
        run(
            &db,
            Raw::new(
                "CREATE old_ship:apollo SET name = 'Apollo', created = d'1970-01-01T00:00:00Z';",
            ),
        )
        .await;
        let copied = insert::<SpaceShip>(
            select(vec![
                ValueLike::from(Field::new("name")),
                ValueLike::from(Field::new("created")),
            ])
            .from(Table::from("old_ship")),
        )
        .execute(&db)
        .await
        .expect("copied");
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].name, "Apollo");
    }
}

mod update {
    use super::*;

    #[test]
    fn set_assignments() {
        let org = Organization::schema();
        let acme = Organization::id("acme");
        assert_sql(
            &update::<Organization>(&acme).set(org.age.increment_by(1u8)),
            "UPDATE organization:acme SET age += 1;",
        );
        assert_sql(
            &update::<Organization>(&acme)
                .set([org.name.equal_to("Acme Corp"), org.age.decrement_by(2u8)]),
            "UPDATE organization:acme SET name = 'Acme Corp', age -= 2;",
        );
        assert_sql(
            &update::<Organization>(&acme).set(org.users.append(User::id("alice"))),
            "UPDATE organization:acme SET users += user:alice;",
        );
        assert_sql(
            &update::<Organization>(&acme).set(org.users.remove(User::id("alice"))),
            "UPDATE organization:acme SET users -= user:alice;",
        );
    }

    #[test]
    fn targets_clauses_and_options() {
        let org = Organization::schema();
        assert_sql(
            &update_only::<Organization>(Organization::id("acme"))
                .set(org.age.equal_to(30u8))
                .where_(org.age.lt(30))
                .timeout(Duration::from_secs(1))
                .return_diff(),
            "UPDATE ONLY organization:acme SET age = 30 WHERE age < 30 RETURN DIFF TIMEOUT 1s;",
        );
        assert_sql(
            &update::<Organization>(Organization::table())
                .set(org.age.increment_by(1u8))
                .where_(cond(org.age.gte(18)).and(org.name.is_not(NONE))),
            "UPDATE organization SET age += 1 WHERE (age >= 18) AND (name IS NOT NONE);",
        );
        // A model instance targets its own record.
        assert_sql(
            &update::<Organization>(organization("acme", "Acme Corp", 1))
                .set(org.age.equal_to(2u8)),
            "UPDATE organization:acme SET age = 2;",
        );
    }

    #[test]
    fn targets_outside_the_models_table_are_rejected() {
        let org = Organization::schema();
        assert_render_error(
            &update::<Organization>(User::id("alice")).set(org.age.equal_to(1u8)),
            "does not belong",
        );
        assert_render_error(
            &update::<Organization>(Table::from("user")).set(org.age.equal_to(1u8)),
            "is not a table of the model",
        );
    }

    #[test]
    fn content_merge_replace_and_patch() {
        let acme = Organization::id("acme");
        let content = update::<Organization>(&acme)
            .content(organization("acme", "Acme Corp", 1))
            .to_query()
            .expect("valid");
        assert_eq!(content.sql(), "UPDATE $_p0 CONTENT $_p1;");

        let merge = update::<Organization>(&acme)
            .merge(OrganizationMerge {
                name: Merge::Set("Globex".to_owned()),
                ..Default::default()
            })
            .to_query()
            .expect("valid");
        assert_eq!(merge.sql(), "UPDATE $_p0 MERGE { 'name': $_p1 };");

        let replace = update::<Organization>(&acme)
            .replace(BTreeMap::from([("name".to_owned(), "Globex".to_owned())]))
            .to_query()
            .expect("valid");
        assert_eq!(replace.sql(), "UPDATE $_p0 REPLACE $_p1;");
        for query in [content, merge, replace] {
            assert_parses(query.sql());
        }

        let org = Organization::schema();
        assert_sql(
            &update::<Organization>(&acme).patch(org.name.patch_replace("Globex")),
            "UPDATE organization:acme PATCH [{ op: 'replace', path: '/name', value: 'Globex' }];",
        );
        assert_sql(
            &update::<Organization>(&acme).patch(vec![
                org.name.patch_add("Globex"),
                org.time().connected.patch_remove(),
            ]),
            "UPDATE organization:acme PATCH [{ op: 'add', path: '/name', value: 'Globex' }, \
             { op: 'remove', path: '/time/connected' }];",
        );
        assert_sql(
            &update::<Organization>(&acme).patch(vec![
                org.name.patch_test("Acme Corp"),
                org.name.patch_copy(Field::new("label")),
                org.name.patch_move(Field::new("title")),
            ]),
            "UPDATE organization:acme PATCH [{ op: 'test', path: '/name', value: 'Acme Corp' }, \
             { op: 'copy', path: '/name', from: '/label' }, \
             { op: 'move', path: '/name', from: '/title' }];",
        );
        assert_render_error(
            &update::<Organization>(&acme).patch(org.name.patch_copy(Field::raw("tags[0]"))),
            "The path you have provided is invalid",
        );
    }

    #[test]
    fn unset_merge_explain_and_element_assignments() {
        let org = Organization::schema();
        let acme = Organization::id("acme");
        assert_sql(
            &update::<Organization>(&acme).unset([Field::from(&org.name), Field::from(&org.age)]),
            "UPDATE organization:acme UNSET name, age;",
        );
        assert_sql(
            &update::<Organization>(&acme)
                .unset(&org.name)
                .unset(Field::new("label")),
            "UPDATE organization:acme UNSET name, label;",
        );
        assert_sql(
            &update::<Organization>(&acme).merge(OrganizationMerge {
                age: Merge::Null,
                time: Merge::Fields(TimeMerge {
                    connected: Merge::None,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            "UPDATE organization:acme MERGE { 'time': { 'connected': NONE }, 'age': NULL };",
        );
        // An empty merge is written as built: the builder never becomes a `SELECT`.
        assert_sql(
            &update_only::<Organization>(&acme)
                .merge(OrganizationMerge::default())
                .where_(org.age.gt(1))
                .return_none(),
            "UPDATE ONLY organization:acme MERGE {} WHERE age > 1 RETURN NONE;",
        );
        assert_sql(
            &update::<Organization>(Organization::table())
                .set(org.age.increment_by(1u8))
                .explain(),
            "UPDATE organization SET age += 1 EXPLAIN;",
        );
        assert_sql(
            &update::<Organization>(Organization::table())
                .set(org.age.increment_by(1u8))
                .explain_full(),
            "UPDATE organization SET age += 1 EXPLAIN FULL;",
        );

        let user = User::schema();
        assert_sql(
            &update::<User>(User::id("alice")).set(user.tags.at(0).equal_to("rust")),
            "UPDATE user:alice SET tags[0] = 'rust';",
        );
        assert_sql(
            &update::<User>(User::id("alice")).set(
                user.tags
                    .filter(Param::new("this").equal("go"))
                    .equal_to("rust"),
            ),
            "UPDATE user:alice SET tags[WHERE $this = 'go'] = 'rust';",
        );
    }

    #[test]
    fn return_modes() {
        let org = Organization::schema();
        let base = || update::<Organization>(Organization::id("acme")).set(org.age.equal_to(1u8));
        for (statement, clause) in [
            (base().return_none().to_debug_sql(), " RETURN NONE;"),
            (base().return_before().to_debug_sql(), " RETURN BEFORE;"),
            (base().return_after().to_debug_sql(), " RETURN AFTER;"),
            (base().return_diff().to_debug_sql(), " RETURN DIFF;"),
            (
                base()
                    .return_value(Organization::schema().age)
                    .to_debug_sql(),
                " RETURN VALUE age;",
            ),
            (
                base()
                    .return_fields::<Value>(Field::new("age"))
                    .to_debug_sql(),
                " RETURN age;",
            ),
        ] {
            assert!(statement.ends_with(clause), "{statement}");
            assert_parses(&statement);
        }
    }

    async fn acme(db: &Surreal<Db>) {
        organization("acme", "Acme Corp", 10)
            .create(db)
            .await
            .expect("acme");
    }

    #[tokio::test]
    async fn numeric_and_array_assignments_change_the_record() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let id = Organization::id("acme");

        let updated = update_only::<Organization>(&id)
            .set([
                org.age.increment_by(5u8),
                org.users.append(User::id("alice")),
            ])
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_eq!(updated.age, 15);
        assert_eq!(updated.users.keys_truthy(), [&User::id("alice")]);

        let updated = update_only::<Organization>(&id)
            .set([
                org.age.decrement_by(3u8),
                org.users.remove(User::id("alice")),
            ])
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_eq!(updated.age, 12);
        assert!(updated.users.is_empty());
    }

    #[tokio::test]
    async fn element_assignments_change_one_element() {
        let db = db().await;
        User {
            id: User::id("alice"),
            name: "Alice".to_owned(),
            tags: vec!["go".to_owned(), "sql".to_owned()],
            ..Default::default()
        }
        .create(&db)
        .await
        .expect("alice");
        let tags = User::schema().tags;
        let updated = update::<User>(User::id("alice"))
            .set(tags.at(1).equal_to("rust"))
            .return_value(User::schema().tags)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [vec!["go".to_owned(), "rust".to_owned()]]);
        let updated = update::<User>(User::id("alice"))
            .set(tags.filter(Param::new("this").equal("go")).equal_to("ts"))
            .return_value(User::schema().tags)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [vec!["ts".to_owned(), "rust".to_owned()]]);
    }

    #[tokio::test]
    async fn unset_removes_fields_and_none_fails_on_a_required_schemafull_field() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        run(
            &db,
            update::<Organization>(Organization::id("acme")).unset(&org.name),
        )
        .await;
        let absent: bool = read(&db, "RETURN type::is_none(organization:acme.name);").await;
        assert!(absent);

        run(
            &db,
            Raw::new(
                "DEFINE TABLE strict_org SCHEMAFULL; DEFINE FIELD name ON strict_org TYPE string; \
                 CREATE strict_org:acme SET name = 'Acme Corp';",
            ),
        )
        .await;
        let failed = Raw::new("UPDATE strict_org:acme SET name = NONE;")
            .run(db.clone())
            .await;
        assert!(matches!(failed, Err(Error::Statement { .. })), "{failed:?}");
    }

    #[tokio::test]
    async fn a_missing_target_is_not_created() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let updated = update::<Organization>(Organization::id("missing"))
            .set(org.age.equal_to(1u8))
            .execute(&db)
            .await
            .expect("updated");
        assert!(updated.is_empty());
        let names: Vec<String> = read(&db, "SELECT VALUE name FROM organization;").await;
        assert_eq!(names, ["Acme Corp"]);
    }

    /// Since SurrealDB 3.3.0 `WHERE` is checked before the data clause, against the record as it
    /// was before the statement.
    #[tokio::test]
    async fn where_is_evaluated_before_the_data_clause() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let updated = update::<Organization>(Organization::table())
            .set(org.age.equal_to(30u8))
            .where_(org.age.equal(10))
            .return_value(Organization::schema().age)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [30]);
        let none = update::<Organization>(Organization::table())
            .set(org.age.equal_to(10u8))
            .where_(org.age.equal(10))
            .execute(&db)
            .await
            .expect("updated");
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn explain_returns_a_plan_and_writes_nothing() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let plan = update::<Organization>(Organization::table())
            .set(org.age.equal_to(99u8))
            .explain()
            .execute(&db)
            .await
            .expect("explained");
        assert!(!plan.is_empty());
        let ages: Vec<u8> = read(&db, "SELECT VALUE age FROM organization;").await;
        assert_eq!(ages, [10]);
    }

    #[tokio::test]
    async fn return_modes_decode_into_their_types() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let id = Organization::id("acme");
        let () = update::<Organization>(&id)
            .set(org.age.equal_to(11u8))
            .return_none()
            .execute(&db)
            .await
            .expect("updated");
        let after = update::<Organization>(&id)
            .set(org.age.equal_to(12u8))
            .return_after()
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(after[0].age, 12);
        let ages = update::<Organization>(&id)
            .set(org.age.equal_to(13u8))
            .return_value(Organization::schema().age)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(ages, [13]);
        let diff = update_only::<Organization>(&id)
            .set(org.age.equal_to(14u8))
            .return_diff()
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_eq!(
            diff,
            [DiffOp::Replace {
                path: "/age".to_owned(),
                value: 14_i64.into_value(),
            }]
        );
        let fields = update::<Organization>(&id)
            .set(org.name.equal_to("Globex"))
            .return_fields::<BTreeMap<String, String>>(Field::new("name"))
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(
            fields,
            [BTreeMap::from([("name".to_owned(), "Globex".to_owned())])]
        );
    }

    #[tokio::test]
    async fn conditions_choose_the_updated_records() {
        let db = db().await;
        acme(&db).await;
        organization("globex", "Globex", 40)
            .create(&db)
            .await
            .expect("globex");
        let org = Organization::schema();

        let updated = update::<Organization>(Organization::table())
            .set(org.age.increment_by(1u8))
            .where_(org.age.gt(20))
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].age, 41);
    }

    #[tokio::test]
    async fn content_replaces_and_merge_combines() {
        let db = db().await;
        acme(&db).await;
        let id = Organization::id("acme");

        let replaced = update_only::<Organization>(&id)
            .content(organization("acme", "Acme Corp", 99))
            .execute(&db)
            .await
            .expect("replaced")
            .expect("returned");
        assert_eq!(replaced.age, 99);

        let merged = update_only::<Organization>(&id)
            .merge(OrganizationMerge {
                name: Merge::Set("Globex".to_owned()),
                ..Default::default()
            })
            .execute(&db)
            .await
            .expect("merged")
            .expect("returned");
        assert_eq!((merged.name.as_str(), merged.age), ("Globex", 99));

        // A nested merge changes only the fields it names; the stored object keeps the rest.
        run(
            &db,
            Raw::new("UPDATE organization:acme SET time.zone = 'UTC', tags = ['a', 'b'];"),
        )
        .await;
        run(
            &db,
            update::<Organization>(&id).merge(OrganizationMerge {
                time: Merge::Fields(TimeMerge {
                    connected: Merge::Set(DateTime::<Utc>::from_timestamp(60, 0).expect("time")),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        )
        .await;
        let zone: String = read(&db, "RETURN organization:acme.time.zone;").await;
        assert_eq!(zone, "UTC");
        let connected: DateTime<Utc> = read(&db, "RETURN organization:acme.time.connected;").await;
        assert_eq!(connected.timestamp(), 60);

        // An array in a merge replaces the stored array whole.
        run(
            &db,
            Raw::new("UPDATE organization:acme MERGE { tags: ['c'] };"),
        )
        .await;
        let tags: Vec<String> = read(&db, "RETURN organization:acme.tags;").await;
        assert_eq!(tags, ["c"]);

        // REPLACE drops fields the replacement lacks.
        run(
            &db,
            update::<Organization>(&id)
                .replace(BTreeMap::from([("name".to_owned(), "Initech".to_owned())])),
        )
        .await;
        let absent: bool = read(&db, "RETURN type::is_none(organization:acme.age);").await;
        assert!(absent);
    }

    #[tokio::test]
    async fn patch_operations_change_the_record() {
        let db = db().await;
        acme(&db).await;
        let id = Organization::id("acme");
        let org = Organization::schema();

        let patched = update_only::<Organization>(&id)
            .patch(org.name.patch_replace("Globex"))
            .execute(&db)
            .await
            .expect("patched")
            .expect("returned");
        assert_eq!(patched.name, "Globex");

        let names = update::<Organization>(&id)
            .patch(org.name.patch_add("Initech"))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("patched");
        assert_eq!(names, ["Initech"]);

        // "test" becomes "text".
        run(
            &db,
            update::<Organization>(&id).set(org.name.equal_to("test")),
        )
        .await;
        let names = update::<Organization>(&id)
            .patch(org.name.patch_change("@@ -1,4 +1,4 @@\n te\n-s\n+x\n t\n"))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("patched");
        assert_eq!(names, ["text"]);

        run(
            &db,
            Raw::new("UPDATE organization:acme SET label = 'Copied', title = 'Moved';"),
        )
        .await;
        let names = update::<Organization>(&id)
            .patch(org.name.patch_copy(Field::new("label")))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("copied");
        assert_eq!(names, ["Copied"]);
        let names = update::<Organization>(&id)
            .patch(org.name.patch_move(Field::new("title")))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("moved");
        assert_eq!(names, ["Moved"]);
        let title_gone: bool = read(&db, "RETURN type::is_none(organization:acme.title);").await;
        assert!(title_gone);

        let names = update::<Organization>(&id)
            .patch(vec![
                org.name.patch_test("Moved"),
                org.name.patch_replace("Tested"),
            ])
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("test passed");
        assert_eq!(names, ["Tested"]);
        let failed = update::<Organization>(&id)
            .patch(vec![
                org.name.patch_test("Wrong"),
                org.name.patch_replace("Never"),
            ])
            .execute(&db)
            .await;
        assert!(matches!(failed, Err(Error::Statement { .. })), "{failed:?}");
        let name: String = read(&db, "RETURN organization:acme.name;").await;
        assert_eq!(name, "Tested");

        run(
            &db,
            update::<Organization>(&id).patch(org.time().connected.patch_remove()),
        )
        .await;
        let removed: bool = read(
            &db,
            "RETURN type::is_none(organization:acme.time.connected);",
        )
        .await;
        assert!(removed);
    }

    #[tokio::test]
    async fn return_before_gives_the_previous_state() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let before = update::<Organization>(Organization::id("acme"))
            .set(org.age.equal_to(50u8))
            .return_before()
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(before[0].age, 10);
    }
}

mod delete {
    use super::*;

    #[test]
    fn targets_clauses_and_options() {
        let org = Organization::schema();
        assert_sql(
            &delete::<Organization>(Organization::id("acme")),
            "DELETE organization:acme;",
        );
        assert_sql(
            &delete::<Organization>(Organization::table())
                .where_(org.age.lt(18))
                .timeout(Duration::from_secs(2))
                .return_before(),
            "DELETE organization WHERE age < 18 RETURN BEFORE TIMEOUT 2s;",
        );
        assert_sql(
            &delete::<Organization>(Organization::id("acme")).return_diff(),
            "DELETE organization:acme RETURN DIFF;",
        );
        assert_sql(
            &delete_only::<Organization>(Organization::id("acme")),
            "DELETE ONLY organization:acme;",
        );
        assert_sql(
            &delete::<Organization>(Organization::table()).explain(),
            "DELETE organization EXPLAIN;",
        );
        assert_sql(
            &delete_edges::<CompanyLikeUser, _>(&Company::with(Company::id("acme")).like__(E)),
            "DELETE company:acme->like;",
        );
        assert_render_error(
            &delete::<Organization>(User::id("alice")),
            "does not belong",
        );
    }

    #[tokio::test]
    async fn delete_returns_nothing_by_default_and_only_before_or_diff_on_request() {
        let db = db().await;
        organization("acme", "Acme Corp", 10)
            .create(&db)
            .await
            .expect("acme");
        let () = delete::<Organization>(Organization::id("missing"))
            .execute(&db)
            .await
            .expect("deleted nothing");
        let missing = delete_only::<Organization>(Organization::id("missing"))
            .return_before()
            .execute(&db)
            .await
            .expect("deleted nothing");
        assert!(missing.is_none());
        let diff = delete_only::<Organization>(Organization::id("acme"))
            .return_diff()
            .execute(&db)
            .await
            .expect("deleted")
            .expect("returned");
        assert!(!diff.is_empty());
    }

    #[tokio::test]
    async fn explain_deletes_nothing() {
        let db = db().await;
        organization("acme", "Acme Corp", 10)
            .create(&db)
            .await
            .expect("acme");
        let plan = delete::<Organization>(Organization::table())
            .explain()
            .execute(&db)
            .await
            .expect("explained");
        assert!(!plan.is_empty());
        let left: Vec<String> = read(&db, "SELECT VALUE name FROM organization;").await;
        assert_eq!(left, ["Acme Corp"]);
    }

    #[tokio::test]
    async fn an_edge_path_deletes_only_those_edges() {
        let db = db().await;
        run(
            &db,
            Raw::new(
                "CREATE company:acme, company:globex, user:alice; \
                 RELATE company:acme->like->user:alice; RELATE company:globex->like->user:alice;",
            ),
        )
        .await;
        run(
            &db,
            delete_edges::<CompanyLikeUser, _>(&Company::with(Company::id("acme")).like__(E)),
        )
        .await;
        let left: Vec<String> = read(&db, "SELECT VALUE record::id(in) FROM like;").await;
        assert_eq!(left, ["globex"]);
    }

    #[tokio::test]
    async fn deletes_by_id_or_condition() {
        let db = db().await;
        organization("acme", "Acme Corp", 10)
            .create(&db)
            .await
            .expect("acme");
        organization("globex", "Globex", 40)
            .create(&db)
            .await
            .expect("globex");
        organization("initech", "Initech", 50)
            .create(&db)
            .await
            .expect("initech");
        let org = Organization::schema();

        let deleted = delete::<Organization>(Organization::id("acme"))
            .return_before()
            .execute(&db)
            .await
            .expect("deleted");
        assert_eq!(deleted[0].name, "Acme Corp");

        run(
            &db,
            delete::<Organization>(Organization::table()).where_(org.age.gt(45)),
        )
        .await;
        let left: Vec<String> = read(&db, "SELECT VALUE name FROM organization;").await;
        assert_eq!(left, ["Globex"]);
    }
}

mod relate {
    use super::*;

    fn acme_likes_alice() -> Field {
        Company::with(Company::id("acme"))
            .like__(E)
            .user(User::id("alice"))
            .into()
    }

    #[test]
    fn relates_records_subqueries_and_options() {
        assert_sql(
            &relate::<CompanyLikeUser>(acme_likes_alice()).return_none(),
            "RELATE company:acme->like->user:alice RETURN NONE;",
        );
        assert_sql(
            &relate::<CompanyLikeUser>(acme_likes_alice())
                .or_update()
                .return_before(),
            "RELATE OR UPDATE company:acme->like->user:alice RETURN BEFORE;",
        );
        assert_sql(
            &relate_only::<CompanyLikeUser>(acme_likes_alice()).or_update(),
            "RELATE ONLY OR UPDATE company:acme->like->user:alice;",
        );
        assert_sql(
            &relate::<CompanyLikeUser>(
                Company::with(Company::id("acme"))
                    .like__(CompanyLikeUser::id("acme_alice"))
                    .user(User::id("alice")),
            ),
            "RELATE company:acme->like:acme_alice->user:alice;",
        );
        assert_sql(
            &relate::<CompanyLikeUser>(
                Company::with(vec![Company::id("acme"), Company::id("globex")])
                    .like__(E)
                    .user([User::id("alice"), User::id("bob")]),
            ),
            "RELATE [company:acme, company:globex]->like->[user:alice, user:bob];",
        );
        let base = || relate::<CompanyLikeUser>(acme_likes_alice());
        for (statement, clause) in [
            (base().return_after().to_debug_sql(), " RETURN AFTER;"),
            (base().return_diff().to_debug_sql(), " RETURN DIFF;"),
            (
                base()
                    .return_value(CompanyLikeUser::schema().time)
                    .to_debug_sql(),
                " RETURN VALUE time;",
            ),
            (
                base()
                    .return_fields::<Value>(Field::new("time"))
                    .to_debug_sql(),
                " RETURN time;",
            ),
        ] {
            assert!(statement.ends_with(clause), "{statement}");
            assert_parses(&statement);
        }
        assert_sql(
            &relate_only::<CompanyLikeUser>(acme_likes_alice()).set(
                CompanyLikeUser::schema()
                    .time()
                    .connected
                    .equal_to(Field::from_expr(umbriel::functions::time::now!())),
            ),
            "RELATE ONLY company:acme->like->user:alice SET time.connected = time::now();",
        );
        assert_sql(
            &relate::<CompanyLikeUser>(
                Company::with(select(All).from(Company::table()))
                    .like__(E)
                    .user(select(All).from(User::table())),
            )
            .timeout(Duration::from_secs(1)),
            "RELATE (SELECT * FROM company)->like->(SELECT * FROM user) TIMEOUT 1s;",
        );
        let content = relate::<CompanyLikeUser>(acme_likes_alice())
            .content(Like::default())
            .to_query()
            .expect("valid");
        assert_eq!(content.sql(), "RELATE $_p0->like->$_p1 CONTENT $_p2;");
        assert_parses(content.sql());
    }

    /// A typed id of another model does not compile (`tests/ui/fail/relate_wrong_endpoint.rs`);
    /// an untyped SDK id is checked when the query is rendered.
    #[test]
    fn untyped_ids_of_another_table_are_rejected() {
        let path: Field = Company::with(surrealdb::types::RecordId::from(User::id("alice")))
            .like__(E)
            .user(User::id("bob"))
            .into();
        assert_render_error(&relate::<CompanyLikeUser>(path), "does not belong to table");
        let path: Field = Company::with(Company::id("acme"))
            .like__(E)
            .user(vec![User::id("bob")])
            .into();
        assert!(relate::<CompanyLikeUser>(path).to_query().is_ok());
    }

    async fn companies_and_users(db: &Surreal<Db>) {
        for (key, name) in [("acme", "Acme Corp"), ("globex", "Globex")] {
            Company {
                id: Company::id(key),
                name: name.to_owned(),
                ..Default::default()
            }
            .create(db)
            .await
            .expect("company");
        }
        for (key, name) in [("alice", "Alice"), ("bob", "Bob")] {
            User {
                id: User::id(key),
                name: name.to_owned(),
                ..Default::default()
            }
            .create(db)
            .await
            .expect("user");
        }
    }

    fn connected_at(seconds: i64) -> umbriel::Setter {
        CompanyLikeUser::schema()
            .time()
            .connected
            .equal_to(DateTime::<Utc>::from_timestamp(seconds, 0).expect("time"))
    }

    #[tokio::test]
    async fn an_edge_id_in_the_path_names_the_edge() {
        let db = db().await;
        companies_and_users(&db).await;
        let path = || -> Field {
            Company::with(Company::id("acme"))
                .like__(CompanyLikeUser::id("acme_alice"))
                .user(User::id("alice"))
                .into()
        };
        let edges = relate::<CompanyLikeUser>(path())
            .set(connected_at(1))
            .execute(&db)
            .await
            .expect("related");
        assert_eq!(edges[0].id, CompanyLikeUser::id("acme_alice"));

        let before = relate::<CompanyLikeUser>(path())
            .or_update()
            .set(connected_at(2))
            .return_before()
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(before[0].time.connected.timestamp(), 1);
        let connected: DateTime<Utc> = read(&db, "RETURN like:acme_alice.time.connected;").await;
        assert_eq!(connected.timestamp(), 2);

        // Assigning an existing edge's id on the create path fails.
        let clash = relate::<CompanyLikeUser>(
            Company::with(Company::id("globex"))
                .like__(E)
                .user(User::id("bob")),
        )
        .set(
            CompanyLikeUser::schema()
                .id
                .equal_to(CompanyLikeUser::id("acme_alice")),
        )
        .execute(&db)
        .await;
        assert!(
            matches!(&clash, Err(Error::RecordExists { .. })),
            "{clash:?}"
        );
    }

    /// What SurrealDB 3.3.4 does with ids in a `RELATE`'s data, which the typed check below
    /// enforces before running. Under a table step nothing is rejected: a record of another table
    /// is moved into the edge table, and `in` and `out` are replaced by the path's endpoints. Under
    /// an edge record step, a different id, `in` or `out` fails the statement.
    #[tokio::test]
    async fn content_ids_follow_the_database_rules() {
        let db = db().await;
        companies_and_users(&db).await;
        run(
            &db,
            Raw::new(
                "RELATE company:globex->like->user:bob \
                 CONTENT { id: other:moved, in: company:acme, out: user:alice };",
            ),
        )
        .await;
        let stored: Vec<Vec<String>> = read(
            &db,
            "SELECT VALUE [<string> id, record::id(in), record::id(out)] FROM like;",
        )
        .await;
        assert_eq!(stored, [["like:moved", "globex", "bob"]]);

        for sql in [
            "RELATE company:acme->like:first->user:alice CONTENT { id: like:other };",
            "RELATE company:acme->like:second->user:alice CONTENT { in: company:globex };",
            "RELATE company:acme->like:third->user:alice CONTENT { out: user:bob };",
        ] {
            let (_, source, _) = statement_error(Raw::new(sql).run(db.clone()).await);
            assert!(
                source
                    .to_string()
                    .contains("but a specific record has been specified"),
                "{sql}: {source}"
            );
        }
    }

    /// A `CONTENT` id, `in` or `out` that contradicts the typed path fails when the statement is
    /// rendered; matching or unset ones run.
    #[tokio::test]
    async fn content_contradicting_the_path_fails_before_it_runs() {
        let db = db().await;
        companies_and_users(&db).await;
        let path = || -> Field {
            Company::with(Company::id("acme"))
                .like__(CompanyLikeUser::id("acme_alice"))
                .user(User::id("alice"))
                .into()
        };
        let like = |key: &str, from: &str, to: &str| CompanyLikeUser {
            id: CompanyLikeUser::id(key),
            r#in: LinkOne::from(Company::id(from)),
            out: LinkOne::from(User::id(to)),
            ..Default::default()
        };
        for (content, message) in [
            (
                like("other", "acme", "alice"),
                "is not the edge record `like:acme_alice` in the path",
            ),
            (
                like("acme_alice", "globex", "alice"),
                "the content's `in` is `company:globex`, but the path relates `company:acme`",
            ),
            (
                like("acme_alice", "acme", "bob"),
                "the content's `out` is `user:bob`, but the path relates `user:alice`",
            ),
        ] {
            assert_render_error(&relate::<CompanyLikeUser>(path()).content(content), message);
        }

        // Every pair of a list must agree, so a single `in` cannot match two endpoints.
        let pairs: Field = Company::with(vec![Company::id("acme"), Company::id("globex")])
            .like__(E)
            .user(User::id("alice"))
            .into();
        assert_render_error(
            &relate::<CompanyLikeUser>(pairs).content(like("pair", "acme", "alice")),
            "but the path relates `company:globex`",
        );

        // A parameter endpoint is left to the database, but the known `out` is still checked.
        let dynamic: Field = Company::with(Param::new("company"))
            .like__(E)
            .user(User::id("alice"))
            .into();
        assert!(
            relate::<CompanyLikeUser>(dynamic.clone())
                .content(like("dynamic", "globex", "alice"))
                .to_query()
                .is_ok()
        );
        assert_render_error(
            &relate::<CompanyLikeUser>(dynamic).content(like("dynamic", "globex", "bob")),
            "the content's `out` is `user:bob`",
        );

        let edges = relate::<CompanyLikeUser>(path())
            .content(like("acme_alice", "acme", "alice"))
            .execute(&db)
            .await
            .expect("related");
        assert_eq!(edges[0].id, CompanyLikeUser::id("acme_alice"));
        let unset = relate::<CompanyLikeUser>(acme_likes_alice())
            .content(CompanyLikeUser {
                id: CompanyLikeUser::id("unset"),
                ..Default::default()
            })
            .execute(&db)
            .await
            .expect("related");
        assert_eq!(unset[0].r#in.key(), Some(&Company::id("acme")));
    }

    #[tokio::test]
    async fn endpoint_lists_relate_every_pair() {
        let db = db().await;
        companies_and_users(&db).await;
        let edges = relate::<CompanyLikeUser>(
            Company::with([Company::id("acme"), Company::id("globex")])
                .like__(E)
                .user(User::id("alice")),
        )
        .set(connected_at(1))
        .execute(&db)
        .await
        .expect("related");
        assert_eq!(edges.len(), 2);
    }

    /// Dynamic endpoints are checked by the database only: `TYPE RELATION IN … OUT …` restricts
    /// their tables, and `ENFORCED` requires the records to exist.
    #[tokio::test]
    async fn type_relation_rejects_a_dynamic_endpoint_of_another_table() {
        let db = db().await;
        companies_and_users(&db).await;
        run(
            &db,
            Raw::new(
                "DEFINE TABLE like TYPE RELATION IN company OUT user; \
                 CREATE organization:acme SET name = 'Acme Corp';",
            ),
        )
        .await;
        // The subquery's records are known only when the statement runs.
        let wrong_table = relate::<CompanyLikeUser>(
            Company::with(select(All).from(Organization::table()))
                .like__(E)
                .user(User::id("alice")),
        )
        .set(connected_at(1))
        .execute(&db)
        .await;
        assert!(wrong_table.is_err(), "{wrong_table:?}");
    }

    #[tokio::test]
    async fn enforced_rejects_a_missing_endpoint_record() {
        let db = db().await;
        companies_and_users(&db).await;
        run(
            &db,
            Raw::new("DEFINE TABLE like TYPE RELATION IN company OUT user ENFORCED;"),
        )
        .await;
        let missing = relate::<CompanyLikeUser>(
            Company::with(Company::id("initech"))
                .like__(E)
                .user(User::id("alice")),
        )
        .set(connected_at(1))
        .execute(&db)
        .await;
        assert!(missing.is_err(), "{missing:?}");
        let existing = relate::<CompanyLikeUser>(acme_likes_alice())
            .set(connected_at(1))
            .execute(&db)
            .await
            .expect("related");
        assert_eq!(existing.len(), 1);
    }

    #[tokio::test]
    async fn relating_creates_edges_between_every_pair() {
        let db = db().await;
        companies_and_users(&db).await;

        let edges = relate::<CompanyLikeUser>(
            Company::with(select(All).from(Company::table()))
                .like__(E)
                .user(select(All).from(User::table())),
        )
        .set(
            CompanyLikeUser::schema()
                .time()
                .connected
                .equal_to(Field::from_expr(umbriel::functions::time::now!())),
        )
        .execute(&db)
        .await
        .expect("related");
        assert_eq!(edges.len(), 4);

        let liked: Vec<String> = read(
            &db,
            "SELECT VALUE ->like->user.name FROM ONLY company:acme;",
        )
        .await;
        assert_eq!(liked.len(), 2);
    }
}

mod select {
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

    async fn seeded() -> Surreal<Db> {
        let db = db().await;
        run(
            &db,
            Raw::new(
                "CREATE user:alice SET name = 'Alice', age = 30, country = 'CA', emails = ['a@example.com', 'alice@example.com'];
                 CREATE user:bob SET name = 'Bob', age = 20, country = 'CA', emails = ['bob@example.com'];
                 CREATE user:carol SET name = 'Carol', age = 40, country = 'NO', emails = ['carol@example.com'];",
            ),
        )
        .await;
        db
    }

    #[tokio::test]
    async fn filters_orders_and_pages() {
        let db = seeded().await;
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
        let db = seeded().await;
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
        let db = db().await;
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
        let db = db().await;
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

mod definitions {
    use super::*;

    #[test]
    fn table_definitions() {
        let age = Field::new("age");
        let name = Field::new("name");
        assert_sql(&define_table(Table::from("user")), "DEFINE TABLE user;");
        assert_sql(
            &define_table(Table::from("user"))
                .schemaless()
                .permissions_none(),
            "DEFINE TABLE user SCHEMALESS PERMISSIONS NONE;",
        );
        assert_sql(
            &define_table(Table::from("user"))
                .schemafull()
                .permissions_full(),
            "DEFINE TABLE user SCHEMAFULL PERMISSIONS FULL;",
        );
        assert_sql(
            &define_table(Table::from("adult"))
                .drop()
                .as_(select(All).from(Table::from("user")).where_(age.gte(18)))
                .permissions(for_permission(Select).where_(age.gte(18)))
                .permissions([
                    for_permission([Create, Delete]).where_(name.is("Alice")),
                    for_permission(Update).where_(age.lte(130)),
                ]),
            "DEFINE TABLE adult DROP AS \n\tSELECT * FROM user WHERE age >= 18\n\
             PERMISSIONS\nFOR select\n\tWHERE age >= 18\n\
             FOR create, delete\n\tWHERE name IS 'Alice'\nFOR update\n\tWHERE age <= 130;",
        );
    }

    #[test]
    fn field_definitions() {
        let name = Field::new("name");
        let age = Field::new("age");
        assert_sql(
            &define_field(Field::new("email"))
                .on_table(Table::from("user"))
                .type_(Kind::String)
                .default("alice@example.com")
                .assert(cond(value().is_not(NONE)).and(value().contains("@")))
                .permissions(for_permission(Select).where_(age.gte(18)))
                .permissions(for_permission([Create, Update]).where_(name.is("Alice"))),
            "DEFINE FIELD email ON TABLE user TYPE string DEFAULT 'alice@example.com' \
             ASSERT ($value IS NOT NONE) AND ($value CONTAINS '@')\n\
             PERMISSIONS\nFOR select\n\tWHERE age >= 18\nFOR create, update\n\tWHERE name IS 'Alice';",
        );
        assert_sql(
            &define_field(Field::new("email"))
                .on_table(Table::from("user"))
                .value(string::lowercase!(Param::new("value"))),
            "DEFINE FIELD email ON TABLE user VALUE string::lowercase($value);",
        );
        assert_sql(
            &define_field(Field::new("is_adult"))
                .on_table(Table::from("user"))
                .computed(age.gte(18))
                .type_(Kind::Bool)
                .permissions_none(),
            "DEFINE FIELD is_adult ON TABLE user COMPUTED age >= 18 TYPE bool PERMISSIONS NONE;",
        );
        // Definitions are stored, so their values are written inline even when bound.
        let query = define_field(Field::new("score"))
            .on_table(Table::from("user"))
            .default(0)
            .to_query()
            .expect("valid");
        assert_eq!(query.sql(), "DEFINE FIELD score ON TABLE user DEFAULT 0;");
        assert!(query.vars().is_empty());
    }

    #[test]
    fn computed_fields_reject_other_clauses_nested_paths_and_id() {
        let age = Field::new("age");
        let computed = |field: Field| {
            define_field(field)
                .on_table(Table::from("person"))
                .computed(age.gte(18))
        };
        assert!(computed(Field::new("is_adult")).to_query().is_ok());
        assert!(computed(Field::new("is-adult")).to_query().is_ok());
        assert_render_error(
            &computed(Field::new("is_adult")).default(false),
            "cannot also have",
        );
        assert_render_error(
            &computed(Field::new("is_adult")).value(false),
            "cannot also have",
        );
        assert_render_error(
            &computed(Field::new("is_adult")).assert(value().is_not(NONE)),
            "cannot also have",
        );
        assert_render_error(
            &computed(Field::new("flags").with_field("is_adult")),
            "top-level field",
        );
        assert_render_error(&computed(Field::raw("flags.is_adult")), "top-level field");
        assert_render_error(&computed(Field::new("id")), "`id` field");
    }

    #[test]
    fn index_definitions() {
        let user = Table::from("user");
        assert_sql(
            &define_index("user_email")
                .on_table(user.clone())
                .fields(Field::new("email"))
                .unique(),
            "DEFINE INDEX user_email ON TABLE user FIELDS email UNIQUE;",
        );
        assert_sql(
            &define_index("user_name")
                .on_table(user.clone())
                .columns(&[Field::new("first"), Field::new("last")]),
            "DEFINE INDEX user_name ON TABLE user COLUMNS first, last;",
        );
        assert_sql(
            &define_index("post_content")
                .on_table(Table::from("post"))
                .fields(Field::new("content"))
                .fulltext(fulltext_analyzer("ascii").bm25(1.2, 0.75).highlights()),
            "DEFINE INDEX post_content ON TABLE post FIELDS content \
             FULLTEXT ANALYZER ascii BM25(1.2, 0.75) HIGHLIGHTS;",
        );
        assert_sql(
            &define_index("post_title")
                .on_table(Table::from("post"))
                .fields(Field::new("title"))
                .fulltext(fulltext_analyzer("ascii")),
            "DEFINE INDEX post_title ON TABLE post FIELDS title FULLTEXT ANALYZER ascii;",
        );
    }

    #[test]
    fn define_analyzer_event_param_namespace_database_and_function() {
        assert_sql(
            &define_analyzer("ascii")
                .tokenizers([Tokenizer::Class, Tokenizer::Blank])
                .filters([
                    AnalyzerFilter::Lowercase,
                    AnalyzerFilter::Ascii,
                    AnalyzerFilter::Edgengram(2, 15),
                    AnalyzerFilter::Snowball(SnowballLanguage::English),
                ]),
            "DEFINE ANALYZER ascii TOKENIZERS Class,Blank \
             FILTERS lowercase,ascii,edgengram(2, 15),snowball(english);",
        );
        assert_sql(
            &define_event("email_changed")
                .on_table(Table::from("user"))
                .when(
                    Field::from(Param::new("before"))
                        .with_field("email")
                        .not_equal(Field::from(Param::new("after")).with_field("email")),
                )
                .then(
                    create::<SpaceShip>(SpaceShip::table())
                        .set(SpaceShip::schema().name.equal_to("Apollo")),
                ),
            "DEFINE EVENT email_changed ON TABLE user WHEN $before.email != $after.email \
             THEN (CREATE space_ship SET name = 'Apollo');",
        );
        umbriel::create_param_name_fn!(endpoint_base);
        assert_sql(
            &define_param(endpoint_base()).value("https://example.com"),
            "DEFINE PARAM $endpoint_base VALUE 'https://example.com';",
        );
        assert_sql(
            &define_namespace(Namespace::new("acme")),
            "DEFINE NAMESPACE acme;",
        );
        assert_sql(&define_database("acme"), "DEFINE DATABASE acme;");

        let total = Param::new("total");
        assert_sql(
            &define_function("add_one")
                .arguments(vec![umbriel::statements::FunctionArgument {
                    name: total.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&total).add(1)))),
            "DEFINE FUNCTION fn::add_one($total: int) {\nRETURN $total + 1;\n};",
        );
    }

    #[test]
    fn user_definitions() {
        assert_sql(
            &define_user("alice")
                .on_root()
                .password("example-password")
                .role(UserRole::Owner),
            "DEFINE USER alice ON ROOT PASSWORD 'example-password' ROLES OWNER;",
        );
        assert_sql(
            &define_user("bob")
                .on_namespace()
                .passhash("$argon2id$example")
                .role(UserRole::Editor),
            "DEFINE USER bob ON NAMESPACE PASSHASH '$argon2id$example' ROLES EDITOR;",
        );
        assert_sql(
            &define_user("carol")
                .on_database()
                .password("example")
                .role(UserRole::Viewer),
            "DEFINE USER carol ON DATABASE PASSWORD 'example' ROLES VIEWER;",
        );
    }

    #[test]
    fn access_definitions() {
        let signup =
            Raw::new("CREATE user SET email = $email, pass = crypto::argon2::generate($pass)");
        let signin = Raw::new(
            "SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)",
        );
        assert_sql(
            &define_access("account").on_database().record(
                record_access()
                    .signup(signup)
                    .signin(signin)
                    .with_jwt(JwtAlgorithm::HS512, "example-key")
                    .duration_for_token(Duration::from_secs(15 * 60))
                    .duration_for_session(Duration::from_secs(12 * 60 * 60)),
            ),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD \
             SIGNUP (CREATE user SET email = $email, pass = crypto::argon2::generate($pass)) \
             SIGNIN (SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)) \
             WITH JWT ALGORITHM HS512 KEY 'example-key' \
             DURATION FOR TOKEN 15m, FOR SESSION 12h;",
        );
        assert_sql(
            &define_access("account").on_database().record(
                record_access()
                    .signin(select(All).from(Table::from("user")))
                    .duration_for_session(Duration::from_secs(60 * 60)),
            ),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD SIGNIN (SELECT * FROM user) \
             DURATION FOR SESSION 1h;",
        );
        assert_sql(
            &define_access("api").on_namespace().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::RS256)
                    .key("example-public-key"),
            ),
            "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key';",
        );
        assert_sql(
            &define_access("issuer").on_root().jwt(
                jwt_access()
                    .url("https://example.com/jwks.json")
                    .duration_for_session(Duration::from_secs(60)),
            ),
            "DEFINE ACCESS issuer ON ROOT TYPE JWT URL 'https://example.com/jwks.json' \
             DURATION FOR SESSION 1m;",
        );
    }

    #[test]
    fn define_access_rejects_invalid_combinations() {
        assert_render_error(
            &define_access("account").on_root().record(record_access()),
            "only be defined on a database",
        );
        assert_render_error(
            &define_access("account")
                .on_namespace()
                .record(record_access()),
            "only be defined on a database",
        );
        assert_render_error(
            &define_access("api").jwt(jwt_access().url("https://example.com")),
            "needs a level",
        );
        assert_render_error(&define_access("api").on_database(), "needs a type");
        assert_render_error(
            &define_access("api").on_database().jwt(jwt_access()),
            "needs `algorithm` and `key`",
        );
        assert_render_error(
            &define_access("api").on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example")
                    .url("https://example.com"),
            ),
            "not both",
        );
    }

    #[test]
    fn for_permissions() {
        let age = Field::new("age");
        assert_eq!(
            for_permission(Select).where_(age.gte(18)).to_debug_sql(),
            "FOR select\n\tWHERE age >= 18"
        );
        assert_eq!(
            for_permission([Create, Update, Delete])
                .where_(age.lte(130))
                .to_debug_sql(),
            "FOR create, update, delete\n\tWHERE age <= 130"
        );
    }

    #[test]
    fn remove_statements() {
        assert_sql(&remove_table(Table::from("user")), "REMOVE TABLE user;");
        assert_sql(
            &remove_field(Field::new("email")).on_table(Table::from("user")),
            "REMOVE FIELD email ON TABLE user;",
        );
        assert_sql(
            &remove_index("user_email").on_table(Table::from("user")),
            "REMOVE INDEX user_email ON TABLE user;",
        );
        assert_sql(
            &remove_event("email_changed").on_table(Table::from("user")),
            "REMOVE EVENT email_changed ON TABLE user;",
        );
        assert_sql(&remove_analyzer("ascii"), "REMOVE ANALYZER ascii;");
        assert_sql(
            &remove_param("endpoint_base"),
            "REMOVE PARAM $endpoint_base;",
        );
        assert_sql(
            &remove_function("fn::add_one"),
            "REMOVE FUNCTION fn::add_one;",
        );
        assert_sql(&remove_namespace("acme"), "REMOVE NAMESPACE acme;");
        assert_sql(&remove_database("acme"), "REMOVE DATABASE acme;");
        assert_sql(
            &remove_user("alice").on_root(),
            "REMOVE USER alice ON ROOT;",
        );
        assert_sql(
            &remove_user("alice").on_namespace(),
            "REMOVE USER alice ON NAMESPACE;",
        );
        assert_sql(
            &remove_user("alice").on_database(),
            "REMOVE USER alice ON DATABASE;",
        );
        assert_sql(
            &remove_access("account").on_database(),
            "REMOVE ACCESS account ON DATABASE;",
        );
        assert_sql(
            &remove_access("api").on_namespace(),
            "REMOVE ACCESS api ON NAMESPACE;",
        );
        assert_sql(
            &remove_access("issuer").on_root(),
            "REMOVE ACCESS issuer ON ROOT;",
        );
        assert_render_error(&remove_access("account"), "needs a level");
        assert_render_error(&remove_param("_p0"), "reserved");
    }

    async fn define_score_with_default(db: &Surreal<Db>) {
        run(db, define_table(Table::from("player")).schemaless()).await;
        run(
            db,
            define_field(Field::new("score"))
                .on_table(Table::from("player"))
                .type_(Kind::Int)
                .default(10),
        )
        .await;
    }

    #[tokio::test]
    async fn default_fills_a_missing_field_on_create() {
        let db = db().await;
        define_score_with_default(&db).await;

        run(&db, Raw::new("CREATE player:alice;")).await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:alice;").await;
        assert_eq!(score, Some(10));
    }

    #[tokio::test]
    async fn default_keeps_falsy_values() {
        let db = db().await;
        define_score_with_default(&db).await;
        run(
            &db,
            define_field(Field::new("active"))
                .on_table(Table::from("player"))
                .type_(Kind::Bool)
                .default(true),
        )
        .await;
        run(
            &db,
            define_field(Field::new("nickname"))
                .on_table(Table::from("player"))
                .type_(Kind::String)
                .default("Al"),
        )
        .await;

        run(
            &db,
            Raw::new("CREATE player:bob SET score = 0, active = false, nickname = '';"),
        )
        .await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:bob;").await;
        let active: Option<bool> = read(&db, "SELECT VALUE active FROM ONLY player:bob;").await;
        let nickname: Option<String> =
            read(&db, "SELECT VALUE nickname FROM ONLY player:bob;").await;
        assert_eq!(score, Some(0));
        assert_eq!(active, Some(false));
        assert_eq!(nickname.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn default_is_not_reapplied_on_update() {
        let db = db().await;
        define_score_with_default(&db).await;

        run(&db, Raw::new("CREATE player:carol SET score = 5;")).await;
        run(&db, Raw::new("UPDATE player:carol SET level = 2;")).await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:carol;").await;
        assert_eq!(score, Some(5));

        // Without `ALWAYS`, removing the field does not bring the default back, so the `int`
        // field rejects the NONE left behind.
        let error = Raw::new("UPDATE player:carol UNSET score;")
            .run(db.clone())
            .await
            .expect_err("NONE is not an int");
        let error = format!("{error:?}");
        assert!(error.contains("Expected `int` but found `NONE`"), "{error}");
    }

    #[tokio::test]
    async fn value_transforms_on_create_and_update() {
        let db = db().await;
        run(&db, define_table(Table::from("player")).schemaless()).await;
        run(
            &db,
            define_field(Field::new("name"))
                .on_table(Table::from("player"))
                .type_(Kind::String)
                .value(string::lowercase!(Param::new("value"))),
        )
        .await;

        run(&db, Raw::new("CREATE player:dave SET name = 'ALICE';")).await;
        let name: Option<String> = read(&db, "SELECT VALUE name FROM ONLY player:dave;").await;
        assert_eq!(name.as_deref(), Some("alice"));

        run(&db, Raw::new("UPDATE player:dave SET name = 'BOB';")).await;
        let name: Option<String> = read(&db, "SELECT VALUE name FROM ONLY player:dave;").await;
        assert_eq!(name.as_deref(), Some("bob"));
    }

    #[tokio::test]
    async fn computed_fields_are_evaluated_on_read() {
        let db = db().await;
        run(&db, define_table(Table::from("person")).schemaless()).await;
        run(
            &db,
            define_field(Field::new("is_adult"))
                .on_table(Table::from("person"))
                .computed(Field::new("age").gte(18))
                .type_(Kind::Bool),
        )
        .await;

        run(&db, Raw::new("CREATE person:erin SET age = 20;")).await;
        let adult: Option<bool> = read(&db, "SELECT VALUE is_adult FROM ONLY person:erin;").await;
        assert_eq!(adult, Some(true));

        run(&db, Raw::new("UPDATE person:erin SET age = 10;")).await;
        let adult: Option<bool> = read(&db, "SELECT VALUE is_adult FROM ONLY person:erin;").await;
        assert_eq!(adult, Some(false));
    }

    #[tokio::test]
    async fn schemafull_tables_reject_undefined_fields() {
        let db = db().await;
        run(&db, define_table(Table::from("person")).schemafull()).await;
        run(
            &db,
            define_field(Field::new("name"))
                .on_table(Table::from("person"))
                .type_(Kind::String),
        )
        .await;

        run(&db, Raw::new("CREATE person:alice SET name = 'Alice';")).await;
        assert!(
            Raw::new("CREATE person:carol SET name = 'Carol', extra = 1;")
                .run(db.clone())
                .await
                .is_err(),
            "an undefined field is rejected"
        );
        assert!(
            Raw::new("CREATE person:bob SET name = 1;")
                .run(db.clone())
                .await
                .is_err(),
            "a field of the wrong type is rejected"
        );
    }

    #[tokio::test]
    async fn unique_indexes_reject_duplicates() {
        let db = db().await;
        run(
            &db,
            define_index("user_email")
                .on_table(Table::from("user"))
                .fields(Field::new("email"))
                .unique(),
        )
        .await;
        run(
            &db,
            Raw::new("CREATE user:alice SET email = 'alice@example.com';"),
        )
        .await;
        assert!(
            Raw::new("CREATE user:bob SET email = 'alice@example.com';")
                .run(db.clone())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn fulltext_indexes_answer_matches_with_scores() {
        let db = db().await;
        run(
            &db,
            define_analyzer("simple")
                .tokenizers([Tokenizer::Blank])
                .filters([AnalyzerFilter::Lowercase]),
        )
        .await;
        run(
            &db,
            define_index("post_content")
                .on_table(Table::from("post"))
                .fields(Field::new("content"))
                .fulltext(fulltext_analyzer("simple").bm25(1.2, 0.75).highlights()),
        )
        .await;
        run(
            &db,
            Raw::new(
                "CREATE post:one SET content = 'Hello world'; CREATE post:two SET content = 'Goodbye moon';",
            ),
        )
        .await;

        let content = Field::new("content");
        let found: Vec<String> = select_value(content.clone())
            .from(Table::from("post"))
            .where_(content.matches("hello"))
            .decode()
            .execute(&db)
            .await
            .expect("searched");
        assert_eq!(found, ["Hello world"]);

        let scored: Vec<BTreeMap<String, Value>> = select(arr![
            search::score!(1).__as__("score"),
            search::highlight!("<b>", "</b>", 1).__as__("marked")
        ])
        .from(Table::from("post"))
        .where_(content.matches_with_ref(1, "hello"))
        .decode()
        .execute(&db)
        .await
        .expect("scored");
        assert_eq!(scored.len(), 1);
        assert!(matches!(scored[0].get("score"), Some(Value::Number(_))));
        assert_eq!(
            scored[0].get("marked"),
            Some(&Value::String("<b>Hello</b> world".to_owned()))
        );
    }

    #[tokio::test]
    async fn events_run_on_changes() {
        let db = db().await;
        run(
            &db,
            define_event("ship_created")
                .on_table(Table::from("space_ship"))
                .when(Param::new("event").equal("CREATE"))
                .then(
                    create::<WeaponStats>(WeaponStats::table())
                        .set(WeaponStats::schema().averageStrength.equal_to(1.5)),
                ),
        )
        .await;
        space_ship("apollo", "Apollo")
            .create(&db)
            .await
            .expect("created");
        let stats: usize = read(&db, "RETURN count(SELECT * FROM weapon_stats);").await;
        assert_eq!(stats, 1);
    }

    #[tokio::test]
    async fn params_and_functions_are_usable_after_definition() {
        let db = db().await;
        umbriel::create_param_name_fn!(greeting);
        run(&db, define_param(greeting()).value("Hello")).await;
        let value: String = read(&db, "RETURN $greeting;").await;
        assert_eq!(value, "Hello");

        let total = Param::new("total");
        run(
            &db,
            define_function("add_one")
                .arguments(vec![umbriel::statements::FunctionArgument {
                    name: total.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&total).add(1)))),
        )
        .await;
        let result: i64 = read(&db, "RETURN fn::add_one(41);").await;
        assert_eq!(result, 42);

        run(&db, remove_function("fn::add_one")).await;
        assert!(
            Raw::new("RETURN fn::add_one(1);")
                .run(db.clone())
                .await
                .is_err()
        );
        run(&db, remove_param("greeting")).await;
    }

    #[tokio::test]
    async fn every_definition_is_accepted_and_removable() {
        let db = db().await;
        let user = Table::from("user");
        run(&db, define_table(user.clone()).schemaless()).await;
        run(
            &db,
            define_field(Field::new("email"))
                .on_table(user.clone())
                .type_(Kind::String),
        )
        .await;
        run(
            &db,
            define_index("user_email")
                .on_table(user.clone())
                .fields(Field::new("email"))
                .unique(),
        )
        .await;
        run(
            &db,
            define_event("audit")
                .on_table(user.clone())
                .when(Param::new("event").equal("CREATE"))
                .then(select(All).from(user.clone())),
        )
        .await;
        run(
            &db,
            define_analyzer("simple").tokenizers([Tokenizer::Blank]),
        )
        .await;
        run(
            &db,
            define_access("account").on_database().record(
                record_access()
                    .signup(Raw::new("CREATE user SET email = $email"))
                    .signin(Raw::new("SELECT * FROM user WHERE email = $email"))
                    .with_jwt(JwtAlgorithm::HS512, "example-key")
                    .duration_for_session(Duration::from_secs(3600)),
            ),
        )
        .await;
        run(
            &db,
            define_access("api").on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example-key"),
            ),
        )
        .await;
        run(
            &db,
            define_user("alice")
                .on_database()
                .password("example-password")
                .role(UserRole::Viewer),
        )
        .await;
        run(&db, define_namespace(Namespace::new("acme"))).await;
        run(&db, define_database("acme")).await;

        let info: Option<BTreeMap<String, Value>> = info_for()
            .database()
            .run(db.clone())
            .await
            .expect("info")
            .take(0)
            .expect("object");
        let info = info.expect("database info");
        for key in ["accesses", "analyzers", "tables", "users"] {
            assert!(info.contains_key(key), "INFO FOR DB has `{key}`: {info:?}");
        }
        let table: Option<BTreeMap<String, Value>> = info_for()
            .table(user.clone())
            .run(db.clone())
            .await
            .expect("info")
            .take(0)
            .expect("object");
        assert!(table.expect("table info").contains_key("fields"));

        run(&db, remove_access("account").on_database()).await;
        run(&db, remove_access("api").on_database()).await;
        run(&db, remove_user("alice").on_database()).await;
        run(&db, remove_analyzer("simple")).await;
        run(&db, remove_event("audit").on_table(user.clone())).await;
        run(&db, remove_index("user_email").on_table(user.clone())).await;
        run(
            &db,
            remove_field(Field::new("email")).on_table(user.clone()),
        )
        .await;
        run(&db, remove_table(user)).await;
        run(&db, remove_database("acme")).await;
        run(&db, remove_namespace("acme")).await;
    }
}

mod info_show_and_utilities {
    use super::*;

    #[test]
    fn info() {
        assert_sql(&info_for().root(), "INFO FOR ROOT;");
        assert_sql(&info_for().namespace(), "INFO FOR NS;");
        assert_sql(&info_for().database(), "INFO FOR DB;");
        assert_sql(&info_for().table("user"), "INFO FOR TABLE user;");
    }

    #[test]
    fn show_changes() {
        assert_sql(
            &show_changes_for_table("reading", 5u64).limit(10),
            "SHOW CHANGES FOR TABLE reading SINCE 5 LIMIT 10;",
        );
        assert_sql(
            &show_changes_for_table("reading", DateTime::<Utc>::UNIX_EPOCH),
            "SHOW CHANGES FOR TABLE reading SINCE d'1970-01-01T00:00:00Z';",
        );
    }

    #[test]
    fn use_sleep_throw_live_and_loop_control() {
        assert_sql(&use_().namespace("test"), "USE NS test;");
        assert_sql(&use_().database("test"), "USE DB test;");
        assert_sql(
            &use_().database("test").namespace("acme"),
            "USE NS acme DB test;",
        );
        assert_render_error(&use_(), "needs a namespace");
        assert_sql(&sleep(Duration::from_secs(43)), "SLEEP 43s;");
        assert_sql(
            &throw("something went wrong"),
            "THROW 'something went wrong';",
        );
        assert_sql(
            &live(select(All).from(Table::from("person"))),
            "LIVE SELECT * FROM person;",
        );
        assert_eq!(break_().to_debug_sql(), "BREAK;");
        assert_eq!(continue_().to_debug_sql(), "CONTINUE;");
    }

    #[test]
    fn return_and_let() {
        assert_sql(
            &return_(User::id("alice")).fetch(&[Field::new("account"), Field::new("friends")]),
            "RETURN user:alice FETCH account, friends;",
        );
        assert_sql(&return_(Field::new("age").add(1)), "RETURN age + 1;");
        assert_sql(&let_("name").equal_to("Alice"), "LET $name = 'Alice';");
        assert_sql(
            &let_("users").equal_to(select(All).from(Table::from("user"))),
            "LET $users = (SELECT * FROM user);",
        );
        assert_render_error(&let_("_p1").equal_to(1), "reserved");
        assert_render_error(&let_("not valid").equal_to(1), "must start with a letter");
    }

    #[tokio::test]
    async fn throw_fails_and_sleep_and_return_succeed() {
        let db = db().await;
        match throw("something went wrong").run(db.clone()).await {
            Err(error) => assert!(error.to_string().contains("something went wrong")),
            Ok(_) => panic!("THROW succeeded"),
        }
        run(&db, sleep(Duration::from_millis(1))).await;
        let value: Option<i64> = return_(41)
            .run(db.clone())
            .await
            .expect("ran")
            .take(0)
            .expect("value");
        assert_eq!(value, Some(41));
    }
}

mod control_flow {
    use super::*;

    #[test]
    fn if_else_branches() {
        let age = Field::new("age");
        let country = Field::new("country");
        assert_sql(
            &if_(age.gte(18)).then("adult").end(),
            "IF age >= 18 \n\t{ 'adult' };",
        );
        assert_sql(
            &if_(age.gte(18))
                .then("adult")
                .else_if(cond(country.is("CA")).or(country.is("NO")))
                .then("cold")
                .else_("other")
                .end(),
            "IF age >= 18 \n\t{ 'adult' }\nELSE IF (country IS 'CA') OR (country IS 'NO') \n\t{ 'cold' }\
             \nELSE\n\t{ 'other' };",
        );
        assert_sql(
            &if_(age.gte(18))
                .then(select(All).from(Table::from("user")))
                .end(),
            "IF age >= 18 \n\t{ (SELECT * FROM user) };",
        );
    }

    #[test]
    fn for_loops_iterate_one_parameter() {
        let name = Param::new("name");
        assert_sql(
            &for_(&name).in_(vec!["Alice", "Bob"]).block(chain(
                create::<SpaceShip>(SpaceShip::table())
                    .set(SpaceShip::schema().name.equal_to(&name)),
            )),
            "FOR $name IN ['Alice', 'Bob'] {\nCREATE space_ship SET name = $name;\n};",
        );
    }

    #[test]
    fn transactions_commit_or_cancel() {
        let create_ship = || {
            create::<SpaceShip>(SpaceShip::table()).set(SpaceShip::schema().name.equal_to("Apollo"))
        };
        let committed = begin_transaction()
            .query(create_ship())
            .query(select(All).from(SpaceShip::table()))
            .commit_transaction();
        assert_eq!(
            committed.to_debug_sql(),
            "BEGIN TRANSACTION;\n\nCREATE space_ship SET name = 'Apollo';\n\n\
             SELECT * FROM space_ship;\n\nCOMMIT TRANSACTION;\n\t"
        );
        assert_parses(&committed.to_debug_sql());
        let cancelled = begin_transaction()
            .query(create_ship())
            .cancel_transaction();
        assert!(
            cancelled
                .to_debug_sql()
                .ends_with("CANCEL TRANSACTION;\n\t")
        );
        assert_parses(cancelled.to_query().expect("valid").sql());
    }

    #[test]
    fn chains_and_blocks() {
        let total = let_("total").equal_to(41);
        let chained = chain(total.clone()).chain(return_(Field::from(&total.get_param()).add(1)));
        assert_eq!(
            chained.to_debug_sql(),
            "LET $total = 41;\n\nRETURN $total + 1;"
        );
        assert_parses(&chained.to_debug_sql());
        assert_eq!(
            chained.clone().parenthesized().to_debug_sql(),
            "(LET $total = 41;\n\nRETURN $total + 1;)"
        );
        let block = block(chained.clone());
        assert_eq!(
            block.to_debug_sql(),
            "{\nLET $total = 41;\n\nRETURN $total + 1;\n}"
        );
        assert_eq!(chained.as_block(), block);
    }

    #[tokio::test]
    async fn control_flow_runs_in_the_database() {
        let db = db().await;
        let name = Param::new("name");
        run(
            &db,
            for_(&name).in_(vec!["Apollo", "Gemini"]).block(chain(
                create::<SpaceShip>(SpaceShip::table())
                    .set(SpaceShip::schema().name.equal_to(&name)),
            )),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);

        let age = Param::new("age");
        let branch = chain(let_("age").equal_to(20))
            .chain(if_(age.gte(18)).then("adult").else_("minor").end());
        let result: Option<String> = branch
            .run(db.clone())
            .await
            .expect("ran")
            .take(1)
            .expect("value");
        assert_eq!(result.as_deref(), Some("adult"));

        let block_value: Option<i64> = return_(block(
            chain(let_("total").equal_to(41))
                .chain(return_(Field::from(Param::new("total")).add(1))),
        ))
        .run(db.clone())
        .await
        .expect("ran")
        .take(0)
        .expect("value");
        assert_eq!(block_value, Some(42));
    }

    #[tokio::test]
    async fn committed_transactions_persist_and_cancelled_ones_do_not() {
        let db = db().await;
        run(
            &db,
            begin_transaction()
                .query(
                    create::<SpaceShip>(SpaceShip::table()).content(space_ship("apollo", "Apollo")),
                )
                .query(
                    create::<SpaceShip>(SpaceShip::table()).content(space_ship("gemini", "Gemini")),
                )
                .commit_transaction(),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);

        run(
            &db,
            begin_transaction()
                .query(
                    create::<SpaceShip>(SpaceShip::table())
                        .content(space_ship("mercury", "Mercury")),
                )
                .cancel_transaction(),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn a_transfer_between_accounts_is_atomic() {
        let db = db().await;
        let acc = Account::schema();
        let one = Account::id("one");
        let two = Account::id("two");
        run(
            &db,
            begin_transaction()
                .query(create::<Account>(Account::table()).content(Account {
                    id: one.clone(),
                    balance: 100.0,
                }))
                .query(create::<Account>(Account::table()).content(Account {
                    id: two.clone(),
                    balance: 50.0,
                }))
                .query(update::<Account>(&one).set(acc.balance.decrement_by(30.0)))
                .query(update::<Account>(&two).set(acc.balance.increment_by(30.0)))
                .commit_transaction(),
        )
        .await;
        let balances: Vec<f64> = read(&db, "SELECT VALUE balance FROM account ORDER BY id;").await;
        assert_eq!(balances, [70.0, 80.0]);

        let failed = begin_transaction()
            .query(update::<Account>(&one).set(acc.balance.decrement_by(30.0)))
            .query(throw("insufficient funds"))
            .commit_transaction()
            .run(db.clone())
            .await;
        assert!(failed.is_err());
        let balances: Vec<f64> = read(&db, "SELECT VALUE balance FROM account ORDER BY id;").await;
        assert_eq!(balances, [70.0, 80.0]);
    }

    #[tokio::test]
    async fn let_parameters_carry_results_between_statements() {
        let db = db().await;
        let balance = let_("balance").equal_to(
            create_only::<Balance>(CreateTarget::table())
                .set(Balance::schema().amount.equal_to(25.0)),
        );
        let created_balance = balance.with_path::<Balance>(E).amount;
        let acc = Account::schema();
        run(
            &db,
            chain(balance.clone())
                .chain(create::<Account>(Account::table()).content(Account {
                    id: Account::id("one"),
                    balance: 0.0,
                }))
                .chain(
                    update::<Account>(Account::id("one"))
                        .set(acc.balance.increment_by(created_balance)),
                ),
        )
        .await;
        let amount: f64 = read(&db, "RETURN account:one.balance;").await;
        assert_eq!(amount, 25.0);
    }
}

/// `MERGE`, `PATCH`, `REPLACE` and `UNSET` on `CREATE` and `RELATE`, and `TIMEOUT` on `INSERT`:
/// forms the 3.3.4 parser accepts and the engine applies through the shared data-clause path,
/// each tested separately.
mod data_clauses {
    use umbriel::{Merge, RecordId};

    use super::*;

    fn acme() -> RecordId<Organization> {
        Organization::id("acme")
    }

    #[test]
    fn every_clause_renders_and_parses() {
        let org = Organization::schema();
        let statements = [
            create::<Organization>(acme())
                .merge(OrganizationMerge {
                    name: Merge::Set("Acme Corp".to_owned()),
                    ..Default::default()
                })
                .to_debug_sql(),
            create::<Organization>(acme())
                .patch(org.name.patch_add("Acme Corp"))
                .to_debug_sql(),
            create::<Organization>(acme())
                .replace(organization("acme", "Acme Corp", 10))
                .to_debug_sql(),
            create::<Organization>(acme())
                .unset(org.name)
                .to_debug_sql(),
            insert(space_ship("apollo", "Apollo"))
                .timeout(Duration::from_secs(1))
                .to_debug_sql(),
        ];
        for (statement, clause) in statements.iter().zip([
            " MERGE {",
            " PATCH [",
            " REPLACE {",
            " UNSET name",
            " TIMEOUT 1s",
        ]) {
            assert!(statement.contains(clause), "{statement}");
            assert_parses(statement);
        }
    }

    #[tokio::test]
    async fn create_merge_patch_and_replace_fill_the_new_record() {
        let db = db().await;
        let org = Organization::schema();
        create::<Organization>(acme())
            .merge(OrganizationMerge {
                name: Merge::Set("Acme Corp".to_owned()),
                ..Default::default()
            })
            .return_none()
            .execute(&db)
            .await
            .expect("merged");
        let name: String = read(&db, "RETURN organization:acme.name;").await;
        assert_eq!(name, "Acme Corp");

        create::<Organization>(Organization::id("globex"))
            .patch(org.name.patch_add("Globex"))
            .return_none()
            .execute(&db)
            .await
            .expect("patched");
        let name: String = read(&db, "RETURN organization:globex.name;").await;
        assert_eq!(name, "Globex");

        // A path that does not exist yet cannot be replaced.
        let missing = create::<Organization>(Organization::id("initech"))
            .patch(org.name.patch_replace("Initech"))
            .execute(&db)
            .await;
        assert!(missing.is_err(), "{missing:?}");

        let replaced = create_only::<Organization>(Organization::id("umbrella"))
            .replace(organization("umbrella", "Umbrella", 7))
            .execute(&db)
            .await
            .expect("replaced")
            .expect("returned");
        let content = create_only::<Organization>(Organization::id("hooli"))
            .content(organization("hooli", "Umbrella", 7))
            .execute(&db)
            .await
            .expect("created")
            .expect("returned");
        assert_eq!((replaced.name, replaced.age), (content.name, content.age));
    }

    /// `UNSET` on a new record, with `DEFAULT`, `VALUE` and `ASSERT` clauses, on a SCHEMAFULL
    /// and a SCHEMALESS table. Recorded as found.
    #[tokio::test]
    async fn create_unset_meets_default_value_and_assert() {
        for mode in ["SCHEMAFULL", "SCHEMALESS"] {
            let db = db().await;
            run(
                &db,
                Raw::new(format!(
                    "DEFINE TABLE organization {mode}; \
                     DEFINE FIELD name ON organization TYPE string DEFAULT 'Unnamed'; \
                     DEFINE FIELD age ON organization TYPE int VALUE 7;"
                )),
            )
            .await;
            let org = Organization::schema();
            create::<Organization>(acme())
                .unset([Field::from(&org.name), Field::from(&org.age)])
                .return_none()
                .execute(&db)
                .await
                .expect("created");
            let stored: (String, u8) = read(
                &db,
                "RETURN [organization:acme.name, organization:acme.age];",
            )
            .await;
            assert_eq!(stored, ("Unnamed".to_owned(), 7), "{mode}");

            run(
                &db,
                Raw::new(
                    "DEFINE FIELD OVERWRITE name ON organization TYPE string ASSERT $value != '';",
                ),
            )
            .await;
            let asserted = create::<Organization>(Organization::id("globex"))
                .unset(org.name)
                .execute(&db)
                .await;
            assert!(asserted.is_err(), "{mode}: {asserted:?}");
        }
    }

    async fn endpoints(db: &Surreal<Db>) {
        run(db, Raw::new("CREATE company:acme, user:alice;")).await;
    }

    fn edge_path() -> Field {
        Company::with(Company::id("acme"))
            .like__(CompanyLikeUser::id("l1"))
            .user(User::id("alice"))
            .into()
    }

    async fn edge_endpoints(db: &Surreal<Db>) -> (String, String) {
        read(
            db,
            "RETURN [record::id(like:l1.in), record::id(like:l1.out)];",
        )
        .await
    }

    #[tokio::test]
    async fn relate_merge_patch_replace_and_unset_on_a_new_edge() {
        let db = db().await;
        endpoints(&db).await;
        let schema = CompanyLikeUser::schema();
        let edge = relate_only::<CompanyLikeUser>(edge_path())
            .merge(LikeMerge::<Company, User> {
                time: Merge::Fields(TimeMerge {
                    connected: Merge::Set(DateTime::<Utc>::UNIX_EPOCH),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .execute(&db)
            .await
            .expect("related")
            .expect("returned");
        assert_eq!(edge.time.connected, DateTime::<Utc>::UNIX_EPOCH);
        run(&db, Raw::new("DELETE like:l1;")).await;

        relate::<CompanyLikeUser>(edge_path())
            .patch(schema.time.patch_add(Time::default()))
            .return_none()
            .execute(&db)
            .await
            .expect("patched");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
        run(&db, Raw::new("DELETE like:l1;")).await;

        relate::<CompanyLikeUser>(edge_path())
            .replace(BTreeMap::from([("note".to_owned(), "new".to_owned())]))
            .return_none()
            .execute(&db)
            .await
            .expect("replaced");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
        run(&db, Raw::new("DELETE like:l1;")).await;

        relate::<CompanyLikeUser>(edge_path())
            .unset(schema.time)
            .return_none()
            .execute(&db)
            .await
            .expect("unset");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
    }

    /// Under `OR UPDATE` the clauses change the existing edge; `UNSET` removes a stored property,
    /// and `REPLACE` and `UNSET` keep `in` and `out`.
    #[tokio::test]
    async fn relate_or_update_changes_the_existing_edge() {
        let db = db().await;
        endpoints(&db).await;
        let schema = CompanyLikeUser::schema();
        run(
            &db,
            Raw::new("RELATE company:acme->like:l1->user:alice SET note = 'old', time = { connected: d'2026-01-01T00:00:00Z' };"),
        )
        .await;

        let before = relate_only::<CompanyLikeUser>(edge_path())
            .or_update()
            .unset(Field::from(&schema.time))
            .return_before()
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_ne!(before.time.connected, DateTime::<Utc>::UNIX_EPOCH);
        let removed: bool = read(&db, "RETURN type::is_none(like:l1.time);").await;
        assert!(removed, "UNSET removes the stored property");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );

        relate::<CompanyLikeUser>(edge_path())
            .or_update()
            .merge(LikeMerge::<Company, User> {
                time: Merge::Set(Time::default()),
                ..Default::default()
            })
            .return_none()
            .execute(&db)
            .await
            .expect("merged");
        let note: String = read(&db, "RETURN like:l1.note;").await;
        assert_eq!(note, "old", "a merge keeps the fields it does not name");

        relate::<CompanyLikeUser>(edge_path())
            .or_update()
            .patch(schema.time.patch_remove())
            .return_none()
            .execute(&db)
            .await
            .expect("patched");
        let removed: bool = read(&db, "RETURN type::is_none(like:l1.time);").await;
        assert!(removed);

        relate::<CompanyLikeUser>(edge_path())
            .or_update()
            .replace(BTreeMap::from([("note".to_owned(), "new".to_owned())]))
            .return_none()
            .execute(&db)
            .await
            .expect("replaced");
        let note: String = read(&db, "RETURN like:l1.note;").await;
        assert_eq!(note, "new");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
    }
}
