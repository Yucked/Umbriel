use std::{collections::BTreeSet, path::PathBuf};

use crate::{statements::*, *};
use surrealdb::{Connection, Surreal, engine::any::Any};

use super::migration_model::history_or_empty;
use crate::migrations::*;

/// Runs `queries` as one transaction, unless there is nothing to run.
///
/// Every migration gets a transaction of its own. SurrealDB builds indexes in transactions of its
/// own, so a single transaction spanning several migrations that define, change or remove the same
/// index fails to commit with a write conflict.
pub(crate) async fn run_transaction(
    db: Surreal<impl Connection>,
    queries: String,
) -> MigrationResult<()> {
    if !queries.trim().is_empty() {
        begin_transaction()
            .query(Raw::new(queries))
            .commit_transaction()
            .run(db)
            .await?;
    }
    Ok(())
}

/// Applies, rolls back, lists and prunes migrations against a database.
///
/// Each migration is applied, or rolled back, in a transaction of its own together with its
/// history record, so a batch is not all-or-nothing: when one migration fails, the ones before
/// it stay committed and recorded, the failing one and those after it leave nothing behind, and
/// running the batch again resumes with the failed migration.
pub struct MigrationRunner {}

/// How a rollback runs.
pub struct RollbackOptions {
    /// How far to roll back.
    pub rollback_strategy: RollbackStrategy,
    /// How closely the files are checked against the history.
    pub mode: Mode,
}

impl Default for RollbackOptions {
    fn default() -> Self {
        Self {
            rollback_strategy: RollbackStrategy::Previous,
            mode: Mode::default(),
        }
    }
}

impl RollbackOptions {
    /// Rolls back the previous migration in strict mode.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the mode is strict.
    pub fn is_strict(&self) -> bool {
        self.mode == Mode::Strict
    }

    /// Sets how far to roll back.
    pub fn strategy(mut self, rollback_strategy: RollbackStrategy) -> Self {
        self.rollback_strategy = rollback_strategy;
        self
    }

    /// Sets the mode.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
}

