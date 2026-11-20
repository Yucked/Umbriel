//! Schema diffs (`orm/src/migrations/resources/diff.rs`): the statements generated migrations
//! hold for each change, applied to a database and rolled back, and the definitions generation
//! refuses.

use std::{fs, path::Path};

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{DbResources, Raw, migrations::*, statements::UserRole};
use umbriel_tests::{
    db,
    migrations::{helpers::*, *},
};

/// Diffs between schemas, as the statements of generated migrations, applied to a database.
mod schema_changes {
    use super::*;
    use pretty_assertions::assert_eq;

    /// #13's completion check, through migration generation: a migration file holding every new
    /// clause and a codebase defining the same give an empty `SchemaDiff`, so no migration is
    /// written.
    #[tokio::test]
    async fn the_new_definition_clauses_give_no_change() {
        let (_temp, dir) = migration_dir();
        let up = NEW_CLAUSES_TABLES
            .iter()
            .chain([&NEW_CLAUSES_PARAM])
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            &up,
            "REMOVE TABLE sale_large; REMOVE TABLE likes; REMOVE TABLE sale; \
             REMOVE TABLE customer; REMOVE PARAM $greeting;",
        );

        generate(&dir, MigrationFlag::TwoWay, NewClauses, disallow_empty())
            .await
            .expect("generates");
        assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
    }

    #[tokio::test]
    async fn functions_analyzers_and_indexes_are_created_and_removed_in_dependency_order() {
        let (up, down) = round_trip(EmptyResources, NoteSearch).await;
        assert_order(
            &up,
            &[
                "DEFINE FUNCTION OVERWRITE fn::normalize(",
                "DEFINE ANALYZER OVERWRITE normalized ",
                "DEFINE TABLE OVERWRITE note ",
                "DEFINE FIELD OVERWRITE body ON note ",
                "DEFINE INDEX OVERWRITE note_body ON note ",
            ],
        );
        // Removing the table removes its field and index.
        assert_order(
            &down,
            &[
                "REMOVE TABLE note;",
                "REMOVE ANALYZER normalized;",
                "REMOVE FUNCTION fn::normalize;",
            ],
        );
        assert!(!down.contains("REMOVE INDEX"), "{down}");
        assert!(!down.contains("REMOVE FIELD"), "{down}");
    }

    #[tokio::test]
    async fn a_replaced_analyzer_is_removed_after_its_index_moves() {
        let (up, down) = round_trip(NoteSearch, NoteSearchV2).await;
        assert_order(
            &up,
            &[
                "DEFINE FUNCTION OVERWRITE fn::normalize_v2(",
                "DEFINE ANALYZER OVERWRITE normalized_v2 ",
                "DEFINE INDEX OVERWRITE note_body ON note ",
                "REMOVE ANALYZER normalized;",
                "REMOVE FUNCTION fn::normalize;",
            ],
        );
        assert_order(
            &down,
            &[
                "DEFINE FUNCTION OVERWRITE fn::normalize(",
                "DEFINE ANALYZER OVERWRITE normalized ",
                "DEFINE INDEX OVERWRITE note_body ON note ",
                "REMOVE ANALYZER normalized_v2;",
                "REMOVE FUNCTION fn::normalize_v2;",
            ],
        );
    }

    #[tokio::test]
    async fn an_index_is_removed_before_its_analyzer() {
        let (up, down) = round_trip(NoteSearch, NoteOnly).await;
        assert_order(
            &up,
            &[
                "REMOVE INDEX note_body ON TABLE note;",
                "REMOVE ANALYZER normalized;",
                "REMOVE FUNCTION fn::normalize;",
            ],
        );
        assert_order(
            &down,
            &[
                "DEFINE FUNCTION OVERWRITE fn::normalize(",
                "DEFINE ANALYZER OVERWRITE normalized ",
                "DEFINE INDEX OVERWRITE note_body ON note ",
            ],
        );
    }

    #[tokio::test]
    async fn a_removed_table_is_restored_before_its_fields() {
        let (up, down) = round_trip(NoteOnly, EmptyResources).await;
        assert!(up.contains("REMOVE TABLE note;"), "{up}");
        assert!(!up.contains("REMOVE FIELD"), "{up}");
        assert_order(
            &down,
            &[
                "DEFINE TABLE OVERWRITE note ",
                "DEFINE FIELD OVERWRITE body ON note ",
            ],
        );
    }

    #[tokio::test]
    async fn creates_and_removes_mix() {
        let (up, _) = round_trip(NoteSearch, Catalog).await;
        assert_order(
            &up,
            &[
                "DEFINE TABLE OVERWRITE catalog ",
                "REMOVE TABLE note;",
                "REMOVE ANALYZER normalized;",
                "REMOVE FUNCTION fn::normalize;",
            ],
        );
    }

    #[tokio::test]
    async fn every_kind_round_trips() {
        let (up, down) = round_trip(EmptyResources, Resources).await;
        assert!(
            up.contains("DEFINE ACCESS OVERWRITE partner_api ON DATABASE TYPE JWT"),
            "{up}"
        );
        assert!(
            down.contains("REMOVE ACCESS partner_api ON DATABASE;"),
            "{down}"
        );
        assert!(down.contains("REMOVE PARAM $__some_test_param1;"), "{down}");
    }

    #[tokio::test]
    async fn equivalent_formatting_is_no_change() {
        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            FORMATTED_NOTES,
            "REMOVE TABLE memo;",
        );

        generate(
            &dir,
            MigrationFlag::TwoWay,
            FormattedNotes,
            disallow_empty(),
        )
        .await
        .expect("generates");
        assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
    }

    #[track_caller]
    fn assert_redacted(result: MigrationResult<()>, expected: ResourceKey) {
        match result {
            Err(MigrationError::RedactedDefinition { key }) => assert_eq!(key, expected),
            other => panic!("expected RedactedDefinition, got {other:?}"),
        }
    }

    fn user() -> ResourceKey {
        key(ResourceKind::User, None, "reporting")
    }

    fn customer() -> ResourceKey {
        key(ResourceKind::Access, None, "customer")
    }

    fn customer_definition(session: &str) -> String {
        format!(
            "DEFINE ACCESS customer ON DATABASE TYPE RECORD SIGNIN (SELECT * FROM customer) \
             DURATION FOR SESSION {session};"
        )
    }

    #[tokio::test]
    async fn unchanged_secrets_are_no_change() {
        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            &format!(
                "{}\n{}",
                reporting_user(UserRole::Viewer),
                customer_definition("1h")
            ),
            "",
        );

        #[derive(Debug, Clone)]
        struct Secrets;
        impl DbResources for Secrets {
            fn accesses(&self) -> Vec<Raw> {
                RecordAccessResources.accesses()
            }

            fn users(&self) -> Vec<Raw> {
                UserResources.users()
            }
        }

        generate(&dir, MigrationFlag::TwoWay, Secrets, disallow_empty())
            .await
            .expect("generates");
        assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
    }

    #[tokio::test]
    async fn creating_a_secret_is_rejected() {
        let (_temp, dir) = migration_dir();
        assert_redacted(
            generate(&dir, MigrationFlag::TwoWay, UserResources, disallow_empty()).await,
            user(),
        );

        let (_temp, dir) = migration_dir();
        assert_redacted(
            generate(
                &dir,
                MigrationFlag::OneWay,
                RecordAccessResources,
                disallow_empty(),
            )
            .await,
            customer(),
        );
    }

    #[tokio::test]
    async fn updating_a_secret_is_rejected() {
        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            reporting_user(UserRole::Viewer).as_str(),
            "",
        );
        assert_redacted(
            generate(
                &dir,
                MigrationFlag::TwoWay,
                UserResourcesChanged,
                disallow_empty(),
            )
            .await,
            user(),
        );

        let (_temp, dir) = migration_dir();
        write_migration(&dir, MigrationFlag::OneWay, &customer_definition("1h"), "");
        assert_redacted(
            generate(
                &dir,
                MigrationFlag::OneWay,
                RecordAccessResourcesChanged,
                disallow_empty(),
            )
            .await,
            customer(),
        );
    }

    #[tokio::test]
    async fn removing_a_secret_needs_a_one_way_migration() {
        // The down migration would have to restore the user.
        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            reporting_user(UserRole::Viewer).as_str(),
            "",
        );
        assert_redacted(
            generate(
                &dir,
                MigrationFlag::TwoWay,
                EmptyResources,
                disallow_empty(),
            )
            .await,
            user(),
        );

        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::OneWay,
            reporting_user(UserRole::Viewer).as_str(),
            "",
        );
        generate(
            &dir,
            MigrationFlag::OneWay,
            EmptyResources,
            disallow_empty(),
        )
        .await
        .expect("generates");
        let newest = migration_files(&dir).pop().expect("a migration");
        let content = fs::read_to_string(dir.join(newest)).expect("readable");
        assert!(
            content.contains("REMOVE USER reporting ON DATABASE;"),
            "{content}"
        );
    }

    #[tokio::test]
    async fn an_applied_rename_is_no_change() {
        // Once both schemas hold the renamed fields, the models' leftover `old_name` attributes
        // are history: generating again finds no change.
        let (_temp, dir) = migration_dir();
        let flag = MigrationFlag::TwoWay;
        generate(&dir, flag, Resources, disallow_empty())
            .await
            .expect("the first migration generates");
        generate(&dir, flag, ResourcesV2, disallow_empty())
            .await
            .expect("the renaming migration generates");
        let files = migration_files(&dir);

        generate(&dir, flag, ResourcesV2, disallow_empty())
            .await
            .expect("an applied rename is no error");
        assert_eq!(migration_files(&dir), files, "no migration is written");
    }

    #[tokio::test]
    async fn an_applied_rename_changes_like_any_field() {
        // A later change to the renamed field, its `old_name` still in place, updates it in
        // place instead of renaming it again.
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        let flag = MigrationFlag::TwoWay;
        generate(&dir, flag, Resources, MockPrompter::default())
            .await
            .expect("the first migration generates");
        generate(&dir, flag, ResourcesV2, MockPrompter::default())
            .await
            .expect("the renaming migration generates");
        apply_up(&dir, &db).await;

        generate(&dir, flag, ResourcesV2Changed, disallow_empty())
            .await
            .expect("the change generates");
        let (up, down) = newest_migration(&dir);
        for sql in [&up, &down] {
            assert!(
                sql.contains("DEFINE FIELD OVERWRITE characteristics ON animal"),
                "{sql}"
            );
            assert!(!sql.contains("Rename field"), "{sql}");
            assert!(!sql.contains("UPDATE animal"), "{sql}");
        }

        apply_up(&dir, &db).await;
        assert_eq!(
            inspect(&db).await,
            schema_of(&ResourcesV2Changed, flag).await
        );
        roll_back(&dir, &db).await;
        assert_eq!(inspect(&db).await, schema_of(&ResourcesV2, flag).await);
    }
}

