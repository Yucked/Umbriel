use std::collections::BTreeMap;
#[cfg(any(feature = "cli", feature = "test-support"))]
use std::path::Path;

use crate::{statements::*, *};
use surrealdb::{Connection, Surreal, engine::any::Any, types::RecordId};

use super::migration_model::HistoryReading;
use crate::migrations::*;

/// Runs `queries` as one transaction, unless there is nothing to run.
///
/// Every migration gets a transaction of its own. SurrealDB builds indexes in transactions of its
/// own, so a single transaction spanning several migrations that define, change or remove the same
/// index fails to commit with a write conflict.
pub(crate) async fn run_transaction(
    db: &Surreal<impl Connection>,
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

/// Reads the migration history once, as every runner does before it changes anything. A test
/// seam, re-exported from `test_support`.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub async fn read_history(db: &Surreal<impl Connection>) -> MigrationResult<HistoryReading> {
    HistoryReading::read(db).await
}

/// Applies the migrations `reading` leaves pending, as a runner that read the history before
/// another runner committed does. A test seam, re-exported from `test_support`.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub async fn apply_after_reading(
    db: &Surreal<impl Connection>,
    all_migrations: Vec<impl Into<MigrationFile>>,
    reading: &HistoryReading,
) -> MigrationResult<()> {
    let migrations = all_migrations.into_iter().map(Into::into).collect();
    let pending = MigrationRunner::pending(migrations, reading)?;
    MigrationRunner::run_up_pending_migrations(db, pending, reading, false).await
}

/// Takes the history's lock for `holder`, as a prune does before it deletes files. A test seam,
/// re-exported from `test_support`.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub async fn lock_history(db: &Surreal<impl Connection>, holder: &str) -> MigrationResult<()> {
    let reading = HistoryReading::read(db).await?;
    MigrationRunner::take_lock(db, &reading, holder).await
}

/// Deletes `selection`, the files a prune listed from `reading`, after the user confirmed them:
/// under the history's lock, taken only if the history is still the one read. A test seam,
/// re-exported from `test_support`. Pruning is a command-line operation, so it is built only for
/// the CLI and the tests.
#[cfg(any(feature = "cli", feature = "test-support"))]
#[doc(hidden)]
pub async fn prune_confirmed(
    db: &Surreal<impl Connection>,
    mig_config: &MigrationConfig,
    reading: &HistoryReading,
    selection: &[MigrationFilename],
) -> MigrationResult<()> {
    let dir = mig_config.get_migration_dir()?;
    MigrationRunner::with_lock(db, reading, |_| async {
        for name in selection {
            delete_file(&dir.join(name.to_string()))?;
        }
        Ok(())
    })
    .await
}

/// Deletes a migration file.
#[cfg(any(feature = "cli", feature = "test-support"))]
pub(crate) fn delete_file(path: &Path) -> MigrationResult<()> {
    log::info!("Deleting file: {}", path.display());
    std::fs::remove_file(path).map_err(|source| MigrationError::Io {
        action: "deleting the migration file",
        path: path.to_owned(),
        source,
    })
}

/// Applies, rolls back, lists and prunes migrations against a database.
///
/// Each migration is applied, or rolled back, in a transaction of its own together with its
/// history record, so a batch is not all-or-nothing: when one migration fails, the ones before
/// it stay committed and recorded, the failing one and those after it leave nothing behind, and
/// running the batch again resumes with the failed migration.
///
/// An interrupted run recovers the same way: a migration whose transaction did not commit left
/// neither its database changes nor its history record, so the next run applies it again. The
/// guarantee covers what the transaction rolls back. A side effect outside the database is not
/// undone: `file::*` writes go straight to the bucket's store, so a migration that writes a file
/// and then fails leaves the file behind, and applying it again writes it again. Keep such
/// migrations idempotent.
///
/// Migrations apply in file name order (timestamp, then name), and only in that order: a pending
/// migration that sorts before the newest applied one is [`MigrationError::OutOfOrder`] in both
/// modes. The history's file order is therefore its application order, which rollback and the
/// generator's replay rely on.
///
/// Runners on one database may race. Each runner reads the history and its revision once, and
/// each of its transactions checks the revision is still the one it expects and writes the next.
/// SurrealDB's snapshot isolation would let two overlapping transactions that write different
/// history records both commit; writing the one revision record makes them conflict, so the
/// later one fails. A runner whose reading went stale, or that overlapped another, changes
/// nothing more and fails with [`MigrationError::HistoryChanged`]; running again reads afresh.
/// While a prune holds the history's lock, every other runner fails with
/// [`MigrationError::HistoryLocked`].
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

