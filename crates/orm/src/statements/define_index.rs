//! `DEFINE INDEX`. Syntax: <https://surrealdb.com/docs/surrealql/statements/define/indexes>.

use surrealdb::types::Table;

use crate::{
    Conditional, Field, Filter, Ident, Name, Render, Renderer, TableIndex, ValueLike,
    keywords::keywords,
};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

/// Define a new database index.
/// Just like in other databases, SurrealDB uses indexes to help optimize query performance.
/// An index can consist of one or more fields in a table and can enforce a uniqueness constraint.
///
/// Example:
/// ```rust
/// use umbriel::{*, statements::{define_index}};
/// # let alien = Table::from("alien");
/// # let name = Field::new("name");
/// # let age = Field::new("age");
/// # let email = Field::new("email");
/// # let dob = Field::new("dob");
///
/// let query = define_index("alien_index")
///                 .on_table(alien)
///                 .fields(&[age, name, email, dob])
///                 .unique();
///
/// assert_eq!(query.to_debug_sql(),
/// "DEFINE INDEX alien_index ON TABLE alien FIELDS age, name, email, dob UNIQUE;");
/// ```
pub fn define_index(index_name: impl Into<Name<TableIndex>>) -> DefineIndexStatement {
    DefineIndexStatement {
        index_name: index_name.into(),
        table: None,
        options: DefineOptions::default(),
        fields: vec![],
        columns: vec![],
        kind: IndexKind::Plain,
        concurrently: false,
    }
}

/// One or more indexed fields.
pub enum Columns {
    /// One field
    Field(Field),
    /// Several fields
    Fields(Vec<Field>),
}

/// One or more indexed fields.
pub type Fields = Columns;

impl From<Columns> for Vec<Field> {
    fn from(value: Columns) -> Self {
        match value {
            Columns::Field(field) => vec![field],
            Columns::Fields(fields) => fields,
        }
    }
}

impl From<Field> for Columns {
    fn from(value: Field) -> Self {
        Self::Field(value)
    }
}

impl<const N: usize> From<&[Field; N]> for Columns {
    fn from(value: &[Field; N]) -> Self {
        Self::Fields(value.to_vec())
    }
}

impl<const N: usize> From<[Field; N]> for Columns {
    fn from(value: [Field; N]) -> Self {
        Self::Fields(value.into())
    }
}

impl<const N: usize> From<[ValueLike; N]> for Columns {
    fn from(value: [ValueLike; N]) -> Self {
        Self::Fields(value.into_iter().map(Field::from_expr).collect())
    }
}

impl From<Vec<ValueLike>> for Columns {
    fn from(value: Vec<ValueLike>) -> Self {
        Self::Fields(value.into_iter().map(Field::from_expr).collect())
    }
}

impl From<Vec<Field>> for Columns {
    fn from(value: Vec<Field>) -> Self {
        Self::Fields(value)
    }
}

/// What an index is: one kind per definition.
enum IndexKind {
    /// A plain index over the fields.
    Plain,
    Unique,
    /// `COUNT [WHERE condition]`, over no fields.
    Count(Option<Filter>),
    FullText(FullTextAnalyzer),
    Hnsw(HnswOptions),
    DiskAnn(DiskAnnOptions),
}

/// A statement for defining a database Index.
pub struct DefineIndexStatement {
    index_name: Name<TableIndex>,
    table: Option<Name<Table>>,
    options: DefineOptions,
    fields: Vec<Field>,
    columns: Vec<Field>,
    kind: IndexKind,
    concurrently: bool,
}

