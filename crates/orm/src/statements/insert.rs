// Statement syntax
// INSERT [ RELATION ] [ IGNORE ] INTO @what
// 	[ @value
// 	  | (@fields) VALUES (@values)
// 		[ ON DUPLICATE KEY UPDATE @field = @value ... ]
// 	]
// 	[ RETURN NONE | RETURN BEFORE | RETURN AFTER | RETURN DIFF | RETURN @statement_param, ... | RETURN VALUE @statement_param ]
// ;
use std::{
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
};

use surrealdb::types::{SurrealValue, Value};

use crate::{Edge, Model, Node, Param, Renderer, SchemaGetter, Setter};

use super::{
    Cardinality, Many, ReturnAfter, ReturnMode, Returning, SelectStatement, Statement, Subquery,
    impl_statement, returning::return_methods,
};

/// Creates an INSERT statement into the table of `T`.
///
/// Every model value carries its full record id, so inserting records of one model this way has
/// the same effect as SurrealDB's targetless `INSERT [ … ]`; that syntax itself is not exposed.
///
/// # Arguments
///
/// * `insertables` - a single or list of nodes, or a select statement to copy from another table.
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::{insert, select}};
/// # #[model(node, table = "user")]
/// # pub struct User { pub id: RecordId<Self>, pub name: String, pub age: u8 }
///
/// let alice = || User { id: User::id("alice"), name: "Alice".to_owned(), age: 30 };
/// let bob = User { id: User::id("bob"), name: "Bob".to_owned(), age: 31 };
///
/// // One record or a list, as rows over their fields.
/// assert_eq!(
///     insert(alice()).to_debug_sql(),
///     "INSERT INTO user (age, id, name) VALUES (30, user:alice, 'Alice');"
/// );
/// assert_eq!(
///     insert(vec![alice(), bob]).return_none().to_debug_sql(),
///     "INSERT INTO user (age, id, name) VALUES (30, user:alice, 'Alice'), \
///      (31, user:bob, 'Bob') RETURN NONE;"
/// );
///
/// // Copies the records another query selects.
/// let adults = select(All).from(Table::from("person")).where_(Field::new("age").gt(18));
/// assert_eq!(
///     insert::<User>(adults).to_debug_sql(),
///     "INSERT INTO user (SELECT * FROM person WHERE age > 18);"
/// );
/// ```
pub fn insert<T: Node>(insertables: impl Into<Insertables<T>>) -> InsertStatement<T> {
    let data = match insertables.into() {
        Insertables::Node(node) => rows([node]),
        Insertables::Nodes(nodes) => rows(nodes),
        Insertables::FromSubQuery(query) => Data::Select(query),
    };
    InsertStatement::new(data, false)
}

/// Creates an `INSERT RELATION` statement: inserts edges, each holding its `in` and `out`
/// endpoints and optionally its id. Since SurrealDB 3.1.5 an edge id that already exists is a
/// [`RecordExists`](crate::Error::RecordExists) error unless [`ignore`](InsertStatement::ignore)
/// or [`on_duplicate_key_update`](InsertStatement::on_duplicate_key_update) handles it. It is
/// rejected on `LIGHTWEIGHT` relation tables (3.3.0).
pub fn insert_relation<E: Edge>(edges: impl Into<Insertables<E>>) -> InsertStatement<E> {
    let data = match edges.into() {
        Insertables::Node(edge) => Data::Values(Value::Array(vec![edge.into_value()].into())),
        Insertables::Nodes(edges) => Data::Values(Value::Array(
            edges
                .into_iter()
                .map(SurrealValue::into_value)
                .collect::<Vec<_>>()
                .into(),
        )),
        Insertables::FromSubQuery(query) => Data::Select(query),
    };
    InsertStatement::new(data, true)
}

/// `$input` as `T`'s schema: in [`on_duplicate_key_update`](InsertStatement::on_duplicate_key_update)
/// the fields of the record that could not be inserted, typed like the model's own fields, e.g.
/// `visits.increment_by(input_of::<Page>().visits)`. Bare field names refer to the existing
/// record.
pub fn input_of<T: SchemaGetter>() -> T::Schema {
    T::schema_prefixed(Param::new("input"))
}

