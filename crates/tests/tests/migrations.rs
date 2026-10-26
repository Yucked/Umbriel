//! Migrations: schema inspection and diffs, migration history, generating, applying, listing,
//! pruning, resetting and rolling back migrations, embedded migrations and the CLI. Every test
//! runs on the in-memory engine.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use pretty_assertions::assert_eq;
use surrealdb::{Surreal, engine::any::Any};
use tempfile::{TempDir, tempdir};
use umbriel::{
    DbResources, Raw, Render, Runnable, create_table_resources,
    migrations::{
        DatabaseConnection, FastForwardDelta, Generate, Init, Migration, MigrationConfig,
        MigrationError, MigrationFilename, MigrationFlag, MigrationResult, MigratorDatabase,
        MockPrompter, Mode, RenameOrDelete, Reset, ResourceKey, ResourceKind, RightDatabase,
        RollbackOptions, RollbackStrategyStruct, Schema, Status, UpdateStrategy,
    },
    statements::{UserRole, info_for},
};
use umbriel_tests::{
    current_function,
    migrations::*,
    migrator_support::{AssertionArg, TestConfig},
};

/// A temporary directory and the migration directory inside it, which does not exist yet.
fn migration_dir() -> (TempDir, PathBuf) {
    let temp = tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations-tests");
    (temp, dir)
}

fn config(dir: &Path) -> MigrationConfig {
    MigrationConfig::builder()
        .mode(Mode::Strict)
        .custom_path(Some(dir.to_path_buf()))
        .build()
}

/// Generates a migration from `resources` into `dir`.
async fn generate(
    dir: &Path,
    flag: MigrationFlag,
    resources: impl DbResources,
    prompter: MockPrompter,
) -> MigrationResult<()> {
    let name = "generated".into();
    match flag {
        MigrationFlag::TwoWay => {
            config(dir)
                .two_way()
                .generate_migrations(&name, resources, prompter)
                .await
        }
        MigrationFlag::OneWay => {
            config(dir)
                .one_way()
                .generate_migrations(&name, resources, prompter)
                .await
        }
    }
}

fn disallow_empty() -> MockPrompter {
    MockPrompter::builder()
        .allow_empty_migrations_gen(false)
        .rename_or_delete_single_field_change(RenameOrDelete::Rename)
        .build()
}

/// The database migrations are applied to: in memory, signed in as root.
async fn database() -> Surreal<Any> {
    DatabaseConnection::default()
        .setup()
        .await
        .db()
        .expect("connected")
}

/// The schema `resources` define, which a database migrated to them must hold.
async fn schema_of(resources: &impl DbResources, flag: MigrationFlag) -> Schema {
    let database = RightDatabase(
        MigratorDatabase::init()
            .await
            .expect("an in-memory database"),
    );
    database
        .run_codebase_schema_queries(resources, flag)
        .await
        .expect("the definitions apply");
    Schema::inspect(&database.db)
        .await
        .expect("the schema is inspected")
}

async fn inspect(db: &Surreal<Any>) -> Schema {
    Schema::inspect(db).await.expect("the schema is inspected")
}

async fn apply_up(dir: &Path, db: &Surreal<Any>) {
    config(dir)
        .two_way()
        .run_up_pending_migrations(db.clone(), UpdateStrategy::Latest)
        .await
        .expect("the pending migrations apply");
}

async fn roll_back(dir: &Path, db: &Surreal<Any>) {
    config(dir)
        .two_way()
        .run_down_migrations(db.clone(), RollbackOptions::default())
        .await
        .expect("the latest migration rolls back");
}

