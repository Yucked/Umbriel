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
    use umbriel::test_support::apply_after_reading;

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
        let stale: Option<Migration> = None;
        apply_up(&dir, &db).await;
        let history = applied(&db).await;
        assert_eq!(history.len(), 1);

        let migrations = config(&dir)
            .two_way()
            .get_migrations()
            .expect("the migrations are read");
        let result = apply_after_reading(&db, migrations, stale.as_ref()).await;
        assert!(
            matches!(&result, Err(MigrationError::HistoryChanged { .. })),
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
        assert_eq!(tables(&db).await, ["first", "migration"], "the table is rolled back");
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
        let written: i64 =
            umbriel_tests::read(&db, "RETURN count(SELECT * FROM second);").await;
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
                &db,
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
        let (db, _guard) = db().await;
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
                &db,
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
}