/// Functions removed by name, changefeeds, sequences, buckets, APIs, configs and modules; and
/// the checks generation makes, run through a synthetic inspection where a live database cannot
/// hold the definition.
mod definitions {
    use super::*;
    use std::time::Duration;

    use umbriel::{
        statements::{
            ApiMethod, DatabaseInfo, InfoOutput, Statements, Structures, api_action, api_config,
            define_api, define_bucket, define_config_api, define_config_graphql,
            define_module_file, define_module_silo, define_sequence, define_table,
        },
        test_support::{credential_in_backend, database_schema, generate_from_schemas},
    };
    use umbriel_tests::{read, succeeds};

    /// The kinds `INFO FOR DB` lists.
    const DATABASE_KINDS: [&str; 12] = [
        "accesses",
        "apis",
        "analyzers",
        "buckets",
        "configs",
        "functions",
        "models",
        "modules",
        "params",
        "sequences",
        "tables",
        "users",
    ];

    /// A schema holding one database-level definition, parsed from a synthetic `INFO FOR DB`
    /// result as a live one is: `structure` listed under `kind` (e.g. `buckets`), and its
    /// `DEFINE` statement `sql` listed under `name`.
    fn fixture(kind: &str, name: &str, structure: &str, sql: &str) -> Schema {
        let info = |entry: &str, empty: &str| {
            let fields = DATABASE_KINDS
                .map(|key| format!("{key}: {}", if key == kind { entry } else { empty }))
                .join(", ");
            surrealdb_syn::value(&format!("{{ {fields} }}")).expect("the fixture is SurrealQL")
        };
        database_schema(
            DatabaseInfo::<Structures>::from_info(info(&format!("[{structure}]"), "[]"))
                .expect("the structures decode"),
            DatabaseInfo::<Statements>::from_info(info(&format!("{{ {name:?}: {sql:?} }}"), "{}"))
                .expect("the statements decode"),
        )
        .expect("the fixture is a schema")
    }

