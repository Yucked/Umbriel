use std::{future::Future, time::Duration};

use surrealdb::{Connection, Surreal, opt::auth};

use crate::{
    DbResources, Raw, Runnable,
    statements::{UserRole, begin_transaction, define_database, define_user, remove_database},
};

use super::db_runner::run_transaction;
use crate::migrations::{resources::SchemaDiff, *};

/// The prefix of every comparison database's name, and of the generator's user in it, so the
/// leftovers of an interrupted generation can be found.
pub const COMPARISON_PREFIX: &str = "umbriel_cmp_";

/// How long the generator waits for the server to confirm each comparison database's removal.
pub const COMPARISON_REMOVAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Where migrations are generated: a connection, and a namespace set aside for the generator.
///
/// Each generation creates two databases in the namespace, `umbriel_cmp_<run>_left` and
/// `umbriel_cmp_<run>_right`, and defines the database user `umbriel_cmp_<run>` in each, with the
/// `OWNER` role and a random password. It replays the migration directory into the first and
/// applies the codebase's definitions to the second, each signed in as that user, so SurrealDB
/// refuses any statement outside that database: a migration cannot add, change or remove a
/// namespace-level definition. The generator's user is left out of both schemas it compares.
/// Both databases, with the users in them, are removed by name on the given connection, whether
/// the generation succeeded or failed. An interrupted generation leaves its two databases behind,
/// and nothing removes them automatically.
///
/// The connection must be signed in as a user that may define and remove databases, and define
/// database users, in the namespace: a namespace owner or a root user. It is cloned for each
/// database, and a clone's session is its own, so the connection's own selection never changes.
/// The server must allow every resource kind the codebase defines, such as the experimental
/// `files` and `surrealism` features for buckets and modules.
///
/// Each removal waits at most [`COMPARISON_REMOVAL_TIMEOUT`]. Every other wait (creating the
/// databases and users, signing in, replaying, inspecting) is bounded only by the connection's own
/// configuration. Generation must run inside a Tokio runtime with timers enabled: the removal
/// bound panics without one.
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
    /// The database the migration directory is replayed into.
    left: MigratorDatabase<C>,
    /// The database holding the codebase's schema.
    right: MigratorDatabase<C>,
    /// The generator's user, defined in both databases.
    user: String,
}

/// The codebase's definitions in creation order (see [`ResourceKind`]), so each one's
/// prerequisites exist when it runs: a module may load from a bucket, an analyzer may use a
/// function, an index an analyzer, and an API a function or table.
pub fn codebase_schema_queries(db_resources: &impl DbResources) -> Vec<Raw> {
    let mut queries = db_resources.buckets();
    queries.extend(db_resources.modules());
    queries.extend(db_resources.functions());
    queries.extend(db_resources.analyzers());
    queries.extend(db_resources.params());
    queries.extend(db_resources.sequences());
    queries.extend(db_resources.accesses());
    queries.extend(db_resources.users());
    queries.extend(db_resources.tables());
    queries.extend(db_resources.apis());
    queries.extend(db_resources.configs());
    queries
}

impl<C: Connection> ComparisonDatabase<C> {
    /// Creates both databases in `comparison`'s namespace and signs a clone of the connection in
    /// to each as the generator's user (see [`Comparison`]). Whatever was created is removed when
    /// a later step fails.
    ///
    /// # Errors
    ///
    /// The database's error when a database or user cannot be defined or the sign-in fails, or
    /// [`MigrationError::ComparisonNotRemoved`] when a database was created but cannot be removed
    /// again.
    pub async fn init(comparison: &Comparison<C>) -> MigrationResult<Self> {
        init_comparison_with(comparison, Self::sign_in).await
    }

    /// Defines the generator's user in both databases, with a new random password, and replaces
    /// each database's connection with a clone signed in as it.
    async fn sign_in(&mut self) -> MigrationResult<()> {
        let password = uuid::Uuid::new_v4().simple().to_string();
        for database in [&mut self.left, &mut self.right] {
            database.sign_in(&self.user, &password).await?;
        }
        Ok(())
    }

