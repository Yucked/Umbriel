//! Applying and rolling back migrations (`orm/src/migrations/database/db_runner.rs`): each
//! migration commits with its history record on its own, failures stop the run, and concurrent
//! runners leave a consistent history.

use std::{fs, path::Path};

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{DbResources, Raw, migrations::*};
use umbriel_tests::{db, migrations::helpers::*};

mod up {
    use super::*;
    use pretty_assertions::assert_eq;
    use umbriel::test_support::{apply_after_reading, lock_history, read_history};

    #[tokio::test]
    async fn a_failing_migration_stops_the_run() {
        // Each migration commits with its history record on its own: a failing one stops the
        // run, keeps those before it, leaves nothing of itself, and a retry resumes with it.
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
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
            .run_up_pending_migrations(&db, UpdateStrategy::Latest)
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

    /// One runner's attempt to apply the pending migrations in `dir` to `db`.
    async fn run_once(dir: &Path, db: &Surreal<Any>) -> MigrationResult<()> {
        config(dir)
            .two_way()
            .run_up_pending_migrations(db, UpdateStrategy::Latest)
            .await
    }

    #[derive(Debug, Clone)]
    struct Readings;

    impl DbResources for Readings {
        fn tables(&self) -> Vec<Raw> {
            vec![Raw::new("DEFINE TABLE reading;")]
        }
    }

    /// Regression: a runner that read the history before another runner applied a generated
    /// first migration must not apply it again. That migration clears the history before
    /// recording itself, so its record id alone cannot refuse the second application.
    #[tokio::test]
    async fn a_stale_runner_does_not_reapply_a_generated_first_migration() {
        let (_temp, dir) = migration_dir();
        let file_manager = config(&dir).set_flag(MigrationFlag::TwoWay);
        let (comparison, _namespace) = umbriel_tests::comparison().await;
        let migration = MigratorDatabase::first_migration(
            &comparison,
            &"readings".into(),
            &file_manager,
            Readings,
            MockPrompter::default(),
        )
        .await
        .expect("the first migration generates");
        fs::create_dir_all(&dir).expect("the migration directory is created");
        migration
            .create_file(&file_manager)
            .expect("the migration is written");
        let (up, _) = newest_migration(&dir);
        assert!(up.contains("DELETE migration;"), "{up}");

        let (db, _guard) = db().await;
        // The stale runner read the history while it was still empty.
        let stale = read_history(&db).await.expect("the history is read");
        apply_up(&dir, &db).await;
        let history = applied(&db).await;
        assert_eq!(history.len(), 1);

        let migrations = config(&dir)
            .two_way()
            .get_migrations()
            .expect("the migrations are read");
        let result = apply_after_reading(&db, migrations, &stale).await;
        assert!(
            matches!(&result, Err(MigrationError::HistoryChanged)),
            "{result:?}"
        );
        assert_eq!(applied(&db).await, history, "the history is unchanged");
    }

    /// Pins the limit of recovery: bucket writes are not part of the transaction, so a failed
    /// migration rolls back its database changes but leaves its file.
    #[tokio::test]
    async fn a_failed_migration_leaves_its_file_writes() {
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        assert!(
            umbriel_tests::succeeds(&db, "DEFINE BUCKET avatars BACKEND 'memory';").await,
            "the bucket is defined"
        );
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        write_numbered(
            &dir,
            2,
            "DEFINE TABLE second;\nfile::put(f\"avatars:/alice.txt\", 'Alice');\n\
             THROW 'the second migration fails';",
            "REMOVE TABLE second;",
        );

        let failed = run_once(&dir, &db).await;
        assert!(failed.is_err(), "{failed:?}");
        assert_eq!(applied(&db).await, std::slice::from_ref(&first));
        assert_eq!(
            tables(&db).await,
            ["first", "migration"],
            "the table is rolled back"
        );
        let kept: bool =
            umbriel_tests::read(&db, "RETURN file::exists(f\"avatars:/alice.txt\");").await;
        assert!(kept, "the file write is not rolled back");
    }

    #[tokio::test]
    async fn racing_runners_leave_a_consistent_history() {
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        // A random record id, so applying the migration twice would leave two records.
        let second = write_numbered(
            &dir,
            2,
            "DEFINE TABLE second;\nCREATE second SET name = 'Alice';",
            "REMOVE TABLE second;",
        );

        // Whether the two runs overlap depends on scheduling; the stale-runner test above pins
        // the refusal deterministically. Either way, the history and data end consistent.
        let (left, right) = tokio::join!(run_once(&dir, &db), run_once(&dir, &db));
        assert!(left.is_ok() || right.is_ok(), "{left:?}\n{right:?}");
        assert_eq!(applied(&db).await, [first.clone(), second.clone()]);
        let written: i64 = umbriel_tests::read(&db, "RETURN count(SELECT * FROM second);").await;
        assert_eq!(written, 1, "the second migration committed once");

        // Whichever runner lost, running again finds nothing pending.
        run_once(&dir, &db).await.expect("nothing is pending");
        assert_eq!(applied(&db).await, [first, second]);
    }

    #[tokio::test]
    async fn connecting_applies_no_schema() {
        let (_temp, dir) = migration_dir();
        write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        let (db, _guard) = db().await;
        assert!(tables(&db).await.is_empty(), "connecting defines nothing");
        let pending = config(&dir)
            .two_way()
            .list_migrations(&db, Status::Pending, Mode::Strict)
            .await
            .expect("listed");
        assert_eq!(pending.len(), 1);
        assert!(tables(&db).await.is_empty(), "listing defines nothing");
    }

    /// `dir`'s pending migrations applied to `db` in `mode`.
    async fn up_in(dir: &Path, db: &Surreal<Any>, mode: Mode) -> MigrationResult<()> {
        config(dir)
            .set_mode(mode)
            .two_way()
            .run_up_pending_migrations(db, UpdateStrategy::Latest)
            .await
    }

    /// The base names `names` (up file names) have.
    fn basenames(names: &[String]) -> Vec<Basename> {
        names
            .iter()
            .map(|name| {
                MigrationFilename::try_from(name.clone())
                    .expect("a migration file name")
                    .basename()
            })
            .collect()
    }

    /// Regression (UMB-007): a file merged with a timestamp older than the newest applied one
    /// was never applied, listed or pruned. It is pending, and `up` refuses it in both modes, so
    /// the history's file order stays its application order; given a newer timestamp, it applies
    /// and rolls back like any other.
    #[tokio::test]
    async fn an_out_of_order_file_is_pending_and_refused_in_both_modes() {
        for mode in [Mode::Strict, Mode::Lax] {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
            let third = write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");
            apply_up(&dir, &db).await;
            let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");

            let pending = config(&dir)
                .two_way()
                .list_migrations(&db, Status::Pending, mode)
                .await
                .expect("listed");
            assert_eq!(
                pending.iter().map(ToString::to_string).collect::<Vec<_>>(),
                std::slice::from_ref(&second),
                "{mode:?}: the older file is pending"
            );

            let error = up_in(&dir, &db, mode).await.expect_err("refused");
            assert!(
                matches!(
                    &error,
                    MigrationError::OutOfOrder { name, .. }
                        if name.basename() == Basename::from("step_2")
                ),
                "{mode:?}: {error:?}"
            );
            assert_eq!(
                applied(&db).await,
                [first.clone(), third.clone()],
                "{mode:?}"
            );
            assert_eq!(
                tables(&db).await,
                ["first", "migration", "third"],
                "{mode:?}"
            );

            // Renamed after the newest applied migration, it applies, and rolls back first.
            let renamed = "20240101000000004_step_2";
            fs::rename(dir.join(&second), dir.join(format!("{renamed}.up.surql")))
                .expect("renamed");
            fs::rename(
                dir.join(second.replace(".up.", ".down.")),
                dir.join(format!("{renamed}.down.surql")),
            )
            .expect("renamed");
            up_in(&dir, &db, mode)
                .await
                .expect("the renamed file applies");
            assert_eq!(
                applied(&db).await,
                [first.clone(), third.clone(), format!("{renamed}.up.surql")],
                "{mode:?}"
            );
            roll_back(&dir, &db).await;
            assert_eq!(applied(&db).await, [first.clone(), third], "{mode:?}");
            roll_back(&dir, &db).await;
            assert_eq!(applied(&db).await, [first], "{mode:?}");
        }
    }

    /// Two files with one timestamp both apply, ordered by their names; a runner that read the
    /// history between them applies nothing more.
    #[tokio::test]
    async fn files_with_one_timestamp_both_apply_and_a_stale_reading_is_refused() {
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        let twin = "20240101000000001_step_1b";
        fs::write(dir.join(format!("{twin}.up.surql")), "DEFINE TABLE twin;").expect("written");
        fs::write(dir.join(format!("{twin}.down.surql")), "REMOVE TABLE twin;").expect("written");

        config(&dir)
            .two_way()
            .run_up_pending_migrations(&db, UpdateStrategy::Number(1))
            .await
            .expect("the first applies");
        let stale = read_history(&db).await.expect("the history is read");
        apply_up(&dir, &db).await;
        let history = applied(&db).await;
        assert_eq!(
            basenames(&history),
            [Basename::from("step_1"), Basename::from("step_1b")]
        );
        assert!(history.contains(&first), "{history:?}");

        write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
        let migrations = config(&dir)
            .two_way()
            .get_migrations()
            .expect("the migrations are read");
        let result = apply_after_reading(&db, migrations, &stale).await;
        assert!(
            matches!(&result, Err(MigrationError::HistoryChanged)),
            "{result:?}"
        );
        assert_eq!(applied(&db).await, history, "the history is unchanged");
    }

    /// Regression (UMB-008): `up` checked no checksums. Strict `up` refuses an edited applied
    /// file before applying anything; lax `up` applies what is pending.
    #[tokio::test]
    async fn an_edited_applied_file_fails_strict_up_only() {
        for mode in [Mode::Strict, Mode::Lax] {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
            let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
            apply_up(&dir, &db).await;
            write_numbered(&dir, 2, "DEFINE TABLE edited;", "REMOVE TABLE edited;");
            let third = write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");

            let result = up_in(&dir, &db, mode).await;
            match mode {
                Mode::Strict => {
                    assert!(
                        matches!(&result, Err(MigrationError::ChecksumMismatch { .. })),
                        "{result:?}"
                    );
                    assert_eq!(applied(&db).await, [first, second]);
                }
                Mode::Lax => {
                    result.expect("lax up proceeds");
                    assert_eq!(applied(&db).await, [first, second, third]);
                }
            }
        }
    }

    /// Strict `up` refuses an applied migration whose files are gone; lax `up` applies what is
    /// pending.
    #[tokio::test]
    async fn a_deleted_applied_file_fails_strict_up_only() {
        for mode in [Mode::Strict, Mode::Lax] {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
            let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
            apply_up(&dir, &db).await;
            fs::remove_file(dir.join(&second)).expect("removed");
            fs::remove_file(dir.join(second.replace(".up.", ".down."))).expect("removed");
            let third = write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");

            let result = up_in(&dir, &db, mode).await;
            match mode {
                Mode::Strict => {
                    assert!(
                        matches!(&result, Err(MigrationError::AppliedFileMissing { .. })),
                        "{result:?}"
                    );
                    assert_eq!(applied(&db).await, [first, second]);
                }
                Mode::Lax => {
                    result.expect("lax up proceeds");
                    assert_eq!(applied(&db).await, [first, second, third]);
                }
            }
        }
    }

    /// Snapshot isolation lets two overlapping transactions that write different history records
    /// both commit. Every history change also writes the one revision record, so they conflict:
    /// the rollback commits while the migration's transaction sleeps, and the migration fails.
    #[tokio::test]
    async fn an_up_overlapping_a_rollback_is_refused() {
        let (_temp, dir) = migration_dir();
        let (db, guard) = db().await;
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
        apply_up(&dir, &db).await;
        write_numbered(
            &dir,
            3,
            "sleep(2s);\nDEFINE TABLE third;",
            "REMOVE TABLE third;",
        );

        let other = umbriel_tests::connect(umbriel_tests::Endpoint::Main).await;
        other.use_ns(guard.namespace()).await.expect("selected");
        other.use_db(guard.database()).await.expect("selected");
        let (up, down) = tokio::join!(run_once(&dir, &db), async {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            config(&dir)
                .two_way()
                .run_down_migrations(&other, RollbackOptions::default())
                .await
        });

        down.expect("the rollback commits first");
        assert!(
            matches!(&up, Err(MigrationError::HistoryChanged)),
            "{up:?}"
        );
        assert_eq!(applied(&db).await, [first]);
        assert_eq!(tables(&db).await, ["first", "migration"]);
    }

    /// A prune holds the history's lock while it deletes files; `up` applies nothing meanwhile.
    #[tokio::test]
    async fn a_locked_history_refuses_up() {
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        apply_up(&dir, &db).await;
        write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");

        lock_history(&db, "prune-1")
            .await
            .expect("the history is locked");
        let error = run_once(&dir, &db).await.expect_err("refused");
        assert!(
            matches!(&error, MigrationError::HistoryLocked { holder } if holder == "prune-1"),
            "{error:?}"
        );
        assert_eq!(applied(&db).await, [first]);
    }
}

mod down {
    use super::*;
    use pretty_assertions::assert_eq;
    use umbriel::migrations::RollbackStrategy;

