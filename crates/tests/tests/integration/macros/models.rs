//! `#[model]` (`umbriel-macros` `models`): the definitions, schema accessors and conversions it
//! generates, including for models declared inside functions.

mod field_meta;

use umbriel_tests::{support::models::*, *};

mod definitions {
    use super::*;
    use umbriel::{LinkOne, Node, model};

    #[model(node, table = "flags_off")]
    #[orm(schemafull = false, drop = false)]
    #[derive(Debug, Clone)]
    pub struct FlagsOff {
        pub id: RecordId<Self>,
    }

    #[model(node, table = "flags_on")]
    #[orm(schemafull = true, drop = true)]
    #[derive(Debug, Clone)]
    pub struct FlagsOn {
        pub id: RecordId<Self>,
    }

    #[model(edge, table = "any_relation")]
    #[orm(relation, relax_table)]
    #[derive(Debug, Clone)]
    pub struct AnyRelationEdge<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    pub type AnyRelation = AnyRelationEdge<Alien, Planet<u64>>;

    #[model(edge, table = "enforced_relation")]
    #[orm(relation(from = "alien", to = "planet | weapon"), enforced, relax_table)]
    #[derive(Debug, Clone)]
    pub struct EnforcedRelationEdge<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    pub type EnforcedRelation = EnforcedRelationEdge<Alien, Planet<u64>>;

    #[model(edge, table = "light_relation")]
    #[orm(relation(from = "alien", to = "planet"), lightweight, relax_table)]
    #[derive(Debug, Clone)]
    pub struct LightRelationEdge<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    pub type LightRelation = LightRelationEdge<Alien, Planet<u64>>;

    #[test]
    fn stored_types_are_inferred_from_rust_types() {
        assert_eq!(Alien::define_table().as_str(), "DEFINE TABLE alien;");
        assert_eq!(
            joined(&Alien::define_fields()),
            "DEFINE FIELD name ON TABLE alien TYPE string;
DEFINE FIELD age ON TABLE alien TYPE int;
DEFINE FIELD created ON TABLE alien TYPE datetime;
DEFINE FIELD lifeExpectancy ON TABLE alien TYPE duration;
DEFINE FIELD lineString ON TABLE alien TYPE geometry<line>;
DEFINE FIELD territoryArea ON TABLE alien TYPE geometry<polygon>;
DEFINE FIELD home ON TABLE alien TYPE geometry<point>;
DEFINE FIELD tags ON TABLE alien TYPE array<string>;
DEFINE FIELD ally ON TABLE alien TYPE none | record<alien>;
DEFINE FIELD weapon ON TABLE alien TYPE none | record<weapon>;
DEFINE FIELD spaceShips ON TABLE alien TYPE array<record<space_ship>>;"
        );
    }

    #[test]
    fn explicit_stored_types_are_kept() {
        assert_eq!(
            AlienWithExplicitAttributes::define_table().as_str(),
            "DEFINE TABLE alien_with_explicit_attributes;"
        );
        assert_eq!(
            joined(&AlienWithExplicitAttributes::define_fields()),
            "DEFINE FIELD id ON TABLE alien_with_explicit_attributes TYPE string;
DEFINE FIELD name ON TABLE alien_with_explicit_attributes TYPE string;
DEFINE FIELD age ON TABLE alien_with_explicit_attributes TYPE int;
DEFINE FIELD created ON TABLE alien_with_explicit_attributes TYPE datetime;
DEFINE FIELD lifeExpectancy ON TABLE alien_with_explicit_attributes TYPE duration;
DEFINE FIELD territoryArea ON TABLE alien_with_explicit_attributes TYPE geometry<polygon>;
DEFINE FIELD home ON TABLE alien_with_explicit_attributes TYPE geometry<point>;
DEFINE FIELD tags ON TABLE alien_with_explicit_attributes TYPE array<string>;
DEFINE FIELD ally ON TABLE alien_with_explicit_attributes TYPE record<alien_with_explicit_attributes>;
DEFINE FIELD weapon ON TABLE alien_with_explicit_attributes TYPE record<weapon>;
DEFINE FIELD spaceShips ON TABLE alien_with_explicit_attributes TYPE array<record<space_ship>>;"
        );
    }

