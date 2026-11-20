//! `DEFINE ANALYZER` (`orm/src/statements/define_analyzer.rs`): tokenizers and filters.

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
