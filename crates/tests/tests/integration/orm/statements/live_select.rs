//! `LIVE SELECT` (`orm/src/statements/live_select.rs`, #20): rendering within the grammar,
//! typed and `DIFF` notifications, and the lifecycle of a subscription: kill, drop, session end,
//! transports and resubscription.

use std::time::Duration;

use futures::StreamExt;
use surrealdb::{Connection, opt::PatchOp, types::Action};
use umbriel::{
    All, Error, Field, Operatable, Raw, Render, Runnable, SurrealCrudNode, Table,
    statements::{LiveEvent, LiveStream, kill, live, select},
    test_support::{decode_patches, live_event},
    types::Value,
};
use umbriel_tests::{support::statements::space_ship, *};

/// The stream's next item, within [`WAIT`].
async fn next<O, C: Connection>(
    stream: &mut LiveStream<O, C>,
) -> Option<umbriel::Result<LiveEvent<O>>> {
    tokio::time::timeout(WAIT, stream.next())
        .await
        .expect("an item within WAIT")
}

/// Whether nothing arrives for a while. Live notifications follow the write that causes them
/// within milliseconds; half a second without one means none is coming.
async fn quiet<O, C: Connection>(stream: &mut LiveStream<O, C>) -> bool {
    tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .is_err()
}

async fn ran(db: &surrealdb::Surreal<surrealdb::engine::any::Any>, sql: &str) {
    Raw::new(sql).run(db).await.expect("ran");
}

/// How many live queries `table` holds, as `INFO FOR TABLE` lists them under `lives`.
async fn lives(db: &surrealdb::Surreal<surrealdb::engine::any::Any>, table: &str) -> i64 {
    read(
        db,
        &format!("RETURN object::len((INFO FOR TABLE {table}).lives);"),
    )
    .await
}

mod rendering {
    use super::*;

    /// L8: `WHERE` and `FETCH` render; every clause outside the LIVE grammar is a render error.
    #[test]
    fn live_takes_only_where_and_fetch() {
        let person = Table::from("person");
        let statement = live(
            select(All)
                .from(person.clone())
                .where_(Field::new("age").greater_than(18))
                .fetch(Field::new("team")),
        );
        assert_sql(
            &statement,
            "LIVE SELECT * FROM person WHERE age > 18 FETCH team;",
        );
        assert_sql(
            &live(select(All).from(person.clone())).diff(),
            "LIVE SELECT DIFF FROM person;",
        );

        let refused = [
            (
                live(select(All).from(person.clone()).limit(1)).to_query(),
                "LIMIT",
            ),
            (
                live(select(All).from(person.clone()).order_rand()).to_query(),
                "ORDER BY",
            ),
            (
                live(select(All).from(person.clone()).group_all()).to_query(),
                "GROUP",
            ),
            (
                live(select(All).from_only(person.clone())).to_query(),
                "ONLY",
            ),
            (
                live(
                    select(All)
                        .from(person.clone())
                        .timeout(Duration::from_secs(1)),
                )
                .to_query(),
                "TIMEOUT",
            ),
        ];
        for (rendered, clause) in refused {
            let error = rendered.expect_err("refused");
            assert!(error.to_string().contains(clause), "{error}");
        }
        assert_render_error(
            &live(select(Field::new("name")).from(person)).diff(),
            "must be `*`",
        );
    }
}

mod notifications {
    use super::*;

    /// L7: CREATE, UPDATE and DELETE notifications decode into the select's type.
    #[tokio::test]
    async fn notifications_decode_into_the_projection() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE space_ship;").await;
        let mut ships = live(
            select(All)
                .from(Table::from("space_ship"))
                .decode::<SpaceShip>(),
        )
        .subscribe(&db)
        .await
        .expect("subscribed");

        space_ship("apollo", "Apollo")
            .create(&db)
            .await
            .expect("created");
        ran(&db, "UPDATE space_ship:apollo SET name = 'Apollo 11';").await;
        ran(&db, "DELETE space_ship:apollo;").await;

