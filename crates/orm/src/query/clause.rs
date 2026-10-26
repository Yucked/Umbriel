use surrealdb::types::{self, Table, ToSql};

use crate::{
    Arrow, Conditional, Fragment, Graph, Ident, IdiomPart, Model, NumberLike, Operatable,
    Operation, Param, RecordId, Render, Renderer, Setter, ValueLike,
    render::Checked,
    statements::{LetStatement, Subquery},
};

use super::Filter;

/// Use to generate a filter for a query. Can be used as part of a statement
/// or filtering of a node or edge.
pub fn where_(condition: impl Conditional) -> Filter {
    Filter::new(condition)
}

/// Used as argument in node and edges for filtering or selecting.
#[derive(Debug, Clone)]
pub enum ClauseType {
    /// Stands for `[*]`
    All,
    /// Stands for `[$]`
    Last,
    /// Stands for a specific index value e.g `[3]`
    Index(Index),
    /// No filter
    Empty,
    /// Stands for a WHERE clause e.g `[WHERE id = 1]`
    Where(Filter),
    /// Stands for a full statement
    Subquery(Subquery),
    /// Stands for a record id e.g `person:charlie`. Useful in a RELATE query.
    Id(types::RecordId),
    /// Stands for a list of record ids, e.g. the endpoints of a RELATE between every pair.
    Ids(Vec<types::RecordId>),
    /// Stands for a parameter e.g `$name`
    Param(Param),
    /// Used for filtering on multiple edges
    AnyEdgeFilter(AnyEdgeFilter),
}

#[derive(Debug, Clone)]
enum ClauseName {
    Table(Table),
    Field(Ident),
}

/// A step in a graph traversal or field path: what to select at this step and how to filter it.
#[derive(Debug, Clone)]
pub struct Clause {
    kind: ClauseType,
    arrow: Option<Arrow>,
    name: Option<ClauseName>,
}

impl Clause {
    /// Create a new Clause
    pub fn new(kind: ClauseType) -> Self {
        Self {
            kind,
            arrow: None,
            name: None,
        }
    }

    /// Create a new Clause with arrow. This is used in the macro for building a graph query.
    pub fn with_arrow(mut self, arrow: impl Into<Arrow>) -> Self {
        self.arrow = Some(arrow.into());
        self
    }

    /// Attach the table this step traverses to. A record id clause must belong to it.
    pub fn with_table(mut self, table: impl Into<Table>) -> Self {
        self.name = Some(ClauseName::Table(table.into()));
        self
    }

    /// Attach the field this step accesses.
    pub fn with_field(mut self, field_name: impl Into<Ident>) -> Self {
        self.name = Some(ClauseName::Field(field_name.into()));
        self
    }

    pub(crate) fn push_parts(self, parts: &mut Vec<IdiomPart>) {
        let Self { kind, arrow, name } = self;
        let target = |node: Fragment| match arrow {
            Some(_) => IdiomPart::Graph(Graph::expr(arrow, node)),
            None => IdiomPart::Expr(node),
        };

        match kind {
            ClauseType::Id(id) => {
                let error = match &name {
                    Some(ClauseName::Table(table)) if id.table != *table => Some(format!(
                        "record `{}` does not belong to table `{}`",
                        id.to_sql(),
                        table.as_str()
                    )),
                    _ => None,
                };
                let id = Checked::new(ValueLike::value(id), error);
                parts.push(target(Fragment::new(id)));
            }
            ClauseType::Ids(ids) => {
                let error = match &name {
                    Some(ClauseName::Table(table)) => {
                        ids.iter().find(|id| id.table != *table).map(|id| {
                            format!(
                                "record `{}` does not belong to table `{}`",
                                id.to_sql(),
                                table.as_str()
                            )
                        })
                    }
                    _ => None,
                };
                let ids = Checked::new(ValueLike::value(ids), error);
                parts.push(target(Fragment::new(ids)));
            }
            ClauseType::Param(param) => parts.push(target(Fragment::new(param))),
            ClauseType::Subquery(subquery) => parts.push(target(subquery.into_fragment())),
            ClauseType::AnyEdgeFilter(filter) => {
                let mut tables = match name {
                    Some(ClauseName::Table(table)) => vec![table],
                    _ => vec![],
                };
                tables.extend(filter.edge_tables);
                parts.push(IdiomPart::Graph(Graph::tables(
                    arrow,
                    tables,
                    filter.condition,
                )));
            }
            kind => {
                match name {
                    Some(ClauseName::Table(table)) => parts.push(match arrow {
                        Some(_) => IdiomPart::Graph(Graph::tables(arrow, vec![table], None)),
                        None => IdiomPart::Expr(Fragment::new(table)),
                    }),
                    Some(ClauseName::Field(field)) => parts.push(IdiomPart::Field(field)),
                    None => {}
                }
                match kind {
                    ClauseType::All => parts.push(IdiomPart::All),
                    ClauseType::Last => parts.push(IdiomPart::Last),
                    ClauseType::Index(index) => parts.push(IdiomPart::Index(index.0)),
                    ClauseType::Where(filter) => parts.push(IdiomPart::Where(filter)),
                    _ => {}
                }
            }
        }
    }
}

