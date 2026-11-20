//! `count()` (`orm/src/functions/count.rs`): every form run, and the counts SurrealDB
//! computes.

use umbriel::{Field, Operatable, count};
use umbriel_tests::*;

/// The form no server test calls.
#[test]
fn rendering() {
    calls!((count!(Field::new("age").gt(18)), "count(age > 18)"));
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(&db, [(count!(), "1"), (count!(vec![1, 0, 1]), "2")]);
}
