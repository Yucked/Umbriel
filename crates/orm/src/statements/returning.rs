//! Return clauses of write statements, chosen by type: the return mode a builder carries decides
//! both the `RETURN` clause it renders and the type `execute` decodes the result into.

use std::{fmt, marker::PhantomData};

use surrealdb::{
    Connection, IndexedResults, Surreal,
    types::{self, ConversionError, Kind, Object, SurrealNull, SurrealValue, Value},
};

use crate::{
    Error, Field, Fragment, Projections, Queryable, Render, Renderer, Result,
    execution::{execute, take_one},
};

use super::Statement;

mod sealed {
    pub trait Sealed {}
}

/// How many results a statement returns: [`Many`] for a table or several targets, [`One`] for
/// `ONLY`.
pub trait Cardinality: sealed::Sealed + Send + Sync + 'static {
    /// Whether the statement is written with `ONLY`.
    #[doc(hidden)]
    const ONLY: bool;

    /// The result of a statement with return mode `R`.
    type Output<R: ReturnMode>;

    /// Decodes the first statement's result.
    #[doc(hidden)]
    fn decode<R: ReturnMode>(results: IndexedResults) -> Result<Self::Output<R>>;
}

/// A statement without `ONLY`: its result is a list, one entry per affected record.
#[derive(Debug, Clone, Copy)]
pub struct Many;

/// A statement with `ONLY`: its result is a single entry, or none.
#[derive(Debug, Clone, Copy)]
pub struct One;

impl sealed::Sealed for Many {}
impl sealed::Sealed for One {}

impl Cardinality for Many {
    const ONLY: bool = false;
    type Output<R: ReturnMode> = R::Many;

    fn decode<R: ReturnMode>(results: IndexedResults) -> Result<R::Many> {
        R::decode_many(results)
    }
}

impl Cardinality for One {
    const ONLY: bool = true;
    type Output<R: ReturnMode> = R::One;

    fn decode<R: ReturnMode>(results: IndexedResults) -> Result<R::One> {
        R::decode_one(results)
    }
}

/// What a write statement returns: its `RETURN` clause and the types its result decodes into.
pub trait ReturnMode: sealed::Sealed + Clone + fmt::Debug + Send + Sync + 'static {
    /// The result without `ONLY`.
    type Many;
    /// The result with `ONLY`.
    type One;

    /// Writes the clause, with its leading space, or nothing for the statement's default.
    #[doc(hidden)]
    fn render(&self, out: &mut Renderer);

    /// Decodes the result of a statement without `ONLY`.
    #[doc(hidden)]
    fn decode_many(results: IndexedResults) -> Result<Self::Many>;

    /// Decodes the result of a statement with `ONLY`.
    #[doc(hidden)]
    fn decode_one(results: IndexedResults) -> Result<Self::One>;
}

fn take_many<V: SurrealValue>(mut results: IndexedResults) -> Result<Vec<V>> {
    results.take::<Vec<V>>(0).map_err(Error::Conversion)
}

/// Implements `Clone` and `Debug` without bounds on the phantom type parameter, which derives
/// would require.
macro_rules! phantom_mode {
    ($name:ident<$param:ident> { $($field:ident),* }) => {
        impl<$param> Clone for $name<$param> {
            fn clone(&self) -> Self {
                Self {
                    $($field: self.$field.clone(),)*
                    model: PhantomData,
                }
            }
        }

        impl<$param> fmt::Debug for $name<$param> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    $(.field(stringify!($field), &self.$field))*
                    .finish()
            }
        }

        impl<$param> sealed::Sealed for $name<$param> {}
    };
}

/// The records as they are after the statement: the default of `CREATE`, `INSERT`, `UPDATE`,
/// `UPSERT` and `RELATE`, or an explicit `RETURN AFTER`.
pub struct ReturnAfter<T> {
    explicit: bool,
    model: PhantomData<fn() -> T>,
}

impl<T> ReturnAfter<T> {
    /// The statement's default, which writes no clause.
    pub(crate) fn default_mode() -> Self {
        Self {
            explicit: false,
            model: PhantomData,
        }
    }

    /// `RETURN AFTER`.
    pub(crate) fn explicit() -> Self {
        Self {
            explicit: true,
            model: PhantomData,
        }
    }
}

