use std::fmt::{self, Display};

use crate::{Ident, Name, Renderer};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

/// A full-text analyzer tokenizer.
#[expect(missing_docs, reason = "each variant is the tokenizer's own name")]
#[derive(Debug, Clone)]
pub enum Tokenizer {
    Blank,
    Camel,
    Class,
    Punct,
}

impl Display for Tokenizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Tokenizer::Blank => "Blank",
            Tokenizer::Camel => "Camel",
            Tokenizer::Class => "Class",
            Tokenizer::Punct => "Punct",
        })
    }
}

/// A language for the snowball stemming filter.
#[expect(missing_docs, reason = "each variant is the language's own name")]
#[derive(Debug, Clone)]
pub enum SnowballLanguage {
    Arabic,
    Danish,
    Dutch,
    English,
    French,
    German,
    Greek,
    Hungarian,
    Italian,
    Norwegian,
    Portuguese,
    Romanian,
    Russian,
    Spanish,
    Swedish,
    Tamil,
    Turkish,
}

impl Display for SnowballLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SnowballLanguage::Arabic => "arabic",
            SnowballLanguage::Danish => "danish",
            SnowballLanguage::Dutch => "dutch",
            SnowballLanguage::English => "english",
            SnowballLanguage::French => "french",
            SnowballLanguage::German => "german",
            SnowballLanguage::Greek => "greek",
            SnowballLanguage::Hungarian => "hungarian",
            SnowballLanguage::Italian => "italian",
            SnowballLanguage::Norwegian => "norwegian",
            SnowballLanguage::Portuguese => "portuguese",
            SnowballLanguage::Romanian => "romanian",
            SnowballLanguage::Russian => "russian",
            SnowballLanguage::Spanish => "spanish",
            SnowballLanguage::Swedish => "swedish",
            SnowballLanguage::Tamil => "tamil",
            SnowballLanguage::Turkish => "turkish",
        })
    }
}

/// A full-text analyzer filter.
#[derive(Debug, Clone)]
pub enum AnalyzerFilter {
    /// `ascii`
    Ascii,
    /// `edgengram(min, max)`
    Edgengram(u32, u32),
    /// `lowercase`
    Lowercase,
    /// `snowball(language)`
    Snowball(SnowballLanguage),
    /// `uppercase`
    Uppercase,
}

impl Display for AnalyzerFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzerFilter::Ascii => f.write_str("ascii"),
            AnalyzerFilter::Edgengram(min, max) => write!(f, "edgengram({min}, {max})"),
            AnalyzerFilter::Lowercase => f.write_str("lowercase"),
            AnalyzerFilter::Snowball(language) => write!(f, "snowball({language})"),
            AnalyzerFilter::Uppercase => f.write_str("uppercase"),
        }
    }
}

/// `DEFINE ANALYZER` statement
#[derive(Clone, Debug)]
pub struct DefineAnalyzerStatement {
    name: Name<Ident>,
    options: DefineOptions,
    tokenizers: Vec<Tokenizer>,
    filters: Vec<AnalyzerFilter>,
}

/// Create a new `DefineAnalyzerStatement`
/// ```
/// use umbriel::{Render, statements::{define_analyzer, AnalyzerFilter, SnowballLanguage, Tokenizer}};
///
/// let analyzer = define_analyzer("ascii").tokenizers([Tokenizer::Class]).filters([
///    AnalyzerFilter::Lowercase,
///    AnalyzerFilter::Ascii,
///    AnalyzerFilter::Edgengram(2, 15),
///    AnalyzerFilter::Snowball(SnowballLanguage::English),
/// ]);
/// assert_eq!(
///     analyzer.to_debug_sql(),
///     "DEFINE ANALYZER ascii TOKENIZERS Class FILTERS lowercase,ascii,edgengram(2, 15),snowball(english);"
/// );
/// ```
pub fn define_analyzer(name: impl Into<Name<Ident>>) -> DefineAnalyzerStatement {
    DefineAnalyzerStatement {
        name: name.into(),
        tokenizers: vec![],
        filters: vec![],
        options: DefineOptions::default(),
    }
}

impl DefineAnalyzerStatement {
    /// Add a tokenizer to the analyzer
    pub fn tokenizers(mut self, tokenizers: impl IntoIterator<Item = Tokenizer>) -> Self {
        self.tokenizers.extend(tokenizers);
        self
    }

    /// Add a filter to the analyzer
    pub fn filters(mut self, filters: impl IntoIterator<Item = AnalyzerFilter>) -> Self {
        self.filters.extend(filters);
        self
    }
}

/// Writes `items` separated by `,`.
pub(crate) fn push_list(out: &mut Renderer, items: &[impl Display]) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(",");
        }
        out.push_display(item);
    }
}

impl Statement for DefineAnalyzerStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE ANALYZER");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.name);

        if !self.tokenizers.is_empty() {
            out.push_str(" TOKENIZERS ");
            push_list(out, &self.tokenizers);
        }

        if !self.filters.is_empty() {
            out.push_str(" FILTERS ");
            push_list(out, &self.filters);
        }

        self.options.render_comment(out);
    }
}

impl_define_options!(DefineAnalyzerStatement);
impl_definition!(DefineAnalyzerStatement);