    /// Generates the migration from `left` to `right` into the existing, empty `dir`, as
    /// generation does once it has inspected both.
    fn generate_between(dir: &Path, left: &Schema, right: &Schema) -> MigrationResult<()> {
        fs::create_dir_all(dir).expect("the migration directory is created");
        generate_from_schemas(
            left,
            right,
            &"generated".into(),
            &config(dir).set_flag(MigrationFlag::TwoWay),
            &EmptyResources,
            &disallow_empty(),
        )
    }

    mod functions {
        use super::*;
        use pretty_assertions::assert_eq;

        #[derive(Debug, Clone)]
        struct WithFunction;

        impl DbResources for WithFunction {
            fn functions(&self) -> Vec<Raw> {
                vec![identity_function("math::add_one")]
            }
        }

        #[tokio::test]
        async fn a_removed_function_is_removed_by_its_path() {
            let (up, down) = round_trip(WithFunction, EmptyResources).await;
            assert!(up.contains("REMOVE FUNCTION fn::math::add_one;"), "{up}");
            assert!(
                down.contains("DEFINE FUNCTION OVERWRITE fn::math::add_one("),
                "{down}"
            );
        }

        /// No live database need hold such a name: the fixture tests that a statement which
        /// cannot be rendered fails generation, whatever SurrealDB accepts.
        #[test]
        fn an_unwritable_function_name_fails_generation_without_a_file() {
            let (_temp, dir) = migration_dir();
            let left = fixture(
                "functions",
                "not-a-path",
                "{ name: 'not-a-path', args: [], block: '{ RETURN 1; }', permissions: true }",
                "DEFINE FUNCTION fn::`not-a-path`() { RETURN 1; } PERMISSIONS FULL",
            );
            match generate_between(&dir, &left, &Schema::default()) {
                Err(MigrationError::InvalidStatement {
                    key: invalid,
                    errors,
                }) => {
                    assert_eq!(invalid, key(ResourceKind::Function, None, "not-a-path"));
                    assert!(
                        errors
                            .iter()
                            .any(|error| error.contains("invalid function name")),
                        "{errors:?}"
                    );
                }
                other => panic!("expected InvalidStatement, got {other:?}"),
            }
            assert!(migration_files(&dir).is_empty(), "no migration is written");
        }
    }

