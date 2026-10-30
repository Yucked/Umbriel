use std::borrow::Cow;

use surrealdb::types::{self, Table};

use crate::{
    Aliasable, Arrow, Clause, Conditional, Filter, Fragment, Ident, NumberLike, Operatable, Param,
    Render, Renderer,
};

/// A path into a record: a field, a nested field, or a graph traversal.
///
/// A path is kept as typed parts, so names are escaped and filters keep their bound values.
/// [`Field::new`] is one name; [`with_field`](Field::with_field), indexes, filters, graph steps,
/// [`optional`](Field::optional), [`flatten`](Field::flatten) and
/// [`destructure`](Field::destructure) continue it. [`Field::raw`] is the one verbatim form.
///
/// # Examples
///
/// ```
/// # use umbriel::*;
/// assert_eq!(Field::new("name").to_debug_sql(), "name");
/// assert_eq!(Field::new("display-name").to_debug_sql(), "`display-name`");
/// assert_eq!(Field::new("address").with_field("city").to_debug_sql(), "address.city");
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
    /// `.?`: the rest of the path applies only when the value so far is not `NONE`.
    Optional,
    /// `...`: flattens nested arrays into one.
    Flatten,
    /// `.{ a, b: c.d, e.*, f.{ g } }`: an object of the named parts of the value.
    Destructure(Vec<Destructure>),
}

/// One part of a destructuring [`Field::destructure`]: `.{ part, … }`.
#[derive(Debug, Clone, PartialEq)]
pub enum Destructure {
    /// `name`: the field itself.
    Field(Ident),
    /// `alias: path`: the value at `path`, under the key `alias`.
    Aliased(Ident, Field),
    /// `name.*`: every field of the value at `name`.
    All(Ident),
    /// `name.{ … }`: a destructuring of the value at `name`.
    Nested(Ident, Vec<Destructure>),
}

impl Destructure {
    /// `alias: path`.
    pub fn aliased(alias: impl Into<Ident>, path: impl Into<Field>) -> Self {
        Self::Aliased(alias.into(), path.into())
    }

    /// `name.*`.
    pub fn all(name: impl Into<Ident>) -> Self {
        Self::All(name.into())
    }

    /// `name.{ … }`.
    pub fn nested(
        name: impl Into<Ident>,
        parts: impl IntoIterator<Item = impl Into<Destructure>>,
    ) -> Self {
        Self::Nested(name.into(), parts.into_iter().map(Into::into).collect())
    }
}

/// `name`.
impl From<&str> for Destructure {
    fn from(name: &str) -> Self {
        Self::Field(name.into())
    }
}

/// `name`.
impl From<Ident> for Destructure {
    fn from(name: Ident) -> Self {
        Self::Field(name)
    }
}

impl Render for Destructure {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::Field(name) => {
                out.push(name);
            }
            Self::Aliased(alias, path) => {
                out.push(alias).push_str(": ").push(path);
            }
            Self::All(name) => {
                out.push(name).push_str(".*");
            }
            Self::Nested(name, parts) => {
                out.push(name).push_str(".{ ");
                out.push_joined(parts, ", ").push_str(" }");
            }
        }
    }
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
    /// Creates a field from one name, escaped when it is not a plain identifier: `name`, or
    /// `` `display-name` `` and `` `select` `` written as names. A path continues from it with
    /// [`with_field`](Self::with_field), indexes and filters. An empty name fails when the query
    /// is rendered.
    pub fn new(name: impl Into<Ident>) -> Self {
        Self {
            parts: vec![IdiomPart::Field(name.into())],
        }
    }

    /// Creates a field from SurrealQL idiom text, written verbatim, such as `address.city` or
    /// `tags[*]`. It is the one raw field: like [`Raw`](crate::Raw) it is tracked, so a query
    /// holding it may bind no values, and it has no known name or JSON pointer.
    pub fn raw(idiom: impl Into<Cow<'static, str>>) -> Self {
        Self {
            parts: vec![IdiomPart::Raw(idiom.into())],
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

    /// Returns this path extended by a field name, escaped as [`new`](Self::new) escapes it:
    /// `Field::new("address").with_field("city")` is `address.city`.
    pub fn with_field(mut self, name: impl Into<Ident>) -> Self {
        self.parts.push(IdiomPart::Field(name.into()));
        self
    }

    /// A path that starts with a graph step over one table: `->table` or `<-table`.
    #[doc(hidden)]
    pub fn graph_step(arrow: Arrow, table: Table) -> Self {
        Self::default().with_graph_step(arrow, table)
    }

    /// Returns this path extended by a graph step over one table.
    #[doc(hidden)]
    pub fn with_graph_step(mut self, arrow: Arrow, table: Table) -> Self {
        self.parts.push(IdiomPart::Graph(Graph::tables(
            Some(arrow),
            vec![table],
            None,
        )));
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

    /// `.?`: the rest of the path applies only when the value so far is not `NONE`, so
    /// `address.?.city` is `NONE` for a record without an address. (`.?` replaced `?` in
    /// SurrealDB 3.0.0.)
    pub fn optional(mut self) -> Self {
        self.parts.push(IdiomPart::Optional);
        self
    }

    /// `...`: flattens the nested arrays at this path into one array.
    pub fn flatten(mut self) -> Self {
        self.parts.push(IdiomPart::Flatten);
        self
    }

    /// `.{ … }`: an object of the named parts of the value at this path. Over an array it
    /// destructures each item, keeping every item's parts together. Omitting a destructured path
    /// (`OMIT opts.{ security, enabled }`) omits each part.
    ///
    /// # Example
    ///
    /// ```
    /// use umbriel::*;
    ///
    /// let address = Field::new("address").destructure([
    ///     Destructure::from("city"),
    ///     Destructure::aliased("zip", Field::new("postal").with_field("code")),
    ///     Destructure::nested("geo", ["lat", "lng"]),
    /// ]);
    /// assert_eq!(
    ///     address.to_debug_sql(),
    ///     "address.{ city, zip: postal.code, geo.{ lat, lng } }"
    /// );
    /// ```
    pub fn destructure(mut self, parts: impl IntoIterator<Item = impl Into<Destructure>>) -> Self {
        self.parts.push(IdiomPart::Destructure(
            parts.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// The field's name when the path is a single top-level field.
    pub(crate) fn top_level_name(&self) -> Option<&str> {
        match self.parts.as_slice() {
            [IdiomPart::Field(name)] => Some(name.as_str()),
            _ => None,
        }
    }

    /// The JSON pointer for this path, e.g. `/name/first`, when it consists of field names only.
    pub(crate) fn json_pointer(&self) -> Option<String> {
        let mut pointer = String::new();
        for part in &self.parts {
            let IdiomPart::Field(name) = part else {
                return None;
            };
            push_pointer_segment(&mut pointer, name.as_str());
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
                    out.push_raw(text);
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
                IdiomPart::Optional => {
                    out.push_str(".?");
                }
                IdiomPart::Flatten => {
                    out.push_str("...");
                }
                IdiomPart::Destructure(parts) => {
                    out.push_str(".{ ").push_joined(parts, ", ").push_str(" }");
                }
            }
        }
    }
}

impl Aliasable for Field {}

impl Conditional for Field {}

impl Operatable for Field {}

/// One field name, escaped as [`Field::new`] escapes it.
impl From<String> for Field {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// One field name, escaped as [`Field::new`] escapes it.
impl From<&str> for Field {
    fn from(value: &str) -> Self {
        Self::new(value)
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
        Self::new(name)
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