    #[test]
    fn edge_fields_are_inferred_and_in_and_out_link_any_record() {
        assert_eq!(
            AlienVisitsPlanet::define_table().as_str(),
            "DEFINE TABLE visits;"
        );
        assert_eq!(
            joined(&AlienVisitsPlanet::define_fields()),
            "DEFINE FIELD in ON TABLE visits TYPE record;
DEFINE FIELD out ON TABLE visits TYPE record;
DEFINE FIELD hairColor ON TABLE visits TYPE none | string;
DEFINE FIELD timeVisited ON TABLE visits TYPE duration;
DEFINE FIELD mana ON TABLE visits TYPE none | record<planet>;
DEFINE FIELD name ON TABLE visits TYPE string;
DEFINE FIELD age ON TABLE visits TYPE int;
DEFINE FIELD created ON TABLE visits TYPE datetime;
DEFINE FIELD lifeExpectancy ON TABLE visits TYPE duration;
DEFINE FIELD lineString ON TABLE visits TYPE geometry<line>;
DEFINE FIELD multiLineString ON TABLE visits TYPE geometry<multiline>;
DEFINE FIELD polygon ON TABLE visits TYPE geometry<polygon>;
DEFINE FIELD multiPolygon ON TABLE visits TYPE geometry<multipolygon>;
DEFINE FIELD point ON TABLE visits TYPE geometry<point>;
DEFINE FIELD multiPoint ON TABLE visits TYPE geometry<multipoint>;
DEFINE FIELD geometryCollection ON TABLE visits TYPE geometry;
DEFINE FIELD territoryArea ON TABLE visits TYPE geometry<polygon>;
DEFINE FIELD home ON TABLE visits TYPE geometry<point>;
DEFINE FIELD pointExplicit ON TABLE visits TYPE geometry<point>;
DEFINE FIELD multiPointExplicit ON TABLE visits TYPE geometry<multipoint>;
DEFINE FIELD lineStringExplicit ON TABLE visits TYPE geometry<line>;
DEFINE FIELD multiLineStringExplicit ON TABLE visits TYPE geometry<multiline>;
DEFINE FIELD polygonExplicit ON TABLE visits TYPE geometry<polygon>;
DEFINE FIELD multiPolygonExplicit ON TABLE visits TYPE geometry<multipolygon>;
DEFINE FIELD geometryCollectionExplicit ON TABLE visits TYPE geometry<collection>;
DEFINE FIELD tags ON TABLE visits TYPE array<string>;
DEFINE FIELD weapon ON TABLE visits TYPE none | record<weapon>;
DEFINE FIELD spaceShips ON TABLE visits TYPE array<record<space_ship>>;"
        );
    }

    #[test]
    fn edge_fields_with_explicit_types() {
        assert_eq!(
            AlienVisitsPlanetExplicit::define_table().as_str(),
            "DEFINE TABLE visits_explicit;"
        );
        assert_eq!(
            joined(&AlienVisitsPlanetExplicit::define_fields()),
            "DEFINE FIELD id ON TABLE visits_explicit TYPE int;
DEFINE FIELD in ON TABLE visits_explicit TYPE record;
DEFINE FIELD out ON TABLE visits_explicit TYPE record;
DEFINE FIELD timeVisited ON TABLE visits_explicit TYPE duration;"
        );
    }

    #[test]
    fn nested_objects_are_stored_as_objects() {
        assert_eq!(
            joined(&Weapon::define_fields()),
            "DEFINE FIELD name ON TABLE weapon TYPE string;
DEFINE FIELD strength ON TABLE weapon TYPE float;
DEFINE FIELD created ON TABLE weapon TYPE datetime;
DEFINE FIELD rocket ON TABLE weapon TYPE object;"
        );
    }

    #[test]
    fn optional_nullable_set_and_map_types() {
        assert_eq!(
            joined(&Profile::define_fields()),
            "DEFINE FIELD name ON TABLE profile TYPE string;
DEFINE FIELD nickname ON TABLE profile TYPE none | string;
DEFINE FIELD bio ON TABLE profile TYPE none | null | string;
DEFINE FIELD scores ON TABLE profile TYPE array<int>;
DEFINE FIELD settings ON TABLE profile TYPE object;"
        );
    }

    #[test]
    fn generic_fields_take_their_declared_type() {
        assert_eq!(
            joined(&Planet::<u64>::define_fields()),
            "DEFINE FIELD name ON TABLE planet TYPE string;
DEFINE FIELD population ON TABLE planet TYPE int;
DEFINE FIELD created ON TABLE planet TYPE datetime;
DEFINE FIELD tags ON TABLE planet TYPE array<int>;
DEFINE FIELD data ON TABLE planet TYPE int;"
        );
    }

    #[test]
    fn table_settings_given_inline() {
        let expected = define_table(StudentWithGranularAttributes::table())
            .drop()
            .schemafull()
            .permissions(student_permissions());
        assert_eq!(
            StudentWithGranularAttributes::define_table().as_str(),
            expected.to_debug_sql()
        );
    }

    #[test]
    fn table_settings_given_as_paths() {
        let expected = define_table(StudentFnAttrs::table())
            .drop()
            .schemafull()
            .as_(as_fn())
            .permissions(student_permissions());
        assert_eq!(
            StudentFnAttrs::define_table().as_str(),
            expected.to_debug_sql()
        );
    }

