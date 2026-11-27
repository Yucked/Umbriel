use std::fmt::{self, Display};

use crate::{Ident, Name, Renderer, keywords::keywords, render::push_path};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

/// A full-text analyzer tokenizer.
#[derive(Debug, Clone)]
pub enum Tokenizer {
    /// `blank`: splits on whitespace.
    Blank,
    /// `camel`: splits on case changes.
    Camel,
    /// `class`: splits where the character class changes.
    Class,
    /// `punct`: splits on punctuation.
    Punct,
    /// `segment(language)`: dictionary word segmentation of Chinese, Japanese or Korean text.
    /// SurrealDB needs to be built with its `cjk` feature, which WebAssembly builds lack.
    Segment(SegmentLanguage),
}

impl Display for Tokenizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tokenizer::Blank => f.write_str("Blank"),
            Tokenizer::Camel => f.write_str("Camel"),
            Tokenizer::Class => f.write_str("Class"),
            Tokenizer::Punct => f.write_str("Punct"),
            Tokenizer::Segment(language) => write!(f, "Segment({language})"),
        }
    }
}

keywords! {
    /// A language for the `segment` tokenizer.
    #[expect(missing_docs, reason = "each variant is the language's own name")]
    #[derive(Debug, Clone)]
    pub enum SegmentLanguage {
        Chinese => "CHINESE",
        Japanese => "JAPANESE",
        Korean => "KOREAN",
    }
}

keywords! {
    /// A language for the snowball stemming filter.
    #[expect(missing_docs, reason = "each variant is the language's own name")]
    #[derive(Debug, Clone)]
    pub enum SnowballLanguage {
        Arabic => "arabic",
        Danish => "danish",
        Dutch => "dutch",
        English => "english",
        French => "french",
        German => "german",
        Greek => "greek",
        Hungarian => "hungarian",
        Italian => "italian",
        Norwegian => "norwegian",
        Portuguese => "portuguese",
        Romanian => "romanian",
        Russian => "russian",
        Spanish => "spanish",
        Swedish => "swedish",
        Tamil => "tamil",
        Turkish => "turkish",
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
    /// `mapper(path)`: replaces terms through the mapping file at `path` on the server, one
    /// tab-separated `replacement term` pair per line. The server reads it only from a
    /// directory on its file allowlist.
    Mapper(String),
    /// `ngram(min, max)`: every substring of `min` to `max` characters.
    Ngram(u32, u32),
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
            AnalyzerFilter::Mapper(path) => {
                f.write_str("mapper('")?;
                for c in path.chars() {
                    match c {
                        '\\' => f.write_str("\\\\")?,
                        '\'' => f.write_str("\\'")?,
                        c => fmt::Write::write_char(f, c)?,
                    }
                }
                f.write_str("')")
            }
            AnalyzerFilter::Ngram(min, max) => write!(f, "ngram({min}, {max})"),
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
    function: Option<String>,
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
        function: None,
        tokenizers: vec![],
        filters: vec![],
        options: DefineOptions::default(),
    }
}

impl DefineAnalyzerStatement {
    /// `FUNCTION @path`: a function the analyzer passes the text through before tokenizing it.
    /// `path` is the full name of a defined function, `fn::…`, each `::`-separated part an
    /// identifier; anything else fails the query when it is rendered. A module's function
    /// (`mod::…`) is not supported yet and fails too.
    ///
    /// ```rust
    /// use umbriel::{*, statements::{Tokenizer, define_analyzer}};
    ///
    /// let analyzer = define_analyzer("cleaned")
    ///     .function("fn::strip_tags")
    ///     .tokenizers([Tokenizer::Blank]);
    /// assert_eq!(
    ///     analyzer.to_debug_sql(),
    ///     "DEFINE ANALYZER cleaned FUNCTION fn::strip_tags TOKENIZERS Blank;"
    /// );
    /// assert!(define_analyzer("cleaned").function("strip_tags").to_query().is_err());
    /// ```
    pub fn function(mut self, path: impl Into<String>) -> Self {
        self.function = Some(path.into());
        self
    }

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
    out.push_joined_with(items, ",", |out, item| {
        out.push_display(item);
    });
}

impl Statement for DefineAnalyzerStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE ANALYZER");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.name);

        if let Some(path) = &self.function {
            out.push_str(" FUNCTION ");
            match path.strip_prefix("fn::") {
                Some(name) => {
                    push_path(out, "fn", name);
                }
                None if path.starts_with("mod::") => {
                    out.push_error(format!(
                        "analyzer function `{path}`: module functions are not supported yet"
                    ));
                }
                None => {
                    out.push_error(format!(
                        "invalid analyzer function `{path}`: write the full name, `fn::…`"
                    ));
                }
            }
        }

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
