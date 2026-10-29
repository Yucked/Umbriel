//! Built-in functions: each call's rendering, SurrealDB's parser accepting the function name and
//! arguments, and the result SurrealDB computes for it.

use std::time::Duration;

use chrono::{DateTime, Utc};
use surrealdb::{Surreal, engine::local::Db, types::Value};
// `geo` and `rand` name crates too, so Umbriel's function modules are imported under other names.
use umbriel::{
    Field, Function, Model, Operatable, Param, Render, Table, arr, count,
    functions::{
        array, crypto, geo as geo_fn, http, math, parse, rand as rand_fn, record, search, session,
        sleep, string, time, type_, vector,
    },
};
use umbriel_tests::*;

/// Asserts a call's SurrealQL and that SurrealDB's parser accepts it.
#[track_caller]
fn call(function: &Function, expected: &str) {
    assert_eq!(function.to_debug_sql(), expected);
    assert_parses(&format!("RETURN {expected};"));
}

/// Asserts that SurrealDB evaluates the call to a value equal to the SurrealQL `expected`.
async fn holds(db: &Surreal<Db>, function: &Function, expected: &str) {
    let call = function.to_debug_sql();
    let equal: bool = read(db, &format!("RETURN {call} = {expected};")).await;
    if !equal {
        let actual: Value = read(db, &format!("RETURN {call};")).await;
        panic!("`{call}` returned {actual:?}, expected {expected}");
    }
}

/// Asserts that SurrealDB evaluates the call to a value of the given type, e.g. `float`.
async fn is_a(db: &Surreal<Db>, function: &Function, kind: &str) {
    let call = function.to_debug_sql();
    let matches: bool = read(db, &format!("RETURN type::is_{kind}({call});")).await;
    assert!(matches, "`{call}` is not a {kind}");
}

fn datetime() -> DateTime<Utc> {
    // 2021-11-01T08:30:17Z, a Monday.
    DateTime::from_timestamp(1_635_755_417, 0).expect("valid timestamp")
}

mod array_functions {
    use super::*;