    mod changefeeds {
        use super::*;
        use pretty_assertions::assert_eq;

        /// A table with the changefeed clause `changefeed`, written as SurrealQL so its spelling
        /// can differ.
        #[derive(Debug, Clone)]
        struct Changefeed(&'static str);

        impl DbResources for Changefeed {
            fn tables(&self) -> Vec<Raw> {
                vec![Raw::new(format!("DEFINE TABLE reading {};", self.0))]
            }
        }

        #[tokio::test]
        async fn stored_durations_compare_by_value() {
            let flag = MigrationFlag::TwoWay;
            let week = schema_of(&Changefeed("CHANGEFEED 1w"), flag).await;
            assert_eq!(schema_of(&Changefeed("CHANGEFEED 7d"), flag).await, week);
            assert_ne!(schema_of(&Changefeed("CHANGEFEED 2w"), flag).await, week);
            assert_ne!(
                schema_of(&Changefeed("CHANGEFEED 1w INCLUDE ORIGINAL"), flag).await,
                week
            );

            let (_temp, dir) = migration_dir();
            generate(&dir, flag, Changefeed("CHANGEFEED 7d"), disallow_empty())
                .await
                .expect("generates");
            generate(&dir, flag, Changefeed("CHANGEFEED 1w"), disallow_empty())
                .await
                .expect("generates");
            assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
        }

        #[tokio::test]
        async fn a_changed_changefeed_redefines_the_table() {
            let (up, _) =
                round_trip(Changefeed("CHANGEFEED 1w"), Changefeed("CHANGEFEED 2w")).await;
            assert!(up.contains("DEFINE TABLE OVERWRITE reading"), "{up}");
            assert!(up.contains("CHANGEFEED 2w"), "{up}");
        }
    }

