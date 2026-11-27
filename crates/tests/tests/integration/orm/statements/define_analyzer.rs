//! `DEFINE ANALYZER` (`orm/src/statements/define_analyzer.rs`): tokenizers, filters and the
//! `FUNCTION` clause, rendered and run through `search::analyze`.

use umbriel::statements::SegmentLanguage;
use umbriel_tests::{support::statements::*, *};

#[test]
fn tokenizers_and_filters_render_in_order() {
    assert_sql(
        &define_analyzer("ascii")
            .tokenizers([Tokenizer::Class, Tokenizer::Blank])
            .filters([
                AnalyzerFilter::Lowercase,
                AnalyzerFilter::Ascii,
                AnalyzerFilter::Edgengram(2, 15),
                AnalyzerFilter::Snowball(SnowballLanguage::English),
            ]),
        "DEFINE ANALYZER ascii TOKENIZERS Class,Blank \
         FILTERS lowercase,ascii,edgengram(2, 15),snowball(english);",
    );
}

#[test]
fn analyzer_clauses_render() {
    let cases = [
        (
            define_analyzer("cjk").tokenizers([Tokenizer::Segment(SegmentLanguage::Japanese)]),
            "DEFINE ANALYZER cjk TOKENIZERS Segment(JAPANESE);",
        ),
        (
            define_analyzer("grams")
                .tokenizers([Tokenizer::Blank])
                .filters([AnalyzerFilter::Ngram(2, 3)]),
            "DEFINE ANALYZER grams TOKENIZERS Blank FILTERS ngram(2, 3);",
        ),
        (
            define_analyzer("mapped")
                .tokenizers([Tokenizer::Blank])
                .filters([AnalyzerFilter::Mapper("/data/it's.txt".to_owned())]),
            "DEFINE ANALYZER mapped TOKENIZERS Blank FILTERS mapper('/data/it\\'s.txt');",
        ),
        (
            define_analyzer("cleaned")
                .function("fn::text::clean")
                .tokenizers([Tokenizer::Blank]),
            "DEFINE ANALYZER cleaned FUNCTION fn::text::clean TOKENIZERS Blank;",
        ),
    ];
    for (statement, expected) in cases {
        assert_sql(&statement, expected);
    }
}

#[test]
fn a_function_path_that_is_not_fn_fails_to_render() {
    for path in [
        "clean",
        "string::lowercase",
        "fn::bad-name",
        "fn::a; REMOVE TABLE x",
    ] {
        let statement = define_analyzer("cleaned").function(path);
        assert!(statement.to_query().is_err(), "`{path}` renders");
        assert!(!statement.to_debug_sql().contains("REMOVE"), "{path}");
    }
    // Module functions are deferred until a module fixture can test their preprocessing.
    assert_render_error(
        &define_analyzer("cleaned").function("mod::text::clean"),
        "module functions are not supported yet",
    );
}

/// The tokens `analyzer` makes of `text`, through `search::analyze`.
async fn tokens(db: &Surreal<Any>, analyzer: &str, text: &str) -> Vec<String> {
    read(
        db,
        &format!("RETURN {};", search::analyze(analyzer, text).to_debug_sql()),
    )
    .await
}

#[tokio::test]
async fn ngram_splits_into_grams() {
    let (db, _guard) = db().await;
    run(
        &db,
        define_analyzer("grams")
            .tokenizers([Tokenizer::Blank])
            .filters([AnalyzerFilter::Ngram(2, 2)]),
    )
    .await;
    assert_eq!(tokens(&db, "grams", "abc").await, ["ab", "bc"]);
}

#[tokio::test]
async fn a_function_preprocesses_input_before_tokenizing() {
    let (db, _guard) = db().await;
    run(
        &db,
        Raw::new("DEFINE FUNCTION fn::text::clean($text: string) { RETURN string::replace($text, '-', ' '); };"),
    )
    .await;
    run(
        &db,
        define_analyzer("cleaned")
            .function("fn::text::clean")
            .tokenizers([Tokenizer::Blank]),
    )
    .await;
    assert_eq!(tokens(&db, "cleaned", "red-green").await, ["red", "green"]);
}

/// Needs `UMBRIEL_TEST_MAPPER` (see `crates/tests/README.md`): a mapper file mapping `hello` to
/// `hi`.
#[tokio::test]
async fn a_mapper_replaces_terms() {
    let path = std::env::var("UMBRIEL_TEST_MAPPER")
        .expect("`UMBRIEL_TEST_MAPPER` names the mapper file on the server");
    let (db, _guard) = db().await;
    run(
        &db,
        define_analyzer("mapped")
            .tokenizers([Tokenizer::Blank])
            .filters([AnalyzerFilter::Mapper(path)]),
    )
    .await;
    assert_eq!(tokens(&db, "mapped", "hello world").await, ["hi", "world"]);
}

/// Needs a server built with SurrealDB's `cjk` feature (see `crates/tests/README.md`).
#[tokio::test]
async fn segment_splits_cjk_text() {
    let (db, _guard) = db().await;
    run(
        &db,
        define_analyzer("japanese").tokenizers([Tokenizer::Segment(SegmentLanguage::Japanese)]),
    )
    .await;
    // Blank-free text that only dictionary segmentation splits into words.
    let segmented = tokens(&db, "japanese", "東京都に住む").await;
    assert!(segmented.len() > 1, "{segmented:?}");
}
