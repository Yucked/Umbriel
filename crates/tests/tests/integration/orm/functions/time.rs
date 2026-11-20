//! `time::*` (`orm/src/functions/time.rs`): every function and method form run, the
//! optional datetime of the part readers, and the results SurrealDB computes.

use std::time::Duration;

use chrono::{DateTime, Utc};
use umbriel::{DatetimeLike, Field, Param, Render, arr, functions::time};
use umbriel_tests::*;

/// The form no server test calls, and a datetime literal as an argument.
#[test]
fn rendering() {
    calls!((time::from_uuid!(Field::new("key")), "time::from_uuid(key)"));
    let literal = time::day!(datetime()).to_debug_sql();
    assert!(
        literal.starts_with("time::day(d'2021-11-01T08:30:17"),
        "{literal}"
    );
    assert_parses(&format!("RETURN {literal};"));
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    let at = datetime();
    holds_all!(
        &db,
        [
            (time::day!(at), "1"),
            (time::hour!(at), "8"),
            (time::minute!(at), "30"),
            (time::month!(at), "11"),
            (time::second!(at), "17"),
            (time::unix!(at), "1635755417"),
            (time::wday!(at), "1"),
            (time::week!(at), "44"),
            (time::yday!(at), "305"),
            (time::year!(at), "2021"),
            (time::nano!(at), "1635755417000000000"),
            (time::micros!(at), "1635755417000000"),
            (time::millis!(at), "1635755417000"),
            (time::is_leap_year!(at), "false"),
            (time::format_!(at, "%Y-%m-%d"), "'2021-11-01'"),
            (time::group!(at, "year"), "d'2021-01-01T00:00:00Z'"),
            (time::group!(at, "month"), "d'2021-11-01T00:00:00Z'"),
            (
                time::floor!(at, Duration::from_secs(3600)),
                "d'2021-11-01T08:00:00Z'"
            ),
            (
                time::ceil!(at, Duration::from_secs(3600)),
                "d'2021-11-01T09:00:00Z'"
            ),
            (
                time::round!(at, Duration::from_secs(3600)),
                "d'2021-11-01T09:00:00Z'"
            ),
            (time::set_year!(at, 2020), "d'2020-11-01T08:30:17Z'"),
            (time::set_month!(at, 1), "d'2021-01-01T08:30:17Z'"),
            (time::set_day!(at, 2), "d'2021-11-02T08:30:17Z'"),
            (time::set_hour!(at, 9), "d'2021-11-01T09:30:17Z'"),
            (time::set_minute!(at, 0), "d'2021-11-01T08:00:17Z'"),
            (time::set_second!(at, 0), "d'2021-11-01T08:30:00Z'"),
            (time::from_secs!(1_635_755_417), "d'2021-11-01T08:30:17Z'"),
            (time::from_unix!(1_635_755_417), "d'2021-11-01T08:30:17Z'"),
            (
                time::from_millis!(1_635_755_417_000i64),
                "d'2021-11-01T08:30:17Z'"
            ),
            (
                time::from_micros!(1_635_755_417_000_000i64),
                "d'2021-11-01T08:30:17Z'"
            ),
            (
                time::from_nanos!(1_635_755_417_000_000_000i64),
                "d'2021-11-01T08:30:17Z'"
            ),
            (
                time::max!(arr![at, DateTime::<Utc>::UNIX_EPOCH]),
                "d'2021-11-01T08:30:17Z'"
            ),
            (
                time::min!(arr![at, DateTime::<Utc>::UNIX_EPOCH]),
                "d'1970-01-01T00:00:00Z'"
            ),
            (DatetimeLike::from(at).year(), "2021"),
            (DatetimeLike::from(at).set_day(2), "d'2021-11-02T08:30:17Z'"),
        ]
    );
    is_each!(
        &db,
        [
            (time::now!(), "datetime"),
            (time::timezone!(), "string"),
            (time::year!(), "int"),
            (time::from_ulid!("01ARZ3NDEKTSV4RRFFQ69G5FAV"), "datetime"),
        ]
    );
}

#[test]
fn generated_bindings() {
    let nested = time::year!(time::floor!(Param::new("at"), Duration::from_secs(60)));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "time::year(time::floor($at, $_p0))");
    assert_eq!(vars.len(), 1);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(&db, &time::set_month!(datetime(), 13)).await;
    assert!(error.contains("month 13"), "{error}");
}

/// Every mapped method runs on a datetime and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let at = datetime();
    let d = || DatetimeLike::from(datetime());
    let hour = || Duration::from_secs(3600);
    same_all!(
        &db,
        [
            (d().ceil(hour()), time::ceil!(at, hour())),
            (d().day(), time::day!(at)),
            (d().floor(hour()), time::floor!(at, hour())),
            (d().format("%Y-%m-%d"), time::format_!(at, "%Y-%m-%d")),
            (d().group("month"), time::group!(at, "month")),
            (d().hour(), time::hour!(at)),
            (d().is_leap_year(), time::is_leap_year!(at)),
            (d().micros(), time::micros!(at)),
            (d().millis(), time::millis!(at)),
            (d().minute(), time::minute!(at)),
            (d().month(), time::month!(at)),
            (d().nano(), time::nano!(at)),
            (d().round(hour()), time::round!(at, hour())),
            (d().second(), time::second!(at)),
            (d().set_day(2), time::set_day!(at, 2)),
            (d().set_hour(9), time::set_hour!(at, 9)),
            (d().set_minute(0), time::set_minute!(at, 0)),
            (d().set_month(1), time::set_month!(at, 1)),
            (d().set_nanosecond(5), time::set_nanosecond!(at, 5)),
            (d().set_second(0), time::set_second!(at, 0)),
            (d().set_year(2020), time::set_year!(at, 2020)),
            (d().unix(), time::unix!(at)),
            (d().wday(), time::wday!(at)),
            (d().week(), time::week!(at)),
            (d().yday(), time::yday!(at)),
            (d().year(), time::year!(at)),
        ]
    );
}
