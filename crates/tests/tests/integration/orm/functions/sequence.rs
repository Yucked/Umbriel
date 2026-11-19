//! `sequence::*` (`orm/src/functions/sequence.rs`): `sequence::nextval` rendered and drawing
//! increasing values from a defined sequence.

use umbriel::{Param, Runnable, functions::sequence, statements::define_sequence};
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (
            sequence::nextval!("order_number"),
            "sequence::nextval('order_number')"
        ),
        (
            sequence::nextval!(Param::new("name")),
            "sequence::nextval($name)"
        ),
    );
}

#[tokio::test]
async fn values_increase_from_the_start() {
    let (db, _guard) = db().await;
    define_sequence("order_number")
        .start(10)
        .run(&db)
        .await
        .expect("defined");
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn.push(
            bound::<i64, _>(&db, &sequence::nextval!("order_number"))
                .await
                .expect("a value"),
        );
    }
    assert!(drawn.iter().all(|value| *value >= 10), "{drawn:?}");
    assert!(drawn.windows(2).all(|pair| pair[0] < pair[1]), "{drawn:?}");
}

#[tokio::test]
async fn an_undefined_sequence_fails() {
    let (db, _guard) = db().await;
    let error = call_failure(&db, &sequence::nextval!("missing")).await;
    assert!(error.contains("missing"), "{error}");
}
