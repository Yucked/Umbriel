use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::DbResources;
use surrealdb::{Connection, Surreal, engine::any::Any};
use typed_builder::TypedBuilder;

use crate::migrations::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
struct FileManager {}

impl FileManager {}

/// Where the migration files are and how they are checked.
#[derive(Debug, Clone, Default, TypedBuilder)]
pub struct MigrationConfig {
    /// How closely migration files are checked against the history.
    pub mode: Mode,
    /// The migration directory relative to `CARGO_MANIFEST_DIR`; `migrations` by default.
    pub custom_path: Option<PathBuf>,
    /// Whether migrations are one-way or two-way. `None` until known: the directory cannot tell
    /// before it is initialized.
    #[builder(default)]
    pub migration_flag: Option<MigrationFlag>,
}

impl MigrationConfig {
    /// Strict mode, the default directory and no migration type.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the migration type.
    pub fn set_flag(mut self, flag: MigrationFlag) -> Self {
        self.migration_flag = Some(flag);
        self
    }

    /// The migration type, or an error when it is not set.
    pub fn migration_flag_checked(&self) -> MigrationResult<MigrationFlag> {
        self.migration_flag
            .ok_or(MigrationError::MigrationFlagNotSet)
    }

    /// Sets the mode.
    pub fn set_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// The mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Switches to strict mode.
    pub fn make_strict(mut self) -> Self {
        self.mode = Mode::Strict;
        self
    }

    /// Switches to lax mode.
    pub fn relax(mut self) -> Self {
        self.mode = Mode::Lax;
        self
    }

    /// Sets the migration directory, relative to `CARGO_MANIFEST_DIR`.
    pub fn set_custom_path(mut self, custom_path: impl Into<PathBuf>) -> Self {
        let custom_path = custom_path.into();
        self.custom_path = Some(custom_path);
        self
    }

    /// Manages one-way migrations with this configuration.
    pub fn one_way(&self) -> FileManagerUni {
        let mut config = self.clone();
        config.migration_flag = Some(MigrationFlag::OneWay);

        FileManagerUni::new(config)
    }

    /// Manages two-way migrations with this configuration.
    pub fn two_way(&self) -> FileManagerBi {
        let mut config = self.clone();
        config.migration_flag = Some(MigrationFlag::TwoWay);

        FileManagerBi::new(config)
    }

    /// The migration directory, created if missing.
    pub fn get_migration_dir_create_if_none(&self) -> MigrationResult<PathBuf> {
        self.resolve_migration_directory(true)
    }

    /// The migration type from the directory's files; an error unless all are of one type.
    pub fn detect_migration_type(&self) -> MigrationResult<MigrationFlag> {
        let filenames = self.get_migrations_filenames(false)?;
        let oneway = filenames.unidirectional();
        let twoway = filenames.bidirectional();

        match (oneway.is_empty(), twoway.is_empty()) {
            (false, true) => Ok(MigrationFlag::OneWay),
            (true, false) => Ok(MigrationFlag::TwoWay),
            (false, false) | (true, true) => Err(MigrationError::AmbiguousMigrationDirection {
                one_way_filecount: oneway.len(),
                two_way_filecount: twoway.len(),
            }),
        }
    }

    /// Whether the migration directory has no migrations yet.
    pub fn is_first_migration(&self) -> MigrationResult<bool> {
        Ok(self.get_migrations_filenames(false)?.all().is_empty())
    }

    /// The migration directory, which must exist.
    pub fn get_migration_dir(&self) -> MigrationResult<PathBuf> {
        self.resolve_migration_directory(false)
    }

    pub(crate) fn resolve_migration_directory(
        &self,
        create_dir_if_not_exists: bool,
    ) -> MigrationResult<PathBuf> {
        let cargo_toml_directory =
            env::var("CARGO_MANIFEST_DIR").map_err(|_| MigrationError::PathDoesNotExist)?;
        let cargo_manifests_dir = Path::new(&cargo_toml_directory);
        let default_path = cargo_manifests_dir.join("migrations");
        let path = self
            .custom_path
            .as_ref()
            .map_or(default_path, |fp| cargo_manifests_dir.join(Path::new(&fp)));

        if path.exists() && path.is_dir() {
            Ok(path)
        } else {
            if create_dir_if_not_exists {
                fs::create_dir(&path).map_err(|e| MigrationError::IoError(e.to_string()))?;
                return Ok(path);
            }
            Err(MigrationError::MigrationDirectoryDoesNotExist(
                path.to_string_lossy().to_string(),
            ))
        }
    }