impl DefineIndexStatement {
    /// Set the table where the index is defined. The name may come from a parameter.
    pub fn on_table(mut self, table: impl Into<Name<Table>>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Set the columns on the table where the index should be defined. This is alternative to
    /// fields just like in a relational database
    pub fn columns(mut self, columns: impl Into<Columns>) -> Self {
        self.columns.extend(Vec::<Field>::from(columns.into()));
        self
    }

    /// Set the fields on the table where the index should be defined. This is alternative to
    /// columns
    pub fn fields(mut self, fields: impl Into<Fields>) -> Self {
        self.fields.extend(Vec::<Field>::from(fields.into()));
        self
    }

    /// Makes this a unique index.
    pub fn unique(mut self) -> Self {
        self.kind = IndexKind::Unique;
        self
    }

    /// Makes this a full-text index using the given analyzer.
    pub fn fulltext(mut self, fulltext: FullTextAnalyzer) -> Self {
        self.kind = IndexKind::FullText(fulltext);
        self
    }

    /// Makes this a `COUNT` index: SurrealDB keeps the table's record count, so `count()` over
    /// the table reads it instead of scanning. It has no fields; giving it any fails the query.
    pub fn count(mut self) -> Self {
        self.kind = IndexKind::Count(None);
        self
    }

    /// Makes this a `COUNT WHERE condition` index, counting only the records matching
    /// `condition`.
    pub fn count_where(mut self, condition: impl Conditional) -> Self {
        self.kind = IndexKind::Count(Some(Filter::new(condition)));
        self
    }

    /// Makes this an HNSW vector index over its one field, which `<|k,ef|>` searches, and
    /// `<|k,METRIC|>` searches of its distance, use.
    pub fn hnsw(mut self, options: HnswOptions) -> Self {
        self.kind = IndexKind::Hnsw(options);
        self
    }

    /// Makes this a DiskANN vector index over its one field, which `<|k,l|>` searches (`l` the
    /// search list size), and `<|k,METRIC|>` searches of its distance, use. Available on 64-bit,
    /// non-WebAssembly builds of SurrealDB only.
    pub fn diskann(mut self, options: DiskAnnOptions) -> Self {
        self.kind = IndexKind::DiskAnn(options);
        self
    }

    /// `CONCURRENTLY`: the statement returns before the index is built, and the build runs in
    /// the background. `INFO FOR INDEX` ([`IndexStatus`](crate::statements::IndexStatus))
    /// reports its progress.
    pub fn concurrently(mut self) -> Self {
        self.concurrently = true;
        self
    }
}

impl Statement for DefineIndexStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE INDEX");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.index_name);

        if let Some(table) = &self.table {
            out.push_str(" ON TABLE ").push(table);
        }

        if !self.fields.is_empty() {
            out.push_str(" FIELDS ").push(&self.fields);
        }

        if !self.columns.is_empty() {
            out.push_str(" COLUMNS ").push(&self.columns);
        }

        match &self.kind {
            IndexKind::Plain => {}
            IndexKind::Unique => {
                out.push_str(" UNIQUE");
            }
            IndexKind::Count(condition) => {
                if !self.fields.is_empty() || !self.columns.is_empty() {
                    out.push_error("a COUNT index has no fields");
                }
                out.push_str(" COUNT");
                if let Some(condition) = condition {
                    out.push_str(" WHERE ").push(condition);
                }
            }
            IndexKind::FullText(fulltext) => {
                out.push_str(" ").push(fulltext);
            }
            IndexKind::Hnsw(hnsw) => {
                out.push_str(" ").push(hnsw);
            }
            IndexKind::DiskAnn(diskann) => {
                out.push_str(" ").push(diskann);
            }
        }

        self.options.render_comment(out);

        if self.concurrently {
            out.push_str(" CONCURRENTLY");
        }
    }
}

impl_define_options!(DefineIndexStatement);
impl_definition!(DefineIndexStatement);

/// The full-text options of an index: `FULLTEXT ANALYZER @analyzer [ BM25(@k1, @b) ] [ HIGHLIGHTS ]`.
pub struct FullTextAnalyzer {
    analyzer: Ident,
    bm25: Option<(f32, f32)>,
    highlights: bool,
}

impl FullTextAnalyzer {
    /// Enables `HIGHLIGHTS`, which `search::highlight` needs.
    pub fn highlights(mut self) -> Self {
        self.highlights = true;
        self
    }

