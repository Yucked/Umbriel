//! Writes over several models in one statement. A `#[derive(Models)]` enum names the models; its
//! `SurrealValue` decodes each returned record into the variant whose table family holds the
//! record's id, so the statement's results stay typed and in the order the database returns them.

use std::marker::PhantomData;

use surrealdb::types::{self, SurrealValue, Table, ToSql, Value};

use crate::{DurationLike, Model, RecordId, Renderer, Setter, TableOf, TableSpec, ValueLike};

use super::{
    Cardinality, Many, One, ReturnAfter, ReturnDiff, ReturnMode, ReturnNone, ReturnNull, Returning,
    Statement, data::DataClause, impl_statement,
};

/// A union of models, one per variant, from `#[derive(Models)]`. No two variants' table families
/// overlap, which the derive checks at compile time, so every record decodes into at most one
/// variant.
pub trait Models: SurrealValue + 'static {
    /// The table family of each variant, in declaration order.
    const MEMBERS: &'static [TableSpec];

    /// Whether `table` belongs to a variant.
    fn accepts_table(table: &str) -> bool {
        Self::MEMBERS.iter().any(|member| member.accepts(table))
    }
}

/// The first pair of overlapping families among `members`, by index. The `Models` derive asserts
/// in a `const` that there is none.
#[doc(hidden)]
pub const fn overlapping_members(members: &[TableSpec]) -> Option<(usize, usize)> {
    let mut first = 0;
    while first < members.len() {
        let mut second = first + 1;
        while second < members.len() {
            if members[first].overlaps(members[second]) {
                return Some((first, second));
            }
            second += 1;
        }
        first += 1;
    }
    None
}

/// One target of a mixed-model `CREATE`: a record id or a table of any variant's model. A target
/// outside every variant's family fails when the statement is rendered.
#[derive(Debug, Clone)]
pub struct AnyTarget(AnyTargetKind);

#[derive(Debug, Clone)]
enum AnyTargetKind {
    Table(Table),
    Record(types::RecordId),
}

impl<T: Model> From<RecordId<T>> for AnyTarget {
    fn from(id: RecordId<T>) -> Self {
        Self(AnyTargetKind::Record(id.into()))
    }
}

impl<T: Model> From<&RecordId<T>> for AnyTarget {
    fn from(id: &RecordId<T>) -> Self {
        Self(AnyTargetKind::Record(id.into()))
    }
}

impl<T: Model> From<TableOf<T>> for AnyTarget {
    fn from(table: TableOf<T>) -> Self {
        Self(AnyTargetKind::Table(table.into()))
    }
}

impl AnyTarget {
    fn render<M: Models>(&self, out: &mut Renderer) {
        let (table, target) = match &self.0 {
            AnyTargetKind::Table(table) => (table, ValueLike::table(table.clone())),
            AnyTargetKind::Record(id) => (&id.table, ValueLike::value(id.clone())),
        };
        if !M::accepts_table(table.as_str()) {
            out.push_error(format!(
                "target `{}` is not in a table of the union's models",
                match &self.0 {
                    AnyTargetKind::Table(table) => table.as_str().to_owned(),
                    AnyTargetKind::Record(id) => id.to_sql(),
                }
            ));
        }
        out.push(&target);
    }
}

/// Creates a `CREATE` over targets of several models; each created record decodes into its
/// variant of `M`.
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::create_any};
/// # #[model(node, table = "user")]
/// # #[derive(Debug, Clone)]
/// # pub struct User { pub id: RecordId<Self> }
/// # #[model(node, table = "post")]
/// # #[derive(Debug, Clone)]
/// # pub struct Post { pub id: RecordId<Self> }
/// #[derive(Debug, Clone, Models)]
/// enum Any {
///     User(User),
///     Post(Post),
/// }
///
/// let statement = create_any::<Any>([User::id("alice").into(), Post::id("hello").into()]);
/// assert_eq!(statement.to_debug_sql(), "CREATE user:alice, post:hello;");
/// ```
pub fn create_any<M: Models>(
    targets: impl IntoIterator<Item = AnyTarget>,
) -> CreateAnyStatement<M> {
    CreateAnyStatement::new(targets.into_iter().collect())
}