        assert!(matches!(
            next(&mut ships).await,
            Some(Ok(LiveEvent::Create(ship))) if ship.name == "Apollo"
        ));
        assert!(matches!(
            next(&mut ships).await,
            Some(Ok(LiveEvent::Update(ship))) if ship.name == "Apollo 11"
        ));
        assert!(matches!(
            next(&mut ships).await,
            Some(Ok(LiveEvent::Delete(ship))) if ship.name == "Apollo 11"
        ));
    }

    /// L9: `DIFF` notifications decode to the exact operations SurrealDB 3.3.4's `Value::diff`
    /// (`surrealdb-expr` `val/value/diff.rs`) writes: a `replace` of the whole record (path `""`)
    /// for CREATE and DELETE; for an UPDATE, the removed keys first, then the new and changed
    /// keys in key order, a changed string as a text `change`.
    #[tokio::test]
    async fn diff_notifications_are_exact_operations() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE doc;").await;
        let mut diffs = live(select(All).from(Table::from("doc")))
            .diff()
            .subscribe(&db)
            .await
            .expect("subscribed");

        ran(&db, "CREATE doc:one SET name = 'a', count = 1, old = true;").await;
        // SurrealDB 3.3.4 doesn't parse `SET` and `UNSET` in one `UPDATE`; `NONE` removes `old`.
        ran(
            &db,
            "UPDATE doc:one MERGE { count: 2, extra: 'x', old: NONE };",
        )
        .await;
        ran(&db, "UPDATE doc:one SET name = 'b';").await;
        ran(&db, "DELETE doc:one;").await;

        let Some(Ok(LiveEvent::Create(created))) = next(&mut diffs).await else {
            panic!("a CREATE");
        };
        assert!(
            matches!(created.as_slice(), [PatchOp::Replace { path, value: Value::Object(_) }] if path.is_empty()),
            "{created:?}"
        );

        let Some(Ok(LiveEvent::Update(updated))) = next(&mut diffs).await else {
            panic!("an UPDATE");
        };
        assert!(
            matches!(
                updated.as_slice(),
                [
                    PatchOp::Remove { path: removed },
                    PatchOp::Replace { path: replaced, value: Value::Number(_) },
                    PatchOp::Add { path: added, value: Value::String(extra) },
                ] if removed == "/old" && replaced == "/count" && added == "/extra" && extra == "x"
            ),
            "{updated:?}"
        );

        let Some(Ok(LiveEvent::Update(renamed))) = next(&mut diffs).await else {
            panic!("an UPDATE");
        };
        assert!(
            matches!(
                renamed.as_slice(),
                [PatchOp::Change { path, value }]
                    if path == "/name" && value.starts_with("@@") && value.contains("-a") && value.contains("+b")
            ),
            "{renamed:?}"
        );

        let Some(Ok(LiveEvent::Delete(deleted))) = next(&mut diffs).await else {
            panic!("a DELETE");
        };
        assert!(
            matches!(deleted.as_slice(), [PatchOp::Replace { path, value: Value::None }] if path.is_empty()),
            "{deleted:?}"
        );
    }

    /// L10: `Action::Error` is reported with its message, never decoded; an undecodable record
    /// is `Conversion`; `Killed` ends the stream. SurrealDB 3.3.4 never sends `Action::Error`
    /// (`doc/lives.rs`), so the decoding is tested directly.
    #[test]
    fn errors_and_kills_are_not_decoded_as_records() {
        let message = Value::String("evaluation failed".to_owned());
        assert!(matches!(
            live_event::<i64>(Action::Error, message),
            Some(Ok(LiveEvent::Error(Value::String(text)))) if text == "evaluation failed"
        ));
        assert!(matches!(
            live_event::<i64>(Action::Create, Value::String("one".to_owned())),
            Some(Err(Error::Conversion(_)))
        ));
        assert!(live_event::<i64>(Action::Killed, Value::None).is_none());

        for (payload, why) in [
            (
                "[{ op: 'copy', path: '/a', from: '/b' }]",
                "an op DIFF doesn't carry",
            ),
            ("[{ path: '/a', value: 1 }]", "no op"),
            (
                "[{ op: 'change', path: '/a', value: 1 }]",
                "a change that isn't text",
            ),
            ("[{ op: 'add', path: '/a' }]", "an add without a value"),
            ("{ op: 'add', path: '/a', value: 1 }", "not an array"),
        ] {
            let value = umbriel::parse::value(payload).expect("parses");
            assert!(
                matches!(decode_patches(value), Err(Error::Conversion(_))),
                "{why}"
            );
        }
    }
}

mod lifecycle {
    use super::*;