phantom_mode!(ReturnAfter<T> { explicit });

impl<T: SurrealValue + 'static> ReturnMode for ReturnAfter<T> {
    type Many = Vec<T>;
    type One = Option<T>;

    fn render(&self, out: &mut Renderer) {
        if self.explicit {
            out.push_str(" RETURN AFTER");
        }
    }

    fn decode_many(results: IndexedResults) -> Result<Vec<T>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<T>> {
        take_one(results)
    }
}

/// What a `SELECT` returns: its rows, each decoded as `T`. It writes no clause, and is only a
/// type: a `SELECT` holds no mode value.
pub struct Rows<T> {
    model: PhantomData<fn() -> T>,
}

phantom_mode!(Rows<T> {});

impl<T: SurrealValue + 'static> ReturnMode for Rows<T> {
    type Many = Vec<T>;
    type One = Option<T>;

    fn render(&self, _: &mut Renderer) {}

    fn decode_many(results: IndexedResults) -> Result<Vec<T>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<T>> {
        take_one(results)
    }
}

/// `RETURN BEFORE`: the records as they were before the statement.
pub struct ReturnBefore<T> {
    model: PhantomData<fn() -> T>,
}

impl<T> ReturnBefore<T> {
    pub(crate) fn new() -> Self {
        Self { model: PhantomData }
    }
}

phantom_mode!(ReturnBefore<T> {});

impl<T: SurrealValue + 'static> ReturnMode for ReturnBefore<T> {
    type Many = Vec<T>;
    type One = Option<T>;

    fn render(&self, out: &mut Renderer) {
        out.push_str(" RETURN BEFORE");
    }

    fn decode_many(results: IndexedResults) -> Result<Vec<T>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<T>> {
        take_one(results)
    }
}

/// `RETURN NONE`: nothing.
#[derive(Debug, Clone, Copy)]
pub struct ReturnNone;

impl sealed::Sealed for ReturnNone {}

impl ReturnMode for ReturnNone {
    type Many = ();
    type One = ();

    fn render(&self, out: &mut Renderer) {
        out.push_str(" RETURN NONE");
    }

    fn decode_many(_: IndexedResults) -> Result<()> {
        Ok(())
    }

    fn decode_one(_: IndexedResults) -> Result<()> {
        Ok(())
    }
}

/// `RETURN NULL`: one `NULL` for each record the statement returns, where `RETURN NONE` returns
/// nothing. A `WHERE`, `IGNORE` or permissions decide which records are returned; the count of
/// nulls is not documented as a count of writes.
#[derive(Debug, Clone, Copy)]
pub struct ReturnNull;

impl sealed::Sealed for ReturnNull {}

impl ReturnMode for ReturnNull {
    type Many = Vec<SurrealNull>;
    type One = Option<SurrealNull>;

    fn render(&self, out: &mut Renderer) {
        out.push_str(" RETURN NULL");
    }

    fn decode_many(results: IndexedResults) -> Result<Vec<SurrealNull>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<SurrealNull>> {
        take_one(results)
    }
}

/// The default of `DELETE`, which returns nothing and writes no clause.
#[derive(Debug, Clone, Copy)]
pub struct ReturnDefaultNone;

impl sealed::Sealed for ReturnDefaultNone {}

impl ReturnMode for ReturnDefaultNone {
    type Many = ();
    type One = ();

    fn render(&self, _: &mut Renderer) {}

    fn decode_many(_: IndexedResults) -> Result<()> {
        Ok(())
    }

    fn decode_one(_: IndexedResults) -> Result<()> {
        Ok(())
    }
}

/// `RETURN DIFF`: for each record, the JSON Patch operations the statement applied.
#[derive(Debug, Clone, Copy)]
pub struct ReturnDiff;

impl sealed::Sealed for ReturnDiff {}

impl ReturnMode for ReturnDiff {
    type Many = Vec<Vec<DiffOp>>;
    type One = Option<Vec<DiffOp>>;

    fn render(&self, out: &mut Renderer) {
        out.push_str(" RETURN DIFF");
    }

    fn decode_many(results: IndexedResults) -> Result<Vec<Vec<DiffOp>>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<Vec<DiffOp>>> {
        take_one(results)
    }
}

