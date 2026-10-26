// Statement syntax
// RELATE [ ONLY ] [ OR UPDATE ] @from_record -> @table | @edge_record -> @to_record
// 	[ CONTENT @value
// 	  | SET @field = @value ...
// 	]
// 	[ RETURN NONE | RETURN BEFORE | RETURN AFTER | RETURN DIFF | RETURN @statement_param, ... | RETURN VALUE @statement_param ]
// 	[ TIMEOUT @duration ]
// ;
use std::marker::PhantomData;

use crate::{DurationLike, Edge, Field, Renderer, Setter, ValueLike};

use super::{
    Cardinality, Many, One, ReturnAfter, ReturnMode, Returning, Statement, data::DataClause,
    impl_statement, returning::return_methods,
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
        out.push(&self.relation).push(&self.data);
        mode.render(out);
        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push_inlined(timeout);
        }
    }
}

/// Implements the data clauses and `TIMEOUT` both RELATE builders share.
macro_rules! relate_clauses {
    ($statement:ident) => {
        impl<T: Edge, R: ReturnMode, C: Cardinality> $statement<T, R, C> {
            /// Sets the edge's content (`CONTENT`).
            pub fn content(mut self, content: T) -> Self {
                self.body.data = Some(DataClause::Content(ValueLike::value(content)));
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

            /// Sets the timeout duration for the query. It is written as a literal.
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
return_methods!([T: Edge] RelateStatement<T> { none, after, diff, value, fields, links });

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

return_methods!([T: Edge] RelateOrUpdateStatement<T> { none, before, after, diff, value, fields, links });

impl<T: Edge, R: ReturnMode, C: Cardinality> Statement for RelateOrUpdateStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        self.body.render(true, C::ONLY, &self.mode, out);
    }
}
