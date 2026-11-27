//! What the migration test files share: a migration directory, generating into it in a
//! comparison namespace of its own, the schema a codebase defines, applying and rolling back, and
//! reading the files and the history.

use std::{
    fs,
    path::{Path, PathBuf},
};

use surrealdb::{Surreal, engine::any::Any};
use tempfile::{TempDir, tempdir};
use umbriel::{
    DbResources, Render, Runnable,
    migrations::{
        Basename, DatabaseConnection, Migration, MigrationConfig, MigrationFilename, MigrationFlag,
        MigrationResult, Migrator, MockPrompter, Mode, RenameOrDelete, ResourceKey, ResourceKind,
        RollbackOptions, Schema, SubCommand, UpdateStrategy, codebase_schema_queries,
        config::UrlDb,
    },
    statements::begin_transaction,
};

use super::{
    Resources, ResourcesV2, ResourcesV3, ResourcesV4, ResourcesV5, ResourcesV6, ResourcesV7,
    ResourcesV8, ResourcesV9, ResourcesV10,
};
use crate::{Endpoint, TestDb, TestNamespace, comparison, db};

/// A temporary directory and the migration directory inside it, which does not exist yet.
pub fn migration_dir() -> (TempDir, PathBuf) {
    let temp = tempdir().expect("a temporary directory");
    let dir = temp.path().join("migrations");
    (temp, dir)
}

/// Strict mode and the migration directory `dir`.
pub fn config(dir: &Path) -> MigrationConfig {
    MigrationConfig::new()
        .set_mode(Mode::Strict)
        .set_custom_path(dir)
}

/// Generates a migration named `generated` from `resources` into `dir`, comparing in a
/// namespace of its own.
pub async fn generate(
    dir: &Path,
    flag: MigrationFlag,
    resources: impl DbResources,
    prompter: MockPrompter,
) -> MigrationResult<()> {
    let (comparison, _namespace) = comparison().await;
    let name = "generated".into();
    match flag {
        MigrationFlag::TwoWay => {
            config(dir)
                .two_way()
                .generate_migrations(&comparison, &name, resources, prompter)
                .await
        }
        MigrationFlag::OneWay => {
            config(dir)
                .one_way()
                .generate_migrations(&comparison, &name, resources, prompter)
                .await
        }
    }
}

/// A prompter that refuses migrations without changes and renames a replaced field.
pub fn disallow_empty() -> MockPrompter {
    MockPrompter::new(false, RenameOrDelete::Rename)
}

/// A prompter that allows migrations without changes and renames a replaced field.
pub fn allow_empty() -> MockPrompter {
    MockPrompter::new(true, RenameOrDelete::Rename)
}

/// The schema `resources` define with the history table of `flag`, which a database migrated to
/// them must hold: the generator's own definitions, applied to a database of its own.
pub async fn schema_of(resources: &impl DbResources, flag: MigrationFlag) -> Schema {
    let (db, _guard) = db().await;
    let mut transaction = begin_transaction().query(Migration::define_table());
    for definition in Migration::define_fields(flag)
        .into_iter()
        .chain(codebase_schema_queries(resources))
    {
        transaction = transaction.query(definition);
    }
    transaction
        .commit_transaction()
        .run(&db)
        .await
        .expect("the definitions apply");
    inspect(&db).await
}

pub async fn inspect(db: &Surreal<Any>) -> Schema {
    Schema::inspect(db).await.expect("the schema is inspected")
}

pub async fn apply_up(dir: &Path, db: &Surreal<Any>) {
    config(dir)
        .two_way()
        .run_up_pending_migrations(db, UpdateStrategy::Latest)
        .await
        .expect("the pending migrations apply");
}

pub async fn roll_back(dir: &Path, db: &Surreal<Any>) {
    config(dir)
        .two_way()
        .run_down_migrations(db, RollbackOptions::default())
        .await
        .expect("the latest migration rolls back");
}