/// `RETURN VALUE field`: the value of one field per record, of the field's own type `V`.
pub struct ReturnValue<V> {
    field: Field,
    model: PhantomData<fn() -> V>,
}

impl<V> ReturnValue<V> {
    pub(crate) fn new(field: Field) -> Self {
        Self {
            field,
            model: PhantomData,
        }
    }
}

phantom_mode!(ReturnValue<V> { field });

impl<V: SurrealValue + 'static> ReturnMode for ReturnValue<V> {
    type Many = Vec<V>;
    type One = Option<V>;

    fn render(&self, out: &mut Renderer) {
        out.push_str(" RETURN VALUE ").push(&self.field);
    }

    fn decode_many(results: IndexedResults) -> Result<Vec<V>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<V>> {
        take_one(results)
    }
}

/// `RETURN fields…`: the listed fields or expressions per record, decoded into the caller's `P`.
pub struct ReturnFields<P> {
    projections: Projections,
    model: PhantomData<fn() -> P>,
}

impl<P> ReturnFields<P> {
    pub(crate) fn new(projections: Projections) -> Self {
        Self {
            projections,
            model: PhantomData,
        }
    }
}

phantom_mode!(ReturnFields<P> { projections });

impl<P: SurrealValue + 'static> ReturnMode for ReturnFields<P> {
    type Many = Vec<P>;
    type One = Option<P>;

    fn render(&self, out: &mut Renderer) {
        out.push_str(" RETURN ").push(&self.projections);
    }

    fn decode_many(results: IndexedResults) -> Result<Vec<P>> {
        take_many(results)
    }

    fn decode_one(results: IndexedResults) -> Result<Option<P>> {
        take_one(results)
    }
}

/// One operation of a `RETURN DIFF` result, as SurrealDB reports it: a JSON Patch operation whose
/// `path` is a JSON pointer into the record.
///
/// Decoded by its `op` tag, so `add` and `replace`, which share a shape, stay distinct.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DiffOp {
    /// A field or element was added.
    Add {
        /// Where, as a JSON pointer.
        path: String,
        /// The value added.
        value: Value,
    },
    /// A field or element was removed.
    Remove {
        /// Where, as a JSON pointer.
        path: String,
    },
    /// A field or element was replaced.
    Replace {
        /// Where, as a JSON pointer.
        path: String,
        /// The new value.
        value: Value,
    },
    /// A string was changed, described by a text diff.
    Change {
        /// Where, as a JSON pointer.
        path: String,
        /// The diff, in the `diff-match-patch` patch format.
        value: String,
    },
}

impl DiffOp {
    fn op(&self) -> &'static str {
        match self {
            Self::Add { .. } => "add",
            Self::Remove { .. } => "remove",
            Self::Replace { .. } => "replace",
            Self::Change { .. } => "change",
        }
    }
}

fn diff_error(value: &Value, context: impl Into<String>) -> types::Error {
    ConversionError::from_value(DiffOp::kind_of(), value)
        .with_context(context.into())
        .into()
}

impl SurrealValue for DiffOp {
    fn kind_of() -> Kind {
        Kind::Object
    }

    fn is_value(value: &Value) -> bool {
        matches!(value, Value::Object(object) if matches!(object.get("op"), Some(Value::String(_))))
    }

    fn into_value(self) -> Value {
        let op = self.op();
        let mut object = Object::new();
        object.insert("op".to_owned(), Value::String(op.to_owned()));
        let (path, value) = match self {
            Self::Add { path, value } | Self::Replace { path, value } => (path, Some(value)),
            Self::Remove { path } => (path, None),
            Self::Change { path, value } => (path, Some(Value::String(value))),
        };
        object.insert("path".to_owned(), Value::String(path));
        if let Some(value) = value {
            object.insert("value".to_owned(), value);
        }
        Value::Object(object)
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        let Value::Object(object) = value else {
            return Err(diff_error(&value, "a diff operation is an object"));
        };
        let mut fields = object.into_inner();
        let op = match fields.remove("op") {
            Some(Value::String(op)) => op,
            _ => {
                return Err(diff_error(
                    &Value::Object(fields.into()),
                    "a diff operation has an `op` string",
                ));
            }
        };
        let path = match fields.remove("path") {
            Some(Value::String(path)) => path,
            _ => {
                return Err(diff_error(
                    &Value::Object(fields.into()),
                    "a diff operation has a `path` string",
                ));
            }
        };
        let value = fields.remove("value");
        match (op.as_str(), value) {
            ("add", Some(value)) => Ok(Self::Add { path, value }),
            ("remove", _) => Ok(Self::Remove { path }),
            ("replace", Some(value)) => Ok(Self::Replace { path, value }),
            ("change", Some(Value::String(value))) => Ok(Self::Change { path, value }),
            (op, value) => Err(diff_error(
                &value.unwrap_or(Value::None),
                format!("unsupported or incomplete diff operation `{op}`"),
            )),
        }
    }
}