    #[test]
    fn rendering() {
        let goals = Field::new("goals");
        call(&array::add!(vec![1, 2], 3), "array::add([1, 2], 3)");
        call(&array::add!(goals.clone(), 3), "array::add(goals, 3)");
        call(&array::all!(vec![1, 2]), "array::all([1, 2])");
        call(&array::any!(Param::new("goals")), "array::any($goals)");
        call(&array::append!(vec![1, 2], 3), "array::append([1, 2], 3)");
        call(&array::at!(vec![1, 2], 1), "array::at([1, 2], 1)");
        call(
            &array::boolean_and!(vec![true], vec![false]),
            "array::boolean_and([true], [false])",
        );
        call(
            &array::boolean_not!(vec![true]),
            "array::boolean_not([true])",
        );
        call(
            &array::boolean_or!(vec![true], vec![false]),
            "array::boolean_or([true], [false])",
        );
        call(
            &array::boolean_xor!(vec![true], vec![false]),
            "array::boolean_xor([true], [false])",
        );
        call(
            &array::clump!(vec![1, 2, 3, 4], 2),
            "array::clump([1, 2, 3, 4], 2)",
        );
        call(
            &array::combine!(vec![1, 2], vec![3]),
            "array::combine([1, 2], [3])",
        );
        call(
            &array::complement!(vec![1, 2], vec![2]),
            "array::complement([1, 2], [2])",
        );
        call(&array::concat!(vec![1], vec![2]), "array::concat([1], [2])");
        call(
            &array::difference!(vec![1, 2], vec![2]),
            "array::difference([1, 2], [2])",
        );
        call(&array::distinct!(vec![1, 1]), "array::distinct([1, 1])");
        call(
            &array::filter_index!(vec![1, 2, 1], 1),
            "array::filter_index([1, 2, 1], 1)",
        );
        call(&array::find!(vec![1, 2], 2), "array::find([1, 2], 2)");
        call(
            &array::find_index!(vec![1, 2], 2),
            "array::find_index([1, 2], 2)",
        );
        call(&array::first!(vec![1, 2]), "array::first([1, 2])");
        call(
            &array::flatten!(vec![vec![1], vec![2, 3]]),
            "array::flatten([[1], [2, 3]])",
        );
        call(
            &array::group!(vec![vec![1], vec![1, 2]]),
            "array::group([[1], [1, 2]])",
        );
        call(
            &array::insert!(vec![1, 2], 5, 1),
            "array::insert([1, 2], 5, 1)",
        );
        call(
            &array::intersect!(vec![1, 2], vec![2]),
            "array::intersect([1, 2], [2])",
        );
        call(
            &array::join!(vec!["a", "b"], "-"),
            "array::join(['a', 'b'], '-')",
        );
        call(&array::last!(vec![1, 2]), "array::last([1, 2])");
        call(&array::len!(goals.clone()), "array::len(goals)");
        call(
            &array::logical_and!(vec![true], vec![1]),
            "array::logical_and([true], [1])",
        );
        call(
            &array::logical_or!(vec![true], vec![1]),
            "array::logical_or([true], [1])",
        );
        call(
            &array::logical_xor!(vec![true], vec![1]),
            "array::logical_xor([true], [1])",
        );
        call(&array::matches!(vec![1, 2], 1), "array::matches([1, 2], 1)");
        call(&array::max!(vec![1, 2]), "array::max([1, 2])");
        call(&array::min!(vec![1, 2]), "array::min([1, 2])");
        call(&array::pop!(vec![1, 2]), "array::pop([1, 2])");
        call(&array::prepend!(vec![1], 0), "array::prepend([1], 0)");
        call(&array::push!(vec![1], 2), "array::push([1], 2)");
        call(&array::remove!(vec![1, 2], 0), "array::remove([1, 2], 0)");
        call(&array::reverse!(vec![1, 2]), "array::reverse([1, 2])");
        call(
            &array::slice!(vec![1, 2, 3], 1, 2),
            "array::slice([1, 2, 3], 1, 2)",
        );
        call(&array::sort!(vec![2, 1]), "array::sort([2, 1])");
        call(
            &array::sort!(vec![2, 1], "asc"),
            "array::sort([2, 1], 'asc')",
        );
        call(
            &array::sort!(vec![2, 1], "desc"),
            "array::sort([2, 1], 'desc')",
        );
        call(
            &array::sort!(vec![2, 1], false),
            "array::sort([2, 1], false)",
        );
        call(&array::sort::asc!(vec![2, 1]), "array::sort::asc([2, 1])");
        call(&array::sort::desc!(vec![2, 1]), "array::sort::desc([2, 1])");
        call(
            &array::transpose!(vec![vec![1, 2], vec![3, 4]]),
            "array::transpose([[1, 2], [3, 4]])",
        );
        call(&array::union!(vec![1], vec![2]), "array::union([1], [2])");
        call(&array::len!(arr![1, goals]), "array::len([1, goals])");
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (array::add!(vec![1, 2], 3), "[1, 2, 3]"),
            (array::add!(vec![1, 2], 2), "[1, 2]"),
            (array::all!(vec![1, 2]), "true"),
            (array::all!(vec![1, 0]), "false"),
            (array::any!(vec![0, 1]), "true"),
            (array::append!(vec![1, 2], 3), "[1, 2, 3]"),
            (array::at!(vec![1, 2, 3], 1), "2"),
            (
                array::boolean_and!(vec![true, false], vec![true, true]),
                "[true, false]",
            ),
            (array::boolean_not!(vec![true, false]), "[false, true]"),
            (
                array::boolean_or!(vec![true, false], vec![false, false]),
                "[true, false]",
            ),
            (
                array::boolean_xor!(vec![true, false], vec![true, true]),
                "[false, true]",
            ),
            (array::clump!(vec![1, 2, 3, 4], 2), "[[1, 2], [3, 4]]"),
            (array::combine!(vec![1, 2], vec![3]), "[[1, 3], [2, 3]]"),
            (array::complement!(vec![1, 2, 3], vec![2]), "[1, 3]"),
            (array::concat!(vec![1], vec![2]), "[1, 2]"),
            (array::difference!(vec![1, 2], vec![2, 3]), "[1, 3]"),
            (array::distinct!(vec![1, 1, 2]), "[1, 2]"),
            (array::filter_index!(vec![1, 2, 1], 1), "[0, 2]"),
            (array::find!(vec![1, 2, 3], 2), "2"),
            (array::find_index!(vec![1, 2, 3], 2), "1"),
            (array::first!(vec![1, 2]), "1"),
            (array::flatten!(vec![vec![1], vec![2, 3]]), "[1, 2, 3]"),
            (array::insert!(vec![1, 2, 3, 4], 5, 2), "[1, 2, 5, 3, 4]"),
            (array::intersect!(vec![1, 2], vec![2, 3]), "[2]"),
            (array::join!(vec!["a", "b"], "-"), "'a-b'"),
            (array::last!(vec![1, 2]), "2"),
            (array::len!(vec![1, 2]), "2"),
            (array::matches!(vec![1, 2, 1], 1), "[true, false, true]"),
            (array::max!(vec![1, 3]), "3"),
            (array::min!(vec![1, 3]), "1"),
            (array::pop!(vec![1, 2]), "2"),
            (array::prepend!(vec![1], 0), "[0, 1]"),
            (array::push!(vec![1], 2), "[1, 2]"),
            (array::remove!(vec![1, 2, 3], 1), "[1, 3]"),
            (array::reverse!(vec![1, 2]), "[2, 1]"),
            (array::sort!(vec![3, 1, 2]), "[1, 2, 3]"),
            (array::sort!(vec![3, 1, 2], "desc"), "[3, 2, 1]"),
            (array::sort::asc!(vec![3, 1, 2]), "[1, 2, 3]"),
            (array::sort::desc!(vec![3, 1, 2]), "[3, 2, 1]"),
            (
                array::transpose!(vec![vec![1, 2], vec![3, 4]]),
                "[[1, 3], [2, 4]]",
            ),
            (array::union!(vec![1, 2], vec![2, 3]), "[1, 2, 3]"),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
        for function in [
            array::group!(vec![vec![1], vec![1, 2]]),
            array::logical_and!(vec![true, false], vec![1, 1]),
            array::logical_or!(vec![true, false], vec![1, 1]),
            array::logical_xor!(vec![true, false], vec![1, 1]),
            array::slice!(vec![1, 2, 3, 4, 5], 1, 3),
        ] {
            is_a(&db, &function, "array").await;
        }
    }
}

