//! `sleep()` (`orm/src/functions/sleep.rs`): rendered, and run by SurrealDB.

use std::time::Duration;

use umbriel::{Render, functions::sleep, types::Value};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!((sleep::sleep!(Duration::from_millis(5)), "sleep(5ms)"));
}

#[tokio::test]
async fn sleep_runs() {
    let (db, _guard) = db().await;
    let _: Value = read(
        &db,
        &format!(
            "RETURN {};",
            sleep::sleep!(Duration::from_millis(1)).to_debug_sql()
        ),
    )
    .await;
}