    #[test]
    fn field_settings_given_inline_as_paths_and_as_calls() {
        let table = StudentWithGranularAttributes::table();
        let schema = StudentWithGranularAttributes::schema();
        let fields = StudentWithGranularAttributes::define_fields();
        let age = |name: &'static str| {
            define_field(Field::new(name))
                .on_table(table.clone())
                .type_(Kind::Int)
        };

        let inline = age("ageInlineExpr")
            .assert(cond(value().is_not(NONE)).and(value().greater_than_or_equal(18)))
            .default(18)
            .permissions(for_permission([Create, Delete]).where_(schema.firstName.is("Alice")));
        assert_eq!(
            definitions_of(&fields, "ageInlineExpr"),
            [inline.to_debug_sql()]
        );

        let invoked = age("ageDefaultExternalFunctionInvokedExpr")
            .assert(get_age_assertion())
            .default(get_age_default_value())
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageDefaultExternalFunctionInvokedExpr"),
            [invoked.to_debug_sql()]
        );

        let with_argument = age("ageTeenExternalFunctionInvokedExpr")
            .assert(get_age_assertion())
            .default(get_age_by_group_default_value(AgeGroup::Teen))
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageTeenExternalFunctionInvokedExpr"),
            [with_argument.to_debug_sql()]
        );

        let without_permissions = age("ageSeniorExternalFunctionInvokedExpr")
            .assert(get_age_assertion())
            .default(get_age_by_group_default_value(AgeGroup::Senior));
        assert_eq!(
            definitions_of(&fields, "ageSeniorExternalFunctionInvokedExpr"),
            [without_permissions.to_debug_sql()]
        );

        let without_assertion = age("ageChildExternalFunctionInvokedExpr")
            .default(get_age_by_group_default_value(AgeGroup::Child))
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageChildExternalFunctionInvokedExpr"),
            [without_assertion.to_debug_sql()]
        );

        let default_only = age("ageAdultExternalFunctionInvokedExpr")
            .default(get_age_by_group_default_value(AgeGroup::Adult));
        assert_eq!(
            definitions_of(&fields, "ageAdultExternalFunctionInvokedExpr"),
            [default_only.to_debug_sql()]
        );

        let paths = age("ageExternalFnAttrs")
            .assert(get_age_assertion())
            .default(get_age_default_value())
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageExternalFnAttrs"),
            [paths.to_debug_sql()]
        );

        let week = define_field(Field::new("timeToKelowna"))
            .on_table(table.clone())
            .type_(Kind::Duration)
            .assert(value().is_not(NONE))
            .default(default_duration_value());
        assert_eq!(
            definitions_of(&fields, "timeToKelowna"),
            [week.to_debug_sql()]
        );
        assert_eq!(
            definitions_of(&fields, "timeToKelowna")[0]
                .replace("timeToKelowna", "timeToKelownaInline"),
            definitions_of(&fields, "timeToKelownaInline")[0]
        );
    }

    #[test]
    fn item_assertions_define_the_array_elements() {
        let table = StudentWithGranularAttributes::table();
        let items = define_field(Field::new("semesterCourses").all())
            .on_table(table.clone())
            .assert(value().is_not(NONE));
        assert_eq!(
            definitions_of(&StudentWithGranularAttributes::define_fields(), "semesterCourses"),
            [
                "DEFINE FIELD semesterCourses ON TABLE student_with_granular_attributes TYPE array<record<book>>;"
                    .to_owned(),
                items.to_debug_sql(),
            ]
        );
    }

    #[test]
    fn renamed_fields_are_defined_under_their_stored_name() {
        let fields = Student::define_fields();
        assert_eq!(
            definitions_of(&fields, "unoBook"),
            ["DEFINE FIELD unoBook ON TABLE student TYPE none | record<book>;"]
        );
        assert!(definitions_of(&fields, "favBook").is_empty());
    }

    #[test]
    fn relations_are_never_defined() {
        let fields = joined(&Student::define_fields());
        assert!(!fields.contains("writtenBooks"));
        assert!(!fields.contains("blogs"));
    }

    #[test]
    fn define_replaces_the_table_definition() {
        let expected = define_student_with_define_attr().to_debug_sql();
        assert_eq!(StudentWithDefineAttr::define_table().as_str(), expected);
        // Given as a path instead of a call.
        assert_eq!(StudentWithDefineFnAttr::define_table().as_str(), expected);
    }

    /// Regression (UMB-013): `define = …` was emitted on top of the generated `DEFINE FIELD`.
    /// It is the field's whole definition.
    #[test]
    fn define_replaces_the_field_definition() {
        let fields = StudentWithDefineAttr::define_fields();
        let schema = StudentWithDefineAttr::schema();

        assert_eq!(
            definitions_of(&fields, "firstName"),
            [define_first_name(&schema.firstName, StudentWithDefineAttr::table()).to_debug_sql()]
        );
        assert_eq!(
            definitions_of(&fields, "lastName"),
            [define_last_name().to_debug_sql()]
        );
        assert_eq!(
            definitions_of(&fields, "ageDefineExternalInvoke"),
            [define_age(&schema.ageDefineExternalInvoke).to_debug_sql()]
        );
    }

    /// Regression (UMB-012): `schemafull = false` and `drop = false` emitted the flags.
    #[test]
    fn false_table_flags_are_left_out() {
        assert_eq!(
            FlagsOff::define_table().as_str(),
            "DEFINE TABLE flags_off;"
        );
        assert_eq!(FlagsOn::define_table().as_str(), "DEFINE TABLE flags_on DROP SCHEMAFULL;");
    }

    #[test]
    fn edges_are_relation_tables_only_when_asked() {
        assert_eq!(
            AlienVisitsPlanet::define_table().as_str(),
            "DEFINE TABLE visits;",
            "an edge without `relation`"
        );
        assert_eq!(
            AnyRelation::define_table().as_str(),
            "DEFINE TABLE any_relation TYPE RELATION;"
        );
        assert_eq!(
            EnforcedRelation::define_table().as_str(),
            "DEFINE TABLE enforced_relation TYPE RELATION IN alien OUT planet | weapon ENFORCED;"
        );
        assert_eq!(
            LightRelation::define_table().as_str(),
            "DEFINE TABLE light_relation TYPE RELATION IN alien OUT planet LIGHTWEIGHT;"
        );
        assert!(
            LightRelation::define_fields().is_empty(),
            "a lightweight edge defines no field, not even `in` and `out`"
        );
    }

    /// Server checks of the relation tables the derive defines.
    mod relation_tables {
        use super::*;

        async fn alien_and_planet(db: &Surreal<Any>) {
            assert!(
                succeeds(
                    db,
                    "DEFINE TABLE alien; DEFINE TABLE planet; DEFINE TABLE weapon; \
                     CREATE alien:zorg; CREATE planet:mars; CREATE weapon:laser;"
                )
                .await,
                "the endpoints are created"
            );
        }

        #[tokio::test]
        async fn an_enforced_relation_refuses_a_missing_or_wrong_endpoint() {
            let (db, _guard) = db().await;
            alien_and_planet(&db).await;
            apply_definitions::<EnforcedRelation>(&db).await;
            assert!(
                succeeds(&db, "RELATE alien:zorg->enforced_relation->planet:mars;").await,
                "a relation between the endpoint tables"
            );
            assert!(
                !succeeds(&db, "RELATE alien:zorg->enforced_relation->planet:venus;").await,
                "a missing endpoint record"
            );
            assert!(
                !succeeds(&db, "RELATE planet:mars->enforced_relation->alien:zorg;").await,
                "endpoints from the wrong tables"
            );
        }

        #[tokio::test]
        async fn a_relation_table_refuses_a_plain_record() {
            let (db, _guard) = db().await;
            apply_definitions::<AnyRelation>(&db).await;
            assert!(!succeeds(&db, "CREATE any_relation:one;").await);
        }

        /// Its schema installs, a `RELATE` without data succeeds and is idempotent, and the
        /// relation can be deleted.
        #[tokio::test]
        async fn a_lightweight_relation_installs_relates_and_deletes() {
            let (db, _guard) = db().await;
            alien_and_planet(&db).await;
            apply_definitions::<LightRelation>(&db).await;
            for _ in 0..2 {
                assert!(
                    succeeds(&db, "RELATE alien:zorg->light_relation->planet:mars;").await,
                    "a data-free relation"
                );
            }
            let related: Vec<RecordId<Planet<u64>>> =
                read(&db, "RETURN alien:zorg->light_relation->planet;").await;
            assert_eq!(related.len(), 1, "{related:?}");
            let statement = umbriel::statements::delete::<LightRelation>(LightRelation::table());
            assert!(succeeds(&db, &statement.to_debug_sql()).await, "deleted");
        }

        /// SurrealDB refuses `LIGHTWEIGHT` on a table that already holds edges.
        #[tokio::test]
        async fn lightweight_is_refused_on_a_table_holding_edges() {
            let (db, _guard) = db().await;
            alien_and_planet(&db).await;
            assert!(
                succeeds(
                    &db,
                    "DEFINE TABLE light_relation TYPE RELATION IN alien OUT planet; \
                     RELATE alien:zorg->light_relation->planet:mars;"
                )
                .await,
                "an edge record"
            );
            let redefined = LightRelation::define_table()
                .as_str()
                .replace("DEFINE TABLE", "DEFINE TABLE OVERWRITE");
            assert!(!succeeds(&db, &redefined).await, "{redefined}");
        }
    }

    #[tokio::test]
    async fn definitions_are_accepted_and_enforced() {
        let (db, _guard) = db().await;
        apply_definitions::<Weapon>(&db).await;

        weapon(1, 10.0)
            .save(&db)
            .await
            .expect("matches the definitions");
        let wrong_type = Raw::new("CREATE weapon:2 SET name = 1, strength = 1.0;")
            .run(&db)
            .await;
        assert!(wrong_type.is_err(), "a field of the wrong type is rejected");
    }

    #[tokio::test]
    async fn definitions_accept_unset_links() {
        let (db, _guard) = db().await;
        apply_definitions::<Alien>(&db).await;

        alien("alice", "Alice")
            .save(&db)
            .await
            .expect("a model whose links are unset matches its definitions");
    }

    #[test]
    fn id_clauses_are_defined_without_a_key_type() {
        assert_eq!(
            joined(&Ticket::define_fields()),
            "DEFINE FIELD id ON TABLE ticket DEFAULT 'first' ASSERT $value != ticket:blocked;
DEFINE FIELD title ON TABLE ticket TYPE string;"
        );
    }

    #[tokio::test]
    async fn id_assertions_and_defaults_apply_to_the_record_id() {
        let (db, _guard) = db().await;
        apply_definitions::<Ticket>(&db).await;

        let blocked = Raw::new("CREATE ticket:blocked SET title = 'Blocked';")
            .run(&db)
            .await;
        assert!(blocked.is_err(), "the assertion rejects the id");
        Raw::new("CREATE ticket:open SET title = 'Open';")
            .run(&db)
            .await
            .expect("an id the assertion accepts");
        Raw::new("CREATE ticket SET title = 'Defaulted';")
            .run(&db)
            .await
            .expect("an id from the default");

        let ids: Vec<RecordId<Ticket>> =
            read(&db, "SELECT VALUE id FROM ticket ORDER BY id;").await;
        assert_eq!(ids, [Ticket::id("first"), Ticket::id("open")]);
    }

    #[test]
    fn an_array_key_type_defines_the_id_as_a_record_id() {
        assert_eq!(
            joined(&Reading::define_fields()),
            "DEFINE FIELD id ON TABLE reading TYPE array<any>;
DEFINE FIELD celsius ON TABLE reading TYPE float;"
        );
        assert!(Reading::get_link_many_fields().is_empty());
    }

    #[tokio::test]
    async fn array_keys_are_enforced_and_round_trip() {
        let (db, _guard) = db().await;
        apply_definitions::<Reading>(&db).await;

        let reading = Reading {
            id: Reading::id(vec![1i64, 2]),
            celsius: 21.5,
        };
        reading.clone().save(&db).await.expect("an array key");
        assert_eq!(
            Reading::get(&db, &reading.id).await.expect("read"),
            Some(reading)
        );

        let string_key = Raw::new("CREATE reading:abc SET celsius = 1.0;")
            .run(&db)
            .await;
        assert!(string_key.is_err(), "the key type is enforced");
    }

    #[tokio::test]
    async fn optional_and_nullable_definitions_accept_none_and_null() {
        let (db, _guard) = db().await;
        apply_definitions::<Profile>(&db).await;

        profile("alice", None, None).save(&db).await.expect("NONE");
        profile("bob", Some("Bobby"), Some(Nullable::Null))
            .save(&db)
            .await
            .expect("NULL");
        profile("carol", None, Some(Nullable::Value("Hello".to_owned())))
            .save(&db)
            .await
            .expect("a value");
    }
}

