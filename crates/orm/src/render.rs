//! Structural query rendering.
//!
//! Builders keep typed nodes (fields, operators, clauses, statements and values) and never hold
//! rendered text. Composing a node into another moves it there, or shares it through a [`Fragment`]
//! where one node is reused, so composition never copies text. A query is rendered by a single
//! traversal that writes one buffer and names each bound value in order (`$_p0`, `$_p1`, …): nested
//! and reused nodes cannot collide, and a query renders identically every time. Debug SQL is the
//! same traversal with values written inline, so no text is ever searched and replaced.

use std::{fmt, fmt::Write as _, mem, sync::Arc};

use surrealdb::types::{SqlFormat, ToSql, Value, Variables};

use crate::{Error, Result};

/// Prefix of the parameter names allocated for bound values. Caller-named parameters may not
/// start with it.
pub const GENERATED_PARAM_PREFIX: &str = "_p";

/// How values are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// As generated parameters, bound in the query's variables.
    Bind,
    /// As SurrealQL literals.
    Inline,
}

/// Writes a query as its nodes are traversed: the text, the variables bound so far, and the errors
/// found in the nodes.
#[derive(Debug)]
pub struct Renderer {
    sql: String,
    vars: Variables,
    bound: usize,
    mode: Mode,
    errors: Vec<String>,
    /// Whether caller-written SurrealQL is part of the query. Such text could name a generated
    /// parameter, so a query holding it may bind no values.
    raw: bool,
}

impl Renderer {
    fn new(mode: Mode) -> Self {
        Self {
            sql: String::new(),
            vars: Variables::new(),
            bound: 0,
            mode,
            errors: vec![],
            raw: false,
        }
    }

    /// Appends SurrealQL syntax. Never pass caller data here; values go through `push_value`.
    pub(crate) fn push_str(&mut self, text: &str) -> &mut Self {
        self.sql.push_str(text);
        self
    }

    /// Appends SurrealQL written by the caller, verbatim.
    pub(crate) fn push_raw(&mut self, text: &str) -> &mut Self {
        self.raw = true;
        self.push_str(text)
    }

    /// Appends an identifier, escaped when it is not a plain SurrealQL identifier.
    pub(crate) fn push_ident(&mut self, name: &str) -> &mut Self {
        if name.is_empty() {
            self.push_error("identifiers cannot be empty");
        }
        write_ident(&mut self.sql, name);
        self
    }

    /// Appends a syntax element (a table name, a type) in SurrealDB's own formatting.
    pub(crate) fn push_syntax(&mut self, syntax: &impl ToSql) -> &mut Self {
        syntax.fmt_sql(&mut self.sql, SqlFormat::SingleLine);
        self
    }

    /// Appends SurrealQL syntax produced by a `Display` impl, such as a keyword enum.
    pub(crate) fn push_display(&mut self, syntax: &impl fmt::Display) -> &mut Self {
        // Writing to a `String` cannot fail.
        let _ = write!(self.sql, "{syntax}");
        self
    }

    /// Appends a value: a generated parameter bound to it, or its literal in debug SQL and
    /// definitions.
    pub fn push_value(&mut self, value: &Value) -> &mut Self {
        match self.mode {
            Mode::Bind => {
                let name = format!("{GENERATED_PARAM_PREFIX}{}", self.bound);
                self.bound += 1;
                self.sql.push('$');
                self.sql.push_str(&name);
                self.vars.insert(name, value.clone());
            }
            Mode::Inline => value.fmt_sql(&mut self.sql, SqlFormat::SingleLine),
        }
        self
    }

    /// Appends another node.
    pub fn push(&mut self, node: &(impl Render + ?Sized)) -> &mut Self {
        node.render(self);
        self
    }

    /// Appends a node with its values written as literals. Used where SurrealDB stores the
    /// expression for later evaluation, such as definitions, where query parameters no longer exist.
    pub(crate) fn push_inlined(&mut self, node: &(impl Render + ?Sized)) -> &mut Self {
        let mode = mem::replace(&mut self.mode, Mode::Inline);
        node.render(self);
        self.mode = mode;
        self
    }

    /// Appends `items` separated by `separator`.
    pub(crate) fn push_joined<I>(&mut self, items: I, separator: &str) -> &mut Self
    where
        I: IntoIterator,
        I::Item: Render,
    {
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                self.push_str(separator);
            }
            item.render(self);
        }
        self
    }

    /// Records an error that makes the query fail before it is executed.
    pub(crate) fn push_error(&mut self, error: impl Into<String>) -> &mut Self {
        self.errors.push(error.into());
        self
    }

    /// The parameterised query and its variables, or the errors recorded while rendering it.
    fn into_query(self) -> Result<Query> {
        if !self.errors.is_empty() {
            return Err(Error::QueryBuilder(self.errors.join("; ")));
        }
        if self.raw && self.bound > 0 {
            return Err(Error::QueryBuilder(format!(
                "raw SurrealQL cannot share a query with bound values: it could refer to their \
                 generated `${GENERATED_PARAM_PREFIX}…` parameters; run it as a query of its own"
            )));
        }
        Ok(Query {
            sql: self.sql,
            vars: self.vars,
        })
    }
}