mod math_functions {
    use super::*;

    #[test]
    fn rendering() {
        let score = Field::new("score");
        call(&math::abs!(-5), "math::abs(-5)");
        call(&math::abs!(score.clone()), "math::abs(score)");
        call(&math::ceil!(1.5), "math::ceil(1.5f)");
        call(&math::fixed!(1.2345, 2), "math::fixed(1.2345f, 2)");
        call(&math::floor!(1.5), "math::floor(1.5f)");
        call(&math::max!(vec![1, 2]), "math::max([1, 2])");
        call(&math::mean!(vec![1, 2]), "math::mean([1, 2])");
        call(&math::median!(vec![1, 2]), "math::median([1, 2])");
        call(&math::min!(vec![1, 2]), "math::min([1, 2])");
        call(&math::mode!(vec![1, 2]), "math::mode([1, 2])");
        call(&math::product!(vec![1, 2]), "math::product([1, 2])");
        call(&math::round!(1.5), "math::round(1.5f)");
        call(&math::sqrt!(16), "math::sqrt(16)");
        call(&math::sum!(Param::new("scores")), "math::sum($scores)");
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (math::abs!(-5), "5"),
            (math::ceil!(1.2), "2"),
            (math::fixed!(1.2345, 2), "1.23"),
            (math::floor!(1.8), "1"),
            (math::max!(vec![1, 4, 2]), "4"),
            (math::mean!(vec![1, 2, 3]), "2"),
            (math::median!(vec![1, 5, 3]), "3"),
            (math::min!(vec![4, 1, 2]), "1"),
            (math::mode!(vec![1, 2, 2]), "2"),
            (math::product!(vec![2, 3, 4]), "24"),
            (math::round!(1.5), "2"),
            (math::sqrt!(16), "4"),
            (math::sum!(vec![1, 2, 3]), "6"),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
    }
}

mod string_functions {
    use super::*;

    #[test]
    fn rendering() {
        let name = Field::new("name");
        call(
            &string::concat!("a", "b", name.clone()),
            "string::concat('a', 'b', name)",
        );
        call(
            &string::contains!("Alice", "lic"),
            "string::contains('Alice', 'lic')",
        );
        call(
            &string::ends_with!("Alice", "ce"),
            "string::ends_with('Alice', 'ce')",
        );
        call(
            &string::join!(", ", "a", "b"),
            "string::join(', ', 'a', 'b')",
        );
        call(&string::len!(name.clone()), "string::len(name)");
        call(&string::lowercase!("ABC"), "string::lowercase('ABC')");
        call(&string::repeat!("a", 3), "string::repeat('a', 3)");
        call(
            &string::replace!("Alice", "A", "a"),
            "string::replace('Alice', 'A', 'a')",
        );
        call(&string::reverse!("abc"), "string::reverse('abc')");
        call(
            &string::slice!("Alice", 1, 3),
            "string::slice('Alice', 1, 3)",
        );
        call(&string::slug!("Hello World"), "string::slug('Hello World')");
        call(&string::split!("a,b", ","), "string::split('a,b', ',')");
        call(
            &string::starts_with!("Alice", "Al"),
            "string::starts_with('Alice', 'Al')",
        );
        call(&string::trim!(" a "), "string::trim(' a ')");
        call(&string::uppercase!("abc"), "string::uppercase('abc')");
        call(&string::words!("a b"), "string::words('a b')");
        call(&string::is_alpha!("abc"), "string::is_alpha('abc')");
        call(&string::is_alphanum!("abc1"), "string::is_alphanum('abc1')");
        call(&string::is_ascii!("abc"), "string::is_ascii('abc')");
        call(
            &string::is_datetime!("2021-11-01"),
            "string::is_datetime('2021-11-01')",
        );
        call(
            &string::is_domain!("example.com"),
            "string::is_domain('example.com')",
        );
        call(
            &string::is_email!("alice@example.com"),
            "string::is_email('alice@example.com')",
        );
        call(
            &string::is_hexadecimal!("ff"),
            "string::is_hexadecimal('ff')",
        );
        call(&string::is_latitude!("51.5"), "string::is_latitude('51.5')");
        call(
            &string::is_longitude!("-0.1"),
            "string::is_longitude('-0.1')",
        );
        call(&string::is_numeric!("123"), "string::is_numeric('123')");
        call(&string::is_semver!("1.0.0"), "string::is_semver('1.0.0')");
        call(&string::is_uuid!(name.clone()), "string::is_uuid(name)");
        call(
            &string::distance::hamming!("abc", "abd"),
            "string::distance::hamming('abc', 'abd')",
        );
        call(
            &string::distance::levenshtein!("kitten", "sitting"),
            "string::distance::levenshtein('kitten', 'sitting')",
        );
        call(
            &string::similarity::fuzzy!("abc", "ab"),
            "string::similarity::fuzzy('abc', 'ab')",
        );
        call(
            &string::similarity::jaro!("abc", "ab"),
            "string::similarity::jaro('abc', 'ab')",
        );
        call(
            &string::similarity::smithwaterman!("abc", "ab"),
            "string::similarity::smithwaterman('abc', 'ab')",
        );
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (string::concat!("a", "b", 1), "'ab1'"),
            (string::contains!("Alice", "lic"), "true"),
            (string::ends_with!("Alice", "ce"), "true"),
            (string::join!(", ", "a", "b"), "'a, b'"),
            (string::len!("Alice"), "5"),
            (string::lowercase!("ABC"), "'abc'"),
            (string::repeat!("ab", 2), "'abab'"),
            (string::replace!("Alice", "A", "a"), "'alice'"),
            (string::reverse!("abc"), "'cba'"),
            (string::slug!("Hello World"), "'hello-world'"),
            (string::split!("a,b", ","), "['a', 'b']"),
            (string::starts_with!("Alice", "Al"), "true"),
            (string::trim!(" a "), "'a'"),
            (string::uppercase!("abc"), "'ABC'"),
            (string::words!("a b"), "['a', 'b']"),
            (string::is_alpha!("abc"), "true"),
            (string::is_alpha!("ab1"), "false"),
            (string::is_alphanum!("ab1"), "true"),
            (string::is_ascii!("abc"), "true"),
            (string::is_domain!("example.com"), "true"),
            (string::is_email!("alice@example.com"), "true"),
            (string::is_email!("alice"), "false"),
            (string::is_hexadecimal!("ff"), "true"),
            (string::is_latitude!("51.5"), "true"),
            (string::is_longitude!("-0.1"), "true"),
            (string::is_numeric!("123"), "true"),
            (string::is_semver!("1.0.0"), "true"),
            (
                string::is_uuid!("018a6680-bef9-701b-9025-e1754f296a0f"),
                "true",
            ),
            (string::distance::hamming!("abc", "abd"), "1"),
            (string::distance::levenshtein!("kitten", "sitting"), "3"),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
        is_a(&db, &string::slice!("Alice", 1, 3), "string").await;
        is_a(&db, &string::is_datetime!("2021-11-01T08:30:17Z"), "bool").await;
        for function in [
            string::similarity::fuzzy!("abc", "ab"),
            string::similarity::jaro!("abc", "ab"),
            string::similarity::smithwaterman!("abc", "ab"),
        ] {
            is_a(&db, &function, "number").await;
        }
    }
}

