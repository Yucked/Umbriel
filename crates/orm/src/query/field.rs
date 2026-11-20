use std::borrow::Cow;

use surrealdb::types::{self, Table};

use crate::{
    Aliasable, Arrow, Clause, Conditional, Filter, Fragment, Ident, NumberLike, Operatable, Param,
    Render, Renderer,
    statements::{Order, Orderables, Splittables},
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
    /// `.{depth[+instruction]}[(path)]`: a recursion.
    Recurse(Recurse),
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
    step: Step,
    target: GraphTarget,
}

/// What a step follows: graph edges in a direction, or record references (`<~`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// `->`, `<-` or `<->`; none after a clause that set no arrow.
    Edges(Option<Arrow>),
    /// `<~`: the records whose `REFERENCE` fields point to the value.
    References,
}

#[derive(Debug, Clone, PartialEq)]
enum GraphTarget {
    Lookup(Box<Lookup>),
    Expr(Fragment),
    /// Record ids, as for [`IdiomPart::Records`].
    Records(Fragment, Vec<types::RecordId>),
}

/// What one graph or reference step looks up, for [`Field::outgoing`], [`Field::incoming`],
/// [`Field::both_ways`] and [`Field::referenced_by`]: tables, or any table (`?`), optionally as a
/// `SELECT` over the edges with its clauses.
///
/// A step over one table and no clauses is written bare (`->likes`); otherwise it is
/// parenthesized (`->(likes, follows WHERE since > $date LIMIT 5)`).
///
/// ```rust
/// use umbriel::{*, statements::order};
///
/// let person = Field::from_expr(Param::new("person"));
/// assert_eq!(person.clone().both_ways(Lookup::any()).to_debug_sql(), "$person<->?");
/// assert_eq!(
///     person
///         .outgoing(
///             Lookup::tables(["likes"])
///                 .select([Field::new("out"), Field::new("since")])
///                 .where_(Field::new("since").gt(2020))
///                 .order_by(order(Field::new("since")).desc())
///                 .limit(5),
///         )
///         .to_debug_sql(),
///     "$person->(SELECT out, since FROM likes WHERE since > 2020 ORDER BY since DESC LIMIT 5)"
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Lookup {
    /// The tables, each with the `REFERENCE` field a `<~` step follows; empty for `?`.
    subjects: Vec<(Table, Option<Ident>)>,
    select: Option<Vec<Field>>,
    only: bool,
    condition: Option<Filter>,
    split: Vec<Field>,
    group: Vec<Field>,
    order: Vec<Order>,
    limit: Option<NumberLike>,
    start: Option<NumberLike>,
    alias: Option<Field>,
}

impl Lookup {
    /// A step over `tables`; with none, over any table (`?`).
    pub fn tables(tables: impl IntoIterator<Item = impl Into<Table>>) -> Self {
        Self::of(
            tables
                .into_iter()
                .map(|table| (table.into(), None))
                .collect(),
        )
    }

    /// `?`: a step over any table.
    pub fn any() -> Self {
        Self::of(Vec::new())
    }

    /// `table FIELD field`: for [`Field::referenced_by`], the records of `table` whose
    /// `REFERENCE` field `field` points to the value.
    pub fn referencing(table: impl Into<Table>, field: impl Into<Ident>) -> Self {
        Self::of(vec![(table.into(), Some(field.into()))])
    }

    fn of(subjects: Vec<(Table, Option<Ident>)>) -> Self {
        Self {
            subjects,
            select: None,
            only: false,
            condition: None,
            split: Vec::new(),
            group: Vec::new(),
            order: Vec::new(),
            limit: None,
            start: None,
            alias: None,
        }
    }

    /// `SELECT fields FROM …`: the step gives these fields of each edge, not the edge.
    pub fn select(mut self, fields: impl IntoIterator<Item = impl Into<Field>>) -> Self {
        self.select = Some(fields.into_iter().map(Into::into).collect());
        self
    }

    /// `ONLY`, with [`select`](Self::select): one edge rather than an array of them.
    pub fn only(mut self) -> Self {
        self.only = true;
        self
    }

    /// `WHERE condition`: only the edges matching `condition`.
    pub fn where_(mut self, condition: impl Conditional) -> Self {
        self.condition = Some(Filter::new(condition));
        self
    }

    /// `SPLIT fields`. SurrealDB requires them to be selected, and refuses `SPLIT` with
    /// `GROUP BY` (a step with both fails to render).
    pub fn split(mut self, fields: impl Into<Splittables>) -> Self {
        self.split.extend(Vec::<Field>::from(fields.into()));
        self
    }