impl MigrationRunner {
    /// Rolls back two-way migrations as far as `rollback_options` says, most recent first, each
    /// in its own transaction with the removal of its history record. A failure leaves the
    /// migrations rolled back before it rolled back.
    pub async fn rollback_migrations(
        db: Surreal<Any>,
        fm: &MigrationConfig,
        rollback_options: RollbackOptions,
    ) -> MigrationResult<()> {
        let RollbackOptions {
            ref rollback_strategy,
            mode: ref strictness,
        } = rollback_options;

        let all_migrations_from_dir = fm.get_two_way_migrations_sorted_desc(false)?;

        let (queries_to_run, _rolledback_file_paths) = match rollback_strategy {
            RollbackStrategy::Previous => {
                let latest_migration = Self::get_latest_migration(db.clone()).await?;
                match latest_migration {
                    Some(latest_migration) => {
                        let latest_migration_name: MigrationFilename =
                            latest_migration.name.clone().try_into()?;
                        let migrations_from_dir = all_migrations_from_dir
                            .iter()
                            .find(|m| m.up.name == latest_migration_name.to_up())
                            .ok_or(MigrationError::RollbackFailed(format!(
                "The latest migration - {} - does not have a corresponding down migration file",
                latest_migration_name
            )))?;
                        Self::generate_rollback_queries_and_filepaths(
                            fm,
                            vec![migrations_from_dir.clone()],
                            vec![latest_migration],
                            strictness,
                        )?
                    }
                    None => Self::generate_rollback_queries_and_filepaths(
                        fm,
                        vec![],
                        vec![],
                        strictness,
                    )?,
                }
            }
            RollbackStrategy::Number(count) => {
                let migrations_from_db = history_or_empty(
                    select(All)
                        .from(Migration::table())
                        .order_by(Migration::schema().timestamp.desc())
                        .limit(*count)
                        .decode::<Migration>()
                        .execute(&db)
                        .await,
                )?;

                let latest_migration = migrations_from_db
                    .first()
                    .ok_or(MigrationError::NoMigrationsRegisteredYetInDb)?;

                let migrations_to_rollback = all_migrations_from_dir
                    .into_iter()
                    .filter(|m| m.up.name.timestamp() <= latest_migration.timestamp)
                    .take(*count as usize)
                    .collect::<Vec<_>>();

                Self::generate_rollback_queries_and_filepaths(
                    fm,
                    migrations_to_rollback,
                    migrations_from_db,
                    strictness,
                )?
            }
            RollbackStrategy::Till(file_cursor) => {
                let MigrationSchema {
                    timestamp, name, ..
                } = &Migration::schema();

                // By using to_up(), we also allow using the down migration file name
                // counterpart to the up migration filename as the cursor, giving user
                // more flexibility to use either.
                let file_cursor = &file_cursor.to_up();
                let timestamp_value = file_cursor.timestamp().into_inner();
                if Migration::get_by_filename(db.clone(), file_cursor)
                    .await?
                    .is_none()
                {
                    return Err(MigrationError::MigrationDoesNotExist {
                        filename: file_cursor.clone(),
                    });
                }

                let migrations_from_db = history_or_empty(
                    select(All)
                        .from(Migration::table())
                        .where_(
                            cond(timestamp.gt(timestamp_value))
                                .or(cond(timestamp.equal(timestamp_value))
                                    .and(name.equal(file_cursor.to_string()))),
                        )
                        .order_by(timestamp.desc())
                        .decode::<Migration>()
                        .execute(&db)
                        .await,
                )?;

                let latest_migration =
                    migrations_from_db
                        .first()
                        .ok_or(MigrationError::MigrationDoesNotExist {
                            filename: file_cursor.clone(),
                        })?;

                let migrations_files_to_rollback = all_migrations_from_dir
                    .clone()
                    .into_iter()
                    .filter(|m| {
                        let is_latest = m.up.name
                            == latest_migration
                                .name
                                .clone()
                                .try_into()
                                .expect("Invalid migration name");
                        let is_before_db_latest_migration =
                            m.up.name.timestamp() < latest_migration.timestamp;

                        let is_after_file_cursor = m.up.name.timestamp() > file_cursor.timestamp();
                        let is_file_cursor = &m.up.name == file_cursor;

                        (is_before_db_latest_migration || is_latest)
                            && (is_after_file_cursor || is_file_cursor)
                    })
                    .collect::<Vec<_>>();

                Self::generate_rollback_queries_and_filepaths(
                    fm,
                    migrations_files_to_rollback,
                    migrations_from_db,
                    strictness,
                )?
            }
        };

        if queries_to_run.is_empty() {
            log::info!("No migrations to rollback");
        } else {
            let count = queries_to_run.len();
            for queries in queries_to_run {
                run_transaction(db.clone(), queries).await?;
            }
            log::info!("Rolled back {count} migration(s)");
        }

        Ok(())
    }

    async fn get_latest_migration(
        db: Surreal<impl Connection>,
    ) -> MigrationResult<Option<Migration>> {
        Migration::get_latest(db).await
    }

    /// The migrations newer than the latest applied one, oldest first.
    pub async fn get_pending_migrations(
        all_migrations: Vec<impl Into<MigrationFile>>,
        db: Surreal<impl Connection>,
    ) -> MigrationResult<Vec<PendingMigrationFile>> {
        let latest_migration = Self::get_latest_migration(db.clone()).await?;

        let mut pending_migrations = all_migrations
            .into_iter()
            .map(|m| {
                let m: MigrationFile = m.into();
                PendingMigrationFile::from(m)
            })
            .filter(|m| {
                latest_migration.as_ref().is_none_or(|latest_migration| {
                    m.name_forward().timestamp() > latest_migration.timestamp
                })
            })
            .collect::<Vec<_>>();

        pending_migrations.sort_by_key(|m| m.name_forward().timestamp());

        Ok(pending_migrations)
    }

    /// The migration directory's files newer than the latest applied migration.
    pub async fn get_pending_migration_filenames(
        db: Surreal<impl Connection>,
        mig_config: &MigrationConfig,
    ) -> MigrationResult<MigrationFilenames> {
        let latest_migration = Self::get_latest_migration(db.clone()).await?;

        let pending_migrations = mig_config
            .get_migrations_filenames(false)?
            .all()
            .into_iter()
            .filter(|filename| {
                latest_migration.as_ref().is_none_or(|latest_migration| {
                    filename.timestamp() > latest_migration.timestamp
                })
            })
            .collect::<Vec<_>>();

        Ok(pending_migrations.into())
    }