mod schema {
    use super::*;

    #[test]
    fn arithmetic_on_a_model_with_a_parameter_named_like_the_operand() {
        let counter = Counter::<u8>::schema();
        // `total`'s arithmetic impls take the model's `__UmbrielOperand` and a generated operand.
        assert_eq!((counter.total.clone() + 1).to_debug_sql(), "total + 1");
        assert_eq!((&counter.total * 2).to_debug_sql(), "total * 2");
    }

    #[test]
    fn fields_are_named_after_their_stored_names() {
        assert_eq!(
            Alien::schema().lifeExpectancy.to_debug_sql(),
            "lifeExpectancy"
        );
        assert_eq!(Student::schema().unoBook.to_debug_sql(), "unoBook");
        assert_eq!(StudentLikesBook::schema().r#in.to_debug_sql(), "in");
        assert_eq!(AlienVisitsPlanet::schema().r#in.to_debug_sql(), "in");
    }

    #[test]
    fn links_traverse_into_the_linked_model() {
        let schema = Alien::schema();
        assert_eq!(schema.weapon().name.to_debug_sql(), "weapon.name");
        assert_eq!(schema.ally().name.to_debug_sql(), "ally.name");
        assert_eq!(schema.spaceShips(E).name.to_debug_sql(), "spaceShips.name");
        assert_eq!(
            schema.spaceShips([1usize]).name.to_debug_sql(),
            "spaceShips[1].name"
        );

        let apollo = SpaceShip::schema().name.equal("Apollo");
        assert_eq!(
            schema.spaceShips(apollo.clone()).name.to_debug_sql(),
            format!("spaceShips[WHERE {}].name", cond(apollo).to_debug_sql())
        );
        assert_eq!(
            Alien::schema().weapon().rocket().strength.to_debug_sql(),
            "weapon.rocket.strength"
        );
    }

    #[test]
    fn nested_objects_traverse_into_the_object() {
        assert_eq!(
            Weapon::schema().rocket().strength.to_debug_sql(),
            "rocket.strength"
        );
        assert_eq!(
            Organization::schema().time().connected.to_debug_sql(),
            "time.connected"
        );
    }

    #[test]
    fn graph_traversals_follow_edges() {
        let path = Company::with(Company::id("acme")).like__(E).user(E).name;
        assert!(
            path.to_debug_sql().contains("->like->user"),
            "{}",
            path.to_debug_sql()
        );
    }

    #[test]
    fn relations_are_aliased_by_their_stored_name() {
        let aliased = Alien::get_fields_relations_aliased();
        assert_eq!(aliased.len(), 1);
        assert_eq!(
            aliased[0].to_debug_sql(),
            "->visits->planet AS planetsToVisit"
        );
    }

    #[test]
    fn tables_and_field_lists() {
        assert_eq!(Alien::table().as_str(), "alien");
        assert_eq!(<Alien as Node>::get_table().as_str(), "alien");
        assert_eq!(AlienVisitsPlanet::table().as_str(), "visits");
        assert_eq!(<AlienVisitsPlanet as Edge>::get_table().as_str(), "visits");
        // `WeaponOld` shares `Weapon`'s table under `relax_table`.
        assert_eq!(WeaponOld::table(), Weapon::table());

        assert_eq!(
            sorted(Alien::get_linked_fields()),
            ["ally", "spaceShips", "weapon"]
        );
        assert_eq!(sorted(Alien::get_link_one_fields()), ["weapon"]);
        assert_eq!(sorted(Alien::get_link_self_fields()), ["ally"]);
        assert_eq!(
            sorted(Alien::get_link_one_and_self_fields()),
            ["ally", "weapon"]
        );
        assert_eq!(sorted(Alien::get_link_many_fields()), ["spaceShips"]);

        let serializable = sorted(Alien::get_serializable_fields());
        assert!(serializable.contains(&"lifeExpectancy".to_owned()));
        assert!(!serializable.contains(&"planetsToVisit".to_owned()));
    }

    #[test]
    fn edges_name_their_nodes() {
        fn nodes<T: Edge<In = Alien, Out = Planet<u64>>>() {}
        nodes::<AlienVisitsPlanet>();
    }
}

mod declared_in_functions {
    use super::*;

    #[tokio::test]
    async fn a_model_declared_in_a_function_works() {
        #[model(node, table = "pet")]
        #[derive(Debug, Clone)]
        struct Pet {
            id: RecordId<Self>,
            name: String,
        }

        let (db, _guard) = db().await;
        let rex = Pet {
            id: Pet::id("rex"),
            name: "Rex".to_owned(),
        };
        rex.clone().save(&db).await.expect("saved");

        let found = Pet::get(&db, &rex.id).await.expect("read");
        assert_eq!(found.map(|found| found.name), Some("Rex".to_owned()));
        assert_eq!(Pet::schema().name.to_debug_sql(), "name");
    }
}

mod conversions {
    use super::*;

    #[tokio::test]
    async fn enums_are_stored_and_decoded() {
        let (db, _guard) = db().await;
        apply_definitions::<Plan>(&db).await;
        let acme = Plan {
            id: Plan::id("acme"),
            tier: Tier::Pro,
        };
        acme.clone().create(&db).await.expect("created");

        let stored: String = read(&db, "RETURN plan:acme.tier;").await;
        assert_eq!(stored, "pro");
        assert_eq!(Plan::get(&db, &acme.id).await.expect("read"), Some(acme));
        assert_eq!(
            Tier::from_value(Value::String("free".to_owned())).expect("a tier"),
            Tier::Free
        );

        let unknown = Raw::new("CREATE plan:globex SET tier = 'gold';")
            .run(&db)
            .await;
        assert!(unknown.is_err(), "the literal type rejects other names");
    }

    #[tokio::test]
    async fn a_hand_written_conversion_is_used() {
        let (db, _guard) = db().await;
        assert_eq!(
            joined(&Thermostat::define_fields()),
            "DEFINE FIELD tenths ON TABLE thermostat TYPE int;"
        );
        apply_definitions::<Thermostat>(&db).await;
        let hall = Thermostat {
            id: Thermostat::id("hall"),
            tenths: 215,
        };

        assert_eq!(hall.clone().create(&db).await.expect("created"), hall);
        let stored: i64 = read(&db, "RETURN thermostat:hall.tenths;").await;
        assert_eq!(stored, 215);
        assert_eq!(
            Thermostat::get(&db, &hall.id).await.expect("read"),
            Some(hall)
        );
    }
}

/// Declarations `#[model]` must accept: names that differ only in case, edges, objects, and a
/// model inside a function.
#[expect(
    dead_code,
    non_snake_case,
    reason = "compile-only declarations, some named in camel case on purpose"
)]
mod compiles_declarations {
    use umbriel::{E, LinkOne, Model, Node, RecordId, Relate, SchemaGetter, model};