    mod sequences {
        use super::*;
        use pretty_assertions::assert_eq;

        #[derive(Debug, Clone)]
        struct Sequenced {
            batch: u32,
            start: i64,
            timeout: Option<Duration>,
        }

        const ORDERS: Sequenced = Sequenced {
            batch: 100,
            start: 0,
            timeout: None,
        };

        impl DbResources for Sequenced {
            fn sequences(&self) -> Vec<Raw> {
                let sequence = define_sequence("order_number")
                    .batch(self.batch)
                    .start(self.start);
                vec![raw(match self.timeout {
                    Some(timeout) => sequence.timeout(timeout),
                    None => sequence,
                })]
            }
        }

        async fn next_value(db: &Surreal<Any>) -> i64 {
            read(db, "RETURN sequence::nextval('order_number');").await
        }

        /// Three values drawn from `order_number`.
        async fn draw(db: &Surreal<Any>) -> [i64; 3] {
            [
                next_value(db).await,
                next_value(db).await,
                next_value(db).await,
            ]
        }

        #[tokio::test]
        async fn a_new_sequence_is_defined_without_overwrite() {
            let (up, down) = round_trip(EmptyResources, ORDERS).await;
            assert!(
                up.contains("DEFINE SEQUENCE order_number BATCH 100 START 0;"),
                "{up}"
            );
            assert!(!up.contains("DEFINE SEQUENCE OVERWRITE"), "{up}");
            assert!(down.contains("REMOVE SEQUENCE order_number;"), "{down}");
        }

        #[tokio::test]
        async fn an_advanced_sequence_shows_no_change() {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            generate(&dir, MigrationFlag::TwoWay, ORDERS, disallow_empty())
                .await
                .expect("generates");
            apply_up(&dir, &db).await;
            draw(&db).await;
            assert_eq!(
                inspect(&db).await,
                schema_of(&ORDERS, MigrationFlag::TwoWay).await
            );
        }

        /// Regression: a sequence update must keep the values the sequence has issued.
        #[tokio::test]
        async fn a_timeout_change_alters_the_sequence_and_keeps_its_values() {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            generate(&dir, MigrationFlag::TwoWay, ORDERS, disallow_empty())
                .await
                .expect("generates");
            apply_up(&dir, &db).await;
            let drawn = draw(&db).await;

            let timed = Sequenced {
                timeout: Some(Duration::from_secs(5)),
                ..ORDERS
            };
            generate(&dir, MigrationFlag::TwoWay, timed, disallow_empty())
                .await
                .expect("generates");
            let (up, down) = newest_migration(&dir);
            assert!(
                up.contains("ALTER SEQUENCE order_number TIMEOUT 5s;"),
                "{up}"
            );
            assert!(!up.contains("DEFINE SEQUENCE"), "{up}");
            assert!(
                down.contains("ALTER SEQUENCE order_number TIMEOUT NONE;"),
                "{down}"
            );

            apply_up(&dir, &db).await;
            let next = next_value(&db).await;
            assert!(
                drawn.iter().all(|value| next > *value),
                "{next} after {drawn:?}"
            );
        }

        #[tokio::test]
        async fn a_batch_or_start_change_fails_without_a_file() {
            for changed in [
                Sequenced {
                    batch: 200,
                    ..ORDERS
                },
                Sequenced {
                    start: 10,
                    ..ORDERS
                },
            ] {
                let (_temp, dir) = migration_dir();
                generate(&dir, MigrationFlag::TwoWay, ORDERS, disallow_empty())
                    .await
                    .expect("generates");
                let result = generate(&dir, MigrationFlag::TwoWay, changed, disallow_empty()).await;
                assert!(
                    matches!(
                        &result,
                        Err(MigrationError::UnsafeSequenceUpdate { key: unsafe_key })
                            if *unsafe_key == key(ResourceKind::Sequence, None, "order_number")
                    ),
                    "{result:?}"
                );
                assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
            }
        }

