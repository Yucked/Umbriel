//! `string::*` (`orm/src/functions/string.rs`): every function and method form rendered, and the
//! results SurrealDB computes, including `smithwaterman` in both forms.

use umbriel::{Field, Param, Render, StrandLike, functions::string, types::Value};
use umbriel_tests::*;

#[test]
fn rendering() {
    let name = Field::new("name");
    calls!(
        (string::capitalize!("alice smith"), "string::capitalize('alice smith')"),
        (string::concat!("a", "b", name.clone()), "string::concat('a', 'b', name)"),
        (string::contains!("Alice", "lic"), "string::contains('Alice', 'lic')"),
        (string::ends_with!("Alice", "ce"), "string::ends_with('Alice', 'ce')"),
        (string::join!(", ", "a", "b"), "string::join(', ', 'a', 'b')"),
        (string::len!(name.clone()), "string::len(name)"),
        (string::lowercase!("ABC"), "string::lowercase('ABC')"),
        (string::matches!("abc", "^a"), "string::matches('abc', '^a')"),
        (string::repeat!("a", 3), "string::repeat('a', 3)"),
        (string::replace!("Alice", "A", "a"), "string::replace('Alice', 'A', 'a')"),
        (string::reverse!("abc"), "string::reverse('abc')"),
        (string::slice!("Alice", 1, 3), "string::slice('Alice', 1, 3)"),
        (string::slice!("Alice", 1), "string::slice('Alice', 1)"),
        (string::slug!("Hello World"), "string::slug('Hello World')"),
        (string::split!("a,b", ","), "string::split('a,b', ',')"),
        (string::starts_with!("Alice", "Al"), "string::starts_with('Alice', 'Al')"),
        (string::trim!(" a "), "string::trim(' a ')"),
        (string::uppercase!("abc"), "string::uppercase('abc')"),
        (string::words!("a b"), "string::words('a b')"),
        (string::is_alpha!("abc"), "string::is_alpha('abc')"),
        (string::is_alphanum!("abc1"), "string::is_alphanum('abc1')"),
        (string::is_ascii!("abc"), "string::is_ascii('abc')"),
        (string::is_datetime!("2021-11-01"), "string::is_datetime('2021-11-01')"),
        (string::is_datetime!("2021-11-01", "%Y-%m-%d"), "string::is_datetime('2021-11-01', '%Y-%m-%d')"),
        (string::is_domain!("example.com"), "string::is_domain('example.com')"),
        (string::is_email!("alice@example.com"), "string::is_email('alice@example.com')"),
        (string::is_hexadecimal!("ff"), "string::is_hexadecimal('ff')"),
        (string::is_ip!("127.0.0.1"), "string::is_ip('127.0.0.1')"),
        (string::is_ipv4!("127.0.0.1"), "string::is_ipv4('127.0.0.1')"),
        (string::is_ipv6!("::1"), "string::is_ipv6('::1')"),
        (string::is_latitude!("51.5"), "string::is_latitude('51.5')"),
        (string::is_longitude!("-0.1"), "string::is_longitude('-0.1')"),
        (string::is_numeric!("123"), "string::is_numeric('123')"),
        (string::is_record!("user:alice"), "string::is_record('user:alice')"),
        (string::is_record!("user:alice", "user"), "string::is_record('user:alice', 'user')"),
        (string::is_semver!("1.0.0"), "string::is_semver('1.0.0')"),
        (string::is_ulid!(name.clone()), "string::is_ulid(name)"),
        (string::is_url!("https://example.com"), "string::is_url('https://example.com')"),
        (string::is_uuid!(name.clone()), "string::is_uuid(name)"),
        (string::distance::damerau_levenshtein!("ab", "ba"), "string::distance::damerau_levenshtein('ab', 'ba')"),
        (string::distance::hamming!("abc", "abd"), "string::distance::hamming('abc', 'abd')"),
        (string::distance::levenshtein!("kitten", "sitting"), "string::distance::levenshtein('kitten', 'sitting')"),
        (string::distance::normalized_damerau_levenshtein!("ab", "ba"), "string::distance::normalized_damerau_levenshtein('ab', 'ba')"),
        (string::distance::normalized_levenshtein!("ab", "ba"), "string::distance::normalized_levenshtein('ab', 'ba')"),
        (string::distance::osa!("ab", "ba"), "string::distance::osa('ab', 'ba')"),
        (string::html::encode!("<b>"), "string::html::encode('<b>')"),
        (string::html::sanitize!("<b>"), "string::html::sanitize('<b>')"),
        (string::similarity::fuzzy!("abc", "ab"), "string::similarity::fuzzy('abc', 'ab')"),
        (string::similarity::jaro!("abc", "ab"), "string::similarity::jaro('abc', 'ab')"),
        (string::similarity::jaro_winkler!("abc", "ab"), "string::similarity::jaro_winkler('abc', 'ab')"),
        (string::similarity::smithwaterman!("abc", "ab"), "string::similarity::smithwaterman('abc', 'ab')"),
        (string::similarity::sorensen_dice!("abc", "ab"), "string::similarity::sorensen_dice('abc', 'ab')"),
        (string::semver::compare!("1.0.0", "1.1.0"), "string::semver::compare('1.0.0', '1.1.0')"),
        (string::semver::major!("1.2.3"), "string::semver::major('1.2.3')"),
        (string::semver::minor!("1.2.3"), "string::semver::minor('1.2.3')"),
        (string::semver::patch!("1.2.3"), "string::semver::patch('1.2.3')"),
        (string::semver::inc::major!("1.2.3"), "string::semver::inc::major('1.2.3')"),
        (string::semver::inc::minor!("1.2.3"), "string::semver::inc::minor('1.2.3')"),
        (string::semver::inc::patch!("1.2.3"), "string::semver::inc::patch('1.2.3')"),
        (string::semver::set::major!("1.2.3", 4), "string::semver::set::major('1.2.3', 4)"),
        (string::semver::set::minor!("1.2.3", 4), "string::semver::set::minor('1.2.3', 4)"),
        (string::semver::set::patch!("1.2.3", 4), "string::semver::set::patch('1.2.3', 4)"),
    );
}