/// Renders `node` with its values inline, returning the text and the errors found in it.
pub(crate) fn render_inline(node: &(impl Render + ?Sized)) -> (String, Vec<String>) {
    let mut renderer = Renderer::new(Mode::Inline);
    node.render(&mut renderer);
    (renderer.sql, renderer.errors)
}

/// A rendered query: SurrealQL text and the variables its parameters refer to.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    sql: String,
    vars: Variables,
}

impl Query {
    /// The SurrealQL text.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// The bound variables, keyed by parameter name without `$`.
    pub fn vars(&self) -> &Variables {
        &self.vars
    }

    /// Splits the query into its text and variables.
    pub fn into_parts(self) -> (String, Variables) {
        (self.sql, self.vars)
    }
}

/// Anything that renders into SurrealQL.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a query fragment",
    label = "cannot be rendered into SurrealQL",
    note = "use an Umbriel statement, field, parameter or value here"
)]
pub trait Render {
    /// Writes this node through `out`.
    fn render(&self, out: &mut Renderer);

    /// Renders the parameterised query and its variables.
    fn to_query(&self) -> Result<Query> {
        let mut renderer = Renderer::new(Mode::Bind);
        self.render(&mut renderer);
        renderer.into_query()
    }

    /// Renders the query with values inline, for diagnostics. Execution always binds.
    fn to_debug_sql(&self) -> String {
        render_inline(self).0
    }
}

impl<T: Render + ?Sized> Render for &T {
    fn render(&self, out: &mut Renderer) {
        (**self).render(out);
    }
}

/// Lists render comma-separated, as in projections, assignments and arguments.
impl<T: Render> Render for [T] {
    fn render(&self, out: &mut Renderer) {
        out.push_joined(self, ", ");
    }
}

impl<T: Render> Render for Vec<T> {
    fn render(&self, out: &mut Renderer) {
        out.push_joined(self, ", ");
    }
}

impl<T: Render> Render for Option<T> {
    fn render(&self, out: &mut Renderer) {
        if let Some(node) = self {
            node.render(out);
        }
    }
}

/// A node shared by every builder that holds it: cloning a fragment shares the node instead of
/// copying it. Used where a builder holds nodes of different types, such as the statements of a
/// chain or the target of a path.
#[derive(Clone)]
pub struct Fragment(Arc<dyn Render + Send + Sync>);

impl Fragment {
    /// Wraps a node.
    pub fn new(node: impl Render + Send + Sync + 'static) -> Self {
        Self(Arc::new(node))
    }
}

impl Render for Fragment {
    fn render(&self, out: &mut Renderer) {
        self.0.render(out);
    }
}

impl fmt::Debug for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Fragment")
            .field(&self.to_debug_sql())
            .finish()
    }
}

/// Fragments are equal when they are the same node or render the same debug SQL.
impl PartialEq for Fragment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.to_debug_sql() == other.to_debug_sql()
    }
}

/// A node with an error found while building it, reported when the query is rendered.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Checked<N> {
    node: N,
    error: Option<String>,
}

impl<N> Checked<N> {
    pub(crate) fn new(node: N, error: Option<String>) -> Self {
        Self { node, error }
    }
}

impl<N: Render> Render for Checked<N> {
    fn render(&self, out: &mut Renderer) {
        if let Some(error) = &self.error {
            out.push_error(error.as_str());
        }
        out.push(&self.node);
    }
}

/// The words SurrealDB reserves where an identifier can stand, matched in any case: the 3.3.4
/// formatter quotes a name equal to one (`surrealdb-common` `keywords::RESERVED_KEYWORD`), since
/// the parser would read it as the keyword.
const RESERVED_KEYWORDS: &[&str] = &[
    "ALTER", "BEGIN", "BREAK", "CANCEL", "COMMIT", "CONTINUE", "CREATE", "DEFINE", "DELETE",
    "FOR", "IF", "INFO", "INSERT", "KILL", "LIVE", "OPTION", "REBUILD", "RETURN", "RELATE",
    "REMOVE", "SELECT", "LET", "SHOW", "SLEEP", "THROW", "UPDATE", "UPSERT", "USE", "DIFF",
    "RAND", "NONE", "NULL", "AFTER", "BEFORE", "VALUE", "BY", "ALL", "TRUE", "FALSE", "WHERE",
    "TABLE", "TB", "SEQUENCE", "FUNCTION",
];

/// Writes `name` as a SurrealQL identifier: bare when it is a plain identifier, otherwise
/// backtick-quoted with the same escapes SurrealDB uses for table names. A reserved keyword, or
/// `NaN` and `Infinity`, which read as numbers, is quoted too.
pub(crate) fn write_ident(out: &mut String, name: &str) {
    let plain = name
        .bytes()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && name != "NaN"
        && name != "Infinity"
        && !RESERVED_KEYWORDS
            .iter()
            .any(|keyword| keyword.eq_ignore_ascii_case(name));
    if plain {
        out.push_str(name);
        return;
    }

    out.push('`');
    for c in name.chars() {
        match c {
            '`' => out.push_str("\\`"),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\x08' => out.push_str("\\u{8}"),
            '\x0C' => out.push_str("\\f"),
            c => out.push(c),
        }
    }
    out.push('`');
}