    /// The migration directory's file names, sorted.
    pub fn get_migrations_filenames(
        &self,
        create_dir_if_not_exists: bool,
    ) -> MigrationResult<MigrationFilenames> {
        let migration_dir_path = self.resolve_migration_directory(create_dir_if_not_exists)?;
        let migration_dir_path_str = migration_dir_path.to_string_lossy().to_string();

        log::info!("Migration dir path: {migration_dir_path_str}");

        let migrations = fs::read_dir(&migration_dir_path).map_err(|e| {
            MigrationError::IoError(format!(
                "Failed to read migration directory: {migration_dir_path_str}. Error: {e}"
            ))
        })?;

        log::info!("Migration dir path: {migration_dir_path_str}");

        let mut filenames = vec![];

        for migration in migrations {
            let migration = migration.map_err(|e| {
                MigrationError::IoError(format!(
                    "Failed to read migration directory: {migration_dir_path_str}. Error: {e}"
                ))
            })?;
            let filename: MigrationFilename = migration
                .file_name()
                .to_string_lossy()
                .into_owned()
                .try_into()?;

            filenames.push(filename);
        }

        filenames.sort();
        Ok(filenames.into())
    }

    /// The two-way migrations, oldest first.
    pub fn get_two_way_migrations_sorted_asc(
        &self,
        create_dir_if_not_exists: bool,
    ) -> MigrationResult<Vec<MigrationFileTwoWayPair>> {
        Ok(self
            .get_two_way_migrations_sorted_desc(create_dir_if_not_exists)?
            .into_iter()
            .rev()
            .collect::<Vec<_>>())
    }

    /// The two-way migrations, newest first.
    pub fn get_two_way_migrations_sorted_desc(
        &self,
        create_dir_if_not_exists: bool,
    ) -> MigrationResult<Vec<MigrationFileTwoWayPair>> {
        self.get_migrations_filenames(create_dir_if_not_exists)?
            .bidirectional_pair_meta_sorted_desc_checked(&self.resolve_migration_directory(false)?)
    }

    /// The one-way migrations, oldest first.
    pub fn get_oneway_migrations_sorted_asc(
        &self,
        create_dir_if_not_exists: bool,
    ) -> MigrationResult<Vec<MigrationFileOneWay>> {
        self.get_migrations_filenames(create_dir_if_not_exists)?
            .unidirectional_pair_meta(&self.resolve_migration_directory(false)?)
    }
}

/// Generates, applies and lists one-way migrations.
#[derive(Debug, Clone)]
pub struct FileManagerUni(MigrationConfig);

