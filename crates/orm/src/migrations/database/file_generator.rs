use surrealdb::{
    Surreal,
    engine::local::{Db, Mem},
};

use crate::{DbResources, Raw, Runnable};

use crate::migrations::{resources::SchemaDiff, *};

/// The namespace and database of the generator's private in-memory databases.
const SCRATCH: &str = "migrations";

/// The two in-memory databases a migration is generated from: `left` replays the migration
/// files, `right` holds the codebase's schema.
#[derive(Debug, Clone)]
pub struct ComparisonDatabase {
    left: LeftDatabase,
    right: RightDatabase,
}

impl ComparisonDatabase {
    /// Creates both databases.
    pub async fn init() -> MigrationResult<Self> {
        Ok(Self {
            left: LeftDatabase(MigratorDatabase::init().await?),
            right: RightDatabase(MigratorDatabase::init().await?),
        })
    }
}

/// A private in-memory database of the generator.
#[derive(Debug, Clone)]
pub struct MigratorDatabase {
    /// The connection to the database.
    pub db: Surreal<Db>,
}

impl MigratorDatabase {
    /// Creates an empty in-memory database.
    pub async fn init() -> MigrationResult<Self> {
        let db = Surreal::new::<Mem>(()).await?;
        db.use_ns(SCRATCH).use_db(SCRATCH).await?;
        Ok(Self { db })
    }

    /// The connection to the database.
    pub fn db(&self) -> &Surreal<Db> {
        &self.db
    }

    /// The schema the database holds. Namespace-level definitions, which migrations do not
    /// manage, are rejected rather than left out.
    pub async fn schema(&self) -> MigrationResult<Schema> {
        Schema::reject_namespace_definitions(&self.db).await?;
        Schema::inspect(&self.db).await
    }

    /// Runs `query` against the database.
    pub async fn execute(&self, query: String) -> MigrationResult<()> {
        log::info!("Executing query: {query}");
        Raw::new(query).run(self.db()).await?;
        Ok(())
    }

    /// Writes a migration holding the difference between the schema the migration files
    /// produce and the codebase's schema.
    pub async fn generate_migrations(
        migration_basename: &Basename,
        file_manager: &MigrationConfig,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        let (migration, changes) = Self::plan_migration(
            migration_basename,
            file_manager,
            &codebase_resources,
            &prompter,
            true,
        )
        .await?;
        if changes || prompter.prompt_empty_migrations_trigger()? {
            migration.create_file(file_manager)?;
            log::info!("New migration generated.");
        } else {
            log::info!("No migration created");
        }
        Ok(())
    }

    /// The first migration for the codebase's schema, as a reset writes it into an emptied
    /// migration directory. Nothing is written, so a reset can validate its migration before it
    /// deletes the existing ones.
    pub async fn first_migration(
        migration_basename: &Basename,
        file_manager: &MigrationConfig,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<MigrationFile> {
        let (migration, _) = Self::plan_migration(
            migration_basename,
            file_manager,
            &codebase_resources,
            &prompter,
            false,
        )
        .await?;
        Ok(migration)
    }

    /// The migration from the schema the migration directory produces, or from an empty one
    /// unless `replay_directory`, to the codebase's, and whether it changes anything. A first
    /// migration always does: it resets the history. Nothing is written.
    async fn plan_migration(
        migration_basename: &Basename,
        file_manager: &MigrationConfig,
        codebase_resources: &impl DbResources,
        prompter: &impl Prompter,
        replay_directory: bool,
    ) -> MigrationResult<(MigrationFile, bool)> {
        let migration_basename = migration_basename.normalize_ensure();
        let migration_flag = file_manager.migration_flag_checked()?;

        let ComparisonDatabase { left, right } = ComparisonDatabase::init().await?;
        if replay_directory {
            match migration_flag {
                MigrationFlag::TwoWay => left.run_twoway_up_migrations(file_manager, true).await?,
                MigrationFlag::OneWay => {
                    left.run_oneway_migrations(file_manager, true).await?;
                }
            }
        }
        right
            .run_codebase_schema_queries(codebase_resources, migration_flag)
            .await?;
        let (left, right) = (left.schema().await?, right.schema().await?);

        let diff = SchemaDiff::new(&left, &right, codebase_resources, prompter)?;
        let mut up = diff.statements(&Direction::Up)?;
        let down = if migration_flag.is_twoway() {
            diff.statements(&Direction::Down)?
        } else {
            Vec::new()
        };

        let first_migration = !replay_directory || file_manager.is_first_migration()?;
        if first_migration {
            // A reset regenerates the first migration against a database that may still hold
            // the old history. Cleared after the definitions, which create the history table.
            if !up.is_empty() {
                up.push(QueryType::Blank);
            }
            up.push(QueryType::Comment("Reset the migration history".to_owned()));
            up.push(QueryType::Statement(Migration::delete_all().to_string()));
        }

        let migration_file = MigrationFile::new(
            &migration_basename,
            &migration_flag,
            &QueryType::join(&up).into(),
            &QueryType::join(&down).into(),
        )?;

        Ok((migration_file, first_migration || !diff.is_empty()))
    }
}