    #[model(node, table = "user")]
    #[surreal(rename_all = "camelCase")]
    pub struct User {
        pub id: RecordId<Self>,
        pub display_name: String,
        pub profile_name: String,
        #[orm(link_one = "Team")]
        pub team: LinkOne<Team>,
    }

    // Its `name` and `User`'s `profile_name` are distinct generated items.
    #[model(node, table = "user_profile")]
    pub struct UserProfile {
        pub id: RecordId<Self>,
        pub name: String,
    }

    // Shares `UserProfile`'s name in snake case.
    #[allow(non_camel_case_types)]
    #[model(node, table = "user_profile")]
    pub struct User_Profile {
        pub id: RecordId<Self>,
        pub name: String,
    }

    // Fields and edges whose names differ only in case convention.
    #[model(node, table = "task")]
    #[allow(non_snake_case)]
    pub struct Task {
        pub id: RecordId<Self>,
        pub page_views: i64,
        pub pageViews: i64,
        #[orm(relate(model = "TaskAssignedToTeam", connection = "->assigned_to->team"))]
        pub assignees: Relate<Team>,
        #[orm(relate(model = "TaskAssignedToCamelTeam", connection = "->assignedTo->team"))]
        pub camel_assignees: Relate<Team>,
    }

    #[model(edge, table = "assigned_to")]
    pub struct AssignedTo<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    #[model(edge, table = "assignedTo")]
    #[orm(relax_table)]
    pub struct AssignedToCamel<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    pub type TaskAssignedToTeam = AssignedTo<Task, Team>;
    pub type TaskAssignedToCamelTeam = AssignedToCamel<Task, Team>;