    #[tokio::test]
    async fn a_failing_rollback_stops_and_keeps_its_migration() {
        // Each rollback commits with its history deletion on its own, most recent first: a
        // failing one stops the rollback, keeps the migrations before it applied, and a retry
        // resumes with it.
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
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
                &db,
                RollbackOptions::default().strategy(RollbackStrategy::Number(3)),
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
                &db,
                RollbackOptions::default()
                    .strategy(RollbackStrategy::Number(2))
                    .mode(Mode::Lax),
            )
            .await
            .expect("the retry rolls back the rest");
        assert!(applied(&db).await.is_empty());
        assert_eq!(tables(&db).await, ["migration"]);
    }

    /// `n` rollbacks in `mode`.
    async fn down_in(dir: &Path, db: &Surreal<Any>, n: u32, mode: Mode) -> MigrationResult<()> {
        config(dir)
            .two_way()
            .run_down_migrations(
                db,
                RollbackOptions::default()
                    .strategy(RollbackStrategy::Number(n))
                    .mode(mode),
            )
            .await
    }

    /// The second migration's files are gone, which lax mode allows. A history record is never
    /// removed without its down migration, so the rollback stops at it and changes nothing: the
    /// first migration, which it depends on, stays applied.
    #[tokio::test]
    async fn a_lax_rollback_stops_at_a_migration_without_files() {
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        let first = write_numbered(
            &dir,
            1,
            "DEFINE TABLE first;",
            "REMOVE TABLE first;\nREMOVE TABLE migration;",
        );
        let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
        apply_up(&dir, &db).await;
        fs::remove_file(dir.join(&second)).expect("removed");
        fs::remove_file(dir.join(second.replace(".up.", ".down."))).expect("removed");

        let error = down_in(&dir, &db, 2, Mode::Lax)
            .await
            .expect_err("the rollback stops");
        assert!(
            matches!(
                &error,
                MigrationError::RollbackStopped { at, rolled_back: 0 }
                    if at.basename() == Basename::from("step_2")
            ),
            "{error:?}"
        );
        assert_eq!(applied(&db).await, [first, second]);
        assert_eq!(tables(&db).await, ["first", "migration", "second"]);
    }