#[test]
fn method_rendering() {
    let name = StrandLike::from(Field::new("name"));
    calls!(
        (name.capitalize(), "name.capitalize()"),
        (name.concat(["b"]), "name.concat('b')"),
        (name.contains("a"), "name.contains('a')"),
        (name.ends_with("a"), "name.ends_with('a')"),
        (name.join(["a", "b"]), "name.join('a', 'b')"),
        (name.len(), "name.len()"),
        (name.lowercase(), "name.lowercase()"),
        (name.matches("^a"), "name.matches('^a')"),
        (name.repeat(2), "name.repeat(2)"),
        (name.replace("a", "b"), "name.replace('a', 'b')"),
        (name.reverse(), "name.reverse()"),
        (name.slice(Some(1), Some(2)), "name.slice(1, 2)"),
        (name.slug(), "name.slug()"),
        (name.split(","), "name.split(',')"),
        (name.starts_with("a"), "name.starts_with('a')"),
        (name.trim(), "name.trim()"),
        (name.uppercase(), "name.uppercase()"),
        (name.words(), "name.words()"),
        (name.is_alpha(), "name.is_alpha()"),
        (name.is_alphanum(), "name.is_alphanum()"),
        (name.is_ascii(), "name.is_ascii()"),
        (name.is_datetime(None::<&str>), "name.is_datetime()"),
        (name.is_domain(), "name.is_domain()"),
        (name.is_email(), "name.is_email()"),
        (name.is_hexadecimal(), "name.is_hexadecimal()"),
        (name.is_ip(), "name.is_ip()"),
        (name.is_ipv4(), "name.is_ipv4()"),
        (name.is_ipv6(), "name.is_ipv6()"),
        (name.is_latitude(), "name.is_latitude()"),
        (name.is_longitude(), "name.is_longitude()"),
        (name.is_numeric(), "name.is_numeric()"),
        (name.is_record(Some("user")), "name.is_record('user')"),
        (name.is_semver(), "name.is_semver()"),
        (name.is_ulid(), "name.is_ulid()"),
        (name.is_url(), "name.is_url()"),
        (name.is_uuid(), "name.is_uuid()"),
        (name.distance_damerau_levenshtein("b"), "name.distance_damerau_levenshtein('b')"),
        (name.distance_hamming("b"), "name.distance_hamming('b')"),
        (name.distance_levenshtein("b"), "name.distance_levenshtein('b')"),
        (name.distance_normalized_damerau_levenshtein("b"), "name.distance_normalized_damerau_levenshtein('b')"),
        (name.distance_normalized_levenshtein("b"), "name.distance_normalized_levenshtein('b')"),
        (name.html_encode(), "name.html_encode()"),
        (name.html_sanitize(), "name.html_sanitize()"),
        (name.similarity_fuzzy("b"), "name.similarity_fuzzy('b')"),
        (name.similarity_jaro("b"), "name.similarity_jaro('b')"),
        (name.similarity_jaro_winkler("b"), "name.similarity_jaro_winkler('b')"),
        (name.similarity_smithwaterman("b"), "name.similarity_smithwaterman('b')"),
        (name.similarity_sorensen_dice("b"), "name.similarity_sorensen_dice('b')"),
        (name.semver_compare("1.0.0"), "name.semver_compare('1.0.0')"),
        (name.semver_major(), "name.semver_major()"),
        (name.semver_minor(), "name.semver_minor()"),
        (name.semver_patch(), "name.semver_patch()"),
        (name.semver_inc_major(), "name.semver_inc_major()"),
        (name.semver_inc_minor(), "name.semver_inc_minor()"),
        (name.semver_inc_patch(), "name.semver_inc_patch()"),
        (name.semver_set_major(2), "name.semver_set_major(2)"),
        (name.semver_set_minor(2), "name.semver_set_minor(2)"),
        (name.semver_set_patch(2), "name.semver_set_patch(2)"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    holds_all!(&db, [
        (string::capitalize!("alice smith"), "'Alice Smith'"),
        (string::concat!("a", "b", 1), "'ab1'"),
        (string::contains!("Alice", "lic"), "true"),
        (string::ends_with!("Alice", "ce"), "true"),
        (string::join!(", ", "a", "b"), "'a, b'"),
        (string::len!("Alice"), "5"),
        (string::lowercase!("ABC"), "'abc'"),
        (string::matches!("abc", "^a"), "true"),
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
        (string::is_datetime!("2021-11-01 08:30:17", "%Y-%m-%d %H:%M:%S"), "true"),
        (string::is_datetime!("2021-11-01", "%Y-%m-%d"), "false"),
        (string::is_domain!("example.com"), "true"),
        (string::is_email!("alice@example.com"), "true"),
        (string::is_email!("alice"), "false"),
        (string::is_hexadecimal!("ff"), "true"),
        (string::is_ip!("127.0.0.1"), "true"),
        (string::is_ipv4!("::1"), "false"),
        (string::is_ipv6!("::1"), "true"),
        (string::is_latitude!("51.5"), "true"),
        (string::is_longitude!("-0.1"), "true"),
        (string::is_numeric!("123"), "true"),
        (string::is_record!("user:alice", "user"), "true"),
        (string::is_record!("user:alice", "post"), "false"),
        (string::is_semver!("1.0.0"), "true"),
        (string::is_ulid!("01ARZ3NDEKTSV4RRFFQ69G5FAV"), "true"),
        (string::is_url!("https://example.com"), "true"),
        (string::is_uuid!("018a6680-bef9-701b-9025-e1754f296a0f"), "true"),
        (string::distance::damerau_levenshtein!("ab", "ba"), "1"),
        (string::distance::hamming!("abc", "abd"), "1"),
        (string::distance::levenshtein!("kitten", "sitting"), "3"),
        (string::distance::osa!("ab", "ba"), "1"),
        (string::html::encode!("<b>"), "'&lt;b&gt;'"),
        (string::semver::compare!("1.0.0", "1.1.0"), "-1"),
        (string::semver::major!("1.2.3"), "1"),
        (string::semver::minor!("1.2.3"), "2"),
        (string::semver::patch!("1.2.3"), "3"),
        (string::semver::inc::major!("1.2.3"), "'2.0.0'"),
        (string::semver::inc::minor!("1.2.3"), "'1.3.0'"),
        (string::semver::inc::patch!("1.2.3"), "'1.2.4'"),
        (string::semver::set::major!("1.2.3", 4), "'4.2.3'"),
        (string::semver::set::minor!("1.2.3", 4), "'1.4.3'"),
        (string::semver::set::patch!("1.2.3", 4), "'1.2.4'"),
        (StrandLike::from("Alice").len(), "5"),
        (StrandLike::from(", ").join(["a", "b"]), "'a, b'"),
        (
            StrandLike::from("alice").similarity_smithwaterman("alicia"),
            "string::similarity::smithwaterman('alice', 'alicia')",
        ),
    ]);
    is_each!(&db, [
        (string::slice!("Alice", 1, 3), "string"),
        (string::is_datetime!("2021-11-01T08:30:17Z"), "bool"),
        (string::html::sanitize!("<script>x</script>"), "string"),
        (string::similarity::fuzzy!("abc", "ab"), "number"),
        (string::similarity::jaro!("abc", "ab"), "number"),
        (string::similarity::jaro_winkler!("abc", "ab"), "number"),
        (string::similarity::smithwaterman!("abc", "ab"), "number"),
        (string::similarity::sorensen_dice!("abc", "ab"), "number"),
        (string::distance::normalized_damerau_levenshtein!("ab", "ba"), "float"),
        (string::distance::normalized_levenshtein!("ab", "ba"), "float"),
    ]);
}

#[tokio::test]
async fn generated_bindings() {
    let db = db().await;
    let nested = string::uppercase!(string::concat!(Param::new("first"), " ", "Smith"));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "string::uppercase(string::concat($first, $_p0, $_p1))");
    assert_eq!(vars.get("_p0"), Some(&Value::String(" ".to_owned())));
    assert_eq!(vars.get("_p1"), Some(&Value::String("Smith".to_owned())));
    let joined: Option<String> = bound(&db, &string::concat!("Alice", " ", "Bob")).await;
    assert_eq!(joined.as_deref(), Some("Alice Bob"));
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
    let error = call_failure(&db, &string::semver::major!("not a version")).await;
    assert!(error.contains("string::semver::major"), "{error}");
}