/// An applied migration a rollback reverses, with its files as found.
pub(crate) struct RollbackEntry {
    /// The up file's name, which the history records.
    name: MigrationFilename,
    /// The history record.
    id: RecordId,
    /// The down file's content; `None` when the file is missing.
    down: Option<FileContent>,
}

/// The migrations a rollback reverses, newest first, planned from one reading of the history.
pub(crate) struct RollbackPlan {
    reading: HistoryReading,
    entries: Vec<RollbackEntry>,
    /// In lax mode, the first entry without a down file, where the rollback stops.
    stop: Option<usize>,
}

impl RollbackPlan {
    /// The reading the plan was made from.
    #[cfg(feature = "cli")]
    pub(crate) fn reading(&self) -> &HistoryReading {
        &self.reading
    }

    /// The files of the migrations the rollback reverses, up and down, newest first.
    #[cfg(feature = "cli")]
    pub(crate) fn files(&self) -> Vec<MigrationFilename> {
        self.reversed()
            .iter()
            .flat_map(|entry| [entry.name.clone(), entry.name.to_down()])
            .collect()
    }

    /// The entries the rollback reverses.
    fn reversed(&self) -> &[RollbackEntry] {
        &self.entries[..self.stop.unwrap_or(self.entries.len())]
    }
}

/// In strict mode, `result`; in lax mode, its error logged as a warning.
fn strictly(mode: Mode, result: MigrationResult<()>) -> MigrationResult<()> {
    match (mode, result) {
        (Mode::Lax, Err(error)) => {
            log::warn!("{error}");
            Ok(())
        }
        (_, result) => result,
    }
}

