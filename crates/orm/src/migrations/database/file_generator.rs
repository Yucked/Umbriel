use std::collections::BTreeSet;

use surrealdb::{Connection, Surreal};

use crate::{
    DbResources, Raw, Runnable,
    statements::{define_database, remove_database},
};

use crate::migrations::{resources::SchemaDiff, *};

/// The prefix of every comparison database's name, so the leftovers of an interrupted generation
/// can be found.
pub const COMPARISON_PREFIX: &str = "umbriel_cmp_";

/// Where migrations are generated: a connection, and a namespace set aside for the generator.
///
/// Each generation creates two databases in the namespace, `umbriel_cmp_<uuid>_left` and
/// `umbriel_cmp_<uuid>_right`. It replays the migration directory into the first and applies the
/// codebase's definitions to the second, then removes both by name, whether it succeeded or
/// failed. It reads nothing else in the namespace except `INFO FOR NS`, to reject namespace-level
/// definitions the comparison added. An interrupted generation leaves its two databases behind,
/// and nothing removes them automatically.
///
/// The connection must be signed in as a user that may define and remove databases in the
/// namespace: a namespace owner or editor, or a root user. It is cloned for each database, and a
/// clone's session is its own, so the connection's own selection never changes. The server must
/// allow every resource kind the codebase defines, such as the experimental `files` and
/// `surrealism` features for buckets and modules. Waits are bounded only by the connection's own
/// configuration.
#[derive(Debug, Clone)]
pub struct Comparison<C: Connection> {
    db: Surreal<C>,
    namespace: String,
}

impl<C: Connection> Comparison<C> {
    /// Compares in `namespace` on `db`.
    pub fn new(db: Surreal<C>, namespace: impl Into<String>) -> Self {
        Self {
            db,
            namespace: namespace.into(),
        }
    }

    /// The namespace the comparison databases are created in.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

/// The two databases a migration is generated from: `left` replays the migration files, `right`
/// holds the codebase's schema.
#[derive(Debug, Clone)]
pub struct ComparisonDatabase<C: Connection> {
    left: LeftDatabase<C>,
    right: RightDatabase<C>,
    /// The namespace-level definitions before the comparison ran.
    namespace_before: BTreeSet<(String, String)>,
}

impl<C: Connection> ComparisonDatabase<C> {
    /// Creates both databases in `comparison`'s namespace. When the second cannot be created, the
    /// first is removed.
    ///
    /// # Errors
    ///
    /// The database's error when a database cannot be created, or
    /// [`MigrationError::ComparisonNotRemoved`] when one was created but cannot be removed again.
    pub async fn init(comparison: &Comparison<C>) -> MigrationResult<Self> {
        let run = uuid::Uuid::new_v4().simple();
        let left =
            MigratorDatabase::create(comparison, format!("{COMPARISON_PREFIX}{run}_left")).await?;
        let created = async {
            let right =
                MigratorDatabase::create(comparison, format!("{COMPARISON_PREFIX}{run}_right"))
                    .await?;
            Ok((right, Schema::namespace_definitions(left.db()).await?))
        }
        .await;
        match created {
            Ok((right, namespace_before)) => Ok(Self {
                left: LeftDatabase(left),
                right: RightDatabase(right),
                namespace_before,
            }),
            Err(error) => {
                let namespace = left.namespace.clone();
                let name = left.name.clone();
                Err(match left.remove().await {
                    Ok(()) => error,
                    Err(cleanup) => MigrationError::ComparisonNotRemoved {
                        namespace,
                        databases: vec![name],
                        cleanup: Box::new(cleanup),
                        generation: Some(Box::new(error)),
                    },
                })
            }
        }
    }