/// The migration files of `dir`, in order. A missing directory has none.
pub fn migration_files(dir: &Path) -> Vec<String> {
    if !fs::exists(dir).expect("the directory can be checked") {
        return Vec::new();
    }
    let mut names = fs::read_dir(dir)
        .expect("the migration directory is readable")
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
pub fn newest_migration(dir: &Path) -> (String, String) {
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

/// Generates two-way migrations to `from` and then to `to`, applying each to a database of its
/// own: after each its schema must be the codebase's, and rolling the second back must restore
/// `from`'s. Returns the second migration's up and down files.
pub async fn round_trip(
    from: impl DbResources + Clone,
    to: impl DbResources + Clone,
) -> (String, String) {
    let (db, _guard) = db().await;
    round_trip_on(&db, from, to).await
}

/// [`round_trip`] on `db`.
pub async fn round_trip_on(
    db: &Surreal<Any>,
    from: impl DbResources + Clone,
    to: impl DbResources + Clone,
) -> (String, String) {
    let (_temp, dir) = migration_dir();
    let flag = MigrationFlag::TwoWay;

    generate(&dir, flag, from.clone(), MockPrompter::default())
        .await
        .expect("the first migration generates");
    apply_up(&dir, db).await;
    assert_eq!(inspect(db).await, schema_of(&from, flag).await);

    generate(&dir, flag, to.clone(), MockPrompter::default())
        .await
        .expect("the second migration generates");
    apply_up(&dir, db).await;
    assert_eq!(inspect(db).await, schema_of(&to, flag).await);

    let files = newest_migration(&dir);
    roll_back(&dir, db).await;
    assert_eq!(inspect(db).await, schema_of(&from, flag).await);
    files
}

/// Asserts that each of `statements` appears in `sql`, in this order.
#[track_caller]
pub fn assert_order(sql: &str, statements: &[&str]) {
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
pub fn history_table(flag: MigrationFlag) -> String {
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
pub fn write_migration(dir: &Path, flag: MigrationFlag, up: &str, down: &str) {
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
pub fn write_numbered(dir: &Path, number: u8, up: &str, down: &str) -> String {
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
pub async fn applied(db: &Surreal<Any>) -> Vec<String> {
    let mut names = Migration::get_all_desc(db)
        .await
        .expect("the history is read")
        .into_iter()
        .map(|migration| migration.name)
        .collect::<Vec<_>>();
    names.reverse();
    names
}

/// The tables of `db`, in name order.
pub async fn tables(db: &Surreal<Any>) -> Vec<String> {
    names(&inspect(db).await, ResourceKind::Table)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

pub fn key(kind: ResourceKind, table: Option<&str>, name: &str) -> ResourceKey {
    ResourceKey {
        kind,
        table: table.map(str::to_owned),
        name: name.to_owned(),
    }
}

/// The names of the definitions of `kind`, in order.
pub fn names(schema: &Schema, kind: ResourceKind) -> Vec<&str> {
    schema
        .iter()
        .filter(|(key, _)| key.kind == kind)
        .map(|(key, _)| key.name.as_str())
        .collect()
}

/// One CLI project: a migration directory, a database of its own and a comparison namespace,
/// driven through [`Migrator`] as a migrator binary is.
pub struct Project {
    pub flag: MigrationFlag,
    pub mode: Mode,
    pub dir: PathBuf,
    pub db: Surreal<Any>,
    _temp: TempDir,
    _db: TestDb,
    comparison: umbriel::migrations::Comparison<Any>,
    _comparison: TestNamespace,
}

impl Project {
    pub async fn new(flag: MigrationFlag, mode: Mode) -> Self {
        let (temp, dir) = migration_dir();
        let (db, db_guard) = db().await;
        let (comparison, comparison_guard) = comparison().await;
        Self {
            flag,
            mode,
            dir,
            db,
            _temp: temp,
            _db: db_guard,
            comparison,
            _comparison: comparison_guard,
        }
    }

    /// Whether the project's migrations are two-way.
    pub fn reversible(&self) -> bool {
        self.flag == MigrationFlag::TwoWay
    }

    /// Runs `command` as the CLI does, on the project's database and comparison.
    pub async fn run(
        &self,
        command: impl Into<SubCommand>,
        resources: impl DbResources,
        prompter: MockPrompter,
    ) {
        self.try_run(command, resources, prompter)
            .await
            .expect("the command succeeds");
    }

    /// Runs `command` as [`run`](Self::run) does and returns its outcome, for the commands a
    /// test expects to fail.
    pub async fn try_run(
        &self,
        command: impl Into<SubCommand>,
        resources: impl DbResources,
        prompter: MockPrompter,
    ) -> MigrationResult<()> {
        let connection = DatabaseConnection::connected(
            UrlDb::Others(Endpoint::Main.url()),
            self.db.clone(),
            self.comparison.clone(),
        );
        let mut migrator = Migrator::connected(connection, self.dir.clone(), self.mode);
        migrator.set_cmd(command.into());
        migrator.run_fn(resources, prompter).await
    }

    /// The migration files' base names, oldest first, one per migration (a two-way pair counts
    /// once).
    pub fn files(&self) -> Vec<Basename> {
        let files = migration_files(&self.dir);
        let migrations = files
            .iter()
            .filter(|name| !name.ends_with(".down.surql"))
            .map(|name| {
                MigrationFilename::try_from(name.clone())
                    .expect("a migration file name")
                    .basename()
            })
            .collect::<Vec<_>>();
        let downs = files.len() - migrations.len();
        let expected_downs = if self.reversible() {
            migrations.len()
        } else {
            0
        };
        assert_eq!(downs, expected_downs, "down files: {files:?}");
        migrations
    }

    /// The applied migrations' base names, oldest first.
    pub async fn history(&self) -> Vec<Basename> {
        applied(&self.db)
            .await
            .into_iter()
            .map(|name| {
                MigrationFilename::try_from(name)
                    .expect("a recorded migration name")
                    .basename()
            })
            .collect()
    }

    /// Asserts the migration files and the history, by base name, oldest first, and that every
    /// recorded migration names a file of the directory exactly (timestamp and extension).
    pub async fn assert_state(&self, files: &[&str], history: &[&str]) {
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|&name| Basename::from(name.to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(self.files(), names(files), "the migration files");
        assert_eq!(self.history().await, names(history), "the history");
        let on_disk = migration_files(&self.dir);
        for recorded in applied(&self.db).await {
            assert!(
                on_disk.contains(&recorded),
                "`{recorded}` is recorded but not a file of {on_disk:?}"
            );
        }
    }

    /// The project's schema.
    pub async fn schema(&self) -> Schema {
        inspect(&self.db).await
    }

    /// The file name `--till` names the `position`th migration by (from 1): the down file of a
    /// two-way pair, the file of a one-way migration.
    pub fn filename_at(&self, position: usize) -> MigrationFilename {
        let mut files = migration_files(&self.dir)
            .into_iter()
            .map(|name| MigrationFilename::try_from(name).expect("a migration file name"))
            .filter(|name| !self.reversible() || name.is_down())
            .collect::<Vec<_>>();
        files.sort();
        files
            .get(position.checked_sub(1).expect("positions start at 1"))
            .unwrap_or_else(|| panic!("there is no migration {position}"))
            .clone()
    }
}

/// The base names of the twelve migrations [`generate_twelve`] writes.
pub const TWELVE: [&str; 12] = [
    "migration_1_init",
    "migration_2_gen_after_init",
    "migration_3_gen_after_init",
    "migration_4_gen_after_init",
    "migration_5_gen_after_init",
    "migration_6_gen_after_init",
    "migration_7_gen_after_init",
    "migration_8_gen_after_init",
    "migration_9_gen_after_init",
    "migration_10_gen_after_init",
    "migration_11_gen_after_init",
    "migration_12_gen_after_init",
];

/// Initializes the project and generates eleven more migrations, none applied, moving through
/// the codebase versions [`schema_at`] names: `Resources` three times, then `ResourcesV2` to
/// `ResourcesV10`. Each changes the schema, or is empty where two versions define the same.
pub async fn generate_twelve(project: &Project) {
    use umbriel::migrations::{Generate, Init};

    let generate =
        |number: usize| Generate::new(format!("migration {number}-gen after init").into(), false);
    project
        .run(
            Init::new("migration 1-init".into(), project.reversible(), false),
            Resources,
            allow_empty(),
        )
        .await;
    project.run(generate(2), Resources, allow_empty()).await;
    project.run(generate(3), Resources, allow_empty()).await;
    project.run(generate(4), ResourcesV2, allow_empty()).await;
    project.run(generate(5), ResourcesV3, allow_empty()).await;
    project.run(generate(6), ResourcesV4, allow_empty()).await;
    project.run(generate(7), ResourcesV5, allow_empty()).await;
    project.run(generate(8), ResourcesV6, allow_empty()).await;
    project.run(generate(9), ResourcesV7, allow_empty()).await;
    project.run(generate(10), ResourcesV8, allow_empty()).await;
    project.run(generate(11), ResourcesV9, allow_empty()).await;
    project.run(generate(12), ResourcesV10, allow_empty()).await;
}

/// The schema after the first `applied` of [`generate_twelve`]'s migrations (1 to 12).
pub async fn schema_at(applied: usize, flag: MigrationFlag) -> Schema {
    match applied {
        1..=3 => schema_of(&Resources, flag).await,
        4 => schema_of(&ResourcesV2, flag).await,
        5 => schema_of(&ResourcesV3, flag).await,
        6 => schema_of(&ResourcesV4, flag).await,
        7 => schema_of(&ResourcesV5, flag).await,
        8 => schema_of(&ResourcesV6, flag).await,
        9 => schema_of(&ResourcesV7, flag).await,
        10 => schema_of(&ResourcesV8, flag).await,
        11 => schema_of(&ResourcesV9, flag).await,
        12 => schema_of(&ResourcesV10, flag).await,
        other => panic!("there are twelve migrations, not {other}"),
    }
}

/// The first `count` of [`TWELVE`].
pub fn first(count: usize) -> &'static [&'static str] {
    &TWELVE[..count]
}
