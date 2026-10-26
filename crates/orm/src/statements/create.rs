// Statement syntax
// CREATE [ ONLY ] @targets
// 	[ CONTENT @value
// 	  | SET @field = @value ...
// 	]
// 	[ RETURN NONE | RETURN BEFORE | RETURN AFTER | RETURN DIFF | RETURN @statement_param, ... | RETURN VALUE @statement_param ]
// 	[ TIMEOUT @duration ]
// ;
use std::{marker::PhantomData, ops::Range};

use surrealdb::types::{self, Table, Value};

use crate::{
    DurationLike, Model, Node, RecordId, Renderer, Setter, ValueLike, render::Checked,
};

use super::{
    Cardinality, Many, One, ReturnAfter, ReturnMode, Returning, Statement, data::DataClause,
    impl_statement, returning::return_methods,
};

/// Creates a `CREATE` statement over one or more targets of `T`'s table: the table, record ids,
/// and mock targets `|t:N|` and `|t:a..b|`, which can be mixed.
///
/// Without a data clause each new record holds only its id. Without a return method the statement
/// returns the created records ([`ReturnAfter`]).
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::{CreateTarget, create}};
/// # #[model(node, table = "user")]
/// # #[derive(Debug, Clone, Default)]
/// # pub struct User { pub id: RecordId<Self>, pub name: String }
///
/// let alice = User { id: User::id("alice"), name: "Alice".to_owned() };
/// assert_eq!(
///     create::<User>(User::table()).content(alice).to_debug_sql(),
///     "CREATE user CONTENT { id: user:alice, name: 'Alice' };"
/// );
///
/// let mocks = create::<User>([CreateTarget::count(3), CreateTarget::range(10..12)]);
/// assert_eq!(mocks.to_debug_sql(), "CREATE |user:3|, |user:10..12|;");
/// ```
pub fn create<T: Node>(targets: impl Into<CreateTargets<T>>) -> CreateStatement<T> {
    CreateStatement {
        body: CreateBody {
            targets: targets.into().0,
            data: None,
            timeout: None,
            model: PhantomData,
        },
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// Creates a `CREATE ONLY` statement over a single target, returning one record rather than a
/// list.
pub fn create_only<T: Node>(
    target: impl Into<CreateTarget<T>>,
) -> CreateStatement<T, ReturnAfter<T>, One> {
    CreateStatement {
        body: CreateBody {
            targets: vec![target.into()],
            data: None,
            timeout: None,
            model: PhantomData,
        },
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// One target of a `CREATE` in `T`'s table.
pub struct CreateTarget<T> {
    kind: TargetKind,
    model: PhantomData<fn() -> T>,
}

#[derive(Debug, Clone)]
enum TargetKind {
    /// The table, written as its name; checked against `T`'s table.
    Table(Checked<ValueLike>),
    /// One record, bound as a value.
    Record(types::RecordId),
    /// `|t:N|`: `N` records with random ids.
    Count(u64),
    /// `|t:start..end|`: one record per integer key, end exclusive.
    Range(i64, i64),
}

impl<T: Model> CreateTarget<T> {
    fn of(kind: TargetKind) -> Self {
        Self {
            kind,
            model: PhantomData,
        }
    }

    /// `T`'s table: one record with a generated id, or the id given in the content.
    pub fn table() -> Self {
        Self::of(TargetKind::Table(Checked::new(
            ValueLike::table(T::table()),
            None,
        )))
    }

    /// `|t:count|`: `count` records with random ids.
    pub fn count(count: u64) -> Self {
        Self::of(TargetKind::Count(count))
    }

    /// `|t:start..end|`: one record per integer key from `start` up to, but not including, `end`.
    /// Mock ranges have excluded their end since SurrealDB 3.0.0.
    pub fn range(range: Range<i64>) -> Self {
        Self::of(TargetKind::Range(range.start, range.end))
    }
}

impl<T> Clone for CreateTarget<T> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            model: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for CreateTarget<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CreateTarget").field(&self.kind).finish()
    }
}

impl<T: Model> CreateTarget<T> {
    fn render(&self, out: &mut Renderer) {
        let table = T::table();
        match &self.kind {
            TargetKind::Table(table) => {
                out.push(table);
            }
            TargetKind::Record(id) => {
                out.push_value(&Value::RecordId(id.clone()));
            }
            TargetKind::Count(count) => {
                out.push_str("|")
                    .push(&table)
                    .push_str(":")
                    .push_display(count)
                    .push_str("|");
            }
            TargetKind::Range(start, end) => {
                if start >= end {
                    out.push_error(format!(
                        "the mock range `{start}..{end}` is empty: its end is excluded"
                    ));
                }
                out.push_str("|")
                    .push(&table)
                    .push_str(":")
                    .push_display(start)
                    .push_str("..")
                    .push_display(end)
                    .push_str("|");
            }
        }
    }
}

impl<T: Model> From<RecordId<T>> for CreateTarget<T> {
    fn from(id: RecordId<T>) -> Self {
        Self::of(TargetKind::Record(id.into()))
    }
}

impl<T: Model> From<&RecordId<T>> for CreateTarget<T> {
    fn from(id: &RecordId<T>) -> Self {
        Self::of(TargetKind::Record(id.into()))
    }
}

/// A table, which must be `T`'s own: another table fails when the statement is rendered.
impl<T: Model> From<Table> for CreateTarget<T> {
    fn from(table: Table) -> Self {
        let expected = T::table();
        let error = (table != expected).then(|| {
            format!(
                "table `{}` is not the table `{}` of the model",
                table.as_str(),
                expected.as_str()
            )
        });
        Self::of(TargetKind::Table(Checked::new(ValueLike::table(table), error)))
    }
}

/// The targets of a `CREATE`: one [`CreateTarget`] or several.
pub struct CreateTargets<T>(Vec<CreateTarget<T>>);

impl<T: Model> From<CreateTarget<T>> for CreateTargets<T> {
    fn from(target: CreateTarget<T>) -> Self {
        Self(vec![target])
    }
}

impl<T: Model> From<Vec<CreateTarget<T>>> for CreateTargets<T> {
    fn from(targets: Vec<CreateTarget<T>>) -> Self {
        Self(targets)
    }
}

impl<T: Model, const N: usize> From<[CreateTarget<T>; N]> for CreateTargets<T> {
    fn from(targets: [CreateTarget<T>; N]) -> Self {
        Self(targets.into())
    }
}

impl<T: Model> From<RecordId<T>> for CreateTargets<T> {
    fn from(id: RecordId<T>) -> Self {
        Self(vec![id.into()])
    }
}

impl<T: Model> From<&RecordId<T>> for CreateTargets<T> {
    fn from(id: &RecordId<T>) -> Self {
        Self(vec![id.into()])
    }
}

impl<T: Model> From<Table> for CreateTargets<T> {
    fn from(table: Table) -> Self {
        Self(vec![table.into()])
    }
}

impl<T: Model> From<&Table> for CreateTargets<T> {
    fn from(table: &Table) -> Self {
        Self(vec![table.clone().into()])
    }
}

#[derive(Debug, Clone)]
struct CreateBody<T> {
    targets: Vec<CreateTarget<T>>,
    data: Option<DataClause>,
    timeout: Option<DurationLike>,
    model: PhantomData<fn() -> T>,
}

/// A `CREATE` statement. `R` is its return mode and `C` whether it is written with `ONLY`; both
/// decide the type [`Returning::execute`] returns.
#[derive(Debug, Clone)]
pub struct CreateStatement<T, R = ReturnAfter<T>, C = Many> {
    body: CreateBody<T>,
    mode: R,
    cardinality: PhantomData<C>,
}

impl<T: Node, R: ReturnMode, C: Cardinality> CreateStatement<T, R, C> {
    /// The record's data as a whole model (`CONTENT`).
    pub fn content(mut self, content: T) -> Self {
        self.body.data = Some(DataClause::Content(ValueLike::value(content)));
        self
    }

    /// The record's data as assignments (`SET`); repeated calls add to the assignments.
    ///
    /// # Arguments
    ///
    /// * `settables` - one assignment from `equal_to`, or a list of them, e.g. from the
    ///   `object!` macro.
    pub fn set(mut self, settables: impl Into<Vec<Setter>>) -> Self {
        self.body.data = Some(DataClause::set(self.body.data.take(), settables.into()));
        self
    }

    /// Sets the timeout duration for the query. It is written as a literal.
    pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
        self.body.timeout = Some(duration.into());
        self
    }

    fn with_mode<M: ReturnMode>(self, mode: M) -> CreateStatement<T, M, C> {
        CreateStatement {
            body: self.body,
            mode,
            cardinality: PhantomData,
        }
    }
}

// `RETURN BEFORE` is not offered: SurrealDB documents it as `NONE` for `CREATE`.
return_methods!([T: Node] CreateStatement<T> { none, after, diff, value, fields, links });

impl<T: Node, R: ReturnMode, C: Cardinality> Statement for CreateStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        let body = &self.body;
        out.push_str("CREATE ");
        if C::ONLY {
            out.push_str("ONLY ");
        }
        for (index, target) in body.targets.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            target.render(out);
        }
        if body.targets.is_empty() {
            out.push_error("a CREATE statement needs at least one target");
        }
        out.push(&body.data);
        self.mode.render(out);
        if let Some(timeout) = &body.timeout {
            out.push_str(" TIMEOUT ").push_inlined(timeout);
        }
    }
}

impl_statement!([T: Node, R: ReturnMode, C: Cardinality] CreateStatement<T, R, C>);

impl<T: Node, R: ReturnMode, C: Cardinality> Returning for CreateStatement<T, R, C> {
    type Mode = R;
    type Cardinality = C;
}
