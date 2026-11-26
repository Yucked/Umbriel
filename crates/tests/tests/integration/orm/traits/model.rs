//! Model methods (`orm/src/traits/model.rs`): create, insert, read, save, delete, find and count
//! records, and relate nodes.

use umbriel_tests::{support::models::*, *};

mod record_operations {
    use super::*;

    #[tokio::test]
    async fn create_stores_a_new_record_and_fails_on_an_existing_one() {
        let (db, _guard) = db().await;
        let apollo = space_ship("apollo", "Apollo");

        let created = apollo.clone().create(&db).await.expect("created");
        assert_eq!(created.record_id(), apollo.record_id());
        assert_eq!(created.name, "Apollo");

        assert!(matches!(
            apollo.create(&db).await,
            Err(umbriel::Error::RecordExists {
                statement: None,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn insert_many_stores_every_record_and_fails_on_a_duplicate() {
        let (db, _guard) = db().await;
        let inserted = SpaceShip::insert_many(
            &db,
            vec![
                space_ship("apollo", "Apollo"),
                space_ship("gemini", "Gemini"),
            ],
        )
        .await
        .expect("inserted");
        assert_eq!(inserted.len(), 2);

        // A batch holding an existing id fails as a whole.
        let duplicate = SpaceShip::insert_many(
            &db,
            vec![
                space_ship("mercury", "Mercury"),
                space_ship("apollo", "Apollo"),
            ],
        )
        .await;
        assert!(
            matches!(
                &duplicate,
                Err(umbriel::Error::RecordExists {
                    statement: None,
                    ..
                })
            ),
            "{duplicate:?}"
        );
        let names: Vec<String> =
            read(&db, "SELECT VALUE name FROM space_ship ORDER BY name;").await;
        assert_eq!(names, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn get_range_and_get_many_read_several_records() {
        let (db, _guard) = db().await;
        for key in 1..=5 {
            SpaceShip {
                id: SpaceShip::id(key),
                name: format!("Ship {key}"),
                created: DateTime::<Utc>::UNIX_EPOCH,
            }
            .create(&db)
            .await
            .expect("created");
        }
        let names =
            |ships: Vec<SpaceShip>| ships.into_iter().map(|ship| ship.name).collect::<Vec<_>>();
        let range = SpaceShip::get_range(&db, SpaceShip::id(2)..SpaceShip::id(4))
            .await
            .expect("read");
        assert_eq!(names(range), ["Ship 2", "Ship 3"]);
        let inclusive = SpaceShip::get_range(&db, SpaceShip::id(4)..=SpaceShip::id(5))
            .await
            .expect("read");
        assert_eq!(names(inclusive), ["Ship 4", "Ship 5"]);

        let many = SpaceShip::get_many(
            &db,
            &[SpaceShip::id(5), SpaceShip::id(99), SpaceShip::id(1)],
        )
        .await
        .expect("read");
        let mut many = names(many);
        many.sort();
        assert_eq!(many, ["Ship 1", "Ship 5"]);
        assert!(
            SpaceShip::get_many(&db, &[])
                .await
                .expect("read")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn get_reads_a_record_by_id() {
        let (db, _guard) = db().await;
        space_ship("apollo", "Apollo")
            .create(&db)
            .await
            .expect("created");

        let found = SpaceShip::get(&db, &SpaceShip::id("apollo"))
            .await
            .expect("read");
        assert_eq!(found.map(|ship| ship.name), Some("Apollo".to_owned()));
        assert!(
            SpaceShip::get(&db, &SpaceShip::id("nobody"))
                .await
                .expect("read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_inserts_or_replaces_the_whole_record() {
        let (db, _guard) = db().await;
        profile("alice", Some("Al"), None)
            .save(&db)
            .await
            .expect("inserted");

        // An omitted `Option` field is stored as absent: the whole record is replaced.
        let replaced = profile("alice", None, None)
            .save(&db)
            .await
            .expect("replaced");
        assert_eq!(replaced.nickname, None);
        let absent: bool = read(&db, "RETURN type::is_none(profile:alice.nickname);").await;
        assert!(absent);
    }

    #[tokio::test]
    async fn delete_removes_a_record_and_returns_it() {
        let (db, _guard) = db().await;
        let apollo = space_ship("apollo", "Apollo");
        apollo.clone().create(&db).await.expect("created");
        space_ship("gemini", "Gemini")
            .create(&db)
            .await
            .expect("created");

        let deleted = apollo.delete(&db).await.expect("deleted");
        assert_eq!(deleted.map(|ship| ship.name), Some("Apollo".to_owned()));
        let deleted = SpaceShip::delete_by_id(&db, &SpaceShip::id("gemini"))
            .await
            .expect("deleted");
        assert!(deleted.is_some());
        assert!(
            SpaceShip::get(&db, &SpaceShip::id("gemini"))
                .await
                .expect("read")
                .is_none()
        );

        let missing = SpaceShip::delete_by_id(&db, &SpaceShip::id("nobody"))
            .await
            .expect("ran");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn find_where_returns_matching_records() {
        let (db, _guard) = db().await;
        space_ship("apollo", "Apollo")
            .save(&db)
            .await
            .expect("saved");
        space_ship("gemini", "Gemini")
            .save(&db)
            .await
            .expect("saved");
        let SpaceShipSchema { id, name, .. } = SpaceShip::schema();

        let all: Vec<SpaceShip> = SpaceShip::find_where(id.is_not(NONE))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(all.len(), 2);

        let found = SpaceShip::find_where(name.equal("Apollo"))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, SpaceShip::id("apollo"));

        let none = SpaceShip::find_where(name.equal("Mercury"))
            .execute(&db)
            .await
            .expect("read");
        assert!(none.is_empty());
    }

    async fn weapons(db: &Surreal<Any>) {
        for strength in 0..100 {
            weapon(strength, strength as f64)
                .save(db)
                .await
                .expect("saved");
        }
    }

    #[tokio::test]
    async fn count_where_counts_matching_records() {
        let (db, _guard) = db().await;
        weapons(&db).await;
        let WeaponSchema { strength, .. } = Weapon::schema();

        let query = Weapon::count_where(strength.gte(50));
        assert_eq!(
            query.to_debug_sql(),
            "SELECT VALUE count FROM (SELECT count(strength >= 50) FROM weapon GROUP ALL);"
        );
        assert_eq!(query.execute(&db).await.expect("count"), 50);

        let query = Weapon::count_where(cond(strength.gte(50)).and(strength.lt(73)));
        assert_eq!(
            query.to_debug_sql(),
            "SELECT VALUE count FROM (SELECT count((strength >= 50) AND (strength < 73)) FROM weapon GROUP ALL);"
        );
        assert_eq!(query.execute(&db).await.expect("count"), 23);

        let none = Weapon::count_where(strength.gt(1000));
        assert_eq!(none.execute(&db).await.expect("count"), 0);
    }

    #[tokio::test]
    async fn count_all_counts_every_record() {
        let (db, _guard) = db().await;
        // SurrealDB 3 reports reading a table that does not exist as an error.
        Weapon::define_table()
            .run(&db)
            .await
            .expect("table defined");
        let query = Weapon::count_all();
        assert_eq!(
            query.to_debug_sql(),
            "SELECT VALUE count FROM (SELECT count() FROM weapon GROUP ALL);"
        );
        assert_eq!(query.execute(&db).await.expect("count"), 0);

        weapons(&db).await;
        assert_eq!(query.execute(&db).await.expect("count"), 100);
    }

    #[tokio::test]
    async fn delete_where_removes_matching_records() {
        let (db, _guard) = db().await;
        space_ship("apollo", "Apollo 11")
            .save(&db)
            .await
            .expect("saved");
        space_ship("gemini", "Gemini 4")
            .save(&db)
            .await
            .expect("saved");
        let SpaceShipSchema { name, .. } = SpaceShip::schema();

        SpaceShip::delete_where(name.contains("Apollo"))
            .run(&db)
            .await
            .expect("deleted");

        let left = SpaceShip::find_where(name.is_not(NONE))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(
            left.into_iter().map(|ship| ship.name).collect::<Vec<_>>(),
            ["Gemini 4"]
        );
    }

    #[tokio::test]
    async fn generic_models_round_trip() {
        let (db, _guard) = db().await;
        let earth = Planet::<u64> {
            id: Planet::id("earth"),
            name: "Earth".to_owned(),
            population: 8,
            tags: vec![1, 2],
            data: 42,
            ..Default::default()
        };
        earth.save(&db).await.expect("saved");

        let found = Planet::<u64>::get(&db, &Planet::id("earth"))
            .await
            .expect("read")
            .expect("exists");
        assert_eq!((found.data, found.tags), (42, vec![1, 2]));
    }

    #[tokio::test]
    async fn models_with_every_stored_type_round_trip() {
        let (db, _guard) = db().await;
        alien("alice", "Alice").save(&db).await.expect("saved");

        let found = Alien::get(&db, &Alien::id("alice"))
            .await
            .expect("read")
            .expect("exists");
        assert_eq!(found.life_expectancy, Duration::from_secs(60));
        assert_eq!(found.home, geo::Point::new(1.0, 2.0));
        assert_eq!(found.territory_area, polygon());
        assert_eq!(found.line_string, line_string());
        assert_eq!(found.created, DateTime::<Utc>::UNIX_EPOCH);
    }
}

mod relations {
    use super::*;

    #[tokio::test]
    async fn relate_connects_two_nodes_and_relations_read_them_back() {
        let (db, _guard) = db().await;
        let acme = Company {
            id: Company::id("acme"),
            name: "Acme Corp".to_owned(),
            ..Default::default()
        };
        let alice = User {
            id: User::id("alice"),
            name: "Alice".to_owned(),
            ..Default::default()
        };
        acme.clone().create(&db).await.expect("acme");
        alice.clone().create(&db).await.expect("alice");

        let path = Company::with(acme.id.clone())
            .like__(E)
            .user(alice.id.clone());
        let edge = relate::<CompanyLikeUser>(path)
            .content(Like {
                id: CompanyLikeUser::id("acme_alice"),
                r#in: LinkOne::from(acme.id.clone()),
                out: LinkOne::from(alice.id.clone()),
                time: Time {
                    connected: DateTime::<Utc>::UNIX_EPOCH,
                },
            })
            .execute(&db)
            .await
            .expect("related")
            .pop()
            .expect("the edge is returned");
        assert_eq!(edge.out.key(), Some(&alice.id));
        assert_eq!(edge.r#in.key(), Some(&acme.id));

        let company: Company = read(
            &db,
            "SELECT *, ->like->user.* AS devs FROM ONLY company:acme;",
        )
        .await;
        let devs = company
            .devs
            .values_truthy()
            .into_iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(devs, ["Alice"]);

        // A relation is never stored on the node.
        let stored: bool = read(&db, "RETURN type::is_none(company:acme.devs);").await;
        assert!(stored);
    }
}

/// A `Model` written by hand, without the derive, still compiles: typed link paths come from the
/// separate `Links` trait, so `Model` gains no required item.
mod compiles_hand_written_model {
    use umbriel::FieldMetadata;

    use super::*;

    #[derive(Debug, Clone, SurrealValue)]
    #[surreal(crate = "umbriel::types")]
    pub struct Note {
        pub id: RecordId<Note>,
        pub text: String,
    }

    impl Model for Note {
        type StructRenamedCreator = ();

        const TABLE: &'static str = "note";
        const TABLE_PREFIX: Option<&'static str> = None;

        fn record_id(&self) -> &RecordId<Self> {
            &self.id
        }

        fn get_serializable_fields() -> Vec<Field> {
            vec![Field::new("id"), Field::new("text")]
        }

        fn get_linked_fields() -> Vec<Field> {
            Vec::new()
        }

        fn get_link_one_fields() -> Vec<Field> {
            Vec::new()
        }

        fn get_link_self_fields() -> Vec<Field> {
            Vec::new()
        }

        fn get_link_one_and_self_fields() -> Vec<Field> {
            Vec::new()
        }

        fn get_link_many_fields() -> Vec<Field> {
            Vec::new()
        }

        fn define_table() -> Raw {
            Raw::new("DEFINE TABLE note;")
        }

        fn define_fields() -> Vec<Raw> {
            Vec::new()
        }

        fn get_field_meta() -> Vec<FieldMetadata> {
            Vec::new()
        }
    }

    #[test]
    fn a_hand_written_model_needs_no_links() {
        assert_eq!(Note::TABLE, "note");
        assert_sql(&Note::select(), "SELECT * FROM note;");
    }
}