    /// Removes both databases, by name, and returns `outcome` unless a removal failed (see
    /// [`remove_comparison`]).
    async fn finish<T>(self, outcome: MigrationResult<T>) -> MigrationResult<T> {
        let namespace = self.left.namespace.clone();
        remove_comparison(
            &namespace,
            [self.left.removal(), self.right.removal()],
            outcome,
        )
        .await
    }

    /// The schema the migration directory produces, or an empty one unless `replay_directory`,
    /// and the codebase's, both without the generator's user.
    async fn schemas(
        &self,
        file_manager: &MigrationConfig,
        codebase_resources: &impl DbResources,
        replay_directory: bool,
    ) -> MigrationResult<(Schema, Schema)> {
        let migration_flag = file_manager.migration_flag_checked()?;
        if replay_directory {
            // Each migration's up (or only) file in order, each in its own transaction.
            let contents = match migration_flag {
                MigrationFlag::TwoWay => file_manager
                    .get_two_way_migrations_sorted_asc(true)?
                    .into_iter()
                    .map(|pair| pair.up().content.to_string())
                    .collect::<Vec<_>>(),
                MigrationFlag::OneWay => file_manager
                    .get_oneway_migrations_sorted_asc(true)?
                    .into_iter()
                    .map(|file| file.content().to_string())
                    .collect(),
            };
            for content in contents {
                run_transaction(self.left.db(), content).await?;
            }
        }
        // The history table and the codebase's definitions, in one transaction.
        let mut transaction = begin_transaction().query(Migration::define_table());
        for definition in Migration::define_fields(migration_flag)
            .into_iter()
            .chain(codebase_schema_queries(codebase_resources))
        {
            transaction = transaction.query(definition);
        }
        transaction
            .commit_transaction()
            .run(self.right.db())
            .await?;
        // Its password hash differs between the databases, so it would compare as a changed
        // secret.
        let user = ResourceKey {
            kind: ResourceKind::User,
            table: None,
            name: self.user.clone(),
        };
        Ok((
            self.left.schema().await?.without(&user),
            self.right.schema().await?.without(&user),
        ))
    }
}

/// Creates both comparison databases in `comparison`'s namespace, then runs `prepare` on them.
/// When the second database cannot be created, or `prepare` fails, the databases created so far
/// are removed ([`remove_comparison`]). [`ComparisonDatabase::init`] prepares by signing in as the
/// generator's user.
///
/// # Errors
///
/// As [`ComparisonDatabase::init`], with `prepare`'s own error in place of the sign-in's.
#[doc(hidden)]
pub async fn init_comparison_with<C: Connection>(
    comparison: &Comparison<C>,
    prepare: impl AsyncFnOnce(&mut ComparisonDatabase<C>) -> MigrationResult<()>,
) -> MigrationResult<ComparisonDatabase<C>> {
    let run = uuid::Uuid::new_v4().simple();
    let left =
        MigratorDatabase::create(comparison, format!("{COMPARISON_PREFIX}{run}_left")).await?;
    let right =
        match MigratorDatabase::create(comparison, format!("{COMPARISON_PREFIX}{run}_right")).await
        {
            Ok(right) => right,
            Err(error) => {
                return remove_comparison(&comparison.namespace, [left.removal()], Err(error))
                    .await;
            }
        };
    let mut databases = ComparisonDatabase {
        left,
        right,
        user: format!("{COMPARISON_PREFIX}{run}"),
    };
    match prepare(&mut databases).await {
        Ok(()) => Ok(databases),
        Err(error) => databases.finish(Err(error)).await,
    }
}