/// Creates a `CREATE ONLY` over one target of a model of `M`.
pub fn create_any_only<M: Models>(
    target: impl Into<AnyTarget>,
) -> CreateAnyStatement<M, ReturnAfter<M>, One> {
    CreateAnyStatement::new(vec![target.into()])
}

/// A mixed-model `CREATE`. It offers the return modes a single-model `CREATE` does that decode
/// without knowing the model in advance: `NONE`, `NULL`, `AFTER` (the default) and `DIFF`. A
/// projection or `RETURN VALUE` has no record id to tell the models apart, so neither is offered.
#[derive(Debug, Clone)]
pub struct CreateAnyStatement<M, R = ReturnAfter<M>, C = Many> {
    targets: Vec<AnyTarget>,
    data: Option<DataClause>,
    timeout: Option<DurationLike>,
    mode: R,
    model: PhantomData<(fn() -> M, C)>,
}

impl<M: Models, C: Cardinality> CreateAnyStatement<M, ReturnAfter<M>, C> {
    fn new(targets: Vec<AnyTarget>) -> Self {
        Self {
            targets,
            data: None,
            timeout: None,
            mode: ReturnAfter::default_mode(),
            model: PhantomData,
        }
    }
}

impl<M: Models, R: ReturnMode, C: Cardinality> CreateAnyStatement<M, R, C> {
    /// Assignments every created record receives (`SET`); repeated calls add to them.
    pub fn set(mut self, settables: impl Into<Vec<Setter>>) -> Self {
        self.data = Some(DataClause::set(self.data.take(), settables.into()));
        self
    }

    /// Sets the timeout duration for the query. It is written as a literal.
    pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
        self.timeout = Some(duration.into());
        self
    }

    /// `RETURN NONE`.
    pub fn return_none(self) -> CreateAnyStatement<M, ReturnNone, C> {
        self.with_mode(ReturnNone)
    }

    /// `RETURN NULL`: one `NULL` per created record.
    pub fn return_null(self) -> CreateAnyStatement<M, ReturnNull, C> {
        self.with_mode(ReturnNull)
    }

    /// `RETURN AFTER`: each created record, as its variant.
    pub fn return_after(self) -> CreateAnyStatement<M, ReturnAfter<M>, C> {
        self.with_mode(ReturnAfter::explicit())
    }

    /// `RETURN DIFF`: for each record, the JSON Patch operations, not tied to a variant.
    pub fn return_diff(self) -> CreateAnyStatement<M, ReturnDiff, C> {
        self.with_mode(ReturnDiff)
    }

    fn with_mode<N: ReturnMode>(self, mode: N) -> CreateAnyStatement<M, N, C> {
        CreateAnyStatement {
            targets: self.targets,
            data: self.data,
            timeout: self.timeout,
            mode,
            model: PhantomData,
        }
    }
}

impl<M: Models, R: ReturnMode, C: Cardinality> Statement for CreateAnyStatement<M, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("CREATE ");
        if C::ONLY {
            out.push_str("ONLY ");
        }
        for (index, target) in self.targets.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            target.render::<M>(out);
        }
        if self.targets.is_empty() {
            out.push_error("a CREATE statement needs at least one target");
        }
        out.push(&self.data);
        self.mode.render(out);
        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push_inlined(timeout);
        }
    }
}

impl_statement!([M: Models, R: ReturnMode, C: Cardinality] CreateAnyStatement<M, R, C>);

impl<M: Models, R: ReturnMode, C: Cardinality> Returning for CreateAnyStatement<M, R, C> {
    type Mode = R;
    type Cardinality = C;
}