/// The migration files of `dir`, in order.
fn migration_files(dir: &Path) -> Vec<String> {
    let mut names = fs::read_dir(dir)
        .expect("the migration directory exists")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// The newest two-way migration's up and down files.
fn newest_migration(dir: &Path) -> (String, String) {
    let up = migration_files(dir)
        .into_iter()
        .rev()
        .find(|name| name.ends_with(".up.surql"))
        .expect("a two-way migration");
    let down = up.replace(".up.surql", ".down.surql");
    (
        fs::read_to_string(dir.join(up)).expect("readable"),
        fs::read_to_string(dir.join(down)).expect("readable"),
    )
}

/// Generates two-way migrations to `from` and then to `to`, applying each to one database: after
/// each its schema must be the codebase's, and rolling the second back must restore `from`'s.
/// Returns the second migration's up and down files.
async fn round_trip(
    from: impl DbResources + Clone,
    to: impl DbResources + Clone,
) -> (String, String) {
    let (_temp, dir) = migration_dir();
    let db = database().await;
    let flag = MigrationFlag::TwoWay;

    generate(&dir, flag, from.clone(), MockPrompter::default())
        .await
        .expect("the first migration generates");
    apply_up(&dir, &db).await;
    assert_eq!(inspect(&db).await, schema_of(&from, flag).await);

    generate(&dir, flag, to.clone(), MockPrompter::default())
        .await
        .expect("the second migration generates");
    apply_up(&dir, &db).await;
    assert_eq!(inspect(&db).await, schema_of(&to, flag).await);

    let files = newest_migration(&dir);
    roll_back(&dir, &db).await;
    assert_eq!(inspect(&db).await, schema_of(&from, flag).await);
    files
}

/// Asserts that each of `statements` appears in `sql`, in this order.
#[track_caller]
fn assert_order(sql: &str, statements: &[&str]) {
    let positions = statements
        .iter()
        .map(|statement| {
            sql.find(statement)
                .unwrap_or_else(|| panic!("`{statement}` is missing from:\n{sql}"))
        })
        .collect::<Vec<_>>();
    assert!(
        positions.is_sorted(),
        "{statements:?} are out of order in:\n{sql}"
    );
}

/// The history table's definitions, which a first migration holds.
fn history_table(flag: MigrationFlag) -> String {
    std::iter::once(Migration::define_table().to_debug_sql())
        .chain(
            Migration::define_fields(flag)
                .iter()
                .map(|field| field.as_str().to_owned()),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

/// Writes a migration as if it had been generated: `up` followed by the history table.
fn write_migration(dir: &Path, flag: MigrationFlag, up: &str, down: &str) {
    fs::create_dir_all(dir).expect("the migration directory is created");
    let up = format!("{up}\n{}", history_table(flag));
    let name = "20240101000000000_handwritten";
    match flag {
        MigrationFlag::TwoWay => {
            fs::write(dir.join(format!("{name}.up.surql")), up).expect("written");
            fs::write(dir.join(format!("{name}.down.surql")), down).expect("written");
        }
        MigrationFlag::OneWay => {
            fs::write(dir.join(format!("{name}.surql")), up).expect("written");
        }
    }
}

/// Writes (or rewrites) the two-way migration numbered `number` (1 to 9, in order) by hand; the
/// first also defines the history table. Returns the up file's name, which the history records.
fn write_numbered(dir: &Path, number: u8, up: &str, down: &str) -> String {
    fs::create_dir_all(dir).expect("the migration directory is created");
    let up = if number == 1 {
        format!("{up}\n{}", history_table(MigrationFlag::TwoWay))
    } else {
        up.to_owned()
    };
    let name = format!("2024010100000000{number}_step_{number}");
    fs::write(dir.join(format!("{name}.up.surql")), up).expect("written");
    fs::write(dir.join(format!("{name}.down.surql")), down).expect("written");
    format!("{name}.up.surql")
}

/// The applied migrations' file names, oldest first.
async fn applied(db: &Surreal<Any>) -> Vec<String> {
    let mut names = Migration::get_all_desc(db.clone())
        .await
        .expect("the history is read")
        .into_iter()
        .map(|migration| migration.name)
        .collect::<Vec<_>>();
    names.reverse();
    names
}

/// The tables of `db`, in name order.
async fn tables(db: &Surreal<Any>) -> Vec<String> {
    names(&inspect(db).await, ResourceKind::Table)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn key(kind: ResourceKind, table: Option<&str>, name: &str) -> ResourceKey {
    ResourceKey {
        kind,
        table: table.map(str::to_owned),
        name: name.to_owned(),
    }
}

/// The names of the definitions of `kind`, in order.
fn names(schema: &Schema, kind: ResourceKind) -> Vec<&str> {
    schema
        .iter()
        .filter(|(key, _)| key.kind == kind)
        .map(|(key, _)| key.name.as_str())
        .collect()
}

/// Inspecting a database into normalized definitions.
mod schema {
    use super::*;
    use pretty_assertions::assert_eq;
    use umbriel::{migrations::ResourceKind::*, test_support::field_definition, types::Object};

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

    fn statements(sql: &[&str]) -> Object {
        let mut statements = Object::new();
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

        let mismatch =
            |statements: &Object, field: &str| match field_definition(statements, "t", field) {
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
        let db = database().await;
        Raw::new(
            "DEFINE TABLE memo SCHEMAFULL;
             DEFINE FIELD title ON TABLE memo TYPE string;
             DEFINE USER reader ON DATABASE PASSWORD 'example-password' ROLES VIEWER;",
        )
        .run(db.clone())
        .await
        .expect("defined");
        db.signin(surrealdb::opt::auth::Database {
            namespace: "test".to_owned(),
            database: "test".to_owned(),
            username: "reader".to_owned(),
            password: "example-password".to_owned(),
        })
        .await
        .expect("signed in as the database user");

        let schema = inspect(&db).await;
        assert!(schema.get(&key(Field, Some("memo"), "title")).is_some());
        assert!(
            info_for().namespace().run(db.clone()).await.is_err(),
            "the database user cannot inspect the namespace"
        );
    }
}

/// Diffs between schemas, as the statements of generated migrations, applied to a database.
mod diff {
    use super::*;
    use pretty_assertions::assert_eq;

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

    #[tokio::test]
    async fn unmanaged_kinds_are_rejected() {
        let unsupported = |result: MigrationResult<()>| match result {
            Err(MigrationError::UnsupportedSchemaResource { level, kind, names }) => {
                (level, kind, names)
            }
            other => panic!("expected UnsupportedSchemaResource, got {other:?}"),
        };

        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            "DEFINE SEQUENCE order_number;",
            "",
        );
        let result = generate(
            &dir,
            MigrationFlag::TwoWay,
            EmptyResources,
            disallow_empty(),
        )
        .await;
        assert_eq!(
            unsupported(result),
            (
                "database".to_owned(),
                "sequences".to_owned(),
                vec!["order_number".to_owned()]
            )
        );

        let (_temp, dir) = migration_dir();
        write_migration(
            &dir,
            MigrationFlag::TwoWay,
            "DEFINE USER ns_admin ON NAMESPACE PASSWORD 'example-password' ROLES OWNER;",
            "",
        );
        let result = generate(
            &dir,
            MigrationFlag::TwoWay,
            EmptyResources,
            disallow_empty(),
        )
        .await;
        assert_eq!(
            unsupported(result),
            (
                "namespace".to_owned(),
                "users".to_owned(),
                vec!["ns_admin".to_owned()]
            )
        );
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
        let db = database().await;
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
        assert_eq!(inspect(&db).await, schema_of(&ResourcesV2Changed, flag).await);
        roll_back(&dir, &db).await;
        assert_eq!(inspect(&db).await, schema_of(&ResourcesV2, flag).await);
    }
}

/// Reading the migration history.
mod history {
    use super::*;
    use surrealdb::types::{ConnectionError, NotAllowedError, NotFoundError};
    use umbriel::test_support::history_or_empty;

    #[tokio::test]
    async fn a_fresh_database_has_no_history() {
        let db = database().await;
        assert!(
            Migration::get_latest(db.clone())
                .await
                .expect("readable")
                .is_none()
        );
        assert!(
            Migration::get_all_desc(db.clone())
                .await
                .expect("readable")
                .is_empty()
        );
        let filename = MigrationFilename::try_from("20240101000000000_init.up.surql".to_owned())
            .expect("a migration file name");
        assert!(
            Migration::get_by_filename(db, &filename)
                .await
                .expect("readable")
                .is_none()
        );
    }

    #[tokio::test]
    async fn an_undecodable_history_fails() {
        let db = database().await;
        Raw::new("DEFINE TABLE migration SCHEMALESS; CREATE migration:broken SET name = 1;")
            .run(db.clone())
            .await
            .expect("created");
        assert!(Migration::get_all_desc(db).await.is_err());
    }

    /// A failed statement, classified as execution classifies it.
    fn statement_error(source: surrealdb::Error) -> umbriel::Result<Vec<Migration>> {
        Err(umbriel::test_support::classify(
            source,
            Some(umbriel::StatementContext {
                index: 0,
                related: vec![],
            }),
        ))
    }

    #[test]
    fn only_a_missing_history_table_reads_as_empty_history() {
        let missing = |details: NotFoundError| {
            statement_error(surrealdb::Error::not_found("missing".to_owned(), details))
        };

        let history = history_or_empty(missing(NotFoundError::Table {
            name: "migration".to_owned(),
        }))
        .expect("no history");
        assert!(history.is_empty());

        for error in [
            missing(NotFoundError::Table {
                name: "other".to_owned(),
            }),
            missing(NotFoundError::Database {
                name: "test".to_owned(),
            }),
            statement_error(surrealdb::Error::not_allowed(
                "permission denied".to_owned(),
                None::<NotAllowedError>,
            )),
            Err(umbriel::Error::Query(surrealdb::Error::connection(
                "connection lost".to_owned(),
                None::<ConnectionError>,
            ))),
            Err(umbriel::Error::Conversion(surrealdb::Error::internal(
                "undecodable".to_owned(),
            ))),
        ] {
            assert!(history_or_empty(error).is_err());
        }
    }
}

mod generate {
    use super::*;
    use test_case::test_case;

    #[test_case(Mode::Strict; "Strict")]
    #[test_case(Mode::Lax; "Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to detect migration type")]
    async fn test_cannot_generate_without_db_run_without_init(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        assert!(
            !migration_dir.exists(),
            "Migration directory cannot be created with generate if not migration not already initialized"
        );
    }

    #[test_case(Mode::Strict; "Strict")]
    #[test_case(Mode::Lax; "Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to detect migration type")]
    async fn test_cannot_generate_with_db_run_without_init(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        assert!(
            !migration_dir.exists(),
            "Migration directory cannot be created with generate if not migration not already initialized"
        );
    }

    /// The definitions of `Animal` and `Planet` and the history table, as the server renders
    /// them with `OVERWRITE`.
    fn assert_animal_and_planet(snapshot: &str) {
        for statement in [
            "DELETE migration;",
            "DEFINE TABLE OVERWRITE animal TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
            "DEFINE FIELD OVERWRITE attributes ON animal TYPE array<string> PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE createdAt ON animal TYPE datetime PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE species ON animal TYPE string PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE updatedAt ON animal TYPE datetime PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE velocity ON animal TYPE int PERMISSIONS FULL;",
            "DEFINE TABLE OVERWRITE migration TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
            "DEFINE FIELD OVERWRITE checksum_up ON migration TYPE string PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE id ON migration TYPE string PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE name ON migration TYPE string PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE timestamp ON migration TYPE int PERMISSIONS FULL;",
            "DEFINE TABLE OVERWRITE planet TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
            "DEFINE FIELD OVERWRITE createdAt ON planet TYPE datetime PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE firstName ON planet TYPE string PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE labels ON planet TYPE array<string> PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE population ON planet TYPE int PERMISSIONS FULL;",
            "DEFINE FIELD OVERWRITE updatedAt ON planet TYPE datetime PERMISSIONS FULL;",
        ] {
            assert!(snapshot.contains(statement), "missing `{statement}`");
        }
        // A model's record id is not a defined field.
        assert!(!snapshot.contains("DEFINE FIELD OVERWRITE id ON animal"));
        assert!(!snapshot.contains("DEFINE FIELD OVERWRITE id ON planet"));
    }

    const CHECKSUM_DOWN: &str =
        "DEFINE FIELD OVERWRITE checksum_down ON migration TYPE string PERMISSIONS FULL;";

    #[test_case(Mode::Strict, true, RenameOrDelete::Rename; "Strict Reversible and single field name change diff strategy is implicit renaming")]
    #[test_case(Mode::Strict, true, RenameOrDelete::Delete; "Strict Reversible and single field name change diff strategy is implicit deletion")]
    #[test_case(Mode::Lax, true, RenameOrDelete::Rename; "Lax Reversible and single field name change diff strategy is implicit renaming")]
    #[test_case(Mode::Lax, true, RenameOrDelete::Delete; "Lax Reversible and single field name change diff strategy is implicit deletion")]
    #[test_case(Mode::Strict, false, RenameOrDelete::Rename; "Strict Non-Reversible and single field name change diff strategy is implicit renaming")]
    #[test_case(Mode::Strict, false, RenameOrDelete::Delete; "Strict Non-Reversible and single field name change diff strategy is implicit deletion")]
    #[test_case(Mode::Lax, false, RenameOrDelete::Rename; "Lax Non-Reversible and single field name change diff strategy is implicit renaming")]
    #[test_case(Mode::Lax, false, RenameOrDelete::Delete; "Lax Non-Reversible and single field name change diff strategy is implicit deletion")]
    #[tokio::test]
    async fn test_successfully_handles_renaming(
        mode: Mode,
        reversible: bool,
        single_field_name_change_diff_strategy: RenameOrDelete,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        #[derive(Debug, Clone)]
        pub struct ResourcesV1;
        impl DbResources for ResourcesV1 {
            create_table_resources!(Animal, Planet);
        }
        #[derive(Debug, Clone)]
        pub struct ResourcesV2;
        impl DbResources for ResourcesV2 {
            create_table_resources!(AnimalV2, PlanetV2);
        }

        let mock_prompter = MockPrompter::builder()
            .allow_empty_migrations_gen(true)
            .rename_or_delete_single_field_change(single_field_name_change_diff_strategy)
            .build();

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(false)
                .build(),
            ResourcesV1,
            mock_prompter,
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        let snapshot = conf.assert_migration_queries_snapshot();
        if reversible {
            assert!(
                snapshot.contains("header: Basename - migration_init. Extension - down.surql"),
                "is reversible and has down"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - surql",),
                "not one way"
            );

            assert!(snapshot.contains("REMOVE TABLE animal;"));
            assert!(snapshot.contains("REMOVE TABLE migration;"));
            assert!(snapshot.contains("REMOVE TABLE planet;"));

            assert!(snapshot.contains("header: Basename - migration_init. Extension - up.surql"));
            assert_animal_and_planet(&snapshot);
            assert!(snapshot.contains(CHECKSUM_DOWN));
        } else {
            assert!(
                snapshot.contains("header: Basename - migration_init. Extension - surql"),
                "should be one way"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - down.surql"),
                "should not have down cos its one way"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - up.surql"),
                "should not have up cos its one way"
            );
            assert!(!snapshot.contains(CHECKSUM_DOWN));
            assert_animal_and_planet(&snapshot);
        }

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1".into())
                .run(false)
                .build(),
            ResourcesV2,
            mock_prompter,
        )
        .await;
        // The implicit renaming strategy is set in mock prompter above
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_gen_1".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        let snapshot = conf.assert_migration_queries_snapshot();
        let assert_up_forward_v2_with_renaming = || {
            assert!(
                snapshot.contains(
                    "DEFINE FIELD OVERWRITE characteristics ON animal TYPE array<string> PERMISSIONS FULL;\
                    \nUPDATE animal SET characteristics = attributes;\
                    \nREMOVE FIELD attributes ON TABLE animal;"
                ),
                "Successfully handles explicit renaming"
            );
            assert!(snapshot.contains("REMOVE FIELD createdAt ON TABLE animal;"));
            assert!(snapshot.contains("REMOVE FIELD updatedAt ON TABLE animal;"));

            match single_field_name_change_diff_strategy {
                RenameOrDelete::Rename => {
                    assert!(
                        snapshot.contains(
                            "DEFINE FIELD OVERWRITE newName ON planet TYPE string PERMISSIONS FULL;\
                            \nUPDATE planet SET newName = firstName;\
                            \nREMOVE FIELD firstName ON TABLE planet;"
                        ),
                        "Successfully handles implicit renaming when single field changed"
                    );
                }
                RenameOrDelete::Delete => {
                    // Definitions come before removals.
                    assert!(snapshot.contains(
                        "DEFINE FIELD OVERWRITE newName ON planet TYPE string PERMISSIONS FULL;"
                    ));
                    assert!(snapshot.contains("REMOVE FIELD firstName ON TABLE planet;"));
                    assert!(!snapshot.contains("UPDATE planet SET newName = firstName;"));
                }
            }
        };
        if reversible {
            assert!(snapshot.contains("header: Basename - migration_gen_1. Extension - up.surql"));
            assert!(
                !snapshot.contains("header: Basename - migration_gen_1. Extension - surql",),
                "not one way"
            );
            assert_animal_and_planet(&snapshot);
            assert_up_forward_v2_with_renaming();

            assert!(
                snapshot.contains("header: Basename - migration_gen_1. Extension - down.surql")
            );
            assert!(
                snapshot.contains(
                    "DEFINE FIELD OVERWRITE attributes ON animal TYPE array<string> PERMISSIONS FULL;\
                    \nUPDATE animal SET attributes = characteristics;\
                    \nREMOVE FIELD characteristics ON TABLE animal;"
                ),
                "Successfully handles explicit renaming reversal"
            );

            match single_field_name_change_diff_strategy {
                RenameOrDelete::Rename => {
                    assert!(
                        snapshot.contains(
                            "DEFINE FIELD OVERWRITE firstName ON planet TYPE string PERMISSIONS FULL;\
                            \nUPDATE planet SET firstName = newName;\
                            \nREMOVE FIELD newName ON TABLE planet;"
                        ),
                        "Successfully handles implicit renaming reversal"
                    );
                }
                RenameOrDelete::Delete => {
                    assert!(
                        snapshot.contains("REMOVE FIELD newName ON TABLE planet;"),
                        "Successfully handles implicit deletion reversal"
                    );
                }
            }
        } else {
            assert_animal_and_planet(&snapshot);
            assert_up_forward_v2_with_renaming();
            assert!(
                snapshot.contains("header: Basename - migration_init. Extension - surql"),
                "should be one way"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - down.surql"),
                "should not have down cos its one way"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - up.surql"),
                "should not have up cos its one way"
            );
        }

        assert!(migration_dir.exists());
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_can_generate_after_first_initializing_no_db_run(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        let snapshot = conf.assert_migration_queries_snapshot();
        let assert_forward_up_migrations_snaps = || {
            assert_animal_and_planet(&snapshot);
            for statement in [
                "DEFINE INDEX OVERWRITE species_speed_idx ON animal FIELDS species, velocity UNIQUE;",
                "DEFINE EVENT OVERWRITE event1 ON animal WHEN ",
                "DEFINE EVENT OVERWRITE event2 ON animal WHEN ",
                "DEFINE TABLE OVERWRITE crop TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
                "DEFINE FIELD OVERWRITE color ON crop TYPE string PERMISSIONS FULL;",
                "DEFINE TABLE OVERWRITE eats TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
                "DEFINE FIELD OVERWRITE createdAt ON eats TYPE datetime PERMISSIONS FULL;",
                "DEFINE FIELD OVERWRITE in ON eats TYPE record",
                "DEFINE FIELD OVERWRITE out ON eats TYPE record",
                "DEFINE FIELD OVERWRITE place ON eats TYPE string PERMISSIONS FULL;",
                "DEFINE TABLE OVERWRITE student TYPE NORMAL SCHEMAFULL PERMISSIONS NONE;",
                "DEFINE FIELD OVERWRITE age ON student TYPE int PERMISSIONS FULL;",
                "DEFINE FIELD OVERWRITE createdAt ON student TYPE datetime PERMISSIONS FULL;",
                "DEFINE FIELD OVERWRITE university ON student TYPE string PERMISSIONS FULL;",
                "DEFINE FIELD OVERWRITE updatedAt ON student TYPE datetime PERMISSIONS FULL;",
                "DEFINE FUNCTION OVERWRITE fn::get_animal_by_id(",
                "DEFINE ANALYZER OVERWRITE ascii ",
                "DEFINE PARAM OVERWRITE $__some_test_param1 ",
                "DEFINE ACCESS OVERWRITE partner_api ON DATABASE TYPE JWT",
            ] {
                assert!(snapshot.contains(statement), "missing `{statement}`");
            }
        };

        if reversible {
            assert!(
                snapshot.contains("header: Basename - migration_init. Extension - down.surql"),
                "is reversible and has down"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - surql",),
                "not one way"
            );

            for table in ["animal", "crop", "eats", "migration", "planet", "student"] {
                assert!(snapshot.contains(&format!("REMOVE TABLE {table};")));
            }

            assert!(snapshot.contains("header: Basename - migration_init. Extension - up.surql"));
            assert_forward_up_migrations_snaps();
            assert!(snapshot.contains(CHECKSUM_DOWN));
        } else {
            assert!(
                snapshot.contains("header: Basename - migration_init. Extension - surql"),
                "should be one way"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - down.surql"),
                "should not have down cos its one way"
            );
            assert!(
                !snapshot.contains("header: Basename - migration_init. Extension - up.surql"),
                "should not have up cos its one way"
            );
            assert!(!snapshot.contains(CHECKSUM_DOWN));
            assert_forward_up_migrations_snaps();
        }

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_gen_1".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        assert!(migration_dir.exists());
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_can_generate_after_first_initializing_with_run(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_gen_1".into()),
            // we didnt run after genreate, so the latest db migration meta should remain the same
            // as the one created at initialization.
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        assert!(migration_dir.exists());
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_can_generate_with_run_after_first_initializing_with_run(
        mode: Mode,
        reversible: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 2,
            expected_latest_migration_file_basename_normalized: Some("migration_gen_1".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_gen_1".into()),
            code_origin_line: std::line!(),
        })
        .await;

        assert!(migration_dir.exists());
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_multiple_generation(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(reversible)
            .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_two_way_can_disallow_empty_migration_gen_on_no_diff(
        mode: Mode,
        reversible: bool,
    ) {
        let resourve_v1 = || Resources;
        let mock_prompter_disallow_gen_on_empty_diff = || {
            MockPrompter::builder()
                // disallow empty migration generation on no diffs
                .allow_empty_migrations_gen(false)
                .rename_or_delete_single_field_change(RenameOrDelete::Delete)
                .build()
        };
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            resourve_v1(),
            mock_prompter_disallow_gen_on_empty_diff(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1 but we are not accept empty generation on no diff in mock prompter".into())
                .run(true)
                .build(),
            resourve_v1(),
            mock_prompter_disallow_gen_on_empty_diff(),
        )
        .await;
        assert!(migration_dir.exists());
        // New files wont be generated because there is no diff (Resources -> Resources), and we
        // disallowed empty migrations in mock prompter above
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        // Redo and make sure
        conf.run_gen(
            Generate::builder()
                .name("migration gen again but we are not accept empty generation on no diff in mock prompter".into())
                .run(true)
                .build(),
            resourve_v1(),
            mock_prompter_disallow_gen_on_empty_diff(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration gen 1 this time we allow mock prompter to generate empty migration on no diff".into())
                .run(true)
                .build(),
            resourve_v1(),
            MockPrompter::builder()
                .allow_empty_migrations_gen(true)
                .rename_or_delete_single_field_change(RenameOrDelete::Delete)
                .build(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 2,
            expected_latest_migration_file_basename_normalized: Some("migration_gen_1_this_time_we_allow_mock_prompter_to_generate_empty_migration_on_no_diff".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_gen_1_this_time_we_allow_mock_prompter_to_generate_empty_migration_on_no_diff".into()),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_can_regenerate_after_an_applied_rename(mode: Mode, reversible: bool) {
        // Once the rename is applied, `ResourcesV2`'s leftover `old_name` attributes are history:
        // generating again finds no change, and the empty migration the prompter allows applies.
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration 2 gen".into())
                .run(true)
                .build(),
            ResourcesV2,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 2,
            expected_latest_migration_file_basename_normalized: Some("migration_2_gen".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_2_gen".into()),
            code_origin_line: std::line!(),
        })
        .await;

        assert!(migration_dir.exists());

        conf.run_gen(
            Generate::builder()
                .name("migration 3 gen".into())
                .run(true)
                .build(),
            ResourcesV2,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 3,
            expected_db_mig_meta_count: 3,
            expected_latest_migration_file_basename_normalized: Some("migration_3_gen".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_3_gen".into()),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to generate migrations")]
    async fn test_should_panic_if_same_field_renaming_using_same_old_field_cos_its_not_allowed(
        mode: Mode,
        reversible: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration 2 gen".into())
                .run(true)
                .build(),
            invalid_cases::ResourcesVRenamingWithSameOldFieldNameDisallowed,
            MockPrompter::default(),
        )
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to generate migrations")]
    async fn test_should_panic_if_renaming_from_currently_used_field(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_gen(
            Generate::builder()
                .name("migration 2 gen".into())
                .run(true)
                .build(),
            invalid_cases::ResourcesVRenamingFromCurrentlyUsedFieldNameDisallowed,
            MockPrompter::default(),
        )
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to generate migrations")]
    async fn test_should_panic_if_renaming_from_non_existing_field_in_migration_directory_state(
        mode: Mode,
        reversible: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            // AnimalV2 renames `attributes`, which the empty migration directory never defined.
            ResourcesV2,
            MockPrompter::default(),
        )
        .await;
        assert!(migration_dir.exists());
    }
}

mod init {
    use super::*;
    use test_case::test_case;

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_init_without_run(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_init_with_run(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(
        expected = "Migrations already initialized. Run 'cargo run -- reset' to reset migration. \
                    You can also specify the '-r' or '--reversible' argument to set as reversible. \
                    Or delete the migrations directory and run 'cargo run -- init' again."
    )]
    async fn test_cannot_init_twice_consecutively_with_same_names(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(
        expected = "Migrations already initialized. Run 'cargo run -- reset' to reset migration. \
                    You can also specify the '-r' or '--reversible' argument to set as reversible. \
                    Or delete the migrations directory and run 'cargo run -- init' again."
    )]
    async fn test_cannot_init_twice_consecutively_with_different_names(
        mode: Mode,
        reversible: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("migration init".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_init(
            Init::builder()
                .reversible(reversible)
                .name("another name".into())
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
    }
}

mod up {
    use super::*;
    use pretty_assertions::assert_eq;
    use test_case::test_case;

    #[test_case(Mode::Strict; "Strict")]
    #[test_case(Mode::Lax; "Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to detect migration type.")]
    async fn test_one_way_cannot_run_up_without_init(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_run_up_after_init_with_no_run(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_init(
            Init::builder()
                .name("migration_init".into())
                .reversible(reversible)
                //  only initialize, do not run against db
                .run(false)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().latest(true).build())
            .await;
        let db_state = conf
            .assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 1,
                expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    "migration_init".into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;

        // Explicit checks besides the snapshots, so changes to the applied schema are
        // intentional.
        let schema = &db_state.resources;
        assert_eq!(names(schema, ResourceKind::Analyzer), ["ascii"]);
        let tables = names(schema, ResourceKind::Table);
        assert_eq!(
            tables,
            [
                "animal",
                "animal_snake_case",
                "crop",
                "eats",
                "eats_snake_case",
                "migration",
                "planet",
                "student"
            ]
        );
        for table in tables {
            assert_eq!(
                schema
                    .get(&key(ResourceKind::Table, None, table))
                    .expect("the table is defined")
                    .sql(),
                format!("DEFINE TABLE {table} TYPE NORMAL SCHEMAFULL PERMISSIONS NONE")
            );
        }
        assert_eq!(
            names(schema, ResourceKind::Param),
            [
                "__some_test_param1",
                "__some_test_param2",
                "__some_test_param3"
            ]
        );
        assert_eq!(
            names(schema, ResourceKind::Function),
            ["get_animal_by_id", "get_animal_by_id2"]
        );
        assert_eq!(names(schema, ResourceKind::Access), ["partner_api"]);
        assert!(names(schema, ResourceKind::User).is_empty());

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_run_up_after_init_with_run(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.run_init(
            Init::builder()
                .name("migration_init".into())
                .reversible(reversible)
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_run_up_default_which_is_latest(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_run_up_with_explicit_number_delta_fwd_strategy(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        conf.run_up(&FastForwardDelta::builder().number(1).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_1_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(5).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 6,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_6_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(0).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 6,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_6_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(1).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 7,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_7_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(5).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(1000).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn text_mixed_run_up_strategies_with_larger_runs(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_test_migrations_arbitrary(69, reversible.into())
            .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 69,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(26).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 69,
            expected_db_mig_meta_count: 26,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_26_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().latest(true).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 69,
            expected_db_mig_meta_count: 69,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_run_up_to_latest_with_number_delta_strategy(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_test_migrations_arbitrary(69, reversible.into())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 69,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(69).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 69,
            expected_db_mig_meta_count: 69,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_69_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_zero_delta_moves_no_needle(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(0).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_apply_till_migration_filename_pointer(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(5, reversible))
                .build(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 5,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_5_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        for i in 6..=11 {
            conf.run_up(
                &FastForwardDelta::builder()
                    .till(conf.get_either_filename_type_at_position(i, reversible))
                    .build(),
            )
            .await;

            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 12,
                expected_db_mig_meta_count: i,
                expected_latest_migration_file_basename_normalized: Some(
                    "migration_12_gen_after_init".into(),
                ),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    format!("migration_{}{}", i, "_gen_after_init").into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        }

        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(12, reversible))
                .build(),
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to run migrations. Migration already run or not found")]
    async fn test_cannot_apply_already_applied(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(12, reversible))
                .build(),
        )
        .await;
        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(12, reversible))
                .build(),
        )
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to run migrations. Migration already run or not found")]
    async fn test_cannot_apply_older(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(12, reversible))
                .build(),
        )
        .await;
        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(5, reversible))
                .build(),
        )
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    #[should_panic(expected = "Failed to run migrations. Migration already run or not found")]
    async fn test_cannot_apply_nonexisting_migration(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        let nonexisting_filename = MigrationFilename::try_from(
            "20231220050955_this_migration_does_not_exist.up.surql".to_string(),
        )
        .expect("Failed to parse file name");
        let non_existing_name = if reversible {
            nonexisting_filename.to_up()
        } else {
            nonexisting_filename.to_unidirectional()
        };

        conf.run_up(&FastForwardDelta::builder().till(non_existing_name).build())
            .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_mixture_of_update_strategies(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(3, reversible))
                .build(),
        )
        .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 3,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_3_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(4).build())
            .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 7,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_7_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(
            &FastForwardDelta::builder()
                .till(conf.get_either_filename_type_at_position(9, reversible))
                .build(),
        )
        .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 9,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_9_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().latest(true).build())
            .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[tokio::test]
    async fn a_failing_migration_stops_the_run() {
        // Each migration commits with its history record on its own: a failing one stops the
        // run, keeps those before it, leaves nothing of itself, and a retry resumes with it.
        let (_temp, dir) = migration_dir();
        let db = database().await;
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        write_numbered(
            &dir,
            2,
            "DEFINE TABLE second;\nTHROW 'the second migration fails';",
            "REMOVE TABLE second;",
        );
        write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");

        let failed = config(&dir)
            .two_way()
            .run_up_pending_migrations(db.clone(), UpdateStrategy::Latest)
            .await;
        assert!(failed.is_err(), "{failed:?}");
        assert_eq!(applied(&db).await, std::slice::from_ref(&first));
        assert_eq!(tables(&db).await, ["first", "migration"]);

        let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
        let third = write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");
        apply_up(&dir, &db).await;
        assert_eq!(applied(&db).await, [first, second, third]);
        assert_eq!(tables(&db).await, ["first", "migration", "second", "third"]);
    }
}

mod down {
    use super::*;
    use pretty_assertions::assert_eq;
    use test_case::test_case;
    use umbriel::migrations::RollbackStrategy;

    #[tokio::test]
    async fn a_failing_rollback_stops_and_keeps_its_migration() {
        // Each rollback commits with its history deletion on its own, most recent first: a
        // failing one stops the rollback, keeps the migrations before it applied, and a retry
        // resumes with it.
        let (_temp, dir) = migration_dir();
        let db = database().await;
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        let second = write_numbered(
            &dir,
            2,
            "DEFINE TABLE second;",
            "REMOVE TABLE second;\nTHROW 'the second rollback fails';",
        );
        write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");
        apply_up(&dir, &db).await;

        let failed = config(&dir)
            .two_way()
            .run_down_migrations(
                db.clone(),
                RollbackOptions::new().strategy(RollbackStrategy::Number(3)),
            )
            .await;
        assert!(failed.is_err(), "{failed:?}");
        assert_eq!(applied(&db).await, [first, second]);
        assert_eq!(tables(&db).await, ["first", "migration", "second"]);

        // The fixed down file no longer matches its applied checksum, so the retry is lax.
        write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
        config(&dir)
            .two_way()
            .run_down_migrations(
                db.clone(),
                RollbackOptions::new()
                    .strategy(RollbackStrategy::Number(2))
                    .mode(Mode::Lax),
            )
            .await
            .expect("the retry rolls back the rest");
        assert!(applied(&db).await.is_empty());
        assert_eq!(tables(&db).await, ["migration"]);
    }

    #[tokio::test]
    async fn a_lax_rollback_through_the_first_migration_removes_orphaned_history() {
        // The second migration's files are gone, which lax mode allows. Its history record is
        // removed before the first migration's down migration removes the history table.
        let (_temp, dir) = migration_dir();
        let db = database().await;
        write_numbered(
            &dir,
            1,
            "DEFINE TABLE first;",
            "REMOVE TABLE first;\nREMOVE TABLE migration;",
        );
        let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
        apply_up(&dir, &db).await;
        fs::remove_file(dir.join(&second)).expect("removed");
        fs::remove_file(dir.join(second.replace(".up.", ".down."))).expect("removed");

        config(&dir)
            .two_way()
            .run_down_migrations(
                db.clone(),
                RollbackOptions::new()
                    .strategy(RollbackStrategy::Number(2))
                    .mode(Mode::Lax),
            )
            .await
            .expect("the rollback succeeds");
        assert!(applied(&db).await.is_empty());
        // Without its down file, the second migration's table stays.
        assert_eq!(tables(&db).await, ["second"]);
    }

    #[test_case(Mode::Strict; "Reversible Strict")]
    #[test_case(Mode::Lax; "Reversible Lax")]
    #[tokio::test]
    async fn test_rollback_previous(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(true).await;

        // First apply all generated migrations to the current db instance
        conf.run_up(&FastForwardDelta::default()).await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder().previous(true).build(),
            false,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 11,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 10,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_10_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder().previous(true).build(),
            false,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 9,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_9_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        let default_fwd_strategy = &FastForwardDelta::builder().latest(true).build();
        conf.run_up(default_fwd_strategy).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        // Prune this time around
        conf.run_down(
            &RollbackStrategyStruct::builder().previous(true).build(),
            true,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 11,
            expected_db_mig_meta_count: 11,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 11,
            expected_db_mig_meta_count: 10,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_10_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder().previous(true).build(),
            false,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 11,
            expected_db_mig_meta_count: 9,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_9_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), true)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 8,
            expected_db_mig_meta_count: 8,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_8_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_8_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        for i in 0..5 {
            conf.run_down(&RollbackStrategyStruct::default(), false)
                .await;
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 8,
                expected_db_mig_meta_count: 7 - i,
                expected_latest_migration_file_basename_normalized: Some(
                    "migration_8_gen_after_init".into(),
                ),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    format!("migration_{}{}", 7 - i, "_gen_after_init").into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        }

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 8,
            expected_db_mig_meta_count: 3,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_8_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_3_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), true)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 2,
            expected_db_mig_meta_count: 2,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_2_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_2_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), true)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_1_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_1_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some("migration_1_init".into()),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), true)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict; "Reversible Strict")]
    #[test_case(Mode::Lax; "Reversible Lax")]
    #[tokio::test]
    async fn test_rollback_number_delta(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(true).await;

        // First apply all generated migrations to the current db instance
        conf.run_up(&FastForwardDelta::default()).await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::builder().number(1).build(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 11,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::default(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 10,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_10_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::builder().number(1).build(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 9,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_9_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        let default_fwd_strategy = &FastForwardDelta::builder().latest(true).build();
        conf.run_up(default_fwd_strategy).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::builder().number(5).build(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 7,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_7_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::builder().number(3).build(), true)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 4,
            expected_db_mig_meta_count: 4,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_4_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_4_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::builder().number(4).build(), false)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 4,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_4_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 4,
            expected_db_mig_meta_count: 4,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_4_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_4_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(&RollbackStrategyStruct::builder().number(400).build(), true)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict; "Reversible Strict")]
    #[test_case(Mode::Lax; "Reversible Lax")]
    #[tokio::test]
    async fn test_rollback_till_pointer_mig_id(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(true).await;

        // First apply all generated migrations to the current db instance
        conf.run_up(&FastForwardDelta::default()).await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(12))
                .build(),
            false,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 11,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(11))
                .build(),
            false,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 10,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_10_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        for i in (1..=10).rev() {
            conf.run_down(
                &RollbackStrategyStruct::builder()
                    .till(conf.get_down_filename_at_position(i))
                    .build(),
                false,
            )
            .await;

            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 12,
                expected_db_mig_meta_count: i as u8 - 1,
                expected_latest_migration_file_basename_normalized: Some(
                    "migration_12_gen_after_init".into(),
                ),
                expected_latest_db_migration_meta_basename_normalized: if i == 1 {
                    None
                } else if i == 2 {
                    Some("migration_1_init".into())
                } else {
                    Some(format!("migration_{}{}", i - 1, "_gen_after_init").into())
                },
                code_origin_line: std::line!(),
            })
            .await;
        }

        // Reset
        conf.run_up(&FastForwardDelta::default()).await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(12))
                .build(),
            true,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 11,
            expected_db_mig_meta_count: 11,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(4))
                .build(),
            true,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 3,
            expected_db_mig_meta_count: 3,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_3_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_3_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(1))
                .build(),
            true,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict; "Reversible Strict")]
    #[test_case(Mode::Lax; "Reversible Lax")]
    #[should_panic]
    #[tokio::test]
    async fn cannot_rollback_twice_to_same_cursor_cos_it_does_not_exist_the_second_time(
        mode: Mode,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        let reversible = true;
        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        // First apply all generated migrations to the current db instance
        conf.run_up(&FastForwardDelta::default()).await;

        let nonexisting_filename = MigrationFilename::try_from(
            "20231220050955_this_migration_does_not_exist.up.surql".to_string(),
        )
        .expect("Failed to parse file name");
        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(nonexisting_filename)
                .build(),
            false,
        )
        .await;
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(Mode::Strict; "Reversible Strict")]
    #[test_case(Mode::Lax; "Reversible Lax")]
    #[should_panic(expected = "Rollback Failed")]
    #[tokio::test]
    async fn rollingback_to_nonexisting_filecursor_panics(mode: Mode) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(true).await;

        // First apply all generated migrations to the current db instance
        conf.run_up(&FastForwardDelta::default()).await;

        // 12th exists the first time but not second
        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(12))
                .build(),
            false,
        )
        .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 11,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_11_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        // 12th exists the first time but not second
        conf.run_down(
            &RollbackStrategyStruct::builder()
                .till(conf.get_down_filename_at_position(12))
                .build(),
            false,
        )
        .await;
    }
}