mod time_functions {
    use super::*;

    #[test]
    fn rendering() {
        let created = Field::new("created");
        call(&time::now!(), "time::now()");
        call(&time::timezone!(), "time::timezone()");
        call(&time::day!(created.clone()), "time::day(created)");
        call(&time::hour!(created.clone()), "time::hour(created)");
        call(&time::minute!(created.clone()), "time::minute(created)");
        call(&time::month!(created.clone()), "time::month(created)");
        call(&time::nano!(created.clone()), "time::nano(created)");
        call(&time::second!(created.clone()), "time::second(created)");
        call(&time::unix!(created.clone()), "time::unix(created)");
        call(&time::wday!(created.clone()), "time::wday(created)");
        call(&time::week!(created.clone()), "time::week(created)");
        call(&time::yday!(created.clone()), "time::yday(created)");
        call(&time::year!(created.clone()), "time::year(created)");
        call(
            &time::floor!(created.clone(), Duration::from_secs(604_800)),
            "time::floor(created, 1w)",
        );
        call(
            &time::ceil!(created.clone(), Duration::from_secs(3600)),
            "time::ceil(created, 1h)",
        );
        call(
            &time::round!(created.clone(), Duration::from_secs(60)),
            "time::round(created, 1m)",
        );
        call(
            &time::format_!(created.clone(), "%Y"),
            "time::format(created, '%Y')",
        );
        call(
            &time::group!(created.clone(), "year"),
            "time::group(created, 'year')",
        );
        call(
            &time::group!(created.clone(), "week"),
            "time::group(created, 'week')",
        );
        call(&time::max!(Param::new("dates")), "time::max($dates)");
        call(&time::min!(Param::new("dates")), "time::min($dates)");
        call(&time::from_micros!(1), "time::from_micros(1)");
        call(&time::from_millis!(1), "time::from_millis(1)");
        call(&time::from_secs!(1), "time::from_secs(1)");
        call(&time::from_unix!(1), "time::from_unix(1)");
        let literal = time::day!(datetime()).to_debug_sql();
        assert!(
            literal.starts_with("time::day(d'2021-11-01T08:30:17"),
            "{literal}"
        );
        assert_parses(&format!("RETURN {literal};"));
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let at = datetime();
        let cases = [
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
            (time::format_!(at, "%Y-%m-%d"), "'2021-11-01'"),
            (time::group!(at, "year"), "d'2021-01-01T00:00:00Z'"),
            (time::group!(at, "month"), "d'2021-11-01T00:00:00Z'"),
            (
                time::floor!(at, Duration::from_secs(3600)),
                "d'2021-11-01T08:00:00Z'",
            ),
            (
                time::ceil!(at, Duration::from_secs(3600)),
                "d'2021-11-01T09:00:00Z'",
            ),
            (
                time::round!(at, Duration::from_secs(3600)),
                "d'2021-11-01T09:00:00Z'",
            ),
            (time::from_secs!(1_635_755_417), "d'2021-11-01T08:30:17Z'"),
            (time::from_unix!(1_635_755_417), "d'2021-11-01T08:30:17Z'"),
            (
                time::from_millis!(1_635_755_417_000i64),
                "d'2021-11-01T08:30:17Z'",
            ),
            (
                time::from_micros!(1_635_755_417_000_000i64),
                "d'2021-11-01T08:30:17Z'",
            ),
            (
                time::max!(arr![at, DateTime::<Utc>::UNIX_EPOCH]),
                "d'2021-11-01T08:30:17Z'",
            ),
            (
                time::min!(arr![at, DateTime::<Utc>::UNIX_EPOCH]),
                "d'1970-01-01T00:00:00Z'",
            ),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
        is_a(&db, &time::now!(), "datetime").await;
        is_a(&db, &time::timezone!(), "string").await;
    }
}

mod type_functions {
    use super::*;