    /// `GROUP BY fields`, which SurrealDB requires to be selected.
    pub fn group_by(mut self, fields: impl Into<Splittables>) -> Self {
        self.group.extend(Vec::<Field>::from(fields.into()));
        self
    }

    /// `ORDER BY …`.
    pub fn order_by(mut self, orderables: impl Into<Orderables>) -> Self {
        self.order.extend(Vec::<Order>::from(orderables.into()));
        self
    }

    /// `LIMIT limit`.
    pub fn limit(mut self, limit: impl Into<NumberLike>) -> Self {
        self.limit = Some(limit.into());
        self
    }

    /// `START start`.
    pub fn start(mut self, start: impl Into<NumberLike>) -> Self {
        self.start = Some(start.into());
        self
    }

    /// `AS alias`: the step's result is kept under `alias`.
    pub fn alias(mut self, alias: impl Into<Field>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    /// Whether the step has no clauses, so `?` or one table is written bare.
    fn is_plain(&self) -> bool {
        self.select.is_none()
            && !self.only
            && self.condition.is_none()
            && self.split.is_empty()
            && self.group.is_empty()
            && self.order.is_empty()
            && self.limit.is_none()
            && self.start.is_none()
            && self.alias.is_none()
    }

    /// The table of a step over one table with nothing else, which is written bare.
    fn bare_table(&self) -> Option<&Table> {
        match self.subjects.as_slice() {
            [(table, None)] if self.is_plain() => Some(table),
            _ => None,
        }
    }

    fn push_subjects(&self, out: &mut Renderer) {
        if self.subjects.is_empty() {
            out.push_str("?");
            return;
        }
        for (index, (table, field)) in self.subjects.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push(table);
            if let Some(field) = field {
                out.push_str(" FIELD ").push(field);
            }
        }
    }
}

impl Render for Lookup {
    fn render(&self, out: &mut Renderer) {
        if let Some(table) = self.bare_table() {
            out.push(table);
            return;
        }
        if self.subjects.is_empty() && self.is_plain() {
            out.push_str("?");
            return;
        }
        if self.only && self.select.is_none() {
            out.push_error("ONLY in a graph step needs a SELECT");
        }
        if !self.split.is_empty() && !self.group.is_empty() {
            out.push_error("SPLIT and GROUP BY cannot be used together in a graph step");
        }
        out.push_str("(");
        if let Some(fields) = &self.select {
            out.push_str("SELECT ")
                .push_joined(fields, ", ")
                .push_str(" FROM ");
            if self.only {
                out.push_str("ONLY ");
            }
        }
        self.push_subjects(out);
        if let Some(condition) = &self.condition {
            out.push_str(" WHERE ").push(condition);
        }
        if !self.split.is_empty() {
            out.push_str(" SPLIT ").push_joined(&self.split, ", ");
        }
        if !self.group.is_empty() {
            out.push_str(" GROUP BY ").push_joined(&self.group, ", ");
        }
        if !self.order.is_empty() {
            out.push_str(" ORDER BY ").push_joined(&self.order, ", ");
        }
        if let Some(limit) = &self.limit {
            out.push_str(" LIMIT ").push(limit);
        }
        if let Some(start) = &self.start {
            out.push_str(" START ").push(start);
        }
        if let Some(alias) = &self.alias {
            out.push_str(" AS ").push(alias);
        }
        out.push_str(")");
    }
}

/// A recursion over the rest of a path, or over a nested path: `.{depth[+instruction]}` and
/// `.{…}(path)`, for [`Field::recurse`]. Recursion follows record links and graph edges; a
/// value that is not a record fails the query.
///
/// SurrealDB requires a minimum of at least 1 and, when given, a maximum of at most its
/// `idiom_recursion_limit` (256 by default); without a maximum, a walk still going at that limit
/// fails. A depth of 0 or a minimum above the maximum fails to render.
///
/// ```rust
/// use umbriel::*;
///
/// let alice = Field::from_expr(Param::new("alice"));
/// let friends = Field::default().outgoing(Lookup::tables(["knows"])).outgoing(Lookup::tables(["person"]));
/// assert_eq!(
///     alice.clone().recurse(Recurse::times(2).nested(friends.clone())).to_debug_sql(),
///     "$alice.{2}(->knows->person)"
/// );
/// assert_eq!(
///     alice.recurse(Recurse::range(1..=3).collect().inclusive().nested(friends)).to_debug_sql(),
///     "$alice.{1..3+collect+inclusive}(->knows->person)"
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Recurse {
    depth: Depth,
    instruction: Option<Instruction>,
    inclusive: bool,
    nested: Option<Field>,
}