/// Implements the return methods a statement offers, each switching its return mode. Every
/// statement struct is `Name<T, R, C>` with a private `with_mode`; the list names the modes its
/// statement documents, so an undocumented or meaningless mode does not exist on that statement.
macro_rules! return_methods {
    ([$($bounds:tt)*] $statement:ident<$model:ident> { $($mode:ident),* $(,)? }) => {
        impl<$($bounds)*, R: $crate::statements::ReturnMode, C: $crate::statements::Cardinality>
            $statement<$model, R, C>
        {
            $( $crate::statements::returning::return_methods!(@$mode $statement $model); )*
        }
    };
    (@none $statement:ident $model:ident) => {
        /// `RETURN NONE`: the statement returns nothing.
        pub fn return_none(self) -> $statement<$model, $crate::statements::ReturnNone, C> {
            self.with_mode($crate::statements::ReturnNone)
        }
    };
    (@null $statement:ident $model:ident) => {
        /// `RETURN NULL`: one `NULL` per returned record.
        pub fn return_null(self) -> $statement<$model, $crate::statements::ReturnNull, C> {
            self.with_mode($crate::statements::ReturnNull)
        }
    };
    (@before $statement:ident $model:ident) => {
        /// `RETURN BEFORE`: each record as it was before the statement.
        pub fn return_before(
            self,
        ) -> $statement<$model, $crate::statements::ReturnBefore<$model>, C> {
            self.with_mode($crate::statements::ReturnBefore::new())
        }
    };
    (@after $statement:ident $model:ident) => {
        /// `RETURN AFTER`: each record as it is after the statement, which is also the default.
        pub fn return_after(
            self,
        ) -> $statement<$model, $crate::statements::ReturnAfter<$model>, C> {
            self.with_mode($crate::statements::ReturnAfter::explicit())
        }
    };
    (@diff $statement:ident $model:ident) => {
        /// `RETURN DIFF`: for each record, the JSON Patch operations the statement applied.
        pub fn return_diff(self) -> $statement<$model, $crate::statements::ReturnDiff, C> {
            self.with_mode($crate::statements::ReturnDiff)
        }
    };
    (@value $statement:ident $model:ident) => {
        /// `RETURN VALUE field`: one value per record, of the field's own type `V`. The field must
        /// be one of this model's generated schema fields.
        pub fn return_value<F, V>(
            self,
            field: F,
        ) -> $statement<$model, $crate::statements::ReturnValue<V>, C>
        where
            F: $crate::__private::ModelField<$model, V>,
            V: ::surrealdb::types::SurrealValue + 'static,
        {
            let field: $crate::Field = (*field).clone();
            self.with_mode($crate::statements::ReturnValue::new(field))
        }
    };
    (@fields $statement:ident $model:ident) => {
        /// `RETURN fields…`: the listed fields or expressions per record, decoded into `P`.
        pub fn return_fields<P>(
            self,
            projections: impl Into<$crate::Projections>,
        ) -> $statement<$model, $crate::statements::ReturnFields<P>, C>
        where
            P: ::surrealdb::types::SurrealValue + 'static,
        {
            self.with_mode($crate::statements::ReturnFields::new(projections.into()))
        }
    };
    (@links $statement:ident $model:ident) => {
        /// Returns every field with the given record links fetched (`RETURN *, link.*`), decoded
        /// into `P`. A link to a missing record becomes `NULL`.
        ///
        /// # Errors
        ///
        /// [`Error::FieldsUnfetchableNotARecordLink`](crate::Error::FieldsUnfetchableNotARecordLink)
        /// when a field is not one of the model's links.
        pub fn load_links<P>(
            self,
            linked_fields: impl IntoIterator<Item = impl Into<$crate::Field>>,
        ) -> $crate::Result<$statement<$model, $crate::statements::ReturnFields<P>, C>>
        where
            P: ::surrealdb::types::SurrealValue + 'static,
        {
            let projections = $crate::traits::statements::fetch_projections::<$model>(
                linked_fields.into_iter().map(Into::into).collect(),
            )?;
            Ok(self.with_mode($crate::statements::ReturnFields::new(projections)))
        }

        /// Returns every field with every record link fetched, decoded into `P`.
        pub fn load_all_links<P>(
            self,
        ) -> $statement<$model, $crate::statements::ReturnFields<P>, C>
        where
            P: ::surrealdb::types::SurrealValue + 'static,
        {
            let projections = $crate::traits::statements::link_projections(
                <$model as $crate::Model>::get_linked_fields(),
            );
            self.with_mode($crate::statements::ReturnFields::new(projections))
        }
    };
}
pub(crate) use return_methods;

