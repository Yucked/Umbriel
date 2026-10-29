use std::borrow::Cow;

use surrealdb::types::{self, Table};

use crate::{
    Aliasable, Arrow, Clause, Conditional, Filter, Fragment, Ident, NumberLike, Operatable, Param,
    Render, Renderer,
};

/// A path into a record: a field, a nested field, or a graph traversal.
///
/// [`Field::new`] takes SurrealQL idiom text written verbatim, as before. Paths Umbriel builds
/// itself, such as generated schema fields and graph traversals, are kept as typed parts so names
/// are escaped and filters keep their bound values.
///
/// # Examples
///
/// ```
/// # use umbriel::*;
/// let field = Field::new("name");
///
/// assert_eq!(field.to_debug_sql(), "name");
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Field {
    parts: Vec<IdiomPart>,
}

/// One step of a [`Field`] path.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub enum IdiomPart {
    /// Caller-written idiom text, rendered verbatim.
    Raw(Cow<'static, str>),
    /// A leading expression: a parameter, record id or subquery.
    Expr(Fragment),
    /// A leading record id or list of them, rendered from the fragment and kept as ids so a
    /// statement can check them before it runs.
    Records(Fragment, Vec<types::RecordId>),
    /// A field name, `.name` after another part.
    Field(Ident),
    /// `.*`, every field.
    AllFields,
    /// `[*]`, every element.
    All,
    /// `[$]`, the last element.
    Last,
    /// `[n]`, one element.
    Index(NumberLike),
    /// `[WHERE condition]`.
    Where(Filter),
    /// A graph hop: `->edge`, `<-(a, b WHERE condition)` or `->$param`.
    Graph(Graph),
}

/// A graph traversal step.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    arrow: Option<Arrow>,
    target: GraphTarget,
}

#[derive(Debug, Clone, PartialEq)]
enum GraphTarget {
    Tables {
        tables: Vec<Table>,
        condition: Option<Filter>,
    },
    Expr(Fragment),
    /// Record ids, as for [`IdiomPart::Records`].
    Records(Fragment, Vec<types::RecordId>),
}

/// The record ids and edge step a `from->edge->to` path names when it is built, for a `RELATE` to
/// check its data against before it runs. An endpoint given as a parameter or subquery has no ids.
#[derive(Debug)]
pub(crate) struct Relation<'a> {
    /// The records the edges start from (`in`).
    pub(crate) r#in: &'a [types::RecordId],
    pub(crate) edge: EdgeStep<'a>,
    /// The records the edges point to (`out`).
    pub(crate) out: &'a [types::RecordId],
}