    /// Scores with BM25 using the given `k1` and `b` parameters. They are literals of the
    /// definition, as SurrealDB stores them with the index.
    pub fn bm25(mut self, k1: f32, b: f32) -> Self {
        self.bm25 = Some((k1, b));
        self
    }
}

impl Render for FullTextAnalyzer {
    fn render(&self, out: &mut Renderer) {
        out.push_str("FULLTEXT ANALYZER ").push(&self.analyzer);

        if let Some((k1, b)) = &self.bm25 {
            out.push_str(" BM25(")
                .push_display(k1)
                .push_str(", ")
                .push_display(b)
                .push_str(")");
        }

        if self.highlights {
            out.push_str(" HIGHLIGHTS");
        }
    }
}

/// Starts the full-text options of an index with the analyzer it uses.
///
/// ```rust
/// use umbriel::{*, statements::{define_index, fulltext_analyzer}};
/// # let content = Field::new("content");
///
/// let query = define_index("post_content")
///     .on_table(Table::from("post"))
///     .fields(content)
///     .fulltext(fulltext_analyzer("ascii").bm25(1.2, 0.75).highlights());
///
/// assert_eq!(
///     query.to_debug_sql(),
///     "DEFINE INDEX post_content ON TABLE post FIELDS content FULLTEXT ANALYZER ascii BM25(1.2, 0.75) HIGHLIGHTS;"
/// );
/// ```
pub fn fulltext_analyzer(analyzer_name: impl Into<Ident>) -> FullTextAnalyzer {
    FullTextAnalyzer {
        analyzer: analyzer_name.into(),
        bm25: None,
        highlights: false,
    }
}

/// A distance between vectors, for a vector index (`DIST`) or a `<|k,METRIC|>` search.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Distance {
    /// `CHEBYSHEV`
    Chebyshev,
    /// `COSINE`
    Cosine,
    /// `COSINE_NORMALIZED`: cosine for vectors already of magnitude 1. DiskANN only.
    CosineNormalized,
    /// `EUCLIDEAN`, SurrealDB's default.
    Euclidean,
    /// `HAMMING`
    Hamming,
    /// `INNER_PRODUCT`: the inner product as a distance. DiskANN only.
    InnerProduct,
    /// `JACCARD`
    Jaccard,
    /// `MANHATTAN`
    Manhattan,
    /// `MINKOWSKI order`. A non-finite order fails the query.
    Minkowski(f64),
    /// `PEARSON`
    Pearson,
}

impl Render for Distance {
    fn render(&self, out: &mut Renderer) {
        let name = match self {
            Self::Chebyshev => "CHEBYSHEV",
            Self::Cosine => "COSINE",
            Self::CosineNormalized => "COSINE_NORMALIZED",
            Self::Euclidean => "EUCLIDEAN",
            Self::Hamming => "HAMMING",
            Self::InnerProduct => "INNER_PRODUCT",
            Self::Jaccard => "JACCARD",
            Self::Manhattan => "MANHATTAN",
            Self::Minkowski(order) => {
                if !order.is_finite() {
                    out.push_error(format!("the Minkowski order {order} is not finite"));
                }
                out.push_str("MINKOWSKI ").push_display(order);
                return;
            }
            Self::Pearson => "PEARSON",
        };
        out.push_str(name);
    }
}

keywords! {
    /// The element type a vector index stores vectors as (`TYPE`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum VectorType: render {
        /// `F64`
        F64 => "F64",
        /// `F32`, SurrealDB's default.
        F32 => "F32",
        /// `F16`
        F16 => "F16",
        /// `I64`
        I64 => "I64",
        /// `I32`
        I32 => "I32",
        /// `I16`
        I16 => "I16",
        /// `I8`
        I8 => "I8",
        /// `U8`
        U8 => "U8",
    }
}

