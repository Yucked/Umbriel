//! `search::*` (`orm/src/functions/search.rs`, owned by #19): rendering, and the results of the
//! functions over a full-text index, including their `NONE` without a match context.

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{Field, Function, Param, Render, functions::search, kinds::KindOf};
use umbriel_tests::*;

#[test]
fn rendering() {
    call(&search::score!(1), "search::score(1)");
    call(&search::offsets!(1), "search::offsets(1)");
    call(
        &search::highlight!("<b>", "</b>", 1),
        "search::highlight('<b>', '</b>', 1)",
    );
}

/// A `note` table with a full-text index, highlights included, on its string `text` and its
/// array `tags`, and one note matching `hello` in both.
async fn notes(db: &Surreal<Any>) {
    let setup = "
        DEFINE ANALYZER words TOKENIZERS blank FILTERS lowercase;
        DEFINE INDEX note_text ON note FIELDS text FULLTEXT ANALYZER words BM25 HIGHLIGHTS;
        DEFINE INDEX note_tags ON note FIELDS tags FULLTEXT ANALYZER words BM25 HIGHLIGHTS;
        CREATE note:one SET text = 'hello world', tags = ['hello there', 'goodbye'];
    ";
    assert!(succeeds(db, setup).await, "the notes fixture is defined");
}

/// Whether `function`, selected from the notes where `field @1@ 'hello'`, equals `expected`.
async fn selected_is<K: KindOf>(
    db: &Surreal<Any>,
    function: &Function<K>,
    field: &str,
    expected: &str,
) -> bool {
    let call = function.to_debug_sql();
    read(
        db,
        &format!(
            "RETURN (SELECT VALUE {call} FROM note WHERE {field} @1@ 'hello') = [{expected}];"
        ),
    )
    .await
}

/// Whether `function`, called on its own, returns `NONE`.
async fn is_none_alone<K: KindOf>(db: &Surreal<Any>, function: &Function<K>) -> bool {
    let call = function.to_debug_sql();
    read(db, &format!("RETURN {call} IS NONE;")).await
}

/// Asserts that a query calling `function` without a `MATCHES` predicate fails when SurrealDB
/// 3.3.4 plans it, rather than returning `NONE`.
async fn refused_without_a_match<K: KindOf>(db: &Surreal<Any>, function: &Function<K>) {
    let call = function.to_debug_sql();
    let message = failure(db, &format!("RETURN {call};")).await;
    assert!(message.contains("no MATCHES clause"), "{call}: {message}");
}

#[tokio::test]
async fn score_fails_without_a_match() {
    let (db, _guard) = db().await;
    refused_without_a_match(&db, &search::score!(1)).await;
}

#[tokio::test]
async fn highlight_is_one_string_for_a_string_field() {
    let (db, _guard) = db().await;
    notes(&db).await;
    assert!(
        selected_is(
            &db,
            &search::highlight!("<b>", "</b>", 1),
            "text",
            "'<b>hello</b> world'"
        )
        .await
    );
}

#[tokio::test]
async fn highlight_is_an_array_for_an_array_field() {
    let (db, _guard) = db().await;
    notes(&db).await;
    assert!(
        selected_is(
            &db,
            &search::highlight!("<b>", "</b>", 1),
            "tags",
            "['<b>hello</b> there', 'goodbye']"
        )
        .await
    );
}

#[tokio::test]
async fn highlight_fails_without_a_match() {
    let (db, _guard) = db().await;
    refused_without_a_match(&db, &search::highlight!("<b>", "</b>", 1)).await;
}

#[tokio::test]
async fn offsets_give_spans_per_position() {
    let (db, _guard) = db().await;
    notes(&db).await;
    assert!(
        selected_is(
            &db,
            &search::offsets!(1),
            "text",
            "{ '0': [{ s: 0, e: 5 }] }"
        )
        .await
    );
}

#[tokio::test]
async fn offsets_fails_without_a_match() {
    let (db, _guard) = db().await;
    refused_without_a_match(&db, &search::offsets!(1)).await;
}

#[test]
fn partial_highlights_and_offsets_render() {
    call(
        &search::highlight!("<b>", "</b>", 1, true),
        "search::highlight('<b>', '</b>', 1, true)",
    );
    call(&search::offsets!(2, false), "search::offsets(2, false)");
    call(
        &search::rrf!(Param::new("lists"), 10),
        "search::rrf($lists, 10)",
    );
    call(
        &search::rrf!(Param::new("lists"), 10, 60),
        "search::rrf($lists, 10, 60)",
    );
}

#[tokio::test]
async fn analyze_tokenizes_with_the_named_analyzer() {
    let (db, _guard) = db().await;
    assert!(
        succeeds(
            &db,
            "DEFINE ANALYZER words TOKENIZERS blank FILTERS lowercase;"
        )
        .await,
        "the analyzer is defined"
    );
    holds(
        &db,
        &search::analyze!("words", "Hello World"),
        "['hello', 'world']",
    )
    .await;
}

#[tokio::test]
async fn analyze_of_a_non_string_parameter_is_none() {
    let (db, _guard) = db().await;
    // A raw `42` stands for a parameter or path that holds a number at run time.
    let function = search::analyze!("words", Field::raw("42"));
    assert!(
        is_none_alone(&db, &function).await,
        "`{}` is not NONE for a number",
        function.to_debug_sql()
    );
}

/// The fused result of two ranked lists, each record with an `id`: `alice` first in both, then
/// `bob` and `carol` in either order.
const LISTS: &str = "[
    [{ id: person:alice, score: 3 }, { id: person:bob, score: 2 }],
    [{ id: person:alice, score: 5 }, { id: person:carol, score: 1 }]
]";

#[tokio::test]
async fn linear_and_rrf_fuse_two_result_lists() {
    let (db, _guard) = db().await;
    let lists = Field::raw(LISTS);
    for function in [
        search::linear!(lists.clone(), vec![1, 1], 2, "minmax"),
        search::rrf!(lists.clone(), 2),
    ] {
        let call = function.to_debug_sql();
        let first: String = read(&db, &format!("RETURN record::id({call}[0].id);")).await;
        assert_eq!(first, "alice", "`{call}` ranks alice first");
        let count: i64 = read(&db, &format!("RETURN array::len({call});")).await;
        assert_eq!(count, 2, "`{call}` keeps the limit");
    }
}

#[tokio::test]
async fn fusing_no_lists_is_empty() {
    let (db, _guard) = db().await;
    holds(&db, &search::rrf!(Vec::<i32>::new(), 5), "[]").await;
    holds(
        &db,
        &search::linear!(Vec::<i32>::new(), Vec::<i32>::new(), 5, "zscore"),
        "[]",
    )
    .await;
}