    /// Removes both databases, by name, and returns `outcome` unless a removal failed. When one
    /// did, the error names the databases left behind and keeps `outcome`'s own failure as its
    /// cause.
    async fn finish<T>(self, outcome: MigrationResult<T>) -> MigrationResult<T> {
        let namespace = self.left.namespace.clone();
        let mut left_behind = Vec::new();
        let mut cleanup = None;
        for database in [self.left.0, self.right.0] {
            let name = database.name.clone();
            if let Err(error) = database.remove().await {
                left_behind.push(name);
                cleanup.get_or_insert(error);
            }
        }
        let Some(cleanup) = cleanup else {
            return outcome;
        };
        Err(MigrationError::ComparisonNotRemoved {
            namespace,
            databases: left_behind,
            cleanup: Box::new(cleanup),
            generation: outcome.err().map(Box::new),
        })
    }

    /// The schema the migration directory produces, or an empty one unless `replay_directory`,
    /// and the codebase's. Namespace-level definitions the comparison added, which migrations do
    /// not manage, are rejected.
    async fn schemas(
        &self,
        file_manager: &MigrationConfig,
        codebase_resources: &impl DbResources,
        replay_directory: bool,
    ) -> MigrationResult<(Schema, Schema)> {
        let migration_flag = file_manager.migration_flag_checked()?;
        if replay_directory {
            match migration_flag {
                MigrationFlag::TwoWay => {
                    self.left
                        .run_twoway_up_migrations(file_manager, true)
                        .await?;
                }
                MigrationFlag::OneWay => {
                    self.left.run_oneway_migrations(file_manager, true).await?;
                }
            }
        }
        self.right
            .run_codebase_schema_queries(codebase_resources, migration_flag)
            .await?;
        let namespace_after = Schema::namespace_definitions(self.left.db()).await?;
        Schema::reject_added_namespace_definitions(&self.namespace_before, &namespace_after)?;
        Ok((self.left.schema().await?, self.right.schema().await?))
    }
}

/// A comparison database the generator created.
#[derive(Debug, Clone)]
pub struct MigratorDatabase<C: Connection> {
    db: Surreal<C>,
    namespace: String,
    name: String,
}

impl<C: Connection> MigratorDatabase<C> {
    /// Creates the database `name` in `comparison`'s namespace, on a clone of its connection
    /// that selects it. Without `IF NOT EXISTS` or `OVERWRITE`, `DEFINE DATABASE` refuses a name
    /// that exists, so the generator never takes over a database it didn't create.
    async fn create(comparison: &Comparison<C>, name: String) -> MigrationResult<Self> {
        let db = comparison.db.clone();
        db.use_ns(comparison.namespace.as_str()).await?;
        define_database(name.as_str()).run(&db).await?;
        let database = Self {
            db,
            namespace: comparison.namespace.clone(),
            name,
        };
        if let Err(error) = database.db.use_db(database.name.as_str()).await {
            let (namespace, name) = (database.namespace.clone(), database.name.clone());
            return Err(match database.remove().await {
                Ok(()) => error.into(),
                Err(cleanup) => MigrationError::ComparisonNotRemoved {
                    namespace,
                    databases: vec![name],
                    cleanup: Box::new(cleanup),
                    generation: Some(Box::new(error.into())),
                },
            });
        }
        Ok(database)
    }

    /// Removes the database, by name.
    async fn remove(self) -> MigrationResult<()> {
        remove_database(self.name.as_str()).run(&self.db).await?;
        Ok(())
    }

    /// The connection to the database.
    pub fn db(&self) -> &Surreal<C> {
        &self.db
    }

    /// The database's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The schema the database holds.
    pub async fn schema(&self) -> MigrationResult<Schema> {
        Schema::inspect(&self.db).await
    }

    /// Runs `query` against the database.
    pub async fn execute(&self, query: String) -> MigrationResult<()> {
        log::info!("Executing query: {query}");
        Raw::new(query).run(self.db()).await?;
        Ok(())
    }

