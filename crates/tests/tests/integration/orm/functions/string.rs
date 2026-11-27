//! `string::*` (`orm/src/functions/string.rs`): every function and method form run, and the
//! results SurrealDB computes, including `smithwaterman` in both forms.

use umbriel::{Param, Render, StrandLike, functions::string, types::Value};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (string::capitalize("alice smith"), "'Alice Smith'"),
            (string::concat!("a", "b", 1), "'ab1'"),
            (string::contains("Alice", "lic"), "true"),
            (string::ends_with("Alice", "ce"), "true"),
            (string::join!(", ", "a", "b"), "'a, b'"),
            (string::len("Alice"), "5"),
            (string::lowercase("ABC"), "'abc'"),
            (string::matches("abc", "^a"), "true"),
            (string::repeat("ab", 2), "'abab'"),
            (string::replace("Alice", "A", "a"), "'alice'"),
            (string::reverse("abc"), "'cba'"),
            (string::slug("Hello World"), "'hello-world'"),
            (string::split("a,b", ","), "['a', 'b']"),
            (string::starts_with("Alice", "Al"), "true"),
            (string::trim(" a "), "'a'"),
            (string::uppercase("abc"), "'ABC'"),
            (string::words("a b"), "['a', 'b']"),
            (string::is_alpha("abc"), "true"),
            (string::is_alpha("ab1"), "false"),
            (string::is_alphanum("ab1"), "true"),
            (string::is_ascii("abc"), "true"),
            (
                string::is_datetime!("2021-11-01 08:30:17", "%Y-%m-%d %H:%M:%S"),
                "true"
            ),
            (string::is_datetime!("2021-11-01", "%Y-%m-%d"), "false"),
            (string::is_domain("example.com"), "true"),
            (string::is_email("alice@example.com"), "true"),
            (string::is_email("alice"), "false"),
            (string::is_hexadecimal("ff"), "true"),
            (string::is_ip("127.0.0.1"), "true"),
            (string::is_ipv4("::1"), "false"),
            (string::is_ipv6("::1"), "true"),
            (string::is_latitude("51.5"), "true"),
            (string::is_longitude("-0.1"), "true"),
            (string::is_numeric("123"), "true"),
            (string::is_record!("user:alice", "user"), "true"),
            (string::is_record!("user:alice", "post"), "false"),
            (string::is_semver("1.0.0"), "true"),
            (string::is_ulid("01ARZ3NDEKTSV4RRFFQ69G5FAV"), "true"),
            (string::is_url("https://example.com"), "true"),
            (
                string::is_uuid("018a6680-bef9-701b-9025-e1754f296a0f"),
                "true"
            ),
            (string::distance::damerau_levenshtein("ab", "ba"), "1"),
            (string::distance::hamming("abc", "abd"), "1"),
            (string::distance::levenshtein("kitten", "sitting"), "3"),
            (string::distance::osa("ab", "ba"), "1"),
            (string::html::encode("<b>"), "'&lt;b&gt;'"),
            (string::semver::compare("1.0.0", "1.1.0"), "-1"),
            (string::semver::major("1.2.3"), "1"),
            (string::semver::minor("1.2.3"), "2"),
            (string::semver::patch("1.2.3"), "3"),
            (string::semver::inc::major("1.2.3"), "'2.0.0'"),
            (string::semver::inc::minor("1.2.3"), "'1.3.0'"),
            (string::semver::inc::patch("1.2.3"), "'1.2.4'"),
            (string::semver::set::major("1.2.3", 4), "'4.2.3'"),
            (string::semver::set::minor("1.2.3", 4), "'1.4.3'"),
            (string::semver::set::patch("1.2.3", 4), "'1.2.4'"),
            (StrandLike::from("Alice").len(), "5"),
            (StrandLike::from(", ").join(["a", "b"]), "'a, b'"),
            (
                StrandLike::from("alice").similarity_smithwaterman("alicia"),
                "string::similarity::smithwaterman('alice', 'alicia')",
            ),
        ]
    );
    is_each!(
        &db,
        [
            (string::slice!("Alice", 1, 3), "string"),
            (string::is_datetime!("2021-11-01T08:30:17Z"), "bool"),
            (string::html::sanitize("<script>x</script>"), "string"),
            (string::similarity::fuzzy("abc", "ab"), "number"),
            (string::similarity::jaro("abc", "ab"), "number"),
            (string::similarity::jaro_winkler("abc", "ab"), "number"),
            (string::similarity::smithwaterman("abc", "ab"), "number"),
            (string::similarity::sorensen_dice("abc", "ab"), "number"),
            (
                string::distance::normalized_damerau_levenshtein("ab", "ba"),
                "float"
            ),
            (
                string::distance::normalized_levenshtein("ab", "ba"),
                "float"
            ),
        ]
    );
}

#[tokio::test]
async fn generated_bindings() {
    let (db, _guard) = db().await;
    let nested = string::uppercase(string::concat!(Param::new("first"), " ", "Smith"));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "string::uppercase(string::concat($first, $_p0, $_p1))");
    assert_eq!(vars.get("_p0"), Some(&Value::String(" ".to_owned())));
    assert_eq!(vars.get("_p1"), Some(&Value::String("Smith".to_owned())));
    let joined: Option<String> = bound(&db, &string::concat!("Alice", " ", "Bob")).await;
    assert_eq!(joined.as_deref(), Some("Alice Bob"));
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = call_failure(&db, &string::semver::major("not a version")).await;
    assert!(error.contains("string::semver::major"), "{error}");
}