/// A statement whose result type is part of its type: a write statement's return mode, or a
/// `SELECT`'s output type.
pub trait Returning: Queryable + Sync {
    /// The return mode, or [`Rows`] for a `SELECT`.
    type Mode: ReturnMode;
    /// Whether the statement is written with `ONLY`.
    type Cardinality: Cardinality;

    /// Runs the statement and decodes what its return clause returns. Every statement's result is
    /// checked first, as [`Runnable::run`](crate::Runnable::run) does.
    ///
    /// # Errors
    ///
    /// Fails when the statement does not render, when it fails (classified as
    /// [`Error::RecordExists`], [`Error::TableNotFound`] or [`Error::Statement`]), or when the
    /// result does not decode into the mode's type ([`Error::Conversion`]).
    fn execute(
        &self,
        db: &Surreal<impl Connection>,
    ) -> impl Future<Output = Result<<Self::Cardinality as Cardinality>::Output<Self::Mode>>> + Send
    {
        async move {
            let results = execute(db, self.to_query()?).await?;
            <Self::Cardinality as Cardinality>::decode::<Self::Mode>(results)
        }
    }
}

/// A statement's body as an owned node, so it can be shared inside another statement.
pub(crate) struct Body<S>(pub(crate) S);

impl<S: Statement> Render for Body<S> {
    fn render(&self, out: &mut Renderer) {
        self.0.render_body(out);
    }
}

/// A statement with `EXPLAIN` or `EXPLAIN FULL`: it returns the query plan instead of its result.
///
/// SurrealDB 3.3.4 plans the two kinds of statement differently, so the plan's shape differs:
/// - **`SELECT`** runs on the newer executor: one object, a tree of `operator`, `attributes` and
///   `children` (`IndexScan`, `TableScan`, …), with `metrics` under `EXPLAIN FULL`.
/// - **`UPDATE`, `UPSERT` and `DELETE`** run on the older one: a list of steps, each
///   `{ operation, detail }` (`Iterate Index`, `Iterate Table`, …).
///
/// `EXPLAIN` alone runs nothing. `EXPLAIN FULL` runs the statement to measure it, so on a write
/// statement it **performs the write** (`dbs/iterator.rs` iterates the documents as usual).
#[derive(Debug, Clone)]
pub struct Explained {
    statement: Fragment,
    full: bool,
}

impl Explained {
    pub(crate) fn new(statement: impl Statement + Send + Sync + 'static, full: bool) -> Self {
        Self {
            statement: Fragment::new(Body(statement)),
            full,
        }
    }

    /// Runs the statement and returns its plan, in the shape described on [`Explained`]. The
    /// plan is SurrealDB's own and documented as informational, so it is not typed further.
    ///
    /// # Errors
    ///
    /// Fails as [`Returning::execute`] does.
    pub async fn execute(&self, db: &Surreal<impl Connection>) -> Result<Value> {
        let mut results = execute(db, self.to_query()?).await?;
        results.take::<Value>(0).map_err(Error::Conversion)
    }
}

impl Statement for Explained {
    fn render_body(&self, out: &mut Renderer) {
        out.push(&self.statement);
        out.push_str(if self.full {
            " EXPLAIN FULL"
        } else {
            " EXPLAIN"
        });
    }
}

