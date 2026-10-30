// Statement syntax
// RELATE [ ONLY ] [ OR UPDATE ] @from_record -> @table | @edge_record -> @to_record
// 	[ CONTENT @value
// 	  | SET @field = @value ...
// 	]
// The parser also accepts `MERGE`, `PATCH`, `REPLACE` and `UNSET`, which the engine applies as for
// every other write (`doc/alter.rs` `process_record_data` in 3.3.4), keeping `in` and `out`.
// 	[ RETURN NONE | RETURN BEFORE | RETURN AFTER | RETURN DIFF | RETURN @statement_param, ... | RETURN VALUE @statement_param ]
// 	[ TIMEOUT @duration ]
// ;
use std::marker::PhantomData;

use surrealdb::types::{SurrealValue, ToSql};

use crate::{
    DurationLike, Edge, Field, ModelMerge, PatchOp, Renderer, Setter, ValueLike,
    query::field::{EdgeStep, Relation},
};

use super::{
    Cardinality, Many, One, ReturnAfter, ReturnMode, Returning, Statement,
    data::{ContentIds, DataClause, Unsettables},
    impl_statement,
    returning::return_methods,
};

/// Creates a new RELATE statement.
///
/// # Arguments
///
/// * `connection` - built using `with` method on a node. e.g `Student::with(..).writes__(..).book(..)`.
///   An endpoint given as a record id of the wrong model does not compile. Endpoints given as a
///   parameter or subquery are checked only by the database, when the edge table defines them:
///   `TYPE RELATION IN … OUT …` restricts the tables an endpoint may belong to, and `ENFORCED`
///   requires the endpoint records to exist. Without either, they are not validated.
///
/// The edge id can be given in the path, e.g. `writes__(Writes::id("w1"))`; relating an existing
/// edge id again needs [`or_update`](RelateStatement::or_update) since SurrealDB 3.1.5.
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::relate};
/// # #[model(node, table = "student")]
/// # pub struct Student {
/// #     pub id: RecordId<Self>,
/// #     #[orm(relate(model = "StudentWritesBook", connection = "->writes->book"))]
/// #     pub books: Relate<Book>,
/// # }
/// # #[model(node, table = "book")]
/// # pub struct Book { pub id: RecordId<Self> }
/// # #[model(edge, table = "writes")]
/// # pub struct Writes<In: Node, Out: Node> {
/// #     pub id: RecordId<Self>,
/// #     #[orm(link_one = "In")]
/// #     pub r#in: LinkOne<In>,
/// #     #[orm(link_one = "Out")]
/// #     pub out: LinkOne<Out>,
/// #     pub score: u8,
/// # }
/// # pub type StudentWritesBook = Writes<Student, Book>;
///
/// let score = StudentWritesBook::schema().score;
///
/// // An edge between two records, setting a field on it.
/// let statement = relate::<StudentWritesBook>(
///     Student::with(Student::id("alice")).writes__(E).book(Book::id("rust")),
/// )
/// .set(score.equal_to(5u8));
/// assert_eq!(
///     statement.to_debug_sql(),
///     "RELATE student:alice->writes->book:rust SET score = 5;"
/// );
///
/// // An edge between every given student and the book.
/// let statement = relate::<StudentWritesBook>(
///     Student::with([Student::id("alice"), Student::id("bob")])
///         .writes__(E)
///         .book(Book::id("rust")),
/// )
/// .return_none();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "RELATE [student:alice, student:bob]->writes->book:rust RETURN NONE;"
/// );
/// ```
pub fn relate<T: Edge>(connection: impl Into<Field>) -> RelateStatement<T> {
    RelateStatement {
        body: RelateBody::new(connection.into()),
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// Creates a new RELATE ONLY statement, which returns a single edge.
pub fn relate_only<T: Edge>(
    connection: impl Into<Field>,
) -> RelateStatement<T, ReturnAfter<T>, One> {
    RelateStatement {
        body: RelateBody::new(connection.into()),
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

#[derive(Debug, Clone)]
struct RelateBody {
    /// The graph path `from->edge->to`.
    relation: Field,
    data: Option<DataClause>,
    timeout: Option<DurationLike>,
}

impl RelateBody {
    fn new(relation: Field) -> Self {
        Self {
            relation,
            data: None,
            timeout: None,
        }
    }

    fn render(&self, or_update: bool, only: bool, mode: &impl ReturnMode, out: &mut Renderer) {
        out.push_str("RELATE ");
        if only {
            out.push_str("ONLY ");
        }
        if or_update {
            out.push_str("OR UPDATE ");
        }
        if let (Some(content), Some(relation)) = (
            DataClause::content_ids(self.data.as_ref()),
            self.relation.relation(),
        ) {
            push_content_errors(content, &relation, out);
        }
        out.push(&self.relation).push(&self.data);
        mode.render(out);
        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push(timeout);
        }
    }
}

/// Adds an error for each record id in `CONTENT` that contradicts what the path names when it is
/// built: the edge record, the edge table, or the `in` and `out` endpoints. Under an edge record
/// step SurrealDB would reject them when the statement runs; under a table step it would silently
/// move a record of another table into the edge table and replace `in` and `out` with the path's
/// endpoints. Parts of the path given as a parameter or subquery are left to the database.
fn push_content_errors(content: &ContentIds, relation: &Relation<'_>, out: &mut Renderer) {
    match relation.edge {
        EdgeStep::Record(edge) if content.id != *edge => {
            out.push_error(format!(
                "the content's record `{}` is not the edge record `{}` in the path",
                content.id.to_sql(),
                edge.to_sql()
            ));
        }
        EdgeStep::Table(table) if content.id.table != *table => {
            out.push_error(format!(
                "the content's record `{}` is not in the path's edge table `{}`",
                content.id.to_sql(),
                table.as_str()
            ));
        }
        _ => {}
    }
    for (name, link, endpoints) in [
        ("in", &content.r#in, relation.r#in),
        ("out", &content.out, relation.out),
    ] {
        let Some(link) = link else {
            continue;
        };
        if let Some(endpoint) = endpoints.iter().find(|endpoint| *endpoint != link) {
            out.push_error(format!(
                "the content's `{name}` is `{}`, but the path relates `{}`",
                link.to_sql(),
                endpoint.to_sql()
            ));
        }
    }
}

/// Implements the data clauses and `TIMEOUT` both RELATE builders share.
macro_rules! relate_clauses {
    ($statement:ident) => {
        impl<T: Edge, R: ReturnMode, C: Cardinality> $statement<T, R, C> {
            /// Sets the edge's content (`CONTENT`). Its id, `in` and `out` must agree with the
            /// record ids the path was built from: the edge record, or a record in the edge
            /// table, and the endpoints. Otherwise the statement fails when it is rendered.
            pub fn content(mut self, content: T) -> Self {
                self.body.data = Some(DataClause::content(content));
                self
            }

            /// Sets fields of the edge (`SET`), including incrementing and decrementing numeric
            /// values and adding or removing values from arrays, as `UPDATE`'s
            /// [`set`](crate::statements::UpdateStatement::set) does. Repeated calls add to the
            /// assignments.
            pub fn set(mut self, settables: impl Into<Vec<Setter>>) -> Self {
                self.body.data = Some(DataClause::set(self.body.data.take(), settables.into()));
                self
            }

            /// Merges the non-`Keep` fields of a generated `<Edge>Merge` into the edge (`MERGE`):
            /// into a new edge, or into the existing one under `OR UPDATE`.
            pub fn merge(mut self, merge: impl ModelMerge<Model = T>) -> Self {
                self.body.data = Some(DataClause::Merge(ValueLike::new(merge.into_object())));
                self
            }

            /// Applies JSON Patch operations to the edge (`PATCH`); repeated calls add to the
            /// operations.
            pub fn patch(mut self, patch_op: impl Into<Vec<PatchOp>>) -> Self {
                self.body.data = Some(DataClause::patch(self.body.data.take(), patch_op.into()));
                self
            }

            /// Replaces the edge's fields with the given value (`REPLACE`), respecting `READONLY`
            /// fields. The engine keeps the edge's `in` and `out`.
            pub fn replace(mut self, replacement: impl SurrealValue) -> Self {
                self.body.data = Some(DataClause::Replace(ValueLike::value(replacement)));
                self
            }

            /// Removes fields from the edge (`UNSET`); under `OR UPDATE` it removes stored
            /// properties of the existing edge. Repeated calls add to the fields.
            pub fn unset(mut self, fields: impl Into<Unsettables>) -> Self {
                self.body.data = Some(DataClause::unset(self.body.data.take(), fields.into().0));
                self
            }

            /// Sets the timeout duration for the query, bound as a query parameter.
            pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
                self.body.timeout = Some(duration.into());
                self
            }

            fn with_mode<M: ReturnMode>(self, mode: M) -> $statement<T, M, C> {
                $statement {
                    body: self.body,
                    mode,
                    cardinality: PhantomData,
                }
            }
        }

        impl_statement!([T: Edge, R: ReturnMode, C: Cardinality] $statement<T, R, C>);

        impl<T: Edge, R: ReturnMode, C: Cardinality> Returning for $statement<T, R, C> {
            type Mode = R;
            type Cardinality = C;
        }
    };
}

/// A RELATE statement. `R` is its return mode and `C` whether it is written with `ONLY`; both
/// decide the type [`Returning::execute`] returns.
#[derive(Debug, Clone)]
pub struct RelateStatement<T, R = ReturnAfter<T>, C = Many> {
    body: RelateBody,
    mode: R,
    cardinality: PhantomData<(fn() -> T, C)>,
}

impl<T: Edge, R: ReturnMode, C: Cardinality> RelateStatement<T, R, C> {
    /// `RELATE OR UPDATE`: an edge whose id already exists is updated instead of failing. Since
    /// SurrealDB 3.1.5, relating an existing edge id without it updates the edge with a warning,
    /// which is to become an error.
    pub fn or_update(self) -> RelateOrUpdateStatement<T, R, C> {
        RelateOrUpdateStatement {
            body: self.body,
            mode: self.mode,
            cardinality: PhantomData,
        }
    }
}

relate_clauses!(RelateStatement);

// `RETURN BEFORE` is offered only with `OR UPDATE`: a new edge has no earlier state.
return_methods!([T: Edge] RelateStatement<T> { none, null, after, diff, value, fields, links });

impl<T: Edge, R: ReturnMode, C: Cardinality> Statement for RelateStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        self.body.render(false, C::ONLY, &self.mode, out);
    }
}

/// A `RELATE OR UPDATE` statement, from [`RelateStatement::or_update`]. It also offers
/// `RETURN BEFORE`, the edge as it was when an existing one is updated.
#[derive(Debug, Clone)]
pub struct RelateOrUpdateStatement<T, R = ReturnAfter<T>, C = Many> {
    body: RelateBody,
    mode: R,
    cardinality: PhantomData<(fn() -> T, C)>,
}

relate_clauses!(RelateOrUpdateStatement);

return_methods!([T: Edge] RelateOrUpdateStatement<T> { none, null, before, after, diff, value, fields, links });

impl<T: Edge, R: ReturnMode, C: Cardinality> Statement for RelateOrUpdateStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        self.body.render(true, C::ONLY, &self.mode, out);
    }
}