        /// Regression: a reset applies its regenerated first migration to the existing
        /// database, where the sequence must fail to be defined rather than be reset.
        #[tokio::test]
        async fn a_regenerated_first_migration_fails_on_an_existing_sequence() {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            generate(&dir, MigrationFlag::TwoWay, ORDERS, disallow_empty())
                .await
                .expect("generates");
            apply_up(&dir, &db).await;
            let history = applied(&db).await;
            let drawn = draw(&db).await;

            let (_regenerated_temp, regenerated) = migration_dir();
            let file_manager = config(&regenerated).set_flag(MigrationFlag::TwoWay);
            let (comparison, _namespace) = umbriel_tests::comparison().await;
            let migration = MigratorDatabase::first_migration(
                &comparison,
                &"reset".into(),
                &file_manager,
                ORDERS,
                MockPrompter::default(),
            )
            .await
            .expect("the first migration generates");
            fs::create_dir_all(&regenerated).expect("the migration directory is created");
            migration
                .create_file(&file_manager)
                .expect("the migration is written");
            let (up, _) = newest_migration(&regenerated);
            assert!(up.contains("DEFINE SEQUENCE order_number "), "{up}");
            assert!(!up.contains("DEFINE SEQUENCE OVERWRITE"), "{up}");

            let error = config(&regenerated)
                .two_way()
                .run_up_pending_migrations(&db, UpdateStrategy::Latest)
                .await
                .expect_err("the existing sequence refuses the migration");
            // The sequence's own refusal, not an earlier check of the history.
            let message = error.to_string();
            assert!(
                matches!(error, MigrationError::ProblemWithQuery(_)),
                "{error:?}"
            );
            assert!(
                message.contains("order_number") && message.contains("already exists"),
                "{message}"
            );

            let next = next_value(&db).await;
            assert!(
                drawn.iter().all(|value| next > *value),
                "{next} after {drawn:?}"
            );
            assert_eq!(applied(&db).await, history, "the history is unchanged");
        }
    }

    mod buckets {
        use super::*;
        use pretty_assertions::assert_eq;

        #[derive(Debug, Clone)]
        struct Bucketed {
            readonly: bool,
        }

        impl DbResources for Bucketed {
            fn buckets(&self) -> Vec<Raw> {
                let bucket = define_bucket("avatars")
                    .backend("memory")
                    .comment("Acme Corp avatars");
                vec![raw(if self.readonly {
                    bucket.readonly()
                } else {
                    bucket
                })]
            }

            fn tables(&self) -> Vec<Raw> {
                vec![raw(define_table("user"))]
            }
        }

        #[tokio::test]
        async fn a_bucket_is_created_first_and_removed_last() {
            let (db, _guard) = db().await;
            let (up, down) = round_trip_on(&db, EmptyResources, Bucketed { readonly: false }).await;
            assert_order(
                &up,
                &["DEFINE BUCKET avatars ", "DEFINE TABLE OVERWRITE user"],
            );
            assert!(!up.contains("DEFINE BUCKET OVERWRITE"), "{up}");
            assert_order(&down, &["REMOVE TABLE user;", "REMOVE BUCKET avatars;"]);
        }

        /// Regression: redefining a `memory` bucket connects a new, empty store, so no update,
        /// however small, is written.
        #[tokio::test]
        async fn a_changed_bucket_fails_without_a_file() {
            let (_temp, dir) = migration_dir();
            let flag = MigrationFlag::TwoWay;
            generate(&dir, flag, Bucketed { readonly: false }, disallow_empty())
                .await
                .expect("generates");
            let result = generate(&dir, flag, Bucketed { readonly: true }, disallow_empty()).await;
            assert!(
                matches!(
                    &result,
                    Err(MigrationError::UnsafeBucketUpdate { key: unsafe_key })
                        if *unsafe_key == key(ResourceKind::Bucket, None, "avatars")
                ),
                "{result:?}"
            );
            assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
        }