mod list {
    use super::*;
    use pretty_assertions::assert_eq;
    use test_case::test_case;

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_migration_listing_based_on_status(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        let db = conf.migrator.db().clone();
        let fm = conf.migrator.file_manager();

        conf.generate_test_migrations_arbitrary(89, reversible.into())
            .await;

        let migrations = |status| async move {
            async move {
                if reversible {
                    fm.two_way().list_migrations(db, status, mode).await
                } else {
                    fm.one_way().list_migrations(db, status, mode).await
                }
            }
            .await
            .unwrap()
        };

        conf.run_up(&FastForwardDelta::builder().number(32).build())
            .await;

        assert_eq!(migrations.clone()(Status::All).await.len(), 89);
        assert_eq!(migrations.clone()(Status::Pending).await.len(), 89 - 32);
        assert_eq!(migrations(Status::Applied).await.len(), 32);
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_migration_listing_is_idempotent_and_does_not_break_migration_state(
        mode: Mode,
        reversible: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_test_migrations_arbitrary(89, reversible.into())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 89,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_89_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(32).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 89,
            expected_db_mig_meta_count: 32,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_89_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_32_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_list(Status::All).await;
        conf.run_list(Status::Pending).await;
        conf.run_list(Status::Applied).await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 89,
            expected_db_mig_meta_count: 32,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_89_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_32_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;
    }
}

mod prune {
    use super::*;
    use test_case::test_case;

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_can_prune_only_unapplied_migrations(mode: Mode, reversible: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_12_test_migrations_reversible(reversible)
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_prune().await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;
    }