/// The options of an HNSW index: `HNSW DIMENSION @d [DIST …] [TYPE …] [EFC @n] [M @n] [M0 @n]
/// [LM @f] [EXTEND_CANDIDATES] [KEEP_PRUNED_CONNECTIONS] [HASHED_VECTOR]`. Options left out take
/// SurrealDB's defaults.
///
/// ```rust
/// use umbriel::{*, statements::{Distance, define_index, hnsw}};
///
/// let query = define_index("doc_embedding")
///     .on_table(Table::from("doc"))
///     .fields(Field::new("embedding"))
///     .hnsw(hnsw(3).distance(Distance::Cosine).efc(150).m(12));
/// assert_eq!(
///     query.to_debug_sql(),
///     "DEFINE INDEX doc_embedding ON TABLE doc FIELDS embedding HNSW DIMENSION 3 DIST COSINE EFC 150 M 12;"
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct HnswOptions {
    dimension: u16,
    distance: Option<Distance>,
    vector_type: Option<VectorType>,
    efc: Option<u16>,
    m: Option<u8>,
    m0: Option<u8>,
    lm: Option<f64>,
    extend_candidates: bool,
    keep_pruned_connections: bool,
    hashed_vector: bool,
}

/// Starts the options of an HNSW index over vectors of `dimension` elements.
pub fn hnsw(dimension: u16) -> HnswOptions {
    HnswOptions {
        dimension,
        distance: None,
        vector_type: None,
        efc: None,
        m: None,
        m0: None,
        lm: None,
        extend_candidates: false,
        keep_pruned_connections: false,
        hashed_vector: false,
    }
}

impl HnswOptions {
    /// `DIST`: the distance the graph is built and searched with.
    pub fn distance(mut self, distance: Distance) -> Self {
        self.distance = Some(distance);
        self
    }

    /// `TYPE`: the element type the vectors are stored as.
    pub fn vector_type(mut self, vector_type: VectorType) -> Self {
        self.vector_type = Some(vector_type);
        self
    }

    /// `EFC`: the candidate list size while building.
    pub fn efc(mut self, efc: u16) -> Self {
        self.efc = Some(efc);
        self
    }

    /// `M`: the connections per node above layer 0, at most 127.
    pub fn m(mut self, m: u8) -> Self {
        self.m = Some(m);
        self
    }

    /// `M0`: the connections per node in layer 0.
    pub fn m0(mut self, m0: u8) -> Self {
        self.m0 = Some(m0);
        self
    }

    /// `LM`: the level multiplier. A non-finite value fails the query.
    pub fn lm(mut self, lm: f64) -> Self {
        self.lm = Some(lm);
        self
    }

    /// `EXTEND_CANDIDATES`
    pub fn extend_candidates(mut self) -> Self {
        self.extend_candidates = true;
        self
    }

    /// `KEEP_PRUNED_CONNECTIONS`
    pub fn keep_pruned_connections(mut self) -> Self {
        self.keep_pruned_connections = true;
        self
    }

    /// `HASHED_VECTOR`: records are mapped to vectors by the vector's hash.
    pub fn hashed_vector(mut self) -> Self {
        self.hashed_vector = true;
        self
    }
}

/// ` DIST …` and ` TYPE …`, each when given.
fn push_vector_options(
    out: &mut Renderer,
    distance: Option<&Distance>,
    vector_type: Option<&VectorType>,
) {
    if let Some(distance) = distance {
        out.push_str(" DIST ").push(distance);
    }
    if let Some(vector_type) = vector_type {
        out.push_str(" TYPE ").push(vector_type);
    }
}

/// ` <keyword> value` when given.
fn push_option(out: &mut Renderer, keyword: &str, value: Option<impl std::fmt::Display>) {
    if let Some(value) = value {
        out.push_str(" ")
            .push_str(keyword)
            .push_str(" ")
            .push_display(&value);
    }
}