    /// Writes a migration holding the difference between the schema the migration files
    /// produce and the codebase's schema, compared in `comparison`'s namespace.
    ///
    /// # Errors
    ///
    /// Any error of creating the comparison databases, replaying the directory, applying the
    /// codebase's definitions, planning or writing the migration, or
    /// [`MigrationError::ComparisonNotRemoved`].
    pub async fn generate_migrations(
        comparison: &Comparison<C>,
        migration_basename: &Basename,
        file_manager: &MigrationConfig,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<()> {
        let (left, right) =
            Self::schemas(comparison, file_manager, &codebase_resources, true).await?;
        generate_from_schemas(
            &left,
            &right,
            migration_basename,
            file_manager,
            &codebase_resources,
            &prompter,
        )
    }

    /// The first migration for the codebase's schema, as a reset writes it into an emptied
    /// migration directory, compared in `comparison`'s namespace. Nothing is written, so a reset
    /// can validate its migration before it deletes the existing ones.
    ///
    /// # Errors
    ///
    /// As [`generate_migrations`](Self::generate_migrations), except writing.
    pub async fn first_migration(
        comparison: &Comparison<C>,
        migration_basename: &Basename,
        file_manager: &MigrationConfig,
        codebase_resources: impl DbResources,
        prompter: impl Prompter,
    ) -> MigrationResult<MigrationFile> {
        let (left, right) =
            Self::schemas(comparison, file_manager, &codebase_resources, false).await?;
        let (migration, _) = plan_migration(
            &left,
            &right,
            migration_basename,
            file_manager,
            true,
            &codebase_resources,
            &prompter,
        )?;
        Ok(migration)
    }

    /// The schemas compared in a pair of comparison databases, which are removed afterwards
    /// whether the comparison succeeded or failed.
    async fn schemas(
        comparison: &Comparison<C>,
        file_manager: &MigrationConfig,
        codebase_resources: &impl DbResources,
        replay_directory: bool,
    ) -> MigrationResult<(Schema, Schema)> {
        let databases = ComparisonDatabase::init(comparison).await?;
        let schemas = databases
            .schemas(file_manager, codebase_resources, replay_directory)
            .await;
        databases.finish(schemas).await
    }
}

/// Writes the migration from `left` (the schema the migration files produce) to `right` (the
/// codebase's), as [`MigratorDatabase::generate_migrations`] does once it has inspected both: a
/// migration without changes is written only if `prompter` allows it. A statement that cannot be
/// planned fails before anything is written.
///
/// # Errors
///
/// The errors of planning the statements, such as [`MigrationError::InvalidStatement`], and of
/// writing the file.
#[doc(hidden)]
pub fn generate_from_schemas(
    left: &Schema,
    right: &Schema,
    migration_basename: &Basename,
    file_manager: &MigrationConfig,
    codebase_resources: &impl DbResources,
    prompter: &impl Prompter,
) -> MigrationResult<()> {
    let first_migration = file_manager.is_first_migration()?;
    let (migration, changes) = plan_migration(
        left,
        right,
        migration_basename,
        file_manager,
        first_migration,
        codebase_resources,
        prompter,
    )?;
    if changes || prompter.prompt_empty_migrations_trigger()? {
        migration.create_file(file_manager)?;
        log::info!("New migration generated.");
    } else {
        log::info!("No migration created");
    }
    Ok(())
}

/// The migration from `left` to `right`, and whether it changes anything. A first migration
/// always does: it resets the history. Nothing is written.
fn plan_migration(
    left: &Schema,
    right: &Schema,
    migration_basename: &Basename,
    file_manager: &MigrationConfig,
    first_migration: bool,
    codebase_resources: &impl DbResources,
    prompter: &impl Prompter,
) -> MigrationResult<(MigrationFile, bool)> {
    let migration_basename = migration_basename.normalize_ensure();
    let migration_flag = file_manager.migration_flag_checked()?;

    let diff = SchemaDiff::new(left, right, codebase_resources, prompter)?;
    let mut up = diff.statements(&Direction::Up)?;
    let down = if migration_flag.is_twoway() {
        diff.statements(&Direction::Down)?
    } else {
        Vec::new()
    };

    if first_migration {
        // A reset regenerates the first migration against a database that may still hold the
        // old history. Cleared after the definitions, which create the history table.
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