    #[model(node, table = "team")]
    pub struct Team {
        pub id: RecordId<Self>,
        pub name: String,
    }

    #[model(edge, table = "member_of")]
    pub struct MemberOf<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    #[model(object)]
    pub struct Address {
        pub city: String,
    }

    #[test]
    fn compiles() {
        // A model can also be declared inside a function.
        #[model(node, table = "pet")]
        struct Pet {
            id: RecordId<Self>,
        }
        let _ = Pet { id: Pet::id("rex") };
        let _ = User::table();
        // Each name reaches its own generated item.
        let task = Task::schema();
        let _ = (task.assigned_to__(E), task.assignedTo__(E));
        let _ = (task.page_views, task.pageViews);
    }
}

/// Every field clause, a view read through every read method, and `Models` unions of views and
/// of writable models.
#[expect(
    dead_code,
    reason = "compile-only declarations: compiling them is the test"
)]
mod compiles_views_and_unions {
    use umbriel::{
        All, Field, LinkOne, Models, Node, Operatable, RecordId, SchemaGetter, SurrealRead, Table,
        TableOf, model,
        statements::{SelectStatement, insert_any, select},
    };

    // Every field clause the derive offers.
    #[model(node, table = "sale")]
    #[orm(kind = "normal", comment = "Acme Corp sales")]
    pub struct Sale {
        pub id: RecordId<Self>,
        pub amount: i64,
        #[orm(computed = Field::new("amount"))]
        pub doubled: i64,
        #[orm(default_always = 0)]
        pub refunds: i64,
        #[orm(value_expr = Field::new("amount"))]
        pub recorded: i64,
        #[orm(readonly, comment = "set once")]
        pub created_by: String,
        #[orm(ty = "object", flexible)]
        pub meta: umbriel::types::Object,
    }