/// Creates a targetless `INSERT [ … ]` of records of several models: each record's id names its
/// table, and each returned record decodes into its variant of `M`. For records of one model it
/// behaves exactly like [`insert`](super::insert).
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::insert_any};
/// # #[model(node, table = "user")]
/// # #[derive(Debug, Clone)]
/// # pub struct User { pub id: RecordId<Self> }
/// # #[model(node, table = "post")]
/// # #[derive(Debug, Clone)]
/// # pub struct Post { pub id: RecordId<Self> }
/// #[derive(Debug, Clone, Models)]
/// enum Any {
///     User(User),
///     Post(Post),
/// }
///
/// let statement = insert_any(vec![
///     Any::User(User { id: User::id("alice") }),
///     Any::Post(Post { id: Post::id("hello") }),
/// ]);
/// assert_eq!(
///     statement.to_debug_sql(),
///     "INSERT [{ id: user:alice }, { id: post:hello }];"
/// );
/// ```
pub fn insert_any<M: Models>(records: impl IntoIterator<Item = M>) -> InsertAnyStatement<M> {
    InsertAnyStatement {
        records: records.into_iter().map(SurrealValue::into_value).collect(),
        ignore: false,
        timeout: None,
        mode: ReturnAfter::default_mode(),
        model: PhantomData,
    }
}

/// A targetless mixed-model `INSERT`. It offers the return modes a single-model `INSERT` does
/// that decode without knowing the model in advance: `NONE`, `NULL`, `AFTER` (the default) and
/// `DIFF`. `INSERT` has no `ONLY`.
#[derive(Debug, Clone)]
pub struct InsertAnyStatement<M, R = ReturnAfter<M>> {
    records: Vec<Value>,
    ignore: bool,
    timeout: Option<DurationLike>,
    mode: R,
    model: PhantomData<fn() -> M>,
}

impl<M: Models, R: ReturnMode> InsertAnyStatement<M, R> {
    /// `INSERT IGNORE`: records whose id or unique index value already exists are skipped.
    pub fn ignore(mut self) -> Self {
        self.ignore = true;
        self
    }

    /// Sets the timeout duration for the query. It is written as a literal.
    pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
        self.timeout = Some(duration.into());
        self
    }

    /// `RETURN NONE`.
    pub fn return_none(self) -> InsertAnyStatement<M, ReturnNone> {
        self.with_mode(ReturnNone)
    }

    /// `RETURN NULL`: one `NULL` per inserted record.
    pub fn return_null(self) -> InsertAnyStatement<M, ReturnNull> {
        self.with_mode(ReturnNull)
    }

    /// `RETURN AFTER`: each inserted record, as its variant.
    pub fn return_after(self) -> InsertAnyStatement<M, ReturnAfter<M>> {
        self.with_mode(ReturnAfter::explicit())
    }

    /// `RETURN DIFF`: for each record, the JSON Patch operations, not tied to a variant.
    pub fn return_diff(self) -> InsertAnyStatement<M, ReturnDiff> {
        self.with_mode(ReturnDiff)
    }

    fn with_mode<N: ReturnMode>(self, mode: N) -> InsertAnyStatement<M, N> {
        InsertAnyStatement {
            records: self.records,
            ignore: self.ignore,
            timeout: self.timeout,
            mode,
            model: PhantomData,
        }
    }
}

impl<M: Models, R: ReturnMode> Statement for InsertAnyStatement<M, R> {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("INSERT ");
        if self.ignore {
            out.push_str("IGNORE ");
        }
        if self.records.is_empty() {
            out.push_error("an INSERT statement needs at least one record");
        }
        out.push_value(&Value::Array(self.records.clone().into()));
        self.mode.render(out);
        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push_inlined(timeout);
        }
    }
}

impl_statement!([M: Models, R: ReturnMode] InsertAnyStatement<M, R>);

impl<M: Models, R: ReturnMode> Returning for InsertAnyStatement<M, R> {
    type Mode = R;
    type Cardinality = Many;
}