#[derive(Debug, Clone)]
enum Data {
    /// Column names and one row of values per record. A record without a column inserts `NONE`.
    Rows {
        fields: Vec<String>,
        rows: Vec<Vec<Value>>,
        errors: Vec<String>,
    },
    /// One object or an array of objects, bound as a value.
    Values(Value),
    /// Copies the result of a subquery.
    Select(Subquery),
}

/// Converts the nodes into rows over the union of their fields, in sorted order.
fn rows<T: Node>(nodes: impl IntoIterator<Item = T>) -> Data {
    let mut errors = vec![];
    let objects: Vec<BTreeMap<String, Value>> = nodes
        .into_iter()
        .map(|node| match node.into_value() {
            Value::Object(object) => object.into_inner(),
            other => {
                errors.push(format!(
                    "a node must convert into an object to be inserted, not {:?}",
                    other.kind()
                ));
                BTreeMap::new()
            }
        })
        .collect();

    let fields: Vec<String> = objects
        .iter()
        .flat_map(BTreeMap::keys)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let rows = objects
        .into_iter()
        .map(|mut object| {
            fields
                .iter()
                .map(|field| object.remove(field).unwrap_or_default())
                .collect()
        })
        .collect();

    Data::Rows {
        fields,
        rows,
        errors,
    }
}

/// Things that can be inserted including a single or list of surrealdb nodes or a select statement
/// when copying from a table to another table.
#[derive(Debug)]
pub enum Insertables<T> {
    /// A single record
    Node(T),
    /// A list of records
    Nodes(Vec<T>),
    /// A select statement
    FromSubQuery(Subquery),
}

impl<T: Model> From<Vec<T>> for Insertables<T> {
    fn from(value: Vec<T>) -> Self {
        Self::Nodes(value)
    }
}

impl<T: Model> From<T> for Insertables<T> {
    fn from(value: T) -> Self {
        Self::Node(value)
    }
}

impl<T: Model> From<SelectStatement> for Insertables<T> {
    fn from(value: SelectStatement) -> Self {
        Self::FromSubQuery(value.into())
    }
}

impl<T: Model> From<&SelectStatement> for Insertables<T> {
    fn from(value: &SelectStatement) -> Self {
        Self::FromSubQuery(value.clone().into())
    }
}

#[derive(Debug, Clone)]
struct InsertBody {
    relation: bool,
    ignore: bool,
    /// Whether the table is bound as a parameter (`INTO $_pN`) rather than written by name.
    table_param: bool,
    data: Data,
    on_duplicate_key_update: Vec<Setter>,
}

/// An INSERT statement. `R` is its return mode, which decides the type
/// [`Returning::execute`] returns. `INSERT` has no `ONLY`, so it always returns a list.
#[derive(Debug, Clone)]
pub struct InsertStatement<T, R = ReturnAfter<T>, C = Many> {
    body: InsertBody,
    mode: R,
    cardinality: PhantomData<(fn() -> T, C)>,
}

impl<T: Model> InsertStatement<T> {
    fn new(data: Data, relation: bool) -> Self {
        Self {
            body: InsertBody {
                relation,
                ignore: false,
                table_param: false,
                data,
                on_duplicate_key_update: vec![],
            },
            mode: ReturnAfter::default_mode(),
            cardinality: PhantomData,
        }
    }
}

impl<T: Model, R: ReturnMode, C: Cardinality> InsertStatement<T, R, C> {
    /// `INSERT IGNORE`: records whose id or unique index value already exists are skipped.
    ///
    /// Since SurrealDB 3.3.4 it skips only those conflicts: any other failure of a record, such as
    /// a type or `ASSERT` error, still fails the statement.
    pub fn ignore(mut self) -> Self {
        self.body.ignore = true;
        self
    }