    #[test]
    fn rendering() {
        call(&type_::bool!("true"), "type::bool('true')");
        call(
            &type_::datetime!(Field::new("created")),
            "type::datetime(created)",
        );
        call(
            &type_::duration!(Duration::from_secs(3600)),
            "type::duration(1h)",
        );
        call(&type_::field!("name"), "type::field('name')");
        call(
            &type_::fields!(vec!["name", "age"]),
            "type::fields(['name', 'age'])",
        );
        call(&type_::float!(1), "type::float(1)");
        call(&type_::int!(1), "type::int(1)");
        call(&type_::number!(1), "type::number(1)");
        call(&type_::point!(vec![1, 2]), "type::point([1, 2])");
        call(
            &type_::record!("user", "alice"),
            "type::record('user', 'alice')",
        );
        call(&type_::string!(5), "type::string(5)");
        call(&type_::table!("user"), "type::table('user')");
        call(
            &type_::datetime!("2021-11-01T08:30:17Z"),
            "type::datetime('2021-11-01T08:30:17Z')",
        );
        call(&type_::float!("1.5"), "type::float('1.5')");
        call(
            &type_::is_array!(Field::new("tags")),
            "type::is_array(tags)",
        );
        call(&type_::is_bool!(true), "type::is_bool(true)");
        call(
            &type_::is_bytes!(Field::new("data")),
            "type::is_bytes(data)",
        );
        call(
            &type_::is_collection!(Field::new("area")),
            "type::is_collection(area)",
        );
        call(
            &type_::is_datetime!(Field::new("created")),
            "type::is_datetime(created)",
        );
        call(
            &type_::is_decimal!(Field::new("price")),
            "type::is_decimal(price)",
        );
        call(
            &type_::is_duration!(Duration::from_secs(60)),
            "type::is_duration(1m)",
        );
        call(&type_::is_float!(1.5), "type::is_float(1.5f)");
        call(
            &type_::is_geometry!(Field::new("area")),
            "type::is_geometry(area)",
        );
        call(&type_::is_int!(1), "type::is_int(1)");
        call(&type_::is_line!(Field::new("area")), "type::is_line(area)");
        call(
            &type_::is_multiline!(Field::new("area")),
            "type::is_multiline(area)",
        );
        call(
            &type_::is_multipoint!(Field::new("area")),
            "type::is_multipoint(area)",
        );
        call(
            &type_::is_multipolygon!(Field::new("area")),
            "type::is_multipolygon(area)",
        );
        call(&type_::is_null!(Field::new("bio")), "type::is_null(bio)");
        call(&type_::is_number!(1), "type::is_number(1)");
        call(
            &type_::is_object!(Field::new("settings")),
            "type::is_object(settings)",
        );
        call(
            &type_::is_point!(Field::new("area")),
            "type::is_point(area)",
        );
        call(
            &type_::is_polygon!(Field::new("area")),
            "type::is_polygon(area)",
        );
        call(
            &type_::is_record!(Field::new("owner")),
            "type::is_record(owner)",
        );
        call(&type_::is_string!("a"), "type::is_string('a')");
        call(&type_::is_uuid!(Field::new("key")), "type::is_uuid(key)");
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (type_::bool!("true"), "true"),
            (type_::duration!("1h"), "1h"),
            (type_::float!("1.5"), "1.5"),
            (type_::int!("5"), "5"),
            (type_::number!("5"), "5"),
            (type_::point!(vec![1, 2]), "(1, 2)"),
            (type_::record!("user", "alice"), "user:alice"),
            (
                type_::datetime!("2021-11-01T08:30:17Z"),
                "d'2021-11-01T08:30:17Z'",
            ),
            (type_::string!(5), "'5'"),
            (type_::is_string!(5), "false"),
            (type_::is_int!("5"), "false"),
            (type_::is_array!(vec![1]), "true"),
            (type_::is_bool!(true), "true"),
            (type_::is_datetime!(datetime()), "true"),
            (type_::is_duration!(Duration::from_secs(60)), "true"),
            (type_::is_float!(1.5), "true"),
            (type_::is_int!(1), "true"),
            (type_::is_null!(umbriel::NULL), "true"),
            (type_::is_number!(1), "true"),
            (type_::is_point!(geo::Point::new(1.0, 2.0)), "true"),
            (type_::is_record!(User::id("alice")), "true"),
            (type_::is_string!("a"), "true"),
            (
                type_::is_uuid!(Field::raw("u'018a6680-bef9-701b-9025-e1754f296a0f'")),
                "true",
            ),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
        let record: Option<umbriel::RecordId<User>> =
            bound(&db, type_::record!("user", "alice")).await;
        assert_eq!(record, Some(User::id("alice")));
        let table: Option<Value> = bound(&db, type_::table!("user")).await;
        assert_eq!(table, Some(Value::Table(Table::from("user"))));
    }
}

/// Runs `RETURN <call>` with the call's values bound, and decodes the result.
async fn bound<T: surrealdb::types::SurrealValue>(
    db: &Surreal<Db>,
    function: Function,
) -> Option<T> {
    use umbriel::Runnable;
    umbriel::statements::return_(function)
        .run(db.clone())
        .await
        .expect("ran")
        .take(0)
        .expect("decodes")
}

mod vector_functions {
    use super::*;