    #[test_case(Mode::Strict, true; "Reversible Strict")]
    #[test_case(Mode::Lax, true; "Reversible Lax")]
    #[test_case(Mode::Strict, false; "Non-Reversible Strict")]
    #[test_case(Mode::Lax, false; "Non-Reversible Lax")]
    #[tokio::test]
    async fn test_can_prune_some_unapplied_and_some_applied_migrations(
        mode: Mode,
        reversible: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        conf.generate_test_migrations_arbitrary(89, reversible.into())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 89,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_89_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_up(&FastForwardDelta::builder().number(32).build())
            .await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 89,
            expected_db_mig_meta_count: 32,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_89_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_32_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_prune().await;
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 32,
            expected_db_mig_meta_count: 32,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_32_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_32_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;
    }
}

mod reset {
    use super::*;
    use pretty_assertions::assert_eq;
    use test_case::test_case;
    use umbriel::migrations::Migrator;

    /// The applied migration history of the test's database.
    async fn history(conf: &TestConfig) -> Vec<Migration> {
        Migration::get_all_desc(conf.migrator.db())
            .await
            .expect("the history is read")
    }

    /// The base name of the most recently applied migration.
    async fn latest_applied(conf: &TestConfig) -> Option<umbriel::migrations::Basename> {
        Migration::get_latest(conf.migrator.db())
            .await
            .expect("the history is read")
            .map(|m| {
                MigrationFilename::try_from(m.name)
                    .expect("Failed to convert migration name to filename")
                    .basename()
            })
    }