/// How many steps a recursion takes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Depth {
    /// `n`: exactly `n` steps.
    Fixed(u32),
    /// `min..max`, each bound optional. An excluded bound has no SurrealQL form, so it fails.
    Range(Option<u32>, Option<u32>, Option<&'static str>),
}

/// What a recursion collects, rather than the value at its last step.
#[derive(Debug, Clone, PartialEq)]
enum Instruction {
    Path,
    Collect,
    Shortest(ShortestTarget),
}

/// The record a `+shortest` recursion searches for: a record id or a parameter holding one.
#[derive(Debug, Clone, PartialEq)]
pub enum ShortestTarget {
    /// `$param`
    Param(Param),
    /// `table:key`
    Record(types::RecordId),
}

impl From<Param> for ShortestTarget {
    fn from(param: Param) -> Self {
        Self::Param(param)
    }
}

impl From<&Param> for ShortestTarget {
    fn from(param: &Param) -> Self {
        Self::Param(param.clone())
    }
}

impl From<types::RecordId> for ShortestTarget {
    fn from(record: types::RecordId) -> Self {
        Self::Record(record)
    }
}

impl Recurse {
    /// `.{n}`: exactly `n` steps; `NONE` when the walk ends sooner.
    pub fn times(steps: u32) -> Self {
        Self::with(Depth::Fixed(steps))
    }

    /// `.{min..max}`: from `min` (1 when unbounded) to `max` steps, both included. `1..=3` is
    /// `.{1..3}`, `2..` is `.{2..}` and `..` is `.{..}`; `1..4` is `.{1..3}`.
    pub fn range(range: impl std::ops::RangeBounds<u32>) -> Self {
        use std::ops::Bound;
        let mut invalid = None;
        let min = match range.start_bound() {
            Bound::Included(min) => Some(*min),
            Bound::Excluded(min) => min.checked_add(1).or_else(|| {
                invalid = Some("the recursion's minimum overflows");
                None
            }),
            Bound::Unbounded => None,
        };
        let max = match range.end_bound() {
            Bound::Included(max) => Some(*max),
            Bound::Excluded(max) => max.checked_sub(1).or_else(|| {
                invalid = Some("the recursion's range is empty");
                None
            }),
            Bound::Unbounded => None,
        };
        Self::with(Depth::Range(min, max, invalid))
    }

    fn with(depth: Depth) -> Self {
        Self {
            depth,
            instruction: None,
            inclusive: false,
            nested: None,
        }
    }

    /// `+path`: every walk, each as an array of the records it passes.
    pub fn path(mut self) -> Self {
        self.instruction = Some(Instruction::Path);
        self
    }

    /// `+collect`: every record reached, once each, in breadth-first order.
    pub fn collect(mut self) -> Self {
        self.instruction = Some(Instruction::Collect);
        self
    }

    /// `+shortest=target`: the first, shortest walk that reaches `target`, or `NONE`.
    pub fn shortest(mut self, target: impl Into<ShortestTarget>) -> Self {
        self.instruction = Some(Instruction::Shortest(target.into()));
        self
    }

    /// `+inclusive`, with [`path`](Self::path), [`collect`](Self::collect) or
    /// [`shortest`](Self::shortest): the result includes the starting record.
    pub fn inclusive(mut self) -> Self {
        self.inclusive = true;
        self
    }

    /// `(path)`: the recursion follows `path` from each record, and the rest of the outer path
    /// applies to the result. Without it, the recursion follows the rest of the outer path.
    pub fn nested(mut self, path: Field) -> Self {
        self.nested = Some(path);
        self
    }
}