    #[model(node, table = "customer")]
    pub struct Customer {
        pub id: RecordId<Self>,
        pub name: String,
    }

    pub fn sales() -> SelectStatement {
        select(All).from(Table::new("sale"))
    }

    // A view: read through every read method, written through none.
    #[model(view, table = "sale_copy")]
    #[orm(as_ = sales)]
    pub struct SaleCopy {
        pub id: RecordId<Self>,
        pub amount: i64,
    }

    // A union holding a view decodes reads.
    #[derive(Models)]
    #[models(read_only)]
    pub enum Reports {
        Sale(Sale),
        Copy(SaleCopy),
    }

    #[model(edge, table = "bought")]
    pub struct Bought<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
    }

    // A union of writable models, a node and an edge, is inserted as before.
    #[derive(Models)]
    pub enum Records {
        Customer(Customer),
        Bought(Bought<Customer, Sale>),
    }

    fn insert(records: Vec<Records>) {
        let _ = insert_any(records);
    }

    #[test]
    fn compiles() {
        let _ = SaleCopy::select();
        let _ = SaleCopy::find_where(Field::new("amount").greater_than_or_equal(1));
        let _ = SaleCopy::count_all();
        let _ = SaleCopy::select_value(SaleCopy::schema().amount);
        let _ = TableOf::<SaleCopy>::default().select();
        let _ = Sale::select_value(Sale::schema().doubled);
        let _: SelectStatement<Reports> = select(All).from(Table::new("sale")).decode();
        insert(vec![]);
    }
}

