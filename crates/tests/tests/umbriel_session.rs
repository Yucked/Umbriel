//! `session::*`, `sleep()` and `count()` (`orm/src/functions/{session, sleep, count}.rs`): every
//! function rendered and the values SurrealDB gives in the test session.

use std::time::Duration;

use umbriel::{
    Field, Operatable, Render, count,
    functions::{session, sleep},
    types::Value,
};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (session::ac!(), "session::ac()"),
        (session::db!(), "session::db()"),
        (session::id!(), "session::id()"),
        (session::ip!(), "session::ip()"),
        (session::ns!(), "session::ns()"),
        (session::origin!(), "session::origin()"),
        (session::rd!(), "session::rd()"),
        (session::token!(), "session::token()"),
        (sleep::sleep!(Duration::from_millis(5)), "sleep(5ms)"),
        (count!(), "count()"),
        (count!(Field::new("age").gt(18)), "count(age > 18)"),
        (count!(vec![1, 0]), "count([1, 0])"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(&db, [
        (session::ns!(), "'test'"),
        (session::db!(), "'test'"),
        (session::rd!(), "NONE"),
        (count!(), "1"),
        (count!(vec![1, 0, 1]), "2"),
    ]);
    let _: Value = read(
        &db,
        &format!("RETURN {};", sleep::sleep!(Duration::from_millis(1)).to_debug_sql()),
    )
    .await;
}