    /// L11: `kill` runs `KILL`; the table holds no live query afterwards.
    #[tokio::test]
    async fn kill_ends_the_live_query() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let stream = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        assert_eq!(lives(&db, "note").await, 1);
        stream.kill().await.expect("killed");
        assert_eq!(lives(&db, "note").await, 0, "the query is gone");
    }

    /// L12: dropping a stream ends its live query on the server, leaves other subscriptions
    /// working, and surfaces nothing. The test never issues a `KILL`: the stream's drop sends it
    /// from a task of its own, so the table's live queries are counted until only the watcher's
    /// remains. The SDK's own drop `KILL` runs in a new, empty session and ends nothing.
    #[tokio::test]
    async fn dropping_a_stream_leaves_the_others() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let dropped = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        let mut watcher = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        assert_eq!(lives(&db, "note").await, 2);
        drop(dropped);
        tokio::time::timeout(WAIT, async {
            while lives(&db, "note").await != 1 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("the dropped query is killed within WAIT");
        ran(&db, "CREATE note:one;").await;
        assert!(matches!(
            next(&mut watcher).await,
            Some(Ok(LiveEvent::Create(_)))
        ));
        assert!(quiet(&mut watcher).await, "one write, one notification");
    }

    /// L13: the stream subscribes through a session of its own, so invalidating the handle it was
    /// given leaves the query registered and delivering. Writes go through another connection.
    #[tokio::test]
    async fn invalidating_the_subscribing_handle_keeps_the_query() {
        let (db, guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let session = connect(Endpoint::Main).await;
        bounded("selecting the test database", async {
            session
                .use_ns(guard.namespace())
                .use_db(guard.database())
                .await
        })
        .await;
        let mut stream = live(select(All).from(Table::from("note")))
            .subscribe(&session)
            .await
            .expect("subscribed");
        session.invalidate().await.expect("invalidated");
        assert_eq!(lives(&db, "note").await, 1, "the query is registered");
        ran(&db, "CREATE note:one;").await;
        assert!(matches!(
            next(&mut stream).await,
            Some(Ok(LiveEvent::Create(_)))
        ));
    }

    /// L13b: dropping the handle the stream was given ends that handle's session, not the
    /// stream's. The write goes through another handle of the same connection, so the SDK sends
    /// the dropped session's detach before it.
    #[tokio::test]
    async fn dropping_the_subscribing_handle_keeps_the_query() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let subscriber = db.clone();
        let mut stream = live(select(All).from(Table::from("note")))
            .subscribe(&subscriber)
            .await
            .expect("subscribed");
        drop(subscriber);
        ran(&db, "CREATE note:one;").await;
        assert!(matches!(
            next(&mut stream).await,
            Some(Ok(LiveEvent::Create(_)))
        ));
        assert_eq!(lives(&db, "note").await, 1, "the query is registered");
    }

    /// L14: the HTTP engine has no live queries.
    #[tokio::test]
    async fn http_refuses_live_queries() {
        let (db, _guard) = http_db().await;
        let refused = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await;
        assert!(refused.is_err(), "HTTP has no live queries");
    }
}

mod resubscription {
    use super::*;

    /// L16a: on a healthy subscription the id changes, one write gives one notification, and the
    /// table holds only the new query.
    #[tokio::test]
    async fn resubscribing_replaces_the_query() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let mut stream = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        let old = stream.query_id();
        stream.resubscribe().await.expect("resubscribed");
        assert_ne!(stream.query_id(), old);
        ran(&db, "CREATE note:one;").await;
        assert!(matches!(
            next(&mut stream).await,
            Some(Ok(LiveEvent::Create(_)))
        ));
        assert!(quiet(&mut stream).await, "only the new query delivers");
        assert_eq!(lives(&db, "note").await, 1, "the old query is gone");
    }

    /// L16b: two resubscriptions in a row; each gives a new id, and only the last delivers.
    #[tokio::test]
    async fn resubscribing_twice_keeps_one_query() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let mut stream = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        let first = stream.query_id();
        stream.resubscribe().await.expect("resubscribed");
        let second = stream.query_id();
        stream.resubscribe().await.expect("resubscribed");
        assert!(first != second && second != stream.query_id() && first != stream.query_id());
        assert_eq!(lives(&db, "note").await, 1, "one query is registered");
        ran(&db, "CREATE note:one;").await;
        assert!(matches!(
            next(&mut stream).await,
            Some(Ok(LiveEvent::Create(_)))
        ));
        assert!(quiet(&mut stream).await);
    }

    /// L16c: when registering fails, the stream keeps its query.
    #[tokio::test]
    async fn a_failed_registration_keeps_the_old_query() {
        let (db, _guard) = strict_db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let mut stream = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        let old = stream.query_id();
        ran(&db, "REMOVE TABLE note;").await;
        assert!(
            stream.resubscribe().await.is_err(),
            "a STRICT database refuses a live query on a missing table"
        );
        assert_eq!(stream.query_id(), old);
    }

    /// L16d: when the old query is already gone, as after a reconnect, resubscribing succeeds:
    /// SurrealDB 3.3.4 accepts a `KILL` of an id it no longer holds. The stream delivers from
    /// the new query.
    #[tokio::test]
    async fn resubscribing_after_the_query_is_gone() {
        let (db, _guard) = db().await;
        ran(&db, "DEFINE TABLE note;").await;
        let mut stream = live(select(All).from(Table::from("note")))
            .subscribe(&db)
            .await
            .expect("subscribed");
        let old = stream.query_id();
        kill(old)
            .run(&db)
            .await
            .expect("killed behind the stream's back");
        assert_eq!(lives(&db, "note").await, 0);
        stream.resubscribe().await.expect("resubscribed");
        assert_ne!(stream.query_id(), old);
        assert_eq!(lives(&db, "note").await, 1);
        ran(&db, "CREATE note:one;").await;
        assert!(matches!(
            next(&mut stream).await,
            Some(Ok(LiveEvent::Create(_)))
        ));
    }
}