impl Render for HnswOptions {
    fn render(&self, out: &mut Renderer) {
        out.push_str("HNSW DIMENSION ")
            .push_display(&self.dimension);
        push_vector_options(out, self.distance.as_ref(), self.vector_type.as_ref());
        push_option(out, "EFC", self.efc);
        push_option(out, "M", self.m);
        push_option(out, "M0", self.m0);
        if let Some(lm) = self.lm
            && !lm.is_finite()
        {
            out.push_error(format!("the HNSW level multiplier {lm} is not finite"));
        }
        push_option(out, "LM", self.lm);
        if self.extend_candidates {
            out.push_str(" EXTEND_CANDIDATES");
        }
        if self.keep_pruned_connections {
            out.push_str(" KEEP_PRUNED_CONNECTIONS");
        }
        if self.hashed_vector {
            out.push_str(" HASHED_VECTOR");
        }
    }
}

/// The options of a DiskANN index: `DISKANN DIMENSION @d [DIST …] [TYPE …] [DEGREE @n]
/// [L_BUILD @n] [ALPHA @f] [HASHED_VECTOR]`. SurrealDB accepts only the distances `EUCLIDEAN`,
/// `COSINE`, `INNER_PRODUCT` and `COSINE_NORMALIZED`, the types `F32`, `F16`, `I8` and `U8`
/// (`F32` and `F16` with `COSINE_NORMALIZED`), and a degree and build size above 0.
///
/// ```rust
/// use umbriel::{*, statements::{Distance, define_index, diskann}};
///
/// let query = define_index("doc_embedding")
///     .on_table(Table::from("doc"))
///     .fields(Field::new("embedding"))
///     .diskann(diskann(3).distance(Distance::Cosine).degree(32));
/// assert_eq!(
///     query.to_debug_sql(),
///     "DEFINE INDEX doc_embedding ON TABLE doc FIELDS embedding DISKANN DIMENSION 3 DIST COSINE DEGREE 32;"
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct DiskAnnOptions {
    dimension: u16,
    distance: Option<Distance>,
    vector_type: Option<VectorType>,
    degree: Option<u16>,
    l_build: Option<u16>,
    alpha: Option<f64>,
    hashed_vector: bool,
}

/// Starts the options of a DiskANN index over vectors of `dimension` elements.
pub fn diskann(dimension: u16) -> DiskAnnOptions {
    DiskAnnOptions {
        dimension,
        distance: None,
        vector_type: None,
        degree: None,
        l_build: None,
        alpha: None,
        hashed_vector: false,
    }
}

impl DiskAnnOptions {
    /// `DIST`: the distance the graph is built and searched with.
    pub fn distance(mut self, distance: Distance) -> Self {
        self.distance = Some(distance);
        self
    }

    /// `TYPE`: the element type the vectors are stored as.
    pub fn vector_type(mut self, vector_type: VectorType) -> Self {
        self.vector_type = Some(vector_type);
        self
    }

    /// `DEGREE`: the graph's maximum degree.
    pub fn degree(mut self, degree: u16) -> Self {
        self.degree = Some(degree);
        self
    }

    /// `L_BUILD`: the search list size while building.
    pub fn l_build(mut self, l_build: u16) -> Self {
        self.l_build = Some(l_build);
        self
    }

    /// `ALPHA`: the pruning factor. A non-finite value fails the query.
    pub fn alpha(mut self, alpha: f64) -> Self {
        self.alpha = Some(alpha);
        self
    }

    /// `HASHED_VECTOR`: records are mapped to vectors by the vector's hash.
    pub fn hashed_vector(mut self) -> Self {
        self.hashed_vector = true;
        self
    }
}

impl Render for DiskAnnOptions {
    fn render(&self, out: &mut Renderer) {
        out.push_str("DISKANN DIMENSION ")
            .push_display(&self.dimension);
        push_vector_options(out, self.distance.as_ref(), self.vector_type.as_ref());
        push_option(out, "DEGREE", self.degree);
        push_option(out, "L_BUILD", self.l_build);
        if let Some(alpha) = self.alpha
            && !alpha.is_finite()
        {
            out.push_error(format!("the DiskANN alpha {alpha} is not finite"));
        }
        push_option(out, "ALPHA", self.alpha);
        if self.hashed_vector {
            out.push_str(" HASHED_VECTOR");
        }
    }
}