/// Every mapped method runs on a string and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let s = |text: &'static str| StrandLike::from(text);
    same_all!(
        &db,
        [
            (
                s("alice smith").capitalize(),
                string::capitalize("alice smith")
            ),
            (s("a").concat(["b", "c"]), string::concat!("a", "b", "c")),
            (
                s("Alice").contains("lic"),
                string::contains("Alice", "lic")
            ),
            (
                s("Alice").ends_with("ce"),
                string::ends_with("Alice", "ce")
            ),
            (s(", ").join(["a", "b"]), string::join!(", ", "a", "b")),
            (s("Alice").len(), string::len("Alice")),
            (s("ABC").lowercase(), string::lowercase("ABC")),
            (s("abc").matches("^a"), string::matches("abc", "^a")),
            (s("ab").repeat(2), string::repeat("ab", 2)),
            (
                s("Alice").replace("A", "a"),
                string::replace("Alice", "A", "a")
            ),
            (s("abc").reverse(), string::reverse("abc")),
            (
                s("Alice").slice(Some(1), Some(3)),
                string::slice!("Alice", 1, 3)
            ),
            (s("Hello World").slug(), string::slug("Hello World")),
            (s("a,b").split(","), string::split("a,b", ",")),
            (
                s("Alice").starts_with("Al"),
                string::starts_with("Alice", "Al")
            ),
            (s(" a ").trim(), string::trim(" a ")),
            (s("abc").uppercase(), string::uppercase("abc")),
            (s("a b").words(), string::words("a b")),
            (s("abc").is_alpha(), string::is_alpha("abc")),
            (s("ab1").is_alphanum(), string::is_alphanum("ab1")),
            (s("abc").is_ascii(), string::is_ascii("abc")),
            (
                s("2021-11-01T08:30:17Z").is_datetime(None::<&str>),
                string::is_datetime!("2021-11-01T08:30:17Z")
            ),
            (
                s("example.com").is_domain(),
                string::is_domain("example.com")
            ),
            (
                s("alice@example.com").is_email(),
                string::is_email("alice@example.com")
            ),
            (s("ff").is_hexadecimal(), string::is_hexadecimal("ff")),
            (s("127.0.0.1").is_ip(), string::is_ip("127.0.0.1")),
            (s("127.0.0.1").is_ipv4(), string::is_ipv4("127.0.0.1")),
            (s("::1").is_ipv6(), string::is_ipv6("::1")),
            (s("51.5").is_latitude(), string::is_latitude("51.5")),
            (s("-0.1").is_longitude(), string::is_longitude("-0.1")),
            (s("123").is_numeric(), string::is_numeric("123")),
            (
                s("user:alice").is_record(Some("user")),
                string::is_record!("user:alice", "user")
            ),
            (s("1.0.0").is_semver(), string::is_semver("1.0.0")),
            (
                s("01ARZ3NDEKTSV4RRFFQ69G5FAV").is_ulid(),
                string::is_ulid("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            ),
            (
                s("https://example.com").is_url(),
                string::is_url("https://example.com")
            ),
            (
                s("018a6680-bef9-701b-9025-e1754f296a0f").is_uuid(),
                string::is_uuid("018a6680-bef9-701b-9025-e1754f296a0f"),
            ),
            (
                s("ab").distance_damerau_levenshtein("ba"),
                string::distance::damerau_levenshtein("ab", "ba")
            ),
            (
                s("abc").distance_hamming("abd"),
                string::distance::hamming("abc", "abd")
            ),
            (
                s("kitten").distance_levenshtein("sitting"),
                string::distance::levenshtein("kitten", "sitting")
            ),
            (
                s("ab").distance_normalized_damerau_levenshtein("ba"),
                string::distance::normalized_damerau_levenshtein("ab", "ba"),
            ),
            (
                s("ab").distance_normalized_levenshtein("ba"),
                string::distance::normalized_levenshtein("ab", "ba"),
            ),
            (s("<b>").html_encode(), string::html::encode("<b>")),
            (
                s("<b>x</b><script>y</script>").html_sanitize(),
                string::html::sanitize("<b>x</b><script>y</script>")
            ),
            (
                s("alice").similarity_fuzzy("ali"),
                string::similarity::fuzzy("alice", "ali")
            ),
            (
                s("alice").similarity_jaro("alicia"),
                string::similarity::jaro("alice", "alicia")
            ),
            (
                s("alice").similarity_jaro_winkler("alicia"),
                string::similarity::jaro_winkler("alice", "alicia")
            ),
            (
                s("alice").similarity_smithwaterman("alicia"),
                string::similarity::smithwaterman("alice", "alicia")
            ),
            (
                s("alice").similarity_sorensen_dice("alicia"),
                string::similarity::sorensen_dice("alice", "alicia")
            ),
            (
                s("1.0.0").semver_compare("1.1.0"),
                string::semver::compare("1.0.0", "1.1.0")
            ),
            (s("1.2.3").semver_major(), string::semver::major("1.2.3")),
            (s("1.2.3").semver_minor(), string::semver::minor("1.2.3")),
            (s("1.2.3").semver_patch(), string::semver::patch("1.2.3")),
            (
                s("1.2.3").semver_inc_major(),
                string::semver::inc::major("1.2.3")
            ),
            (
                s("1.2.3").semver_inc_minor(),
                string::semver::inc::minor("1.2.3")
            ),
            (
                s("1.2.3").semver_inc_patch(),
                string::semver::inc::patch("1.2.3")
            ),
            (
                s("1.2.3").semver_set_major(4),
                string::semver::set::major("1.2.3", 4)
            ),
            (
                s("1.2.3").semver_set_minor(4),
                string::semver::set::minor("1.2.3", 4)
            ),
            (
                s("1.2.3").semver_set_patch(4),
                string::semver::set::patch("1.2.3", 4)
            ),
        ]
    );
}