/// A runner transaction's failure: a stale reading, a lock, a conflict with another runner and
/// a history rewritten by `migration`, the one the transaction applied, are told apart from the
/// migration's own failure.
fn history_error(error: MigrationError, migration: Option<&MigrationFilename>) -> MigrationError {
    let MigrationError::ProblemWithQuery(inner) = error else {
        return error;
    };
    // Every runner transaction writes the revision record, so a commit conflict is another
    // runner's change.
    if matches!(inner, crate::Error::Conflict { .. }) {
        return MigrationError::HistoryChanged;
    }
    // The checks are `THROW`s in the transaction. SDK 3.3.4 marks thrown errors (`is_thrown`)
    // but carries no code to tell them apart, and a migration's own SQL may throw too, so only
    // the message identifies them.
    let text = inner.to_string();
    if text.contains(Migration::HISTORY_CHANGED) {
        return MigrationError::HistoryChanged;
    }
    if let Some(at) = text.find(Migration::HISTORY_LOCKED) {
        let holder = text[at + Migration::HISTORY_LOCKED.len()..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        return MigrationError::HistoryLocked { holder };
    }
    if let Some(migration) = migration
        && text.contains(Migration::HISTORY_REWRITTEN)
    {
        return MigrationError::HistoryRewritten {
            migration: migration.clone(),
        };
    }
    MigrationError::ProblemWithQuery(inner)
}

impl MigrationRunner {
    /// The migrations `reading` doesn't record, in file order.
    ///
    /// # Errors
    ///
    /// [`MigrationError::OutOfOrder`] when one sorts before the newest applied migration.
    pub(crate) fn pending(
        all_migrations: Vec<MigrationFile>,
        reading: &HistoryReading,
    ) -> MigrationResult<Vec<MigrationFile>> {
        let mut pending = all_migrations
            .into_iter()
            .filter(|m| !reading.is_applied(m.name_forward()))
            .collect::<Vec<_>>();
        pending.sort_by(|a, b| a.name_forward().cmp(b.name_forward()));
        Self::check_order(pending.iter().map(MigrationFile::name_forward), reading)?;
        Ok(pending)
    }

    /// Fails for the oldest of `pending` that sorts before the newest applied migration.
    fn check_order<'a>(
        pending: impl IntoIterator<Item = &'a MigrationFilename>,
        reading: &HistoryReading,
    ) -> MigrationResult<()> {
        let Some(newest) = reading.newest() else {
            return Ok(());
        };
        match pending
            .into_iter()
            .filter(|name| name.to_up() < *newest)
            .min()
        {
            Some(name) => Err(MigrationError::OutOfOrder {
                name: name.clone(),
                newest_applied: newest.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Checks every migration `reading` records against `all_migrations`: its file must exist
    /// and still have the checksums it was applied with. Lax mode logs what doesn't.
    ///
    /// # Errors
    ///
    /// In strict mode, [`MigrationError::AppliedFileMissing`],
    /// [`MigrationError::ChecksumMismatch`] or [`MigrationError::NoChecksumInDb`].
    pub(crate) fn check_applied(
        all_migrations: &[MigrationFile],
        reading: &HistoryReading,
        mode: Mode,
    ) -> MigrationResult<()> {
        let files = all_migrations
            .iter()
            .map(|m| (m.name_forward().to_up(), m))
            .collect::<BTreeMap<_, _>>();
        for (key, applied) in &reading.applied {
            let Some(file) = files.get(key) else {
                let name = MigrationFilename::try_from(applied.name.clone())?;
                strictly(mode, Err(MigrationError::AppliedFileMissing { name }))?;
                continue;
            };
            strictly(
                mode,
                applied
                    .checksum_up
                    .verify(file.name_forward(), file.up_content()),
            )?;
            if let Some(down) = file.down_content() {
                let verified = applied
                    .checksum_down
                    .as_ref()
                    .ok_or_else(|| MigrationError::NoChecksumInDb {
                        migration_name: applied.name.clone(),
                    })
                    .and_then(|checksum| checksum.verify(&key.to_down(), down));
                strictly(mode, verified)?;
            }
        }
        Ok(())
    }

    /// Checks `all_migrations` against the history of `db` as `up` does before it applies
    /// anything: the applied files ([`check_applied`](Self::check_applied)) and the order of the
    /// pending ones ([`pending`](Self::pending)), which is checked in both modes.
    pub(crate) async fn check_history(
        db: &Surreal<impl Connection>,
        all_migrations: &[MigrationFile],
        mode: Mode,
    ) -> MigrationResult<()> {
        let reading = HistoryReading::read(db).await?;
        Self::check_applied(all_migrations, &reading, mode)?;
        Self::check_order(
            all_migrations
                .iter()
                .map(MigrationFile::name_forward)
                .filter(|name| !reading.is_applied(name)),
            &reading,
        )
    }

    /// Rolls back two-way migrations as far as `rollback_options` says, most recent first, each
    /// in its own transaction with the removal of its history record. A failure leaves the
    /// migrations rolled back before it rolled back.
    ///
    /// Strict mode checks every selected migration's files before changing anything. Lax mode
    /// rolls back the newest migrations that have down files and stops at the first that
    /// doesn't, since an older migration a newer one depends on must stay applied.
    ///
    /// # Errors
    ///
    /// [`MigrationError::RollbackStopped`] when lax mode stopped; the errors of
    /// [`check_applied`](Self::check_applied) in strict mode; and a transaction's failure.
    pub async fn rollback_migrations(
        db: &Surreal<Any>,
        fm: &MigrationConfig,
        rollback_options: RollbackOptions,
    ) -> MigrationResult<()> {
        let plan = Self::plan_rollback(db, fm, rollback_options).await?;
        let revision = plan.reading.revision;
        Self::execute_rollback(db, &plan, None, revision, |_| Ok(())).await
    }

    /// The migrations a rollback reverses, with their down files read by name.
    pub(crate) async fn plan_rollback(
        db: &Surreal<impl Connection>,
        fm: &MigrationConfig,
        rollback_options: RollbackOptions,
    ) -> MigrationResult<RollbackPlan> {
        let RollbackOptions {
            rollback_strategy,
            mode,
        } = rollback_options;
        let reading = HistoryReading::read(db).await?;
        reading.check_unlocked()?;
        let newest_first = reading.applied.iter().rev();
        let selected = match rollback_strategy {
            RollbackStrategy::Previous => newest_first.take(1).collect::<Vec<_>>(),
            RollbackStrategy::Number(count) => {
                if reading.applied.is_empty() {
                    return Err(MigrationError::NoMigrationsRegisteredYetInDb);
                }
                newest_first.take(count as usize).collect()
            }
            RollbackStrategy::Till(cursor) => {
                // Either of a two-way migration's file names names it.
                let cursor = cursor.to_up();
                if !reading.applied.contains_key(&cursor) {
                    return Err(MigrationError::MigrationDoesNotExist { filename: cursor });
                }
                newest_first
                    .take_while(|(name, _)| **name >= cursor)
                    .collect()
            }
        };

        let dir = fm.get_migration_dir()?;
        let read_if_exists = |name: &MigrationFilename| -> MigrationResult<Option<FileContent>> {
            let path = dir.join(name.to_string());
            let exists = std::fs::exists(&path).map_err(|source| MigrationError::Io {
                action: "checking the migration file",
                path: path.clone(),
                source,
            })?;
            exists.then(|| FileContent::from_file(path)).transpose()
        };

        let mut entries = Vec::with_capacity(selected.len());
        for (name, migration) in selected {
            let down = read_if_exists(&name.to_down())?;
            if mode == Mode::Strict {
                let Some(down) = &down else {
                    return Err(MigrationError::AppliedFileMissing {
                        name: name.to_down(),
                    });
                };
                let Some(up) = read_if_exists(name)? else {
                    return Err(MigrationError::AppliedFileMissing { name: name.clone() });
                };
                migration.checksum_up.verify(name, &up)?;
                migration
                    .checksum_down
                    .as_ref()
                    .ok_or_else(|| MigrationError::NoChecksumInDb {
                        migration_name: migration.name.clone(),
                    })?
                    .verify(&name.to_down(), down)?;
            }
            entries.push(RollbackEntry {
                name: name.clone(),
                id: migration.id.clone(),
                down,
            });
        }
        let stop = entries.iter().position(|entry| entry.down.is_none());
        Ok(RollbackPlan {
            reading,
            entries,
            stop,
        })
    }

    /// Reverses `plan`'s migrations, starting at `revision`, each in a transaction of its own,
    /// and calls `reversed` after each one commits.
    pub(crate) async fn execute_rollback(
        db: &Surreal<impl Connection>,
        plan: &RollbackPlan,
        holder: Option<&str>,
        mut revision: i64,
        mut reversed: impl FnMut(&MigrationFilename) -> MigrationResult<()>,
    ) -> MigrationResult<()> {
        let entries = plan.reversed();
        if entries.is_empty() && plan.stop.is_none() {
            log::info!("No migrations to rollback");
            return Ok(());
        }
        log::info!("Rolling back {} migration(s)", entries.len());
        for entry in entries {
            // The history record first: rolling back the first migration removes the history
            // table, and a `DELETE` from a missing table fails. The revision table stays.
            let transaction = format!(
                "{}\n{}\n{}\n{}",
                Migration::expect_revision_raw(revision, holder),
                Migration::delete_raw(&entry.id),
                entry.down.as_ref().map_or("", |down| down.as_str()),
                Migration::next_revision_raw(revision + 1, None),
            );
            run_transaction(db, transaction)
                .await
                .map_err(|error| history_error(error, None))?;
            revision += 1;
            reversed(&entry.name)?;
        }
        log::info!("Rolled back {} migration(s)", entries.len());
        if let Some(stop) = plan.stop {
            return Err(MigrationError::RollbackStopped {
                at: plan.entries[stop].name.clone(),
                rolled_back: stop,
            });
        }
        Ok(())
    }

    /// The migration directory's files that `db`'s history doesn't record.
    ///
    /// # Errors
    ///
    /// [`MigrationError::NoHistory`] when the database has no history table.
    pub async fn get_pending_migration_filenames(
        db: &Surreal<impl Connection>,
        mig_config: &MigrationConfig,
    ) -> MigrationResult<MigrationFilenames> {
        let reading = HistoryReading::read(db).await?;
        Self::pending_filenames(&reading, mig_config)
    }

    /// The migration directory's files that `reading` doesn't record.
    pub(crate) fn pending_filenames(
        reading: &HistoryReading,
        mig_config: &MigrationConfig,
    ) -> MigrationResult<MigrationFilenames> {
        if !reading.exists {
            return Err(MigrationError::NoHistory);
        }
        Ok(mig_config
            .get_migrations_filenames(false)?
            .into_vec()
            .into_iter()
            .filter(|filename| !reading.is_applied(filename))
            .collect::<Vec<_>>()
            .into())
    }

    /// Takes the history's lock for `holder`, in a transaction that refuses a stale `reading`.
    #[cfg(any(feature = "cli", feature = "test-support"))]
    pub(crate) async fn take_lock(
        db: &Surreal<impl Connection>,
        reading: &HistoryReading,
        holder: &str,
    ) -> MigrationResult<()> {
        let transaction = format!(
            "{}\n{}",
            Migration::expect_revision_raw(reading.revision, None),
            Migration::next_revision_raw(reading.revision + 1, Some(holder)),
        );
        run_transaction(db, transaction)
            .await
            .map_err(|error| history_error(error, None))
    }

    /// Releases the history's lock if `holder` holds it.
    #[cfg(any(feature = "cli", feature = "test-support"))]
    pub(crate) async fn release_lock(
        db: &Surreal<impl Connection>,
        holder: &str,
    ) -> MigrationResult<()> {
        run_transaction(db, Migration::release_lock_raw(holder).to_string()).await
    }

    /// Runs `work` with the history's lock, taken only if `reading` is still current, and
    /// releases it afterwards, whether `work` succeeded or not. `work` gets the holder's name;
    /// its transactions pass the lock's check with it, and start at the revision after
    /// `reading`'s.
    #[cfg(any(feature = "cli", feature = "test-support"))]
    pub(crate) async fn with_lock<T, F: Future<Output = MigrationResult<T>>>(
        db: &Surreal<impl Connection>,
        reading: &HistoryReading,
        work: impl FnOnce(String) -> F,
    ) -> MigrationResult<T> {
        let holder = format!("prune-{}", uuid::Uuid::new_v4().simple());
        Self::take_lock(db, reading, &holder).await?;
        let outcome = work(holder.clone()).await;
        let released = Self::release_lock(db, &holder).await;
        let outcome = outcome?;
        released?;
        Ok(outcome)
    }

    /// Applies `migrations` in order, each in a transaction that first checks the history's
    /// revision is still the one the run expects (`reading`'s for the first, then each
    /// transaction's own), and last checks the history holds the records it should: one more
    /// than before, or, when `replacing`, only the first migration's, which clears the history
    /// the reset replaced. A runner whose reading is stale applies nothing and fails with
    /// [`MigrationError::HistoryChanged`].
    pub(crate) async fn run_up_pending_migrations(
        db: &Surreal<impl Connection>,
        migrations: Vec<MigrationFile>,
        reading: &HistoryReading,
        replacing: bool,
    ) -> MigrationResult<()> {
        if migrations.is_empty() {
            log::info!("No new migrations to apply");
            return Ok(());
        }

        let count = migrations.len();
        log::info!("Running {count} migration(s)");
        let mut size = reading.applied.len();
        // Each transaction expects the revision the one before it wrote.
        for ((index, mf), revision) in migrations.into_iter().enumerate().zip(reading.revision..) {
            let name = mf.name_forward().clone();
            let record = Migration::create_raw(
                &name,
                &Checksum::generate_from_content(mf.up_content()),
                mf.down_content()
                    .map(Checksum::generate_from_content)
                    .as_ref(),
            );
            size = if replacing && index == 0 { 1 } else { size + 1 };
            // The checks, the migration and its history record commit together.
            let transaction = format!(
                "{}\n{}\n{record}\n{}\n{}",
                Migration::expect_revision_raw(revision, None),
                mf.up_content().as_str(),
                Migration::expect_size_raw(size),
                Migration::next_revision_raw(revision + 1, None),
            );
            run_transaction(db, transaction)
                .await
                .map_err(|error| history_error(error, Some(&name)))?;
        }
        log::info!("Applied {count} migration(s)");

        Ok(())
    }

    /// Applies pending migrations as far as `update_strategy` says, each in its own transaction
    /// with its history record. A failure leaves the migrations applied before it applied.
    ///
    /// # Errors
    ///
    /// The errors of [`check_applied`](Self::check_applied) in strict mode,
    /// [`MigrationError::OutOfOrder`] in both, and a transaction's failure.
    pub async fn apply_pending_migrations(
        db: &Surreal<impl Connection>,
        all_migrations: Vec<impl Into<MigrationFile>>,
        update_strategy: UpdateStrategy,
        mode: Mode,
    ) -> MigrationResult<()> {
        log::info!("Running pending migrations");

        // One reading of the history decides what is pending and what each transaction checks.
        let reading = HistoryReading::read(db).await?;
        reading.check_unlocked()?;
        let all_migrations = all_migrations
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        Self::check_applied(&all_migrations, &reading, mode)?;
        let mut pending = Self::pending(all_migrations, &reading)?;
        match update_strategy {
            UpdateStrategy::Latest => {}
            UpdateStrategy::Number(count) => pending.truncate(count as usize),
            UpdateStrategy::Till(filecursor) => {
                let target = filecursor.to_up();
                let Some(position) = pending
                    .iter()
                    .position(|mig| mig.name_forward().to_up() == target)
                else {
                    return Err(MigrationError::MigrationNotFoundFromPendingMigrations(
                        filecursor,
                    ));
                };
                pending.truncate(position + 1);
            }
        }

        Self::run_up_pending_migrations(db, pending, &reading, false).await
    }

    pub(crate) async fn list_migrations(
        db: &Surreal<impl Connection>,
        mut migrations_local_dir: Vec<MigrationFilename>,
        status: Status,
        mode: Mode,
    ) -> MigrationResult<Vec<MigrationFilename>> {
        migrations_local_dir.sort();
        let migrations = match status {
            Status::All => migrations_local_dir,
            Status::Pending => {
                let reading = HistoryReading::read(db).await?;
                migrations_local_dir
                    .into_iter()
                    .filter(|name| !reading.is_applied(name))
                    .collect()
            }
            Status::Applied => {
                let reading = HistoryReading::read(db).await?;
                Self::applied_local_migrations(migrations_local_dir, &reading, mode)?
            }
        };
        Ok(migrations)
    }

    /// The local migrations the history records. In strict mode, a recorded migration missing
    /// locally is an error.
    fn applied_local_migrations(
        local_migrations: Vec<MigrationFilename>,
        reading: &HistoryReading,
        mode: Mode,
    ) -> MigrationResult<Vec<MigrationFilename>> {
        let filtered_local_migrations = local_migrations
            .into_iter()
            .filter(|name| reading.is_applied(name))
            .collect::<Vec<_>>();

        if mode == Mode::Strict && filtered_local_migrations.len() != reading.applied.len() {
            return Err(MigrationError::InvalidMigrationState {
                db_migration_count: reading.applied.len(),
                local_dir_migration_count: filtered_local_migrations.len(),
            });
        }

        Ok(filtered_local_migrations)
    }
}
