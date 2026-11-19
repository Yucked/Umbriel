//! Record ids (`orm/src/query/record_id.rs`): ids in their model's table, generated keys,
//! decoding checks and ranges.

use umbriel_tests::{support::models::*, *};


    #[test]
    fn ids_are_written_in_their_models_table() {
        assert_eq!(Alien::id("alice").to_string(), "alien:alice");
        assert_eq!(Alien::id(7).to_string(), "alien:7");
        assert_eq!(Alien::id(vec![1i64, 2]).to_string(), "alien:[1, 2]");
        assert_eq!(Alien::id("alice").table.as_str(), "alien");
        assert_eq!(Alien::id(7).key(), &RecordIdKey::Number(7));
        assert!(matches!(
            Alien::id(uuid::Uuid::nil()).key(),
            RecordIdKey::Uuid(_)
        ));
    }

    #[test]
    fn generated_keys_have_surrealdbs_formats() {
        match RecordId::<Alien>::rand().key() {
            RecordIdKey::String(key) => {
                assert_eq!(key.len(), 20);
                assert!(
                    key.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                );
            }
            other => panic!("expected a string key, got {other:?}"),
        }
        match RecordId::<Alien>::ulid().key() {
            RecordIdKey::String(key) => assert_eq!(key.len(), 26),
            other => panic!("expected a string key, got {other:?}"),
        }
        assert!(matches!(
            RecordId::<Alien>::uuid().key(),
            RecordIdKey::Uuid(_)
        ));
        assert_ne!(RecordId::<Alien>::default(), RecordId::<Alien>::default());
    }

    #[test]
    fn decoding_checks_the_table() {
        let own = types::RecordId::new("alien", "alice");
        let other = types::RecordId::new("weapon", "alice");
        assert!(RecordId::<Alien>::from_value(Value::RecordId(own.clone())).is_ok());
        assert!(RecordId::<Alien>::from_value(Value::RecordId(other.clone())).is_err());
        assert!(RecordId::<Alien>::try_from(other).is_err());
        assert!(RecordId::<Alien>::is_value(&Value::RecordId(own)));
        assert!(RecordId::<Alien>::from_value(Value::String("alien:alice".to_owned())).is_err());
    }

    #[tokio::test]
    async fn decoding_a_model_checks_the_table_of_its_id() {
        let (db, _guard) = db().await;
        // A record shaped like a space ship, stored in another table.
        Raw::new("CREATE alien:apollo SET name = 'Apollo', created = d'1970-01-01T00:00:00Z';")
            .run(&db)
            .await
            .expect("created");
        let record: Value = read(&db, "SELECT * FROM ONLY alien:apollo;").await;
        let error = SpaceShip::from_value(record).expect_err("an alien id is not a space ship's");
        assert!(error.to_string().contains("alien"), "{error}");

        Raw::new(
            "CREATE space_ship:apollo SET name = 'Apollo', created = d'1970-01-01T00:00:00Z';",
        )
        .run(&db)
        .await
        .expect("created");
        let record: Value = read(&db, "SELECT * FROM ONLY space_ship:apollo;").await;
        assert_eq!(
            SpaceShip::from_value(record).expect("its own table").id,
            SpaceShip::id("apollo")
        );
    }

    #[tokio::test]
    async fn every_key_kind_round_trips_through_the_database() {
        let (db, _guard) = db().await;
        let mut object = types::Object::new();
        object.insert("region", "eu");
        object.insert("seq", 7);
        let ids = [
            SpaceShip::id(42),
            SpaceShip::id("apollo"),
            SpaceShip::id(uuid::Uuid::from_u128(
                0x0190_7a2e_4c1d_7b3a_8f00_0000_0000_0001,
            )),
            SpaceShip::id(vec![1i64, 2]),
            SpaceShip::id(object),
            RecordId::<SpaceShip>::rand(),
            RecordId::<SpaceShip>::ulid(),
            RecordId::<SpaceShip>::uuid(),
        ];
        for id in ids {
            let ship = SpaceShip {
                id: id.clone(),
                name: "Apollo".to_owned(),
                created: DateTime::<Utc>::UNIX_EPOCH,
            };
            let created = ship.create(&db).await.expect("created");
            assert_eq!(created.id, id, "create returns the exact id");
            let stored = SpaceShip::get(&db, &id)
                .await
                .expect("read")
                .expect("stored under that id");
            assert_eq!(stored.id, id);
        }
    }

    #[test]
    fn decoding_rejects_ranges() {
        let range = types::RecordId::new(
            "alien",
            types::RecordIdKeyRange {
                start: std::ops::Bound::Included(RecordIdKey::Number(1)),
                end: std::ops::Bound::Excluded(RecordIdKey::Number(5)),
            },
        );
        assert!(RecordId::<Alien>::from_value(Value::RecordId(range)).is_err());
    }

    #[test]
    fn ranges_render_in_the_models_table() {
        let range = RecordRange::<Alien>::from(Alien::id(1)..Alien::id(5));
        assert_eq!(range.to_debug_sql(), "alien:1..5");
        let inclusive = RecordRange::<Alien>::from(Alien::id(1)..=Alien::id(5));
        assert_eq!(inclusive.to_debug_sql(), "alien:1..=5");
    }

    #[tokio::test]
    async fn ids_round_trip_through_the_database() {
        let (db, _guard) = db().await;
        let ship = space_ship("apollo", "Apollo");
        ship.clone().create(&db).await.expect("created");

        let id: RecordId<SpaceShip> = read(&db, "RETURN space_ship:apollo.id;").await;
        assert_eq!(id, ship.id);
    }