    #[test]
    fn rendering() {
        let a = Field::new("a");
        let b = Field::new("b");
        call(&vector::add!(a.clone(), b.clone()), "vector::add(a, b)");
        call(&vector::angle!(a.clone(), b.clone()), "vector::angle(a, b)");
        call(&vector::cross!(a.clone(), b.clone()), "vector::cross(a, b)");
        call(
            &vector::divide!(a.clone(), b.clone()),
            "vector::divide(a, b)",
        );
        call(&vector::dot!(a.clone(), b.clone()), "vector::dot(a, b)");
        call(&vector::magnitude!(a.clone()), "vector::magnitude(a)");
        call(
            &vector::multiply!(a.clone(), b.clone()),
            "vector::multiply(a, b)",
        );
        call(&vector::normalize!(a.clone()), "vector::normalize(a)");
        call(
            &vector::project!(a.clone(), b.clone()),
            "vector::project(a, b)",
        );
        call(
            &vector::subtract!(a.clone(), b.clone()),
            "vector::subtract(a, b)",
        );
        call(
            &vector::distance::chebyshev!(a.clone(), b.clone()),
            "vector::distance::chebyshev(a, b)",
        );
        call(
            &vector::distance::euclidean!(a.clone(), b.clone()),
            "vector::distance::euclidean(a, b)",
        );
        call(
            &vector::distance::hamming!(a.clone(), b.clone()),
            "vector::distance::hamming(a, b)",
        );
        call(
            &vector::distance::manhattan!(a.clone(), b.clone()),
            "vector::distance::manhattan(a, b)",
        );
        call(
            &vector::distance::minkowski!(a.clone(), b.clone(), 3),
            "vector::distance::minkowski(a, b, 3)",
        );
        call(
            &vector::similarity::cosine!(a.clone(), b.clone()),
            "vector::similarity::cosine(a, b)",
        );
        call(
            &vector::similarity::jaccard!(a, b),
            "vector::similarity::jaccard(a, b)",
        );
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (vector::add!(vec![1, 2], vec![3, 4]), "[4, 6]"),
            (vector::cross!(vec![1, 0, 0], vec![0, 1, 0]), "[0, 0, 1]"),
            (vector::divide!(vec![4, 6], vec![2, 3]), "[2, 2]"),
            (vector::dot!(vec![1, 2], vec![3, 4]), "11"),
            (vector::magnitude!(vec![3, 4]), "5"),
            (vector::multiply!(vec![1, 2], vec![3, 4]), "[3, 8]"),
            (vector::normalize!(vec![3, 4]), "[0.6, 0.8]"),
            (vector::project!(vec![2, 0], vec![1, 0]), "[2, 0]"),
            (vector::subtract!(vec![4, 6], vec![1, 2]), "[3, 4]"),
            (vector::distance::chebyshev!(vec![1, 2], vec![4, 6]), "4"),
            (vector::distance::euclidean!(vec![1, 2], vec![4, 6]), "5"),
            (vector::distance::hamming!(vec![1, 2], vec![1, 3]), "1"),
            (vector::distance::manhattan!(vec![1, 2], vec![4, 6]), "7"),
            (vector::distance::minkowski!(vec![1, 2], vec![4, 6], 2), "5"),
            (vector::similarity::cosine!(vec![1, 0], vec![1, 0]), "1"),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
        is_a(&db, &vector::angle!(vec![1, 0], vec![0, 1]), "number").await;
        is_a(
            &db,
            &vector::similarity::jaccard!(vec![1, 2], vec![2, 3]),
            "number",
        )
        .await;
    }
}

mod crypto_functions {
    use super::*;