impl std::ops::Deref for FileManagerUni {
    type Target = MigrationConfig;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FileManagerUni {
    pub(crate) fn new(file_manager: MigrationConfig) -> Self {
        Self(file_manager)
    }

    /// The migrations, oldest first.
    pub fn get_migrations(&self) -> MigrationResult<Vec<MigrationFileOneWay>> {
        self.0.get_oneway_migrations_sorted_asc(false)
    }

    /// Generate migration directory if it does not exist
    pub async fn generate_migrations(
        &self,
        migration_basename: &Basename,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        MigratorDatabase::generate_migrations(
            migration_basename,
            self,
            codebase_resources,
            prompter,
        )
        .await?;
        Ok(())
    }

    /// Applies the directory's pending migrations to `db`, each in its own transaction (see
    /// [`MigrationRunner`]). The migration directory must exist.
    pub async fn run_pending_migrations(
        &self,
        db: &Surreal<impl Connection>,
        update_strategy: UpdateStrategy,
    ) -> MigrationResult<()> {
        let migrations = self.get_migrations()?;
        MigrationRunner::apply_pending_migrations(db, migrations, update_strategy).await?;

        Ok(())
    }

    /// Applies pending embedded migrations, each in its own transaction (see
    /// [`MigrationRunner`]).
    pub async fn run_embedded_pending_migrations(
        &self,
        db: &Surreal<impl Connection>,
        one_way_embedded_migrations: EmbeddedMigrationsOneWay,
        update_strategy: UpdateStrategy,
    ) -> MigrationResult<()> {
        let migrations = one_way_embedded_migrations.to_migrations_one_way()?;
        MigrationRunner::apply_pending_migrations(db, migrations, update_strategy).await?;

        Ok(())
    }

    /// List all migrations
    pub async fn list_migrations(
        &self,
        db: &Surreal<impl Connection>,
        status: Status,
        mode: Mode,
    ) -> MigrationResult<Vec<MigrationFilename>> {
        let migrations = self
            .get_migrations()?
            .into_iter()
            .map(|m| m.name().to_owned())
            .collect::<Vec<_>>();

        let migrations = MigrationRunner::list_migrations(db, migrations, status, mode).await?;

        Ok(migrations)
    }
}

/// Generates, applies, rolls back and lists two-way migrations.
#[derive(Debug, Clone)]
pub struct FileManagerBi(MigrationConfig);

impl std::ops::Deref for FileManagerBi {
    type Target = MigrationConfig;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FileManagerBi {
    pub(crate) fn new(file_manager: MigrationConfig) -> Self {
        Self(file_manager)
    }

    /// Get all migrations
    pub fn get_migrations(&self) -> MigrationResult<Vec<MigrationFileTwoWayPair>> {
        self.0
            .get_migrations_filenames(false)?
            .bidirectional_pair_meta_sorted_desc_checked(
                &self.0.resolve_migration_directory(false)?,
            )
    }

    /// Generate migration directory if it does not exist
    pub async fn generate_migrations(
        &self,
        migration_basename: &Basename,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        let file_manager = self.0.clone();
        MigratorDatabase::generate_migrations(
            migration_basename,
            &file_manager,
            codebase_resources,
            prompter,
        )
        .await
    }

    /// Applies the directory's pending migrations to `db`, each in its own transaction (see
    /// [`MigrationRunner`]). The migration directory must exist.
    pub async fn run_up_pending_migrations(
        &self,
        db: &Surreal<impl Connection>,
        update_strategy: UpdateStrategy,
    ) -> MigrationResult<()> {
        let migrations = self.get_migrations()?;
        MigrationRunner::apply_pending_migrations(db, migrations, update_strategy).await?;

        Ok(())
    }

    /// Applies pending embedded migrations, each in its own transaction (see
    /// [`MigrationRunner`]).
    pub async fn run_up_embedded_pending_migrations(
        &self,
        db: &Surreal<impl Connection>,
        two_way_embedded_migrations: EmbeddedMigrationsTwoWay,
        update_strategy: UpdateStrategy,
    ) -> MigrationResult<()> {
        let migrations = two_way_embedded_migrations.to_migrations_two_way()?;
        MigrationRunner::apply_pending_migrations(db, migrations, update_strategy).await?;

        Ok(())
    }

    /// Rolls back migrations as `rollback_options` says, each in its own transaction (see
    /// [`MigrationRunner`]).
    pub async fn run_down_migrations(
        &self,
        db: &Surreal<Any>,
        rollback_options: RollbackOptions,
    ) -> MigrationResult<()> {
        MigrationRunner::rollback_migrations(db, self, rollback_options).await?;

        Ok(())
    }

    /// List all migrations
    pub async fn list_migrations(
        &self,
        db: &Surreal<impl Connection>,
        status: Status,
        mode: Mode,
    ) -> MigrationResult<Vec<MigrationFilename>> {
        let migrations = self
            .get_migrations()?
            .into_iter()
            .map(|m| m.up.name)
            .collect::<Vec<_>>();

        let migrations = MigrationRunner::list_migrations(db, migrations, status, mode).await?;

        Ok(migrations)
    }
}