        /// Regression: a reset applies its regenerated first migration to the existing
        /// database, where the bucket must fail to be defined rather than lose its files.
        #[tokio::test]
        async fn a_regenerated_first_migration_keeps_an_existing_buckets_files() {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            let bucketed = Bucketed { readonly: false };
            generate(
                &dir,
                MigrationFlag::TwoWay,
                bucketed.clone(),
                disallow_empty(),
            )
            .await
            .expect("generates");
            apply_up(&dir, &db).await;
            let history = applied(&db).await;
            assert!(
                succeeds(&db, "file::put(f\"avatars:/alice.txt\", 'Alice');").await,
                "the file is written"
            );

            let (_regenerated_temp, regenerated) = migration_dir();
            let file_manager = config(&regenerated).set_flag(MigrationFlag::TwoWay);
            let (comparison, _namespace) = umbriel_tests::comparison().await;
            let migration = MigratorDatabase::first_migration(
                &comparison,
                &"reset".into(),
                &file_manager,
                bucketed,
                MockPrompter::default(),
            )
            .await
            .expect("the first migration generates");
            fs::create_dir_all(&regenerated).expect("the migration directory is created");
            migration
                .create_file(&file_manager)
                .expect("the migration is written");
            let (up, _) = newest_migration(&regenerated);
            assert!(up.contains("DEFINE BUCKET avatars "), "{up}");
            assert!(!up.contains("DEFINE BUCKET OVERWRITE"), "{up}");

            let error = config(&regenerated)
                .two_way()
                .run_up_pending_migrations(&db, UpdateStrategy::Latest)
                .await
                .expect_err("the existing bucket refuses the migration");
            // The bucket's own refusal, not an earlier check of the history.
            let message = error.to_string();
            assert!(
                matches!(error, MigrationError::ProblemWithQuery(_)),
                "{error:?}"
            );
            assert!(
                message.contains("avatars") && message.contains("already exists"),
                "{message}"
            );

            let kept: bool = read(&db, "RETURN file::exists(f\"avatars:/alice.txt\");").await;
            assert!(kept, "the bucket keeps its file");
            assert_eq!(applied(&db).await, history, "the history is unchanged");
        }

        #[test]
        fn a_backend_with_credentials_fails_generation_without_a_file() {
            for (backend, expected) in [
                (
                    "s3:/acme?access_key=AKIAEXAMPLE&secret_key=secret-example",
                    "query parameter access_key",
                ),
                (
                    "s3://AKIAEXAMPLE:secret-example@storage.example.com/acme",
                    "user info",
                ),
            ] {
                let (_temp, dir) = migration_dir();
                let right = fixture(
                    "buckets",
                    "acme_files",
                    &format!(
                        "{{ name: 'acme_files', backend: {backend:?}, permissions: false, \
                         readonly: false }}"
                    ),
                    &format!("DEFINE BUCKET acme_files BACKEND '{backend}' PERMISSIONS NONE"),
                );
                let error = generate_between(&dir, &Schema::default(), &right)
                    .expect_err("the credentials are refused");
                assert!(
                    matches!(
                        &error,
                        MigrationError::CredentialInBucketBackend { key: refused, reason }
                            if *refused == key(ResourceKind::Bucket, None, "acme_files")
                                && reason == expected
                    ),
                    "{error:?}"
                );
                let message = error.to_string();
                assert!(
                    !message.contains("AKIAEXAMPLE") && !message.contains("secret-example"),
                    "{message}"
                );
                assert!(migration_files(&dir).is_empty(), "no migration is written");
            }
        }