    #[test]
    fn rendering() {
        call(&crypto::md5!("a"), "crypto::md5('a')");
        call(&crypto::sha1!("a"), "crypto::sha1('a')");
        call(&crypto::sha256!("a"), "crypto::sha256('a')");
        call(&crypto::sha512!("a"), "crypto::sha512('a')");
        for (generate, compare, name) in [
            (
                crypto::argon2::generate!("pass"),
                crypto::argon2::compare!(Field::new("hash"), "pass"),
                "argon2",
            ),
            (
                crypto::bcrypt::generate!("pass"),
                crypto::bcrypt::compare!(Field::new("hash"), "pass"),
                "bcrypt",
            ),
            (
                crypto::pbkdf2::generate!("pass"),
                crypto::pbkdf2::compare!(Field::new("hash"), "pass"),
                "pbkdf2",
            ),
            (
                crypto::scrypt::generate!("pass"),
                crypto::scrypt::compare!(Field::new("hash"), "pass"),
                "scrypt",
            ),
        ] {
            call(&generate, &format!("crypto::{name}::generate('pass')"));
            call(&compare, &format!("crypto::{name}::compare(hash, 'pass')"));
        }
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (crypto::md5!("a"), "'0cc175b9c0f1b6a831c399e269772661'"),
            (
                crypto::sha1!("a"),
                "'86f7e437faa5a7fce15d1ddcb9eaeaea377667b8'",
            ),
            (
                crypto::sha256!("a"),
                "'ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb'",
            ),
            (
                crypto::sha512!("a"),
                "'1f40fc92da241694750979ee6cf582f2d5d7d28e18335de05abc54d0560e0f5302860c652bf08d560252aa5e74210546f369fbbbce8c12cfc7957b2652fe9a75'",
            ),
            (
                crypto::argon2::compare!(crypto::argon2::generate!("pass"), "pass"),
                "true",
            ),
            (
                crypto::argon2::compare!(crypto::argon2::generate!("pass"), "other"),
                "false",
            ),
            (
                crypto::bcrypt::compare!(crypto::bcrypt::generate!("pass"), "pass"),
                "true",
            ),
            (
                crypto::pbkdf2::compare!(crypto::pbkdf2::generate!("pass"), "pass"),
                "true",
            ),
            (
                crypto::scrypt::compare!(crypto::scrypt::generate!("pass"), "pass"),
                "true",
            ),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
    }
}

mod geo_functions {
    use super::*;

    fn square() -> geo::Polygon {
        geo::Polygon::new(
            geo::LineString::from(vec![
                (0.0, 0.0),
                (1.0, 0.0),
                (1.0, 1.0),
                (0.0, 1.0),
                (0.0, 0.0),
            ]),
            vec![],
        )
    }

    #[test]
    fn rendering() {
        let area = Field::new("area");
        let from = Field::new("from");
        let to = Field::new("to");
        call(&geo_fn::area!(area.clone()), "geo::area(area)");
        call(
            &geo_fn::bearing!(from.clone(), to.clone()),
            "geo::bearing(from, to)",
        );
        call(&geo_fn::centroid!(area), "geo::centroid(area)");
        call(
            &geo_fn::distance!(from.clone(), to),
            "geo::distance(from, to)",
        );
        call(
            &geo_fn::hash::decode!("mpuxk4s24f51"),
            "geo::hash::decode('mpuxk4s24f51')",
        );
        call(
            &geo_fn::hash::encode!(from.clone()),
            "geo::hash::encode(from)",
        );
        call(
            &geo_fn::hash::encode!(from, 5),
            "geo::hash::encode(from, 5)",
        );
        assert_parses(&format!(
            "RETURN {};",
            geo_fn::area!(square()).to_debug_sql()
        ));
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let london = geo::Point::new(-0.118092, 51.509865);
        let paris = geo::Point::new(2.352222, 48.856614);
        for function in [
            geo_fn::area!(square()),
            geo_fn::bearing!(london, paris),
            geo_fn::distance!(london, paris),
        ] {
            is_a(&db, &function, "number").await;
        }
        is_a(&db, &geo_fn::centroid!(square()), "point").await;
        is_a(&db, &geo_fn::hash::encode!(london), "string").await;
        holds(&db, &geo_fn::hash::encode!(london, 5), "'gcpvj'").await;
        is_a(&db, &geo_fn::hash::decode!("gcpvj"), "point").await;
    }
}

mod parse_functions {
    use super::*;

    const URL: &str = "https://example.com:443/topics?arg=value#fragment";

    #[test]
    fn rendering() {
        call(
            &parse::email::host!("alice@example.com"),
            "parse::email::host('alice@example.com')",
        );
        call(
            &parse::email::user!("alice@example.com"),
            "parse::email::user('alice@example.com')",
        );
        call(
            &parse::url::domain!(URL),
            &format!("parse::url::domain('{URL}')"),
        );
        call(
            &parse::url::fragment!(URL),
            &format!("parse::url::fragment('{URL}')"),
        );
        call(
            &parse::url::host!(URL),
            &format!("parse::url::host('{URL}')"),
        );
        call(
            &parse::url::path!(URL),
            &format!("parse::url::path('{URL}')"),
        );
        call(
            &parse::url::port!(URL),
            &format!("parse::url::port('{URL}')"),
        );
        call(
            &parse::url::query!(URL),
            &format!("parse::url::query('{URL}')"),
        );
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        let cases = [
            (parse::email::host!("alice@example.com"), "'example.com'"),
            (parse::email::user!("alice@example.com"), "'alice'"),
            (parse::url::domain!(URL), "'example.com'"),
            (parse::url::fragment!(URL), "'fragment'"),
            (parse::url::host!(URL), "'example.com'"),
            (parse::url::path!(URL), "'/topics'"),
            (parse::url::port!(URL), "443"),
            (parse::url::query!(URL), "'arg=value'"),
        ];
        for (function, expected) in &cases {
            holds(&db, function, expected).await;
        }
    }
}

mod rand_functions {
    use super::*;

