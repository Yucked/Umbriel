//! INSERT (`orm/src/statements/insert.rs`): rows, duplicate keys, `IGNORE`, relations and
//! inserts from a SELECT, rendered and run.

use umbriel_tests::{support::statements::*, *};


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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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

        let (other, _other_guard) = umbriel_tests::db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
