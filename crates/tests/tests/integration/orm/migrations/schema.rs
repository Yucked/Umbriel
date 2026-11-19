//! Schema inspection (`orm/src/migrations/schema.rs`): a database read into normalized
//! definitions, as the migration generator compares them.

use std::collections::BTreeSet;

use umbriel::{
    DbResources, Raw, Runnable, create_table_resources,
    migrations::*,
    statements::{UserRole, info_for},
};
use umbriel_tests::{
    db,
    migrations::{helpers::*, *},
};

    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    use umbriel::{migrations::ResourceKind::*, test_support::field_definition};

    /// Every kind migrations manage, secrets included.
    #[derive(Debug, Clone)]
    struct Everything;

    impl DbResources for Everything {
        fn tables(&self) -> Vec<Raw> {
            Resources.tables()
        }

        fn analyzers(&self) -> Vec<Raw> {
            Resources.analyzers()
        }

        fn functions(&self) -> Vec<Raw> {
            Resources.functions()
        }

        fn params(&self) -> Vec<Raw> {
            Resources.params()
        }

        fn accesses(&self) -> Vec<Raw> {
            let mut accesses = Resources.accesses();
            accesses.push(customer_access(std::time::Duration::from_secs(60 * 60)));
            accesses
        }

        fn users(&self) -> Vec<Raw> {
            vec![reporting_user(UserRole::Viewer)]
        }
    }

    /// `Resources` with its tables created in the reverse order, so their internal ids differ.
    #[derive(Debug, Clone)]
    struct ReorderedResources;

    impl DbResources for ReorderedResources {
        create_table_resources!(
            Planet,
            Student,
            AnimalSnakeCaseEatsCrop,
            AnimalEatsCrop,
            Crop,
            AnimalSnakeCase,
            Animal
        );

        fn analyzers(&self) -> Vec<Raw> {
            Resources.analyzers()
        }

        fn functions(&self) -> Vec<Raw> {
            Resources.functions()
        }

        fn params(&self) -> Vec<Raw> {
            Resources.params()
        }

        fn accesses(&self) -> Vec<Raw> {
            Resources.accesses()
        }
    }

    #[tokio::test]
    async fn every_supported_kind_is_inspected() {
        let schema = schema_of(&Everything, MigrationFlag::TwoWay).await;

        let kinds = schema
            .iter()
            .map(|(key, _)| key.kind)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            kinds,
            BTreeSet::from([
                Function, Analyzer, Param, Access, User, Table, Field, Index, Event
            ])
        );

        assert_eq!(
            names(&schema, Function),
            ["get_animal_by_id", "get_animal_by_id2"]
        );
        assert_eq!(names(&schema, Analyzer), ["ascii"]);
        assert_eq!(names(&schema, Access), ["customer", "partner_api"]);
        assert_eq!(names(&schema, User), ["reporting"]);
        assert!(
            schema
                .get(&key(Index, Some("animal"), "species_speed_idx"))
                .is_some()
        );
        assert!(schema.get(&key(Event, Some("animal"), "event1")).is_some());
        assert!(schema.get(&key(Field, Some("animal"), "species")).is_some());
        assert_eq!(
            schema
                .get(&key(Table, None, "animal"))
                .expect("the table is inspected")
                .sql(),
            "DEFINE TABLE animal TYPE NORMAL SCHEMAFULL PERMISSIONS NONE"
        );

        // The server hides the user's password hash and the record access's signing key, but
        // nothing of the JWT access verified with a public key.
        let redacted = |key: ResourceKey| {
            schema
                .get(&key)
                .expect("the definition is inspected")
                .is_redacted()
        };
        assert!(redacted(key(User, None, "reporting")));
        assert!(redacted(key(Access, None, "customer")));
        assert!(!redacted(key(Access, None, "partner_api")));
    }

    #[tokio::test]
    async fn placeholder_text_is_not_a_secret() {
        // Only the properties the server redacts count: a param's value and an access's comment
        // reading `[REDACTED]` are ordinary text.
        let schema = schema_of(&PlaceholderResources, MigrationFlag::TwoWay).await;
        let checked = schema
            .iter()
            .filter(|(key, _)| matches!(key.kind, Param | Access))
            .inspect(|(key, definition)| assert!(!definition.is_redacted(), "{key}"))
            .count();
        assert_eq!(checked, 2);

        round_trip(EmptyResources, PlaceholderResources).await;
    }

    #[tokio::test]
    async fn table_ids_do_not_affect_equality() {
        assert_eq!(
            schema_of(&Resources, MigrationFlag::TwoWay).await,
            schema_of(&ReorderedResources, MigrationFlag::TwoWay).await
        );
    }

    #[tokio::test]
    async fn fields_join_their_define_statements() {
        let schema = schema_of(&Catalog, MigrationFlag::TwoWay).await;
        let fields = schema.fields("catalog").collect::<Vec<_>>();
        assert_eq!(fields.len(), 9, "{fields:#?}");
        for (key, definition) in fields {
            assert!(
                definition
                    .sql()
                    .starts_with(&format!("DEFINE FIELD {} ON ", key.name)),
                "{key} has `{}`",
                definition.sql()
            );
        }
    }

    fn statements(sql: &[&str]) -> BTreeMap<String, String> {
        let mut statements = BTreeMap::new();
        for (index, sql) in sql.iter().enumerate() {
            statements.insert(format!("field{index}"), (*sql).to_owned());
        }
        statements
    }

    #[test]
    fn a_field_joins_exactly_one_statement() {
        let fields = statements(&[
            "DEFINE FIELD foo ON t TYPE object",
            "DEFINE FIELD foo.bar ON t TYPE string",
            "DEFINE FIELD foobar ON t TYPE string",
        ]);
        assert_eq!(
            field_definition(&fields, "t", "foo").expect("one match"),
            "DEFINE FIELD foo ON t TYPE object"
        );
        assert_eq!(
            field_definition(&fields, "t", "foo.bar").expect("one match"),
            "DEFINE FIELD foo.bar ON t TYPE string"
        );

        let mismatch = |statements: &BTreeMap<String, String>, field: &str| match field_definition(
            statements, "t", field,
        ) {
            Err(MigrationError::FieldDefinitionMismatch { matches, .. }) => matches,
            other => panic!("expected FieldDefinitionMismatch, got {other:?}"),
        };
        // `fo` prefixes three statements' names but is none of them.
        assert_eq!(mismatch(&fields, "fo"), 0);
        assert_eq!(mismatch(&statements(&[]), "foo"), 0);
        let duplicated = statements(&[
            "DEFINE FIELD foo ON t TYPE object",
            "DEFINE FIELD foo ON t TYPE string",
        ]);
        assert_eq!(mismatch(&duplicated, "foo"), 2);
    }

    #[tokio::test]
    async fn a_database_user_inspects_without_namespace_privileges() {
        let (db, guard) = db().await;
        Raw::new(
            "DEFINE TABLE memo SCHEMAFULL;
             DEFINE FIELD title ON TABLE memo TYPE string;
             DEFINE USER reader ON DATABASE PASSWORD 'example-password' ROLES VIEWER;",
        )
        .run(&db)
        .await
        .expect("defined");
        db.signin(surrealdb::opt::auth::Database {
            namespace: guard.namespace().to_owned(),
            database: guard.database().to_owned(),
            username: "reader".to_owned(),
            password: "example-password".to_owned(),
        })
        .await
        .expect("signed in as the database user");

        let schema = inspect(&db).await;
        assert!(schema.get(&key(Field, Some("memo"), "title")).is_some());
        assert!(
            info_for().namespace().run(&db).await.is_err(),
            "the database user cannot inspect the namespace"
        );
    }

    /// #13's completion check: every clause #13 adds survives inspection. A schema using them is
    /// redefined in a fresh database from the server's own `DEFINE` statements, and inspects the
    /// same, so migrations generated from it have no spurious changes.
    #[tokio::test]
    async fn the_new_definition_clauses_round_trip() {
        let (db, _guard) = db().await;
        for sql in NEW_CLAUSES_TABLES.iter().chain([&NEW_CLAUSES_PARAM]) {
            Raw::new(*sql)
                .run(&db)
                .await
                .unwrap_or_else(|error| panic!("`{sql}` defines: {error}"));
        }
        let schema = inspect(&db).await;
        assert!(
            schema
                .get(&key(Table, None, "sale_large"))
                .is_some_and(|view| view.sql().contains(" AS SELECT ")),
            "the view is inspected"
        );

        // Redefined with `OVERWRITE`, as the migrator writes updates: a `TYPE RELATION` table
        // defines its own `in` and `out` fields, so a plain `DEFINE FIELD in` would fail.
        let (fresh, _fresh_guard) = umbriel_tests::db().await;
        for (key, definition) in schema.iter() {
            let keyword = key.kind.keyword();
            let rest = definition
                .sql()
                .strip_prefix(&format!("DEFINE {keyword} "))
                .unwrap_or_else(|| panic!("{key} is a DEFINE {keyword}"));
            Raw::new(format!("DEFINE {keyword} OVERWRITE {rest};"))
                .run(&fresh)
                .await
                .unwrap_or_else(|error| panic!("{key} redefines: {error}"));
        }
        assert_eq!(inspect(&fresh).await, schema);
    }