/// The edge step of a [`Relation`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum EdgeStep<'a> {
    /// `->table->`: the edge's table.
    Table(&'a Table),
    /// `->table:key->`: one edge record.
    Record(&'a types::RecordId),
    /// A parameter, subquery or several tables.
    Unknown,
}

impl Graph {
    pub(crate) fn tables(
        arrow: Option<Arrow>,
        tables: Vec<Table>,
        condition: Option<Filter>,
    ) -> Self {
        Self {
            arrow,
            target: GraphTarget::Tables { tables, condition },
        }
    }

    pub(crate) fn expr(arrow: Option<Arrow>, expr: Fragment) -> Self {
        Self {
            arrow,
            target: GraphTarget::Expr(expr),
        }
    }

    pub(crate) fn records(
        arrow: Option<Arrow>,
        expr: Fragment,
        records: Vec<types::RecordId>,
    ) -> Self {
        Self {
            arrow,
            target: GraphTarget::Records(expr, records),
        }
    }

    fn known_records(&self) -> &[types::RecordId] {
        match &self.target {
            GraphTarget::Records(_, records) => records,
            _ => &[],
        }
    }
}

impl Render for Graph {
    fn render(&self, out: &mut Renderer) {
        out.push(&self.arrow);
        match &self.target {
            GraphTarget::Tables { tables, condition } => match (tables.as_slice(), condition) {
                ([table], None) => {
                    out.push(table);
                }
                _ => {
                    out.push_str("(").push_joined(tables, ", ");
                    if let Some(condition) = condition {
                        out.push_str(" WHERE ").push(condition);
                    }
                    out.push_str(")");
                }
            },
            GraphTarget::Expr(expr) | GraphTarget::Records(expr, _) => {
                out.push(expr);
            }
        }
    }
}

impl Field {
    /// Creates a field from SurrealQL idiom text, written verbatim, e.g. `name`, `address.city`.
    pub fn new(idiom: impl Into<Cow<'static, str>>) -> Self {
        let idiom = idiom.into();
        let parts = if idiom.is_empty() {
            vec![]
        } else {
            vec![IdiomPart::Raw(idiom)]
        };
        Self { parts }
    }

    /// Creates a field from a single name, escaped when it is not a plain identifier.
    pub fn ident(name: impl Into<Ident>) -> Self {
        Self {
            parts: vec![IdiomPart::Field(name.into())],
        }
    }

    /// A path starting from an expression such as a parameter, record id or subquery.
    pub fn from_expr(expr: impl Render + Send + Sync + 'static) -> Self {
        Self {
            parts: vec![IdiomPart::Expr(Fragment::new(expr))],
        }
    }

    /// Whether the path has no parts.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Appends a path step.
    #[doc(hidden)]
    pub fn push(&mut self, part: IdiomPart) -> &mut Self {
        self.parts.push(part);
        self
    }

    /// Returns this path extended by a field name.
    #[doc(hidden)]
    pub fn with_field(mut self, name: impl Into<Ident>) -> Self {
        self.parts.push(IdiomPart::Field(name.into()));
        self
    }

    /// Appends the path steps a clause stands for.
    #[doc(hidden)]
    pub fn push_clause(&mut self, clause: Clause) -> &mut Self {
        clause.push_parts(&mut self.parts);
        self
    }

    /// Appends another path.
    #[doc(hidden)]
    pub fn extend(&mut self, other: Field) -> &mut Self {
        self.parts.extend(other.parts);
        self
    }

    /// `.*`: every field of the value at this path.
    pub fn all(&self) -> Field {
        let mut field = self.clone();
        field.parts.push(IdiomPart::AllFields);
        field
    }

    /// The field's name when the path is a single top-level field. For an escaped identifier in
    /// raw text (`` `display-name` ``, `⟨display-name⟩`) this is its contents, escapes as written.
    pub(crate) fn top_level_name(&self) -> Option<&str> {
        match self.parts.as_slice() {
            [IdiomPart::Field(name)] => Some(name.as_str()),
            [IdiomPart::Raw(text)] if is_plain_segment(text) => Some(text),
            [IdiomPart::Raw(text)] => escaped_ident(text),
            _ => None,
        }
    }

    /// The JSON pointer for this path, e.g. `/name/first`, when it consists of field names only.
    pub(crate) fn json_pointer(&self) -> Option<String> {
        let mut pointer = String::new();
        for part in &self.parts {
            match part {
                IdiomPart::Field(name) => push_pointer_segment(&mut pointer, name.as_str()),
                IdiomPart::Raw(text) => {
                    for segment in text.split('.') {
                        if !is_plain_segment(segment) {
                            return None;
                        }
                        push_pointer_segment(&mut pointer, segment);
                    }
                }
                _ => return None,
            }
        }
        (!pointer.is_empty()).then_some(pointer)
    }

    /// What the path names when it has the `from->edge->to` shape of a `RELATE`; `None` for any
    /// other shape. With `<-` arrows the far end is `in`.
    pub(crate) fn relation(&self) -> Option<Relation<'_>> {
        let [from, IdiomPart::Graph(edge), IdiomPart::Graph(to)] = self.parts.as_slice() else {
            return None;
        };
        let from: &[types::RecordId] = match from {
            IdiomPart::Records(_, records) => records,
            _ => &[],
        };
        let step = match &edge.target {
            GraphTarget::Tables {
                tables,
                condition: None,
            } => match tables.as_slice() {
                [table] => EdgeStep::Table(table),
                _ => EdgeStep::Unknown,
            },
            GraphTarget::Records(_, records) => match records.as_slice() {
                [record] => EdgeStep::Record(record),
                _ => EdgeStep::Unknown,
            },
            _ => EdgeStep::Unknown,
        };
        let to = to.known_records();
        let (r#in, out) = match edge.arrow {
            Some(Arrow::Left) => (to, from),
            _ => (from, to),
        };
        Some(Relation {
            r#in,
            edge: step,
            out,
        })
    }
}