/// Names a stored name or table may have that a Rust identifier may not, and lifetimes.
mod compiles_names_and_lifetimes {
    use umbriel::{Model, RecordId, Render, model};

    /// Regression (UMB-031): an `old_name` that isn't an identifier panicked the derive.
    #[model(node, table = "renamed_person")]
    #[orm(relax_table)]
    pub struct RenamedPerson {
        pub id: RecordId<Self>,
        #[orm(old_name = "first-name")]
        pub first_name: String,
    }

    /// A keyword table: `type` is held as a raw identifier.
    #[model(node, table = "type")]
    pub struct Type {
        pub id: RecordId<Self>,
        pub name: String,
    }

    /// Regression (UMB-035): a model with a lifetime wasn't `Send + Sync`, its marker holding
    /// `&'a dyn Any`.
    #[model(object)]
    pub struct Borrowed<'a> {
        #[orm(ty = "string")]
        pub name: std::borrow::Cow<'a, str>,
    }

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn compiles() {
        assert_eq!(Type::TABLE, "type");
        let renamed = RenamedPerson::get_field_meta();
        assert_eq!(
            renamed[0].old_name.as_ref().map(Render::to_debug_sql),
            Some(umbriel::Field::new("first-name").to_debug_sql())
        );
        send_and_sync::<BorrowedSchema<'static>>();
        send_and_sync::<BorrowedMerge<'static>>();
    }
}

/// Items the issue listed as unverified (UMB-014, UMB-034). Both reproduced, and these tests
/// now cover their fixes.
mod unverified {
    use super::*;
    use umbriel::{RecordId, SurrealCrudNode, model};

    /// UMB-034: a fully qualified `std::string::String` infers the same type as `String`.
    #[model(node, table = "qualified")]
    #[derive(Debug, Clone)]
    pub struct Qualified {
        pub id: RecordId<Self>,
        pub name: std::string::String,
    }

    #[test]
    fn a_qualified_string_is_inferred() {
        assert_eq!(
            joined(&Qualified::define_fields()),
            "DEFINE FIELD name ON TABLE qualified TYPE string;"
        );
    }

    /// Regression (UMB-014): a SCHEMAFULL table refused a nested object's fields ("no such field
    /// exists"). The object is `FLEXIBLE` there and keeps them through a round trip.
    #[model(node, table = "armoury")]
    #[orm(schemafull)]
    #[surreal(rename_all = "camelCase")]
    #[derive(Debug, Clone)]
    pub struct Armoury {
        pub id: RecordId<Self>,
        #[orm(nest_object = "Rocket")]
        pub rocket: Rocket,
    }

    #[tokio::test]
    async fn a_nested_object_round_trips_on_a_schemafull_table() {
        let (db, _guard) = db().await;
        apply_definitions::<Armoury>(&db).await;
        let rocket = Rocket {
            name: "Ares".to_owned(),
            strength: 9,
            nice: true,
            bunch_of_other_fields: 3,
            created: chrono::Utc::now(),
        };
        let stored = Armoury {
            id: Armoury::id("one"),
            rocket: rocket.clone(),
        }
        .create(&db)
        .await
        .expect("created");
        assert_eq!(stored.rocket.name, rocket.name);
        assert_eq!(stored.rocket.strength, rocket.strength);
        assert_eq!(stored.rocket.bunch_of_other_fields, rocket.bunch_of_other_fields);
    }
}
