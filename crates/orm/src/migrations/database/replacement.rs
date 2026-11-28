//! Replacing the migration history with a reset's first migration, and keeping the replaced
//! files recoverable until that is confirmed.
//!
//! A reset writes its migration into a staging directory beside the migration directory, records
//! the history it read and its migration in a backup directory beside it, and swaps the
//! migration files: the old ones into the backup, the new one in. The backup is removed only
//! once the history holds the reset's migration alone, applied by `reset --run` or
//! `up --replace-history`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use surrealdb::{Connection, Surreal};

use super::migration_model::HistoryReading;
use crate::migrations::*;

/// The file in the backup directory recording the reset.
const RECORD_FILE: &str = "reset.record";

/// The directory a reset keeps the replaced migration files of `dir` in, beside it.
pub fn reset_backup_dir(dir: &Path) -> PathBuf {
    sibling(dir, "reset-backup")
}

/// The directory a reset writes its new migration files of `dir` into, beside it.
pub(crate) fn reset_staging_dir(dir: &Path) -> PathBuf {
    sibling(dir, "reset-staging")
}

/// `<dir>.<suffix>` beside `dir`, so moves between them stay on one file system.
fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    dir.with_file_name(name)
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> MigrationError {
    let path = path.to_owned();
    move |source| MigrationError::Io {
        action,
        path,
        source,
    }
}

/// What a reset read and wrote: the history it replaces, at its revision, and its migration,
/// by name and checksums. Kept in the backup directory as `key value` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResetRecord {
    revision: i64,
    /// The replaced history's recorded names, in file order.
    old: Vec<String>,
    /// The reset migration's up (or only) file name.
    replacement: MigrationFilename,
    checksum_up: String,
    checksum_down: Option<String>,
}

impl ResetRecord {
    /// The record of a reset that replaces `reading` with `migration`.
    pub(crate) fn new(reading: &HistoryReading, migration: &MigrationFile) -> Self {
        Self {
            revision: reading.revision,
            old: reading.names().map(str::to_owned).collect(),
            replacement: migration.name_forward().clone(),
            checksum_up: Checksum::generate_from_content(migration.up_content()).to_string(),
            checksum_down: migration
                .down_content()
                .map(|down| Checksum::generate_from_content(down).to_string()),
        }
    }

    /// Writes the record into `backup`.
    pub(crate) fn write(&self, backup: &Path) -> MigrationResult<()> {
        let mut text = format!(
            "revision {}\nreplacement {}\nchecksum_up {}\n",
            self.revision, self.replacement, self.checksum_up
        );
        if let Some(checksum_down) = &self.checksum_down {
            text.push_str(&format!("checksum_down {checksum_down}\n"));
        }
        for name in &self.old {
            text.push_str(&format!("old {name}\n"));
        }
        let path = backup.join(RECORD_FILE);
        fs::write(&path, text).map_err(io_error("writing the reset record", &path))
    }

    /// The record in `backup`, or `None` when there is none.
    fn read(backup: &Path) -> MigrationResult<Option<Self>> {
        let path = backup.join(RECORD_FILE);
        let text = match fs::read_to_string(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result.map_err(io_error("reading the reset record", &path))?,
        };
        let invalid = || MigrationError::ResetIncomplete {
            backup: backup.to_owned(),
            staging: None,
            source: Some(Box::new(MigrationError::Io {
                action: "parsing the reset record",
                path: path.clone(),
                source: io::Error::from(io::ErrorKind::InvalidData),
            })),
        };
        let (mut revision, mut replacement, mut checksum_up) = (None, None, None);
        let (mut checksum_down, mut old) = (None, Vec::new());
        for line in text.lines() {
            let (key, value) = line.split_once(' ').ok_or_else(invalid)?;
            match key {
                "revision" => revision = Some(value.parse().map_err(|_| invalid())?),
                "replacement" => replacement = Some(MigrationFilename::try_from(value.to_owned())?),
                "checksum_up" => checksum_up = Some(value.to_owned()),
                "checksum_down" => checksum_down = Some(value.to_owned()),
                "old" => old.push(value.to_owned()),
                _ => return Err(invalid()),
            }
        }
        let (Some(revision), Some(replacement), Some(checksum_up)) =
            (revision, replacement, checksum_up)
        else {
            return Err(invalid());
        };
        Ok(Some(Self {
            revision,
            old,
            replacement,
            checksum_up,
            checksum_down,
        }))
    }