fn is_plain_segment(segment: &str) -> bool {
    segment
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && segment.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// The contents of `text` when it is exactly one escaped identifier: `` `…` `` or `⟨…⟩`, with no
/// unescaped closing delimiter inside, so `` `a`.b `` and `` `a`.`b` `` are not.
fn escaped_ident(text: &str) -> Option<&str> {
    let close = match text.chars().next()? {
        '`' => '`',
        '⟨' => '⟩',
        _ => return None,
    };
    let inner = text[text.chars().next()?.len_utf8()..].strip_suffix(close)?;
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            // An escape consumes the next character, so `\`` does not close the identifier.
            '\\' => {
                chars.next()?;
            }
            c if c == close => return None,
            _ => {}
        }
    }
    (!inner.is_empty()).then_some(inner)
}

/// Appends `/segment`, escaping `~` and `/` as RFC 6901 requires.
fn push_pointer_segment(pointer: &mut String, segment: &str) {
    pointer.push('/');
    for c in segment.chars() {
        match c {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            c => pointer.push(c),
        }
    }
}

impl Render for Field {
    fn render(&self, out: &mut Renderer) {
        for (index, part) in self.parts.iter().enumerate() {
            match part {
                IdiomPart::Raw(text) => {
                    out.push_str(text);
                }
                IdiomPart::Expr(expr) | IdiomPart::Records(expr, _) => {
                    out.push(expr);
                }
                IdiomPart::Field(name) => {
                    if index > 0 {
                        out.push_str(".");
                    }
                    out.push(name);
                }
                IdiomPart::AllFields => {
                    out.push_str(if index > 0 { ".*" } else { "*" });
                }
                IdiomPart::All => {
                    out.push_str("[*]");
                }
                IdiomPart::Last => {
                    out.push_str("[$]");
                }
                IdiomPart::Index(position) => {
                    out.push_str("[").push(position).push_str("]");
                }
                IdiomPart::Where(condition) => {
                    out.push_str("[WHERE ").push(condition).push_str("]");
                }
                IdiomPart::Graph(graph) => {
                    out.push(graph);
                }
            }
        }
    }
}

impl Aliasable for Field {}

impl Conditional for Field {}

impl Operatable for Field {}

impl From<String> for Field {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for Field {
    fn from(value: &str) -> Self {
        Self::new(value.to_owned())
    }
}

impl From<&Field> for Field {
    fn from(value: &Field) -> Self {
        value.clone()
    }
}

impl<'a> From<Cow<'a, Field>> for Field {
    fn from(value: Cow<'a, Field>) -> Self {
        value.into_owned()
    }
}

impl<'a> From<&'a Field> for Cow<'a, Field> {
    fn from(value: &'a Field) -> Self {
        Cow::Borrowed(value)
    }
}

impl From<Field> for Cow<'static, Field> {
    fn from(value: Field) -> Self {
        Cow::Owned(value)
    }
}

impl From<Ident> for Field {
    fn from(name: Ident) -> Self {
        Self::ident(name)
    }
}

impl From<&Param> for Field {
    fn from(param: &Param) -> Self {
        Self::from_expr(param.clone())
    }
}

impl From<Param> for Field {
    fn from(param: Param) -> Self {
        Self::from_expr(param)
    }
}
