//! `time::*` (`orm/src/functions/time.rs`): every function and method form rendered, the
//! optional datetime of the part readers, and the results SurrealDB computes.

use std::time::Duration;

use chrono::{DateTime, Utc};
use umbriel::{DatetimeLike, Field, Param, Render, arr, functions::time};
use umbriel_tests::*;

#[test]
fn rendering() {
    let created = Field::new("created");
    calls!(
        (time::now!(), "time::now()"),
        (time::timezone!(), "time::timezone()"),
        (time::day!(created.clone()), "time::day(created)"),
        (time::day!(), "time::day()"),
        (time::hour!(created.clone()), "time::hour(created)"),
        (
            time::is_leap_year!(created.clone()),
            "time::is_leap_year(created)"
        ),
        (time::micros!(created.clone()), "time::micros(created)"),
        (time::millis!(created.clone()), "time::millis(created)"),
        (time::minute!(created.clone()), "time::minute(created)"),
        (time::month!(created.clone()), "time::month(created)"),
        (time::nano!(created.clone()), "time::nano(created)"),
        (time::second!(created.clone()), "time::second(created)"),
        (time::unix!(created.clone()), "time::unix(created)"),
        (time::wday!(created.clone()), "time::wday(created)"),
        (time::week!(created.clone()), "time::week(created)"),
        (time::yday!(created.clone()), "time::yday(created)"),
        (time::year!(created.clone()), "time::year(created)"),
        (time::year!(), "time::year()"),
        (
            time::floor!(created.clone(), Duration::from_secs(604_800)),
            "time::floor(created, 1w)"
        ),
        (
            time::ceil!(created.clone(), Duration::from_secs(3600)),
            "time::ceil(created, 1h)"
        ),
        (
            time::round!(created.clone(), Duration::from_secs(60)),
            "time::round(created, 1m)"
        ),
        (
            time::format_!(created.clone(), "%Y"),
            "time::format(created, '%Y')"
        ),
        (
            time::group!(created.clone(), "year"),
            "time::group(created, 'year')"
        ),
        (
            time::group!(created.clone(), "week"),
            "time::group(created, 'week')"
        ),
        (time::max!(Param::new("dates")), "time::max($dates)"),
        (time::min!(Param::new("dates")), "time::min($dates)"),
        (
            time::set_day!(created.clone(), 1),
            "time::set_day(created, 1)"
        ),
        (
            time::set_hour!(created.clone(), 1),
            "time::set_hour(created, 1)"
        ),
        (
            time::set_minute!(created.clone(), 1),
            "time::set_minute(created, 1)"
        ),
        (
            time::set_month!(created.clone(), 1),
            "time::set_month(created, 1)"
        ),
        (
            time::set_nanosecond!(created.clone(), 1),
            "time::set_nanosecond(created, 1)"
        ),
        (
            time::set_second!(created.clone(), 1),
            "time::set_second(created, 1)"
        ),
        (
            time::set_year!(created, 2020),
            "time::set_year(created, 2020)"
        ),
        (time::from_micros!(1), "time::from_micros(1)"),
        (time::from_millis!(1), "time::from_millis(1)"),
        (time::from_nanos!(1), "time::from_nanos(1)"),
        (time::from_secs!(1), "time::from_secs(1)"),
        (
            time::from_ulid!("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
            "time::from_ulid('01ARZ3NDEKTSV4RRFFQ69G5FAV')"
        ),
        (time::from_unix!(1), "time::from_unix(1)"),
        (time::from_uuid!(Field::new("key")), "time::from_uuid(key)"),
    );
    let literal = time::day!(datetime()).to_debug_sql();
    assert!(
        literal.starts_with("time::day(d'2021-11-01T08:30:17"),
        "{literal}"
    );
    assert_parses(&format!("RETURN {literal};"));
}

#[test]
fn method_rendering() {
    let created = DatetimeLike::from(Field::new("created"));
    let hour = || Duration::from_secs(3600);
    calls!(
        (created.ceil(hour()), "created.ceil(1h)"),
        (created.day(), "created.day()"),
        (created.floor(hour()), "created.floor(1h)"),
        (created.format("%Y"), "created.format('%Y')"),
        (created.group("month"), "created.group('month')"),
        (created.hour(), "created.hour()"),
        (created.is_leap_year(), "created.is_leap_year()"),
        (created.micros(), "created.micros()"),
        (created.millis(), "created.millis()"),
        (created.minute(), "created.minute()"),
        (created.month(), "created.month()"),
        (created.nano(), "created.nano()"),
        (created.round(hour()), "created.round(1h)"),
        (created.second(), "created.second()"),
        (created.set_day(1), "created.set_day(1)"),
        (created.set_hour(1), "created.set_hour(1)"),
        (created.set_minute(1), "created.set_minute(1)"),
        (created.set_month(1), "created.set_month(1)"),
        (created.set_nanosecond(1), "created.set_nanosecond(1)"),
        (created.set_second(1), "created.set_second(1)"),
        (created.set_year(2020), "created.set_year(2020)"),
        (created.unix(), "created.unix()"),
        (created.wday(), "created.wday()"),
        (created.week(), "created.week()"),
        (created.yday(), "created.yday()"),
        (created.year(), "created.year()"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
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
    let db = db().await;
    let error = call_failure(&db, &time::set_month!(datetime(), 13)).await;
    assert!(error.contains("month 13"), "{error}");
}