        #[test]
        fn the_backend_check_allows_only_parameters_without_secrets() {
            for refused in [
                "s3://AKIAEXAMPLE:secret-example@storage.example.com/acme",
                "s3:/acme?access_key=AKIAEXAMPLE&secret_key=secret-example",
                "gs://acme?service_account_key=secret-example",
                "az://acme/files?sas_token=secret-example",
                "az://acme/files?access_key=secret-example",
                "s3:/acme?access%5Fkey=secret-example",
                "s3:/acme?Secret_Key=secret-example",
                "s3:/acme?session_token=secret-example",
                "s3:/acme?region=eu-west-1#secret-example",
            ] {
                let reason = credential_in_backend(refused)
                    .unwrap_or_else(|| panic!("`{refused}` is refused"));
                for secret in [
                    "AKIAEXAMPLE",
                    "secret-example",
                    "Secret_Key",
                    "session_token",
                ] {
                    assert!(!reason.contains(secret), "`{reason}` repeats `{secret}`");
                }
            }
            for accepted in [
                "memory",
                "file:/data/avatars",
                "s3:/acme?region=eu-west-1&prefix=avatars",
                "s3:/acme?use_emulator=true",
                "gs://acme?service_account=/keys/example.json",
                "az://acme/files?anonymous",
                "file:/data/avatars?lowercase_paths=true",
            ] {
                assert_eq!(credential_in_backend(accepted), None, "{accepted}");
            }
        }
    }

    mod apis_configs_and_modules {
        use super::*;
        use pretty_assertions::assert_eq;

        #[derive(Debug, Clone)]
        struct Served {
            answer: &'static str,
        }

        impl DbResources for Served {
            fn buckets(&self) -> Vec<Raw> {
                vec![raw(define_bucket("modules").backend("memory"))]
            }

            fn modules(&self) -> Vec<Raw> {
                vec![
                    raw(define_module_file("geo", "modules", "geo.wasm")),
                    raw(define_module_silo("acme", "maps", (1, 2, 3))),
                ]
            }

            fn apis(&self) -> Vec<Raw> {
                vec![raw(
                    define_api("/status").action(api_action([ApiMethod::Get], self.answer))
                )]
            }

            fn configs(&self) -> Vec<Raw> {
                vec![
                    raw(define_config_api(api_config().permissions_full())),
                    raw(define_config_graphql().tables_auto()),
                ]
            }
        }

        const OK: Served = Served { answer: "ok" };

        #[tokio::test]
        async fn every_kind_round_trips_in_creation_order() {
            let (db, _guard) = db().await;
            let (up, down) = round_trip_on(&db, EmptyResources, OK).await;
            assert_order(
                &up,
                &[
                    "DEFINE BUCKET modules ",
                    "DEFINE MODULE OVERWRITE mod::geo",
                    "DEFINE MODULE OVERWRITE silo::acme::maps::<1.2.3>",
                    "DEFINE API OVERWRITE",
                    "DEFINE CONFIG OVERWRITE API",
                    "DEFINE CONFIG OVERWRITE GRAPHQL",
                ],
            );
            assert_order(
                &down,
                &[
                    "REMOVE CONFIG GRAPHQL;",
                    "REMOVE CONFIG API;",
                    "REMOVE API '/status';",
                    "REMOVE MODULE silo::acme::maps::<1.2.3>;",
                    "REMOVE MODULE mod::geo;",
                    "REMOVE BUCKET modules;",
                ],
            );
        }

        #[tokio::test]
        async fn an_unchanged_schema_shows_no_change() {
            let (_temp, dir) = migration_dir();
            generate(&dir, MigrationFlag::TwoWay, OK, disallow_empty())
                .await
                .expect("generates");
            generate(&dir, MigrationFlag::TwoWay, OK, disallow_empty())
                .await
                .expect("generates");
            assert_eq!(migration_files(&dir).len(), 2, "no migration is written");
        }

        #[tokio::test]
        async fn a_changed_action_redefines_only_the_api() {
            let (db, _guard) = db().await;
            let (up, down) = round_trip_on(&db, OK, Served { answer: "moved" }).await;
            assert!(up.contains("DEFINE API OVERWRITE"), "{up}");
            assert!(up.contains("moved"), "{up}");
            assert!(down.contains("DEFINE API OVERWRITE"), "{down}");
            for other in ["BUCKET", "MODULE", "CONFIG"] {
                assert!(!up.contains(other), "{other} in {up}");
            }
        }
    }
}
