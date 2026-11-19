//! CREATE (`orm/src/statements/create.rs`): every clause rendered and parsed, and the records
//! the statement stores and returns.

use umbriel_tests::{support::statements::*, *};


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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
        let (db, _guard) = db().await;
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