impl Render for Recurse {
    fn render(&self, out: &mut Renderer) {
        out.push_str(".{");
        match self.depth {
            Depth::Fixed(steps) => {
                if steps == 0 {
                    out.push_error("a recursion takes at least one step");
                }
                out.push_display(&steps);
            }
            Depth::Range(min, max, invalid) => {
                if let Some(invalid) = invalid {
                    out.push_error(invalid);
                }
                if min == Some(0) {
                    out.push_error("a recursion's minimum is at least 1");
                }
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    out.push_error(format!(
                        "the recursion's minimum {min} exceeds its maximum {max}"
                    ));
                }
                if let Some(min) = min {
                    out.push_display(&min);
                }
                out.push_str("..");
                if let Some(max) = max {
                    out.push_display(&max);
                }
            }
        }
        match &self.instruction {
            Some(Instruction::Path) => {
                out.push_str("+path");
            }
            Some(Instruction::Collect) => {
                out.push_str("+collect");
            }
            Some(Instruction::Shortest(target)) => {
                out.push_str("+shortest=");
                match target {
                    ShortestTarget::Param(param) => {
                        out.push(param);
                    }
                    ShortestTarget::Record(record) => {
                        out.push_syntax(record);
                    }
                }
            }
            None if self.inclusive => {
                out.push_error("+inclusive needs +path, +collect or +shortest");
            }
            None => {}
        }
        if self.inclusive {
            out.push_str("+inclusive");
        }
        out.push_str("}");
        if let Some(nested) = &self.nested {
            out.push_str("(");
            if matches!(
                nested.parts.first(),
                Some(IdiomPart::Field(_) | IdiomPart::AllFields)
            ) {
                out.push_str(".");
            }
            out.push(nested).push_str(")");
        }
    }
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
        let mut lookup = Lookup::tables(tables);
        lookup.condition = condition;
        Self {
            step: Step::Edges(arrow),
            target: GraphTarget::Lookup(Box::new(lookup)),
        }
    }

    pub(crate) fn expr(arrow: Option<Arrow>, expr: Fragment) -> Self {
        Self {
            step: Step::Edges(arrow),
            target: GraphTarget::Expr(expr),
        }
    }

    pub(crate) fn records(
        arrow: Option<Arrow>,
        expr: Fragment,
        records: Vec<types::RecordId>,
    ) -> Self {
        Self {
            step: Step::Edges(arrow),
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
        match self.step {
            Step::Edges(arrow) => {
                out.push(&arrow);
            }
            Step::References => {
                out.push_str("<~");
            }
        }
        match &self.target {
            GraphTarget::Lookup(lookup) => {
                if self.step != Step::References
                    && lookup.subjects.iter().any(|(_, field)| field.is_some())
                {
                    out.push_error("a FIELD subject belongs to a reference step (`<~`)");
                }
                out.push(&**lookup);
            }
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

    /// `->lookup`: follows the outgoing edges the lookup names. (Not `out`/`in_`: an inherent
    /// `in_` would hide [`Operatable::in_`](crate::Operatable::in_), `INSIDE`.)
    pub fn outgoing(self, lookup: Lookup) -> Self {
        self.step(Step::Edges(Some(Arrow::Right)), lookup)
    }

    /// `<-lookup`: follows the incoming edges the lookup names.
    pub fn incoming(self, lookup: Lookup) -> Self {
        self.step(Step::Edges(Some(Arrow::Left)), lookup)
    }

    /// `<->lookup`: follows the edges the lookup names in both directions.
    pub fn both_ways(self, lookup: Lookup) -> Self {
        self.step(Step::Edges(Some(Arrow::Both)), lookup)
    }

    /// `<~lookup`: the records whose `REFERENCE` fields point to the value, from the lookup's
    /// tables; [`Lookup::referencing`] narrows them to one field.
    ///
    /// ```rust
    /// use umbriel::*;
    ///
    /// let author = Field::from_expr(Param::new("author"));
    /// assert_eq!(
    ///     author.referenced_by(Lookup::referencing("post", "author")).to_debug_sql(),
    ///     "$author<~(post FIELD author)"
    /// );
    /// ```
    pub fn referenced_by(self, lookup: Lookup) -> Self {
        self.step(Step::References, lookup)
    }

    fn step(mut self, step: Step, lookup: Lookup) -> Self {
        self.parts.push(IdiomPart::Graph(Graph {
            step,
            target: GraphTarget::Lookup(Box::new(lookup)),
        }));
        self
    }

    /// A recursion: `.{depth[+instruction]}[(path)]`. See [`Recurse`].
    pub fn recurse(mut self, recursion: Recurse) -> Self {
        self.parts.push(IdiomPart::Recurse(recursion));
        self
    }

    /// Whether the path is a single expression, such as a parameter holding a field name, whose
    /// field is known only when SurrealDB evaluates it.
    pub(crate) fn is_expression(&self) -> bool {
        matches!(self.parts.as_slice(), [IdiomPart::Expr(_)])
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
            GraphTarget::Lookup(lookup) => match lookup.bare_table() {
                Some(table) => EdgeStep::Table(table),
                None => EdgeStep::Unknown,
            },
            GraphTarget::Records(_, records) => match records.as_slice() {
                [record] => EdgeStep::Record(record),
                _ => EdgeStep::Unknown,
            },
            GraphTarget::Expr(_) => EdgeStep::Unknown,
        };
        let to = to.known_records();
        // A two-way or reference path names no direction, so it is not a relation.
        let (r#in, out) = match edge.step {
            Step::Edges(Some(Arrow::Left)) => (to, from),
            Step::Edges(Some(Arrow::Right) | None) => (from, to),
            Step::Edges(Some(Arrow::Both)) | Step::References => return None,
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
                IdiomPart::Recurse(recursion) => {
                    out.push(recursion);
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