    /// Deletes the migration directory's files newer than the latest applied migration.
    pub async fn delete_unapplied_migration_files(
        db: Surreal<impl Connection>,
        mig_config: &MigrationConfig,
    ) -> MigrationResult<()> {
        let dir = mig_config.get_migration_dir()?;

        let pending_migration_filenames =
            Self::get_pending_migration_filenames(db.clone(), mig_config).await?;

        let pending_migrations_paths = pending_migration_filenames
            .iter()
            .map(|filename| dir.join(filename.to_string()))
            .collect::<Vec<_>>();

        log::info!(
            "Deleting {} unapplied migration file(s)",
            pending_migrations_paths.len()
        );

        for mig_path in pending_migrations_paths {
            let mig_path_str = mig_path.to_string_lossy();
            log::info!("Deleting file: {mig_path_str}");
            std::fs::remove_file(&mig_path).map_err(|e| {
                MigrationError::IoError(format!(
                    "Failed to delete migration file: {mig_path_str}. Error: {e}"
                ))
            })?;
            log::info!("Deleted file: {mig_path_str}");
        }

        Ok(())
    }

    fn generate_rollback_queries_and_filepaths(
        fm: &MigrationConfig,
        migrations_to_rollback: Vec<MigrationFileTwoWayPair>,
        migrations_from_db: Vec<Migration>,
        mode: &Mode,
    ) -> MigrationResult<(Vec<String>, Vec<PathBuf>)> {
        if mode.is_strict() {
            for (m_from_file, m_from_db) in
                migrations_to_rollback.iter().zip(migrations_from_db.iter())
            {
                let db_mig_name: MigrationFilename = m_from_db.name.clone().try_into()?;

                m_from_db
                    .checksum_up
                    .verify(&m_from_file.up.name, &m_from_file.up.content)?;

                m_from_db
                    .clone()
                    .checksum_down
                    .ok_or(MigrationError::NoChecksumInDb {
                        migration_name: m_from_db.name.clone(),
                    })?
                    .verify(&m_from_file.down.name, &m_from_file.down.content)?;

                if m_from_file.up.name != db_mig_name.to_up() {
                    return Err(MigrationError::MigrationFileVsDbNamesMismatch {
                        migration_file_name: m_from_file.up.name.to_string(),
                        migration_db_name: db_mig_name.to_string(),
                    });
                }
            }
        }

        log::info!("Rolling back {} migration(s)", migrations_to_rollback.len());

        // One transaction per migration, most recent first: its history record, then its down
        // migration. History first: rolling back the first migration removes the history table,
        // and a `DELETE` from a missing table fails. Records are keyed by the up file's name.
        let file_ids = migrations_to_rollback
            .iter()
            .map(|m| Migration::create_id(&m.up.name))
            .collect::<Vec<_>>();
        // History records without a migration file, which lax mode allows, are removed with the
        // first transaction: a later one may roll back the first migration, which removes the
        // history table.
        let orphans = migrations_from_db
            .iter()
            .filter(|m| !file_ids.contains(&m.id))
            .map(|m| Migration::delete_raw(&m.id).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let mut transactions = migrations_to_rollback
            .iter()
            .zip(&file_ids)
            .map(|(m, id)| format!("{}\n{}", Migration::delete_raw(id), m.down.content))
            .collect::<Vec<_>>();
        match transactions.first_mut() {
            Some(first) if !orphans.is_empty() => *first = format!("{orphans}\n{first}"),
            None if !orphans.is_empty() => transactions.push(orphans),
            _ => {}
        }

        let file_paths = migrations_to_rollback
            .iter()
            .map(|m| {
                fm.get_migration_dir().map(|d| {
                    vec![
                        d.join(m.up.name.to_string()),
                        d.join(m.down.name.to_string()),
                    ]
                })
            })
            .collect::<MigrationResult<Vec<_>>>();

        Ok((
            transactions,
            file_paths?.iter().flatten().cloned().collect::<Vec<_>>(),
        ))
    }

    async fn run_up_pending_migrations(
        db: Surreal<impl Connection>,
        filtered_pending_migrations: Vec<PendingMigrationFile>,
    ) -> MigrationResult<()> {
        if filtered_pending_migrations.is_empty() {
            log::info!("No new migrations to apply");
            return Ok(());
        }

        let count = filtered_pending_migrations.len();
        log::info!("Running {count} migration(s)");
        for mf in filtered_pending_migrations {
            let (content, record) = match mf.into() {
                MigrationFile::OneWay(m) => {
                    let record = Migration::create_raw(m.name(), &m.content().as_checksum()?, None);
                    (m.content().to_string(), record)
                }
                MigrationFile::TwoWay(m) => {
                    let record = Migration::create_raw(
                        &m.up.name,
                        &m.up.content.as_checksum()?,
                        Some(&m.down.content.as_checksum()?),
                    );
                    (m.up.content.to_string(), record)
                }
            };
            // The migration and its history record commit together.
            run_transaction(db.clone(), format!("{content}\n{record}")).await?;
        }
        log::info!("Applied {count} migration(s)");

        Ok(())
    }

    /// Applies pending migrations as far as `update_strategy` says, each in its own transaction
    /// with its history record. A failure leaves the migrations applied before it applied.
    pub async fn apply_pending_migrations(
        db: Surreal<impl Connection>,
        all_migrations: Vec<impl Into<MigrationFile> + ::std::fmt::Debug>,
        update_strategy: UpdateStrategy,
    ) -> MigrationResult<()> {
        log::info!("Running pending migrations");

        let filtered_pending_migrations = match update_strategy {
            UpdateStrategy::Latest => {
                Self::get_pending_migrations(all_migrations, db.clone()).await?
            }
            UpdateStrategy::Number(count) => {
                Self::get_pending_migrations(all_migrations, db.clone())
                    .await?
                    .into_iter()
                    .take(count as usize)
                    .collect::<Vec<_>>()
            }
            UpdateStrategy::Till(filecursor) => {
                let pending_migs = Self::get_pending_migrations(all_migrations, db.clone()).await?;

                let mut migration_found = false;
                let mut filtered_migs: Vec<PendingMigrationFile> = vec![];

                for mig in pending_migs {
                    filtered_migs.push(mig.clone());
                    // if *mig.name_forward() == filecursor {
                    if mig.name_forward().to_up() == filecursor.to_up() {
                        migration_found = true;
                        break;
                    }
                }

                if !migration_found {
                    return Err(MigrationError::MigrationNotFoundFromPendingMigrations(
                        filecursor,
                    ));
                }

                filtered_migs
            }
        };

        Self::run_up_pending_migrations(db.clone(), filtered_pending_migrations).await
    }

    pub(crate) async fn list_migrations(
        db: Surreal<impl Connection>,
        migrations_local_dir: Vec<MigrationFilename>,
        status: Status,
        mode: Mode,
    ) -> MigrationResult<Vec<MigrationFilename>> {
        let migrations = match status {
            Status::All => {
                let mut migrations = migrations_local_dir;
                migrations.sort_by_key(MigrationFilename::timestamp);
                migrations.into_iter().collect::<Vec<_>>()
            }
            Status::Pending => {
                let latest_applied_migration = Self::get_latest_migration(db.clone()).await?;

                let mut migrations = migrations_local_dir
                    .into_iter()
                    .filter(|name| {
                        latest_applied_migration
                            .as_ref()
                            .is_none_or(|latest_migration| {
                                name.timestamp() > latest_migration.timestamp
                            })
                    })
                    .collect::<Vec<_>>();
                migrations.sort_by_key(MigrationFilename::timestamp);
                migrations
            }
            Status::Applied => {
                let db_migs = Migration::get_all_desc(db.clone()).await?;
                Self::_get_db_migrations_meta_from_mig_files(migrations_local_dir, db_migs, mode)?
            }
        };
        Ok(migrations)
    }

    fn _get_db_migrations_meta_from_mig_files(
        local_migrations: Vec<MigrationFilename>,
        db_migs: Vec<Migration>,
        mode: Mode,
    ) -> MigrationResult<Vec<MigrationFilename>> {
        let db_migs: BTreeSet<MigrationFilename> = db_migs
            .into_iter()
            .map(|dbm| dbm.name.try_into().expect("Invalid migration file name."))
            .collect();

        let filtered_local_migrations = local_migrations
            .into_iter()
            .filter(|name| db_migs.contains(name))
            .collect::<Vec<_>>();

        // TODO:: Check that the files are contiguous?
        if mode.is_strict() && filtered_local_migrations.len() != db_migs.len() {
            return Err(MigrationError::InvalidMigrationState {
                db_migration_count: db_migs.len(),
                local_dir_migration_count: filtered_local_migrations.len(),
            });
        }

        Ok(filtered_local_migrations)
    }
}