    /// Whether `reading` is the history the reset read.
    fn matches_old(&self, reading: &HistoryReading) -> bool {
        reading.revision == self.revision && reading.names().eq(self.old.iter().map(String::as_str))
    }

    /// Whether `reading` holds the reset's migration alone: the replacement committed.
    fn matches_replacement(&self, reading: &HistoryReading) -> bool {
        reading.applied.len() == 1
            && reading
                .applied
                .get(&self.replacement.to_up())
                .is_some_and(|applied| applied.checksum_up.to_string() == self.checksum_up)
    }

    /// Whether `migration` is the reset's migration, unchanged.
    fn matches_file(&self, migration: &MigrationFile) -> bool {
        *migration.name_forward() == self.replacement
            && Checksum::generate_from_content(migration.up_content()).to_string()
                == self.checksum_up
            && migration
                .down_content()
                .map(|down| Checksum::generate_from_content(down).to_string())
                == self.checksum_down
    }

    /// The reset migration's file names.
    fn files(&self) -> Vec<MigrationFilename> {
        let mut files = vec![self.replacement.clone()];
        if self.checksum_down.is_some() {
            files.push(self.replacement.to_down());
        }
        files
    }
}

/// Moves the migration files of `dir` into `backup` and those of `staging` into `dir`, with
/// `rename`. Other files stay where they are. A failed move undoes the moves before it, in
/// reverse. A test seam, re-exported from `test_support`.
///
/// # Errors
///
/// [`MigrationError::Io`] for the failed move once it is undone, or
/// [`MigrationError::ResetIncomplete`] when the undo failed too: every file is then in `dir`,
/// `staging` or `backup`, and nothing is deleted.
#[doc(hidden)]
pub fn swap_migration_files(
    dir: &Path,
    staging: &Path,
    backup: &Path,
    mut rename: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> MigrationResult<()> {
    fs::create_dir_all(backup).map_err(io_error("creating the reset backup", backup))?;
    let old = migration_filenames_in(dir)?;
    let new = migration_filenames_in(staging)?;
    let moves = old
        .iter()
        .map(|name| (dir.join(name.to_string()), backup.join(name.to_string())))
        .chain(
            new.iter()
                .map(|name| (staging.join(name.to_string()), dir.join(name.to_string()))),
        )
        .collect::<Vec<_>>();
    for (done, (from, to)) in moves.iter().enumerate() {
        let Err(source) = rename(from, to) else {
            continue;
        };
        let error = MigrationError::Io {
            action: "moving the migration file",
            path: from.clone(),
            source,
        };
        for (from, to) in moves[..done].iter().rev() {
            if rename(to, from).is_err() {
                return Err(MigrationError::ResetIncomplete {
                    backup: backup.to_owned(),
                    staging: Some(staging.to_owned()),
                    source: Some(Box::new(error)),
                });
            }
        }
        return Err(error);
    }
    Ok(())
}

/// Removes the reset's backup: its record and the replaced files it kept.
fn remove_backup(backup: &Path) -> MigrationResult<()> {
    fs::remove_dir_all(backup).map_err(io_error("removing the reset backup", backup))
}

/// Puts the replaced files back: deletes the reset's migration files from `dir`, moves the
/// backup's files back into it, and removes the backup.
fn restore(dir: &Path, backup: &Path, record: &ResetRecord) -> MigrationResult<()> {
    for name in record.files() {
        let path = dir.join(name.to_string());
        fs::remove_file(&path).map_err(io_error("deleting the reset's migration file", &path))?;
    }
    for name in migration_filenames_in(backup)? {
        let from = backup.join(name.to_string());
        fs::rename(&from, dir.join(name.to_string()))
            .map_err(io_error("restoring the migration file", &from))?;
    }
    remove_backup(backup)
}

/// Applies the reset's migration in `mig_config`'s directory to `db`, replacing the history the
/// reset read, and removes the backup. Applying it is idempotent: a history that already holds
/// it alone only removes the backup.
///
/// # Errors
///
/// [`MigrationError::NoPendingReset`] without a reset's backup;
/// [`MigrationError::ResetIncomplete`] when the history is neither the one the reset read nor
/// its replacement, or the reset's migration is not in the directory unchanged; and the
/// migration's failure.
pub(crate) async fn replace_history(
    db: &Surreal<impl Connection>,
    mig_config: &MigrationConfig,
) -> MigrationResult<()> {
    let dir = mig_config.get_migration_dir()?;
    let backup = reset_backup_dir(&dir);
    let Some(record) = ResetRecord::read(&backup)? else {
        return Err(MigrationError::NoPendingReset);
    };
    let incomplete = || MigrationError::ResetIncomplete {
        backup: backup.clone(),
        staging: None,
        source: None,
    };
    let reading = HistoryReading::read(db).await?;
    reading.check_unlocked()?;
    if record.matches_replacement(&reading) {
        return remove_backup(&backup);
    }
    if !record.matches_old(&reading) {
        return Err(incomplete());
    }
    let Some(replacement) = mig_config
        .migration_files(false)?
        .into_iter()
        .find(|migration| record.matches_file(migration))
    else {
        return Err(incomplete());
    };
    MigrationRunner::run_up_pending_migrations(db, vec![replacement], &reading, true).await?;
    remove_backup(&backup)
}

/// Replaces `dir`'s migration files with `migration`, recording that it replaces `reading`:
/// staged, recorded in the backup, then swapped in. A failure before the swap completes leaves
/// the original files in place and removes what this created, unless the swap's undo failed.
///
/// # Errors
///
/// [`MigrationError::ResetIncomplete`] when an earlier reset's backup is still there, or when
/// the swap could not be undone; otherwise the failure of writing or moving a file.
pub(crate) fn stage_reset(
    dir: &Path,
    mig_config: &MigrationConfig,
    reading: &HistoryReading,
    migration: &MigrationFile,
) -> MigrationResult<()> {
    let backup = reset_backup_dir(dir);
    let staging = reset_staging_dir(dir);
    let exists = |path: &Path| fs::exists(path).map_err(io_error("checking the directory", path));
    if exists(&backup)? {
        return Err(MigrationError::ResetIncomplete {
            backup,
            staging: exists(&staging)?.then_some(staging),
            source: None,
        });
    }
    let staged = (|| -> MigrationResult<()> {
        fs::create_dir(&staging).map_err(io_error("creating the reset staging", &staging))?;
        migration.create_file(&mig_config.clone().set_custom_path(&staging))?;
        fs::create_dir(&backup).map_err(io_error("creating the reset backup", &backup))?;
        ResetRecord::new(reading, migration).write(&backup)?;
        swap_migration_files(dir, &staging, &backup, |from, to| fs::rename(from, to))
    })();
    match staged {
        Err(error @ MigrationError::ResetIncomplete { .. }) => Err(error),
        Err(error) => {
            // The original files are in place. Only what this reset created goes: the staged
            // files and the record. Left behind, they would block the next reset.
            let removed = [&staging, &backup].into_iter().try_for_each(|created| {
                match fs::remove_dir_all(created) {
                    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                    _ => Ok(()),
                }
            });
            match removed {
                Ok(()) => Err(error),
                Err(_) => Err(MigrationError::ResetIncomplete {
                    backup,
                    staging: Some(staging),
                    source: Some(Box::new(error)),
                }),
            }
        }
        Ok(()) => {
            fs::remove_dir(&staging).map_err(io_error("removing the reset staging", &staging))
        }
    }
}

/// Settles a `reset --run` whose replacement failed with `error`, by reading the history again:
/// unchanged, the replaced files are restored and `error` returned; holding the reset's
/// migration alone, it committed after all, so the backup goes; anything else keeps the backup.
///
/// # Errors
///
/// `error` after a restore, or [`MigrationError::ResetIncomplete`] with it as the cause.
pub(crate) async fn settle_failed_reset(
    db: &Surreal<impl Connection>,
    dir: &Path,
    error: MigrationError,
) -> MigrationResult<()> {
    let backup = reset_backup_dir(dir);
    let incomplete = |source: MigrationError| MigrationError::ResetIncomplete {
        backup: backup.clone(),
        staging: None,
        source: Some(Box::new(source)),
    };
    let record = match ResetRecord::read(&backup) {
        Ok(Some(record)) => record,
        Ok(None) => return Err(error),
        Err(_) => return Err(incomplete(error)),
    };
    match HistoryReading::read(db).await {
        Ok(reading) if record.matches_old(&reading) => match restore(dir, &backup, &record) {
            Ok(()) => Err(error),
            // What is left to do is recover the files, so the restore's failure is the cause;
            // the migration's own failure is reported alongside it.
            Err(restore_error) => {
                log::error!("{error}");
                Err(incomplete(restore_error))
            }
        },
        Ok(reading) if record.matches_replacement(&reading) => {
            log::warn!("{error}; the reset's migration committed nonetheless");
            remove_backup(&backup)
        }
        _ => Err(incomplete(error)),
    }
}
