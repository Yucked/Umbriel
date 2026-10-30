// Statement syntax
// DEFINE INDEX @name ON [ TABLE ] @table [ FIELDS | COLUMNS ] @fields
// 	[ UNIQUE | FULLTEXT ANALYZER @analyzer [ BM25 [(@k1, @b)] ] [ HIGHLIGHTS ] ]
//
// -- Make sure that email addresses in the user table are always unique
// DEFINE INDEX userEmailIndex ON TABLE user COLUMNS email UNIQUE;

use surrealdb::types::Table;

use crate::{Field, Ident, Name, Render, Renderer, TableIndex, ValueLike};

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
        unique: false,
        fulltext: None,
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

/// A statement for defining a database Index.
pub struct DefineIndexStatement {
    index_name: Name<TableIndex>,
    table: Option<Name<Table>>,
    options: DefineOptions,
    fields: Vec<Field>,
    columns: Vec<Field>,
    unique: bool,
    fulltext: Option<FullTextAnalyzer>,
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

    /// Set whether the field should be unique
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Makes this a full-text index using the given analyzer.
    pub fn fulltext(mut self, fulltext: FullTextAnalyzer) -> Self {
        self.fulltext = Some(fulltext);
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

        if self.unique {
            out.push_str(" UNIQUE");
        } else if let Some(fulltext) = &self.fulltext {
            out.push_str(" ").push(fulltext);
        }

        self.options.render_comment(out);
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