    /// Regression (UMB-009): rollback paired files with the history by position. With the middle
    /// migration's down file missing, strict rollback checks every selected migration before
    /// changing anything; lax rollback reverses only the newest run it can and stops, so the
    /// older migrations a newer one depends on stay applied.
    #[tokio::test]
    async fn a_missing_middle_down_file_stops_the_rollback() {
        for mode in [Mode::Strict, Mode::Lax] {
            let (_temp, dir) = migration_dir();
            let (db, _guard) = db().await;
            let first = write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
            let second = write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");
            let third = write_numbered(&dir, 3, "DEFINE TABLE third;", "REMOVE TABLE third;");
            apply_up(&dir, &db).await;
            fs::remove_file(dir.join(second.replace(".up.", ".down."))).expect("removed");

            let error = down_in(&dir, &db, 3, mode).await.expect_err("refused");
            match mode {
                Mode::Strict => {
                    assert!(
                        matches!(&error, MigrationError::AppliedFileMissing { .. }),
                        "{error:?}"
                    );
                    assert_eq!(applied(&db).await, [first, second, third]);
                    assert_eq!(tables(&db).await, ["first", "migration", "second", "third"]);
                }
                Mode::Lax => {
                    assert!(
                        matches!(
                            &error,
                            MigrationError::RollbackStopped { at, rolled_back: 1 }
                                if at.basename() == Basename::from("step_2")
                        ),
                        "{error:?}"
                    );
                    assert_eq!(applied(&db).await, [first, second]);
                    assert_eq!(tables(&db).await, ["first", "migration", "second"]);
                }
            }
        }
    }
}

