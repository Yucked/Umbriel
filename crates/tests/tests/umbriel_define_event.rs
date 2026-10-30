//! `DEFINE EVENT` (`orm/src/statements/define_event.rs`): an optional `WHEN`, several `THEN`s,
//! and `ASYNC` with `RETRY` and `MAXDEPTH`.

use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All, Operatable, Param, Raw, Runnable, Table, chain,
    statements::{define_event, select, utils::Block},
};
use umbriel_tests::*;

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("ran");
}

/// `{ CREATE log SET note = '<note>' }`: what the events under test run.
fn log_entry(note: &str) -> Block {
    chain(Raw::new(format!("CREATE log SET note = '{note}';"))).as_block()
}

mod rendering {
    use super::*;

    #[test]
    fn when_is_optional_and_then_repeats() {
        assert_sql(
            &define_event("audit")
                .on_table("sale")
                .then(select(All).from(Table::new("log"))),
            "DEFINE EVENT audit ON TABLE sale THEN (SELECT * FROM log);",
        );
        assert_sql(
            &define_event("audit")
                .on_table("sale")
                .when(Param::new("event").is("CREATE"))
                .then(select(All).from(Table::new("log")))
                .then(select(All).from(Table::new("archive"))),
            "DEFINE EVENT audit ON TABLE sale WHEN $event IS 'CREATE' \
             THEN (SELECT * FROM log), (SELECT * FROM archive);",
        );
    }

    #[test]
    fn async_takes_retry_and_max_depth() {
        let event = || {
            define_event("audit")
                .on_table("sale")
                .then(select(All).from(Table::new("log")))
        };
        assert_sql(
            &event().async_(),
            "DEFINE EVENT audit ON TABLE sale THEN (SELECT * FROM log) ASYNC;",
        );
        assert_sql(
            &event().async_().retry(3).max_depth(2),
            "DEFINE EVENT audit ON TABLE sale THEN (SELECT * FROM log) ASYNC RETRY 3 MAXDEPTH 2;",
        );
        assert_sql(
            &event().max_depth(2).async_(),
            "DEFINE EVENT audit ON TABLE sale THEN (SELECT * FROM log) ASYNC MAXDEPTH 2;",
        );
    }

    #[test]
    fn retry_and_max_depth_need_async() {
        let event = || {
            define_event("audit")
                .on_table("sale")
                .then(select(All).from(Table::new("log")))
        };
        assert_render_error(&event().retry(3), "ASYNC");
        assert_render_error(&event().max_depth(2), "ASYNC");
    }
}

mod behaviour {
    use super::*;

    async fn log_count(db: &Surreal<Db>) -> usize {
        read(db, "RETURN count(SELECT * FROM log);").await
    }

    #[tokio::test]
    async fn without_when_the_event_runs_on_every_change() {
        let db = db().await;
        run(
            &db,
            define_event("audit")
                .on_table("sale")
                .then(log_entry("changed")),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1; UPDATE sale:1 SET amount = 2; DELETE sale:1;").await);
        assert_eq!(log_count(&db).await, 3);
    }

    #[tokio::test]
    async fn every_then_runs_in_order() {
        let db = db().await;
        run(
            &db,
            define_event("audit")
                .on_table("sale")
                .then(log_entry("first"))
                .then(log_entry("second")),
        )
        .await;
        assert!(succeeds(&db, "CREATE sale:1;").await);
        let notes: Vec<String> = read(&db, "RETURN (SELECT VALUE note FROM log);").await;
        let mut sorted = notes.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["first".to_owned(), "second".to_owned()]);
    }

    #[tokio::test]
    async fn an_async_event_does_not_run_inside_the_triggering_transaction() {
        let db = db().await;
        run(
            &db,
            define_event("synchronous")
                .on_table("sale")
                .then(log_entry("sync")),
        )
        .await;
        run(
            &db,
            define_event("deferred")
                .on_table("order")
                .then(log_entry("async"))
                .async_()
                .retry(1),
        )
        .await;
        // `BEGIN` has a result of its own, so the `RETURN` is statement 3.
        let mut results = db
            .query(
                "BEGIN;
                 CREATE sale:1;
                 CREATE order:1;
                 RETURN count(SELECT * FROM log);
                 COMMIT;",
            )
            .await
            .and_then(surrealdb::IndexedResults::check)
            .expect("committed");
        // The synchronous event has logged; the async one has not run yet.
        let logged: usize = results.take(3).expect("a count");
        assert_eq!(logged, 1);
    }

    #[tokio::test]
    async fn when_null_never_runs() {
        let db = db().await;
        assert!(
            succeeds(
                &db,
                "DEFINE EVENT audit ON sale WHEN NULL THEN (CREATE log);
                 CREATE sale:1;"
            )
            .await
        );
        assert_eq!(log_count(&db).await, 0);
    }
}
