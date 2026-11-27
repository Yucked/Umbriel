//! `duration::*` (`orm/src/functions/duration.rs`): every function and method form run, and
//! the results SurrealDB computes.

use std::time::Duration;

use umbriel::{DurationLike, Field, Param, Render, functions::duration};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    let week_and_a_day = Duration::from_secs(8 * 86_400 + 90);
    holds_all!(
        &db,
        [
            (duration::days(week_and_a_day), "8"),
            (duration::hours(week_and_a_day), "192"),
            (duration::mins(week_and_a_day), "11521"),
            (duration::secs(week_and_a_day), "691290"),
            (duration::millis(Duration::from_millis(1500)), "1500"),
            (duration::micros(Duration::from_millis(2)), "2000"),
            (duration::nanos(Duration::from_micros(3)), "3000"),
            (duration::weeks(week_and_a_day), "1"),
            (duration::years(Duration::from_secs(400 * 86_400)), "1"),
            (duration::from_days(2), "2d"),
            (duration::from_hours(3), "3h"),
            (duration::from_micros(4), "4us"),
            (duration::from_millis(5), "5ms"),
            (duration::from_mins(6), "6m"),
            (duration::from_nanos(7), "7ns"),
            (duration::from_secs(8), "8s"),
            (duration::from_weeks(9), "9w"),
            (DurationLike::from(week_and_a_day).days(), "8"),
        ]
    );
}

#[test]
fn generated_bindings() {
    let nested = duration::days(duration::from_hours(Param::new("hours")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "duration::days(duration::from_hours($hours))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(
        &db,
        &duration::days(DurationLike::from(Field::from_expr(
            umbriel::ValueLike::from("long")
        ))),
    )
    .await;
    assert!(error.contains("duration::days"), "{error}");
}

/// Every mapped method runs on a duration and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let long = Duration::from_secs(400 * 86_400 + 3_661);
    let d = || DurationLike::from(long);
    same_all!(
        &db,
        [
            (d().days(), duration::days(long)),
            (d().hours(), duration::hours(long)),
            (d().micros(), duration::micros(long)),
            (d().millis(), duration::millis(long)),
            (d().mins(), duration::mins(long)),
            (d().nanos(), duration::nanos(long)),
            (d().secs(), duration::secs(long)),
            (d().weeks(), duration::weeks(long)),
            (d().years(), duration::years(long)),
        ]
    );
}
