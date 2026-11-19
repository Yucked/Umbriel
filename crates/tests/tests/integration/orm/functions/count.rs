//! `count()` (`orm/src/functions/count.rs`): every form rendered, and the counts SurrealDB
//! computes.

use umbriel::{Field, Operatable, count};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (count!(), "count()"),
        (count!(Field::new("age").gt(18)), "count(age > 18)"),
        (count!(vec![1, 0]), "count([1, 0])"),
    );
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(&db, [(count!(), "1"), (count!(vec![1, 0, 1]), "2")]);
}