    #[test]
    fn rendering() {
        call(&rand_fn::rand!(), "rand()");
        call(&rand_fn::bool!(), "rand::bool()");
        call(&rand_fn::enum_!("a", "b"), "rand::enum('a', 'b')");
        call(&rand_fn::float!(), "rand::float()");
        call(&rand_fn::float!(1, 2), "rand::float(1, 2)");
        call(&rand_fn::int!(), "rand::int()");
        call(&rand_fn::int!(1, 10), "rand::int(1, 10)");
        call(&rand_fn::string!(), "rand::string()");
        call(&rand_fn::string!(8), "rand::string(8)");
        call(&rand_fn::string!(4, 8), "rand::string(4, 8)");
        call(&rand_fn::time!(), "rand::time()");
        call(&rand_fn::time!(0, 1_000), "rand::time(0, 1000)");
        call(&rand_fn::uuid!(), "rand::uuid()");
        call(&rand_fn::uuid::v4!(), "rand::uuid::v4()");
        call(&rand_fn::uuid::v7!(), "rand::uuid::v7()");
        call(&rand_fn::id!(), "rand::id()");
        call(&rand_fn::id!(10), "rand::id(10)");
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        for (function, kind) in [
            (rand_fn::rand!(), "float"),
            (rand_fn::bool!(), "bool"),
            (rand_fn::float!(1, 2), "float"),
            (rand_fn::int!(1, 10), "int"),
            (rand_fn::string!(8), "string"),
            (rand_fn::time!(), "datetime"),
            (rand_fn::uuid!(), "uuid"),
            (rand_fn::uuid::v4!(), "uuid"),
            (rand_fn::uuid::v7!(), "uuid"),
            (rand_fn::id!(), "string"),
        ] {
            is_a(&db, &function, kind).await;
        }
        holds(&db, &string::len!(rand_fn::string!(8)), "8").await;
        holds(&db, &string::len!(rand_fn::id!(10)), "10").await;
        let picked: String = read(
            &db,
            &format!("RETURN {};", rand_fn::enum_!("a", "b").to_debug_sql()),
        )
        .await;
        assert!(["a", "b"].contains(&picked.as_str()));
    }
}

mod other_functions {
    use super::*;

    #[test]
    fn rendering() {
        call(&count!(), "count()");
        call(&count!(Field::new("age").gt(18)), "count(age > 18)");
        call(&count!(vec![1, 0]), "count([1, 0])");
        call(&record::id!(User::id("alice")), "record::id(user:alice)");
        call(&record::tb!(Field::new("id")), "record::tb(id)");
        call(&session::ac!(), "session::ac()");
        call(&session::db!(), "session::db()");
        call(&session::id!(), "session::id()");
        call(&session::ip!(), "session::ip()");
        call(&session::ns!(), "session::ns()");
        call(&session::origin!(), "session::origin()");
        call(&sleep::sleep!(Duration::from_millis(5)), "sleep(5ms)");
        call(&search::score!(1), "search::score(1)");
        call(&search::offsets!(1), "search::offsets(1)");
        call(
            &search::highlight!("<b>", "</b>", 1),
            "search::highlight('<b>', '</b>', 1)",
        );
        call(
            &http::head!("https://example.com"),
            "http::head('https://example.com')",
        );
        call(
            &http::get!("https://example.com"),
            "http::get('https://example.com')",
        );
        call(
            &http::delete!("https://example.com"),
            "http::delete('https://example.com')",
        );
        let body = std::collections::BTreeMap::from([("name".to_owned(), "Alice".to_owned())]);
        let posted = http::post!("https://example.com", body.clone()).to_debug_sql();
        assert!(
            posted.starts_with("http::post('https://example.com', {"),
            "{posted}"
        );
        assert!(posted.ends_with(", { })"), "{posted}");
        assert_parses(&format!("RETURN {posted};"));
        for function in [
            http::put!("https://example.com", body.clone()),
            http::patch!("https://example.com", body.clone(), body),
        ] {
            assert_parses(&format!("RETURN {};", function.to_debug_sql()));
        }
        let script =
            umbriel::functions::function!((Param::new("value")), "{ return arguments[0] * 2; }");
        assert!(script.to_debug_sql().starts_with("function($value) "));
        assert_parses(&format!("RETURN {};", script.to_debug_sql()));
    }

    #[tokio::test]
    async fn results() {
        let db = db().await;
        holds(&db, &count!(), "1").await;
        holds(&db, &count!(vec![1, 0, 1]), "2").await;
        holds(&db, &record::id!(User::id("alice")), "'alice'").await;
        holds(&db, &record::tb!(User::id("alice")), "'user'").await;
        holds(&db, &session::ns!(), "'test'").await;
        holds(&db, &session::db!(), "'test'").await;
        let _: Value = read(
            &db,
            &format!(
                "RETURN {};",
                sleep::sleep!(Duration::from_millis(1)).to_debug_sql()
            ),
        )
        .await;
    }
}

/// Bound calls run like their debug form.
#[tokio::test]
async fn bound_arguments_run_the_same() {
    let db = db().await;
    let value: Option<String> = bound(&db, string::concat!("Alice", " ", "Bob")).await;
    assert_eq!(value.as_deref(), Some("Alice Bob"));
    let len: Option<i64> = bound(&db, array::len!(vec![1, 2, 3])).await;
    assert_eq!(len, Some(3));
}