mod prune {
    use super::*;
    use pretty_assertions::assert_eq;
    use umbriel::test_support::{prune_confirmed, read_history};

    /// A prune deletes the files it listed only if the history is still the one it read when the
    /// user confirmed: here another runner applied one of them in between, so nothing is deleted.
    #[tokio::test]
    async fn a_selection_applied_after_confirmation_deletes_nothing() {
        let (_temp, dir) = migration_dir();
        let (db, _guard) = db().await;
        write_numbered(&dir, 1, "DEFINE TABLE first;", "REMOVE TABLE first;");
        config(&dir)
            .two_way()
            .run_up_pending_migrations(&db, UpdateStrategy::Number(1))
            .await
            .expect("the first applies");
        write_numbered(&dir, 2, "DEFINE TABLE second;", "REMOVE TABLE second;");

        let reading = read_history(&db).await.expect("the history is read");
        let selection = MigrationRunner::get_pending_migration_filenames(&db, &config(&dir))
            .await
            .expect("the pending files are listed")
            .into_vec();
        assert_eq!(selection.len(), 2, "{selection:?}");
        apply_up(&dir, &db).await;
        let files = migration_files(&dir);

        let error = prune_confirmed(&db, &config(&dir), &reading, &selection)
            .await
            .expect_err("the selection is stale");
        assert!(
            matches!(&error, MigrationError::HistoryChanged),
            "{error:?}"
        );
        assert_eq!(migration_files(&dir), files, "nothing is deleted");
    }
}