macro_rules! clause_wrapper {
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {
        $(
            $(#[$attr])*
            #[derive(Debug, Clone)]
            pub struct $name(Clause);

            impl $name {
                /// Create a new clause with arrow. This is used in the macro for building a graph query.
                pub fn with_arrow(self, arrow: impl Into<Arrow>) -> Self {
                    Self(self.0.with_arrow(arrow))
                }

                /// Attach the table this step traverses to.
                pub fn with_table(self, table: impl Into<Table>) -> Self {
                    Self(self.0.with_table(table))
                }

                /// Attach the field this step accesses.
                pub fn with_field(self, field_name: impl Into<Ident>) -> Self {
                    Self(self.0.with_field(field_name))
                }

                /// The wrapped clause.
                pub fn into_inner(self) -> Clause {
                    self.0
                }
            }

            impl<T: Into<Clause>> From<T> for $name {
                fn from(value: T) -> Self {
                    Self(value.into())
                }
            }
        )*
    };
}

clause_wrapper!(
    /// Clause for a node
    NodeClause,
    /// Clause for an edge
    EdgeClause,
    /// Clause for an embedded object
    ObjectClause,
    /// Clause for a node alias
    NodeAliasClause,
);

impl NodeAliasClause {
    /// The clause as a node clause.
    pub fn into_node_clause(self) -> NodeClause {
        NodeClause(self.0)
    }
}

impl<T: Model> From<RecordId<T>> for Clause {
    fn from(value: RecordId<T>) -> Self {
        Self::new(ClauseType::Id(value.into()))
    }
}

impl<T: Model> From<&RecordId<T>> for Clause {
    fn from(value: &RecordId<T>) -> Self {
        Self::new(ClauseType::Id(value.clone().into()))
    }
}

impl<T: Model> From<Vec<RecordId<T>>> for Clause {
    fn from(value: Vec<RecordId<T>>) -> Self {
        Self::new(ClauseType::Ids(value.into_iter().map(Into::into).collect()))
    }
}

impl<T: Model> From<&[RecordId<T>]> for Clause {
    fn from(value: &[RecordId<T>]) -> Self {
        Self::new(ClauseType::Ids(value.iter().map(Into::into).collect()))
    }
}

impl From<types::RecordId> for Clause {
    fn from(value: types::RecordId) -> Self {
        Self::new(ClauseType::Id(value))
    }
}

impl From<&types::RecordId> for Clause {
    fn from(value: &types::RecordId) -> Self {
        Self::new(ClauseType::Id(value.clone()))
    }
}

impl From<Param> for Clause {
    fn from(value: Param) -> Self {
        Self::new(ClauseType::Param(value))
    }
}

impl From<&Param> for Clause {
    fn from(value: &Param) -> Self {
        Self::new(ClauseType::Param(value.clone()))
    }
}

impl From<LetStatement> for Clause {
    fn from(value: LetStatement) -> Self {
        Self::new(ClauseType::Param(value.get_param()))
    }
}

impl From<Operation> for Clause {
    fn from(value: Operation) -> Self {
        Self::new(ClauseType::Where(Filter::new(value)))
    }
}

impl From<Setter> for Clause {
    fn from(value: Setter) -> Self {
        Self::new(ClauseType::Where(Filter::new(Operation::from(value))))
    }
}

impl From<&Self> for Clause {
    fn from(value: &Self) -> Self {
        value.clone()
    }
}

impl From<Filter> for Clause {
    fn from(value: Filter) -> Self {
        Self::new(ClauseType::Where(value))
    }
}

impl From<&Filter> for Clause {
    fn from(value: &Filter) -> Self {
        Self::new(ClauseType::Where(value.clone()))
    }
}

impl From<Empty> for Clause {
    fn from(_value: Empty) -> Self {
        Self::new(ClauseType::Empty)
    }
}

impl<T: Into<Subquery>> From<T> for Clause {
    fn from(value: T) -> Self {
        Self::new(ClauseType::Subquery(value.into()))
    }
}

impl From<All> for Clause {
    fn from(_value: All) -> Self {
        Self::new(ClauseType::All)
    }
}

impl From<Last> for Clause {
    fn from(_value: Last) -> Self {
        Self::new(ClauseType::Last)
    }
}

impl From<Index> for Clause {
    fn from(value: Index) -> Self {
        Self::new(ClauseType::Index(value))
    }
}

impl From<[usize; 1]> for Clause {
    fn from(value: [usize; 1]) -> Self {
        index(value[0]).into()
    }
}

impl From<AnyEdgeFilter> for Clause {
    fn from(value: AnyEdgeFilter) -> Self {
        Self::new(ClauseType::AnyEdgeFilter(value))
    }
}

/// A clause for a graph step that reaches nodes of `N`, such as the start of `Student::with(..)`
/// or the destination of an edge.
///
/// A record id is accepted only as a `RecordId<N>`, or a list of them, so an endpoint of the wrong
/// model does not compile. Known ids are also checked against the step's table when the query is
/// rendered. Expressions (filters, parameters, subqueries, untyped SDK record ids, `Empty`, …) are
/// accepted as before; their values are known only when the query runs, so the database checks
/// them, and only where the edge table defines `TYPE RELATION IN … OUT …` (endpoint tables) or
/// `ENFORCED` (endpoint records must exist).
pub trait IntoNodeClause<N> {
    /// The clause.
    fn into_node_clause(self) -> NodeClause;
}

impl<N: Model> IntoNodeClause<N> for RecordId<N> {
    fn into_node_clause(self) -> NodeClause {
        Clause::from(self).into()
    }
}

impl<N: Model> IntoNodeClause<N> for &RecordId<N> {
    fn into_node_clause(self) -> NodeClause {
        Clause::from(self).into()
    }
}

impl<N: Model> IntoNodeClause<N> for Vec<RecordId<N>> {
    fn into_node_clause(self) -> NodeClause {
        Clause::from(self).into()
    }
}

impl<N: Model> IntoNodeClause<N> for &[RecordId<N>] {
    fn into_node_clause(self) -> NodeClause {
        Clause::from(self).into()
    }
}

impl<N: Model, const K: usize> IntoNodeClause<N> for [RecordId<N>; K] {
    fn into_node_clause(self) -> NodeClause {
        Clause::from(Vec::from(self)).into()
    }
}

impl<N, S: Into<Subquery>> IntoNodeClause<N> for S {
    fn into_node_clause(self) -> NodeClause {
        Clause::from(self).into()
    }
}

/// Implements `IntoNodeClause<N>` for every `N` for clauses that are not typed record ids.
macro_rules! into_node_clause_untyped {
    ($($source:ty),* $(,)?) => {
        $(
            impl<N> IntoNodeClause<N> for $source {
                fn into_node_clause(self) -> NodeClause {
                    Clause::from(self).into()
                }
            }
        )*
    };
}

into_node_clause_untyped!(
    types::RecordId,
    &types::RecordId,
    Param,
    &Param,
    LetStatement,
    Operation,
    Setter,
    Clause,
    &Clause,
    Filter,
    &Filter,
    Empty,
    All,
    Last,
    Index,
    [usize; 1],
    AnyEdgeFilter,
);

impl<N> IntoNodeClause<N> for NodeClause {
    fn into_node_clause(self) -> NodeClause {
        self
    }
}

impl<N> IntoNodeClause<N> for NodeAliasClause {
    fn into_node_clause(self) -> NodeClause {
        NodeClause(self.0)
    }
}

/// Use when you want no space. Also aliased as `E`.
#[derive(Debug, Clone, Copy)]
pub struct Empty;
pub use Empty as E;

impl Operatable for Empty {}

impl Conditional for Empty {}

impl Render for Empty {
    fn render(&self, _out: &mut Renderer) {}
}

/// stands for `*`
#[derive(Debug, Clone, Copy)]
pub struct All;

impl Render for All {
    fn render(&self, out: &mut Renderer) {
        out.push_str("*");
    }
}

/// stands for `$`
#[derive(Debug, Clone, Copy)]
pub struct Last;

/// stands for index number in a list e.g `[3]`
#[derive(Debug, Clone)]
pub struct Index(NumberLike);

/// Create an index
pub fn index(index: impl Into<NumberLike>) -> Index {
    Index(index.into())
}

impl Render for Index {
    fn render(&self, out: &mut Renderer) {
        out.push(&self.0);
    }
}

/// Creates WHERE filter for one or more edges
#[derive(Debug, Clone)]
pub struct AnyEdgeFilter {
    edge_tables: Vec<Table>,
    condition: Option<Filter>,
}

impl AnyEdgeFilter {
    /// Creates a WHERE condition for one or more edges
    /// # Example
    /// ```rust, ignore
    /// Student::schema()
    ///             .writes__(any_other_edges(&[writes, likes]).where_(timeWritten.less_than_or_equal(50)))
    ///             .book(Empty);
    /// ```
    pub fn where_(mut self, condition: impl Conditional) -> Self {
        self.condition = Some(Filter::new(condition));
        self
    }
}

/// Creates WHERE filter for one or more edges. This allows for pattern like this:
/// ```sql
/// Select all 1st, 2nd, and 3rd level people who this specific person record knows, or likes, as separate outputs
/// ```
/// SELECT ->knows->(? AS f1)->knows->(? AS f2)->(knows, likes AS e3 WHERE influencer = true)->(? AS f3) FROM person:tobie;
///
/// Examples:
///
/// ```rust, ignore
/// # let likes = Table::new("likes");
/// # let writes = StudentWritesBook::table();
/// # let timeWritten = Field::new("timeWritten");
///  Student::with(student_id)
///     .writes__(Empty)
///     .writes__(Empty)
///     .writes__(any_other_edges(&[writes, likes]).where_(timeWritten.less_than_or_equal(50)))
///     .book(book_id)
///     .__as__(Student::aliases().writtenBooks);
/// ```
pub fn any_other_edges(edges: impl Into<crate::Tables>) -> AnyEdgeFilter {
    AnyEdgeFilter {
        edge_tables: edges.into().into(),
        condition: None,
    }
}