/// Runs each removal of a comparison database in `namespace`, given with the database's name,
/// each bounded by [`COMPARISON_REMOVAL_TIMEOUT`], and returns `outcome` unless one failed. Every
/// removal is attempted, even after one failed or timed out.
///
/// # Errors
///
/// [`MigrationError::ComparisonNotRemoved`] when a removal failed: it names the databases left
/// behind, has the first removal's failure as its cause ([`MigrationError::ComparisonTimedOut`]
/// for a timeout), and keeps `outcome`'s own failure. Otherwise `outcome`'s.
#[doc(hidden)]
pub async fn remove_comparison<T>(
    namespace: &str,
    removals: impl IntoIterator<Item = (String, impl Future<Output = MigrationResult<()>>)>,
    outcome: MigrationResult<T>,
) -> MigrationResult<T> {
    let mut left_behind = Vec::new();
    let mut cleanup = None;
    for (database, removal) in removals {
        let removed = tokio::time::timeout(COMPARISON_REMOVAL_TIMEOUT, removal)
            .await
            .unwrap_or_else(|_| {
                Err(MigrationError::ComparisonTimedOut {
                    database: database.clone(),
                })
            });
        if let Err(error) = removed {
            left_behind.push(database);
            cleanup.get_or_insert(error);
        }
    }
    let Some(cleanup) = cleanup else {
        return outcome;
    };
    Err(MigrationError::ComparisonNotRemoved {
        namespace: namespace.to_owned(),
        databases: left_behind,
        cleanup: Box::new(cleanup),
        generation: outcome.err().map(Box::new),
    })
}

/// A comparison database the generator created.
#[derive(Debug, Clone)]
pub struct MigratorDatabase<C: Connection> {
    /// The connection statements run on: signed in as the generator's user once prepared.
    db: Surreal<C>,
    /// The caller's connection, selecting the database, which removes it.
    admin: Surreal<C>,
    namespace: String,
    name: String,
}

impl<C: Connection> MigratorDatabase<C> {
    /// Creates the database `name` in `comparison`'s namespace, on a clone of its connection
    /// that selects it. Without `IF NOT EXISTS` or `OVERWRITE`, `DEFINE DATABASE` refuses a name
    /// that exists, so the generator never takes over a database it didn't create.
    async fn create(comparison: &Comparison<C>, name: String) -> MigrationResult<Self> {
        let admin = comparison.db.clone();
        admin.use_ns(comparison.namespace.as_str()).await?;
        define_database(name.as_str()).run(&admin).await?;
        let selected = admin.use_db(name.as_str()).await;
        let database = Self {
            db: admin.clone(),
            admin,
            namespace: comparison.namespace.clone(),
            name,
        };
        if let Err(error) = selected {
            return remove_comparison(
                &comparison.namespace,
                [database.removal()],
                Err(error.into()),
            )
            .await;
        }
        Ok(database)
    }

    /// Defines `user` in the database, with the `OWNER` role, and replaces the database's
    /// connection with a clone of the caller's signed in as it. SurrealDB takes the password only
    /// as a string literal, so it is written into the definition.
    async fn sign_in(&mut self, user: &str, password: &str) -> MigrationResult<()> {
        define_user(user)
            .on_database()
            .password(password)
            .role(UserRole::Owner)
            .run(&self.admin)
            .await?;
        let db = self.admin.clone();
        db.signin(auth::Database {
            namespace: self.namespace.clone(),
            database: self.name.clone(),
            username: user.to_owned(),
            password: password.to_owned(),
        })
        .await?;
        self.db = db;
        Ok(())
    }

    /// The database's name and the removal of the database, by name, on the caller's connection.
    fn removal(self) -> (String, impl Future<Output = MigrationResult<()>>) {
        let name = self.name.clone();
        let removal = async move {
            remove_database(self.name.as_str()).run(&self.admin).await?;
            Ok(())
        };
        (name, removal)
    }

    /// The connection to the database, signed in as the generator's user once prepared.
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
    let down = if migration_flag == MigrationFlag::TwoWay {
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