    #[test_case(true,Mode::Strict,  true; "Reversible Strict Run")]
    #[test_case(true,Mode::Lax,  false; "Reversible Lax No Run")]
    #[test_case(false,Mode::Strict,  true; "Non-Reversible Strict Run")]
    #[test_case(false, Mode::Lax,  false; "Non-Reversible Lax No Run")]
    #[tokio::test]
    async fn test_can_reset_before_init(reversible: bool, mode: Mode, run: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        assert!(!migration_dir.exists());

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;
        conf.assert_migration_queries_snapshot();

        conf.run_reset(
            Reset::builder()
                .name("migration reset".into())
                .reversible(reversible)
                .run(run)
                .build(),
            ResourcesV3,
            MockPrompter::default(),
        )
        .await;

        assert!(migration_dir.exists());
        if run {
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 1,
                expected_latest_migration_file_basename_normalized: Some("migration_reset".into()),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    "migration_reset".into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        } else {
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 0,
                expected_latest_migration_file_basename_normalized: Some("migration_reset".into()),
                expected_latest_db_migration_meta_basename_normalized: None,
                code_origin_line: std::line!(),
            })
            .await;
        }
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(true,Mode::Strict,  true; "Reversible Strict No Run")]
    #[test_case(true,Mode::Lax,  false; "Reversible Lax No Run")]
    #[test_case(false,Mode::Strict,  true; "Non-Reversible Strict Run")]
    #[test_case(false, Mode::Lax,  false; "Non-Reversible Lax No Run")]
    #[tokio::test]
    async fn test_can_reset_after_init_run(reversible: bool, mode: Mode, run: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;
        assert!(!migration_dir.exists());
        conf.assert_migration_queries_snapshot();
        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 0,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: None,
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_init(
            Init::builder()
                .name("migration init".into())
                .reversible(reversible)
                .run(true)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;
        conf.assert_migration_queries_snapshot();
        assert!(migration_dir.exists());

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 1,
            expected_db_mig_meta_count: 1,
            expected_latest_migration_file_basename_normalized: Some("migration_init".into()),
            expected_latest_db_migration_meta_basename_normalized: Some("migration_init".into()),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_reset(
            Reset::builder()
                .name("migration reset".into())
                .reversible(reversible)
                .run(run)
                .build(),
            ResourcesV3,
            MockPrompter::default(),
        )
        .await;
        conf.assert_migration_queries_snapshot();

        assert!(migration_dir.exists());
        if run {
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 1,
                expected_latest_migration_file_basename_normalized: Some("migration_reset".into()),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    "migration_reset".into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        } else {
            // When we reset, and not apply against the current database instance, the old
            // stale latest migration metadataa would still exist in the database.
            let mig_files = conf.read_migrations_from_dir_sorted_asc();
            let latest_file = mig_files.last().map(MigrationFilename::basename);

            assert_eq!(latest_applied(&conf).await, Some("migration_init".into()));
            assert_eq!(latest_file, Some("migration_reset".into()));

            assert_eq!(mig_files.len(), if reversible { 2 } else { 1 });
            assert_eq!(history(&conf).await.len(), 1);
        }
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(true,Mode::Strict,  true; "Reversible Strict Run")]
    #[test_case(true,Mode::Lax,  false; "Reversible Lax No Run")]
    #[test_case(false,Mode::Strict,  true; "Non-Reversible Strict Run")]
    #[test_case(false, Mode::Lax,  false; "Non-Reversible Lax No Run")]
    #[tokio::test]
    async fn test_reset_after_multiple_unapplied_migrations(
        reversible: bool,
        mode: Mode,
        run: bool,
    ) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 0,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: None,
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_reset(
            Reset::builder()
                .name("migration reset".into())
                .reversible(reversible)
                .run(run)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;

        if run {
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 1,
                expected_latest_migration_file_basename_normalized: Some("migration_reset".into()),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    "migration_reset".into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        } else {
            // When we reset, and not apply against the current database instance, the old
            // stale latest migration metadataa would still exist in the database.
            let mig_files = conf.read_migrations_from_dir_sorted_asc();
            let latest_file = mig_files.last().map(MigrationFilename::basename);

            assert_eq!(latest_applied(&conf).await, None);
            assert_eq!(history(&conf).await.len(), 0);

            assert_eq!(latest_file, Some("migration_reset".into()));
            assert_eq!(mig_files.len(), if reversible { 2 } else { 1 });
        }
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(true,Mode::Strict,  true; "Reversible Strict Run")]
    #[test_case(true,Mode::Lax,  false; "Reversible Lax No Run")]
    #[test_case(false,Mode::Strict,  true; "Non-Reversible Strict Run")]
    #[test_case(false, Mode::Lax,  false; "Non-Reversible Lax No Run")]
    #[tokio::test]
    async fn test_reset_after_multiple_applied_migrations(reversible: bool, mode: Mode, run: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;
        conf.run_up(&FastForwardDelta::default()).await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 12,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_reset(
            Reset::builder()
                .name("migration reset".into())
                .reversible(reversible)
                .run(run)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;

        if run {
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 1,
                expected_latest_migration_file_basename_normalized: Some("migration_reset".into()),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    "migration_reset".into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        } else {
            // When we reset, and not apply against the current database instance, the old
            // stale latest migration metadataa would still exist in the database.
            let mig_files = conf.read_migrations_from_dir_sorted_asc();
            let latest_file = mig_files.last().map(MigrationFilename::basename);

            assert_eq!(history(&conf).await.len(), 12);
            assert_eq!(
                latest_applied(&conf).await,
                Some("migration_12_gen_after_init".into())
            );

            assert_eq!(mig_files.len(), if reversible { 2 } else { 1 });
            assert_eq!(latest_file, Some("migration_reset".into()));
        }
        conf.assert_migration_queries_snapshot();
    }

    #[test_case(true,Mode::Strict,  true; "Reversible Strict Run")]
    #[test_case(true,Mode::Lax,  false; "Reversible Lax No Run")]
    #[test_case(false,Mode::Strict,  true; "Non-Reversible Strict Run")]
    #[test_case(false, Mode::Lax,  false; "Non-Reversible Lax No Run")]
    #[tokio::test]
    async fn test_reset_after_multiple_pending_migrations(reversible: bool, mode: Mode, run: bool) {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = &migration_dir.path().join("migrations-tests");
        let mut conf = TestConfig::new(mode, migration_dir, current_function!()).await;

        conf.generate_12_test_migrations_reversible(reversible)
            .await;
        conf.run_up(&FastForwardDelta::builder().number(7).build())
            .await;

        conf.assert_with_db_instance(AssertionArg {
            expected_mig_files_count: 12,
            expected_db_mig_meta_count: 7,
            expected_latest_migration_file_basename_normalized: Some(
                "migration_12_gen_after_init".into(),
            ),
            expected_latest_db_migration_meta_basename_normalized: Some(
                "migration_7_gen_after_init".into(),
            ),
            code_origin_line: std::line!(),
        })
        .await;

        conf.run_reset(
            Reset::builder()
                .name("migration reset".into())
                .reversible(reversible)
                .run(run)
                .build(),
            Resources,
            MockPrompter::default(),
        )
        .await;

        if run {
            conf.assert_with_db_instance(AssertionArg {
                expected_mig_files_count: 1,
                expected_db_mig_meta_count: 1,
                expected_latest_migration_file_basename_normalized: Some("migration_reset".into()),
                expected_latest_db_migration_meta_basename_normalized: Some(
                    "migration_reset".into(),
                ),
                code_origin_line: std::line!(),
            })
            .await;
        } else {
            // When we reset, and not apply against the current database instance, the old
            // stale latest migration metadataa would still exist in the database.
            let mig_files = conf.read_migrations_from_dir_sorted_asc();
            let latest_file = mig_files.last().map(MigrationFilename::basename);

            assert_eq!(history(&conf).await.len(), 7);
            assert_eq!(
                latest_applied(&conf).await,
                Some("migration_7_gen_after_init".into())
            );

            assert_eq!(mig_files.len(), if reversible { 2 } else { 1 });
            assert_eq!(latest_file, Some("migration_reset".into()));
        }
        conf.assert_migration_queries_snapshot();
    }

    #[tokio::test]
    async fn a_failed_reset_keeps_the_existing_migrations() {
        // The reset's migration is generated before anything is deleted: one that fails, here
        // for a redacted secret, leaves the existing files in place.
        let (_temp, dir) = migration_dir();
        generate(
            &dir,
            MigrationFlag::TwoWay,
            Resources,
            MockPrompter::default(),
        )
        .await
        .expect("the first migration generates");
        let files = migration_files(&dir);

        let reset = tokio::spawn({
            let dir = dir.clone();
            async move {
                let mut cli = Migrator::builder()
                    .verbose(3)
                    .dir(dir)
                    .db_connection(DatabaseConnection::default())
                    .mode(Mode::Strict)
                    .build();
                Reset::builder()
                    .name("reset".into())
                    .reversible(true)
                    .run(false)
                    .build()
                    .run(&mut cli, UserResources, MockPrompter::default())
                    .await;
            }
        })
        .await;
        let panic = reset.expect_err("the reset panics").into_panic();
        let message = panic
            .downcast_ref::<String>()
            .expect("a formatted panic message");
        assert!(
            message.starts_with("Failed to generate migrations"),
            "{message}"
        );
        assert_eq!(migration_files(&dir), files);
    }
}

mod embedded {
    use super::*;
    use pretty_assertions::assert_eq;
    use umbriel::migrations::{
        self, EmbeddedMigrationsTwoWay, FileMetadataStatic, embed_migrations,
    };

    const MIGRATIONS_ONE_WAY: migrations::EmbeddedMigrationsOneWay =
        embed_migrations!("tests/migrations-oneway");

    const MIGRATIONS_TWO_WAY: migrations::EmbeddedMigrationsTwoWay =
        embed_migrations!("tests/migrations-twoway");

    const MIGRATOR_ONE_WAY: migrations::EmbeddedMigrationsOneWay =
        embed_migrations!("tests/migrator-oneway");

    #[test]
    fn test_embedded() {
        assert_eq!(MIGRATIONS_ONE_WAY.files().len(), 2);
        // One up/down pair.
        assert_eq!(MIGRATIONS_TWO_WAY.files().len(), 2);
        assert_eq!(MIGRATOR_ONE_WAY.files().len(), 2);

        let migs = MIGRATIONS_ONE_WAY.to_migrations_one_way().unwrap();
        assert_eq!(migs.len(), 2);
        // check the meta data
        assert_eq!(
            migs[0].name().to_string(),
            "20231029202315_create_new_stuff.surql"
        );

        assert_eq!(
            migs[0].name().basename(),
            "create_new_stuff".to_string().into()
        );
        insta::assert_snapshot!(migs[0].content());
        assert_eq!(
            migs[1].name().to_string(),
            "20231029224601_create_another.surql"
        );
        assert_eq!(migs[1].name().basename(), "create_another".into());
        assert_eq!(migs.len(), 2);
        insta::assert_snapshot!(migs[1].content());

        let migs = MIGRATIONS_TWO_WAY.to_migrations_two_way().unwrap();
        assert_eq!(migs.len(), 1);

        // check the meta data
        assert_eq!(
            migs[0].up.name.to_string(),
            "20231030025711_migration_name_example.up.surql"
        );
        assert_eq!(migs[0].up.name.basename(), "migration_name_example".into());
        insta::assert_snapshot!(migs[0].up.content);
        insta::assert_snapshot!(migs[0].down.content);
    }

    #[tokio::test]
    async fn test_embedded_run_one_way() {
        let db = database().await;

        MIGRATIONS_ONE_WAY
            .run(db.clone(), UpdateStrategy::Latest, Mode::Strict)
            .await
            .unwrap();

        let db_migrations_meta = Migration::get_all_desc(db.clone()).await.unwrap();
        insta::assert_debug_snapshot!(db_migrations_meta);
        assert_eq!(db_migrations_meta.len(), 2);
    }

    #[tokio::test]
    async fn test_embedded_run_two_way() {
        let db = database().await;

        MIGRATIONS_TWO_WAY
            .run(db.clone(), UpdateStrategy::Latest, Mode::Strict)
            .await
            .unwrap();

        let db_migrations_meta = Migration::get_all_desc(db.clone()).await.unwrap();
        insta::assert_debug_snapshot!(db_migrations_meta);
        assert_eq!(db_migrations_meta.len(), 1);
    }

    const PAIR_UP: FileMetadataStatic = FileMetadataStatic {
        name: "20231030025711_create_user.up.surql",
        content: "DEFINE TABLE user;",
    };
    const PAIR_DOWN: FileMetadataStatic = FileMetadataStatic {
        name: "20231030025711_create_user.down.surql",
        content: "REMOVE TABLE user;",
    };
    const ORPHAN_DOWN: FileMetadataStatic = FileMetadataStatic {
        name: "20231031000000_create_company.down.surql",
        content: "REMOVE TABLE company;",
    };

    fn assert_missing_up(migrations: EmbeddedMigrationsTwoWay) {
        match migrations.to_migrations_two_way() {
            Err(MigrationError::MigrationFileDoesNotExist(name)) => {
                assert_eq!(name, "20231031000000_create_company.up.surql");
            }
            other => panic!("expected MigrationFileDoesNotExist, got {other:?}"),
        }
    }

    #[test]
    fn test_two_way_orphan_down_beside_pair() {
        assert_missing_up(EmbeddedMigrationsTwoWay::new(&[
            PAIR_DOWN,
            PAIR_UP,
            ORPHAN_DOWN,
        ]));
    }

    #[test]
    fn test_two_way_only_orphan_down() {
        assert_missing_up(EmbeddedMigrationsTwoWay::new(&[ORPHAN_DOWN]));
    }
}

/// The CLI's commands parsed from arguments, as a migrator binary receives them.
mod cli {
    use super::*;
    use clap::Parser;
    use pretty_assertions::assert_eq;
    use umbriel::migrations::Migrator;

    fn migrator(args: &[&str]) -> Migrator {
        Migrator::try_parse_from(std::iter::once("migrator").chain(args.iter().copied()))
            .expect("the arguments parse")
    }

    #[tokio::test]
    async fn test_init_and_up_commands() {
        let migration_dir = tempdir().expect("Failed to create temp directory");
        let migration_dir = migration_dir.path().join("migrations-tests");
        let dir = migration_dir.to_str().expect("a UTF-8 path");

        let mut init = migrator(&[
            "init",
            "--name",
            "test migration 1",
            "--dir",
            dir,
            "--reversible",
            "--run",
            "--url",
            "memory",
        ]);
        init.run_test(Some(Resources), MockPrompter::default())
            .await;
        assert_eq!(
            Migration::get_all_desc(init.db())
                .await
                .expect("the history is read")
                .len(),
            1
        );

        // Another run is another in-memory database, which `up` brings to the latest migration.
        let mut up = migrator(&[
            "up", "--dir", dir, "--db", "test", "--ns", "test", "--user", "root", "--pass", "root",
            "--url", "memory",
        ]);
        up.run_test(None::<Resources>, MockPrompter::default())
            .await;
        assert_eq!(
            Migration::get_all_desc(up.db())
                .await
                .expect("the history is read")
                .len(),
            1
        );
    }
}