    /// `INSERT INTO $param`: the target is a parameter bound to `T`'s table, the form SurrealDB
    /// documents for a table held in a parameter and the one the SDK's own `insert` writes. The
    /// parameter is bound by Umbriel, so it always holds `T`'s table and the result stays typed as
    /// `T`; the records are the same as with the table written by name.
    ///
    /// # Examples
    ///
    /// ```
    /// use umbriel::{*, statements::insert};
    /// # #[model(node, table = "user")]
    /// # pub struct User { pub id: RecordId<Self>, pub name: String }
    ///
    /// let alice = User { id: User::id("alice"), name: "Alice".to_owned() };
    /// let query = insert(alice).into_table_param().to_query().expect("valid");
    /// assert_eq!(query.sql(), "INSERT INTO $_p0 (id, name) VALUES ($_p1, $_p2);");
    /// assert_eq!(query.vars().get("_p0"), Some(&types::Value::Table(User::table())));
    /// ```
    pub fn into_table_param(mut self) -> Self {
        self.body.table_param = true;
        self
    }

    /// `ON DUPLICATE KEY UPDATE`: updates a record that already exists instead. Bare fields name
    /// the existing record, and [`input_of`] the fields of the one that could not be inserted.
    /// Repeated calls add to the assignments.
    ///
    /// # Examples
    ///
    /// ```
    /// use umbriel::{*, statements::{input_of, insert}};
    /// # #[model(node, table = "page")]
    /// # pub struct Page { pub id: RecordId<Self>, pub visits: u32 }
    ///
    /// let page = Page { id: Page::id("home"), visits: 1 };
    /// let PageSchema { visits, .. } = Page::schema();
    /// let statement =
    ///     insert(page).on_duplicate_key_update(visits.increment_by(input_of::<Page>().visits));
    /// assert_eq!(
    ///     statement.to_debug_sql(),
    ///     "INSERT INTO page (id, visits) VALUES (page:home, 1) \
    ///      ON DUPLICATE KEY UPDATE visits += $input.visits;"
    /// );
    /// ```
    pub fn on_duplicate_key_update(mut self, setters: impl Into<Vec<Setter>>) -> Self {
        self.body.on_duplicate_key_update.extend(setters.into());
        self
    }

    fn with_mode<M: ReturnMode>(self, mode: M) -> InsertStatement<T, M, C> {
        InsertStatement {
            body: self.body,
            mode,
            cardinality: PhantomData,
        }
    }
}

// `RETURN BEFORE` is not offered: SurrealDB documents it as `NONE` for `INSERT`.
return_methods!([T: Model] InsertStatement<T> { none, after, diff, value, fields, links });

impl<T: Model, R: ReturnMode, C: Cardinality> Statement for InsertStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        let body = &self.body;
        out.push_str("INSERT ");
        if body.relation {
            out.push_str("RELATION ");
        }
        if body.ignore {
            out.push_str("IGNORE ");
        }
        out.push_str("INTO ");
        if body.table_param {
            out.push_value(&Value::Table(T::table()));
        } else {
            out.push(&T::table());
        }
        out.push_str(" ");

        match &body.data {
            Data::Select(query) => {
                out.push(query);
            }
            Data::Values(values) => {
                out.push_value(values);
            }
            Data::Rows {
                fields,
                rows,
                errors,
            } => {
                for error in errors {
                    out.push_error(error.as_str());
                }
                out.push_str("(");
                for (index, field) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    out.push_ident(field);
                }
                out.push_str(") VALUES ");
                for (index, row) in rows.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    out.push_str("(");
                    for (index, value) in row.iter().enumerate() {
                        if index > 0 {
                            out.push_str(", ");
                        }
                        out.push_value(value);
                    }
                    out.push_str(")");
                }
            }
        }

        if !body.on_duplicate_key_update.is_empty() {
            out.push_str(" ON DUPLICATE KEY UPDATE ")
                .push(&body.on_duplicate_key_update);
        }

        self.mode.render(out);
    }
}

impl_statement!([T: Model, R: ReturnMode, C: Cardinality] InsertStatement<T, R, C>);

impl<T: Model, R: ReturnMode, C: Cardinality> Returning for InsertStatement<T, R, C> {
    type Mode = R;
    type Cardinality = C;
}