super::impl_statement!(Explained);

/// The output format of an `EXPLAIN` prefix: [`PlanText`] or [`PlanJson`].
pub trait PlanFormat: sealed::Sealed + Send + Sync + 'static {
    /// What the plan decodes into.
    type Plan: SurrealValue;

    /// Writes the `FORMAT` clause, with its trailing space, or nothing for the default.
    #[doc(hidden)]
    fn render(out: &mut Renderer);
}

/// `FORMAT TEXT`, the default: the plan as one string, a tree of operators.
#[derive(Debug, Clone, Copy)]
pub struct PlanText;

/// `FORMAT JSON`: the plan as an object of `operator`, `context`, `attributes` and
/// `expressions`, with `metrics` and `total_rows` under `ANALYZE`.
#[derive(Debug, Clone, Copy)]
pub struct PlanJson;

impl sealed::Sealed for PlanText {}
impl sealed::Sealed for PlanJson {}

impl PlanFormat for PlanText {
    type Plan = String;

    fn render(_: &mut Renderer) {}
}

impl PlanFormat for PlanJson {
    type Plan = Value;

    fn render(out: &mut Renderer) {
        out.push_str("FORMAT JSON ");
    }
}

/// `EXPLAIN [ANALYZE] [FORMAT JSON] SELECT …`: the plan of a `SELECT` instead of its records.
///
/// SurrealDB documents the prefix for statements that do not modify data, and 3.3.4 refuses it
/// over a write statement (its planner does not plan writes), so it takes a `SELECT` only; a
/// write statement's `explain()` is the suffix form. The plan is informational and its format may change between SurrealDB releases, so it
/// is not typed beyond text or JSON.
///
/// # Example
///
/// ```
/// use umbriel::{*, statements::{explain, select}};
///
/// let plan = explain(select(All).from(Table::new("user"))).analyze().json();
/// assert_eq!(plan.to_debug_sql(), "EXPLAIN ANALYZE FORMAT JSON SELECT * FROM user;");
/// ```
pub fn explain<O: 'static, C: Cardinality>(
    statement: super::SelectStatement<O, C>,
) -> ExplainStatement {
    ExplainStatement {
        statement: Fragment::new(Body(statement)),
        analyze: false,
        format: PhantomData,
    }
}

/// An `EXPLAIN` prefix over a `SELECT`; `F` is its output format.
pub struct ExplainStatement<F = PlanText> {
    statement: Fragment,
    analyze: bool,
    format: PhantomData<fn() -> F>,
}

impl<F> Clone for ExplainStatement<F> {
    fn clone(&self) -> Self {
        Self {
            statement: self.statement.clone(),
            analyze: self.analyze,
            format: PhantomData,
        }
    }
}

impl<F> fmt::Debug for ExplainStatement<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExplainStatement")
            .field("statement", &self.statement)
            .field("analyze", &self.analyze)
            .finish()
    }
}

impl<F: PlanFormat> ExplainStatement<F> {
    /// `ANALYZE`: runs the statement and adds each operator's metrics (rows, batches and elapsed
    /// time) to the plan.
    pub fn analyze(mut self) -> Self {
        self.analyze = true;
        self
    }

    /// `FORMAT JSON`: the plan as an object instead of text.
    pub fn json(self) -> ExplainStatement<PlanJson> {
        ExplainStatement {
            statement: self.statement,
            analyze: self.analyze,
            format: PhantomData,
        }
    }

    /// Runs the statement and returns its plan.
    ///
    /// # Errors
    ///
    /// Fails as [`Returning::execute`] does.
    pub async fn execute(&self, db: &Surreal<impl Connection>) -> Result<F::Plan> {
        // `take` decodes into `Option`, `Vec` or `Value` only, so the plan is decoded from its
        // value.
        let mut results = execute(db, self.to_query()?).await?;
        let plan = results.take::<Value>(0).map_err(Error::Conversion)?;
        F::Plan::from_value(plan).map_err(Error::Conversion)
    }
}

impl<F: PlanFormat> Statement for ExplainStatement<F> {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("EXPLAIN ");
        if self.analyze {
            out.push_str("ANALYZE ");
        }
        F::render(out);
        out.push(&self.statement);
    }
}

super::impl_statement!([F: PlanFormat] ExplainStatement<F>);
