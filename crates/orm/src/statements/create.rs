//! `CREATE`. Syntax: <https://surrealdb.com/docs/surrealql/statements/create>. The parser also
//! accepts `MERGE`, `PATCH`, `REPLACE` and `UNSET`, which the engine applies to the new, empty
//! document through the same path as every other write (`doc/alter.rs` `process_record_data` in
//! 3.3.4).

use std::{marker::PhantomData, ops::Range};

use surrealdb::types::{self, Table, ToSql, Value};

use crate::{
    DurationLike, Model, Node, RecordId, Renderer, TableOf, TableSpec, ValueLike, render::Checked,
};

use super::{
    Cardinality, Many, One, ReturnAfter, ReturnMode, Returning, Statement,
    data::{DataClause, data_clause_methods},
    impl_statement,
    returning::return_methods,
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
    /// The table, written as its name, with the error when it is not in `T`'s family.
    Table(Table, Option<String>),
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

    /// `T`'s default table: one record with a generated id, or the id given in the content.
    pub fn table() -> Self {
        Self::of(TargetKind::Table(T::table(), None))
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
    /// The error when the content's record id `id` cannot be created at this target: a table
    /// other than the id's, or a different record.
    fn content_error(&self, id: &types::RecordId) -> Option<String> {
        match &self.kind {
            TargetKind::Table(table, _) if id.table != *table => Some(format!(
                "the content's record `{}` is not in the target table `{}`",
                id.to_sql(),
                table.as_str()
            )),
            TargetKind::Record(target) if id != target => Some(format!(
                "the content's record `{}` is not the target record `{}`",
                id.to_sql(),
                target.to_sql()
            )),
            _ => None,
        }
    }

    fn render(&self, out: &mut Renderer) {
        let table = T::table();
        match &self.kind {
            TargetKind::Table(table, error) => {
                out.push(&Checked::new(
                    ValueLike::table(table.clone()),
                    error.clone(),
                ));
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

/// A table, which must be in `T`'s family: another table fails when the statement is rendered.
impl<T: Model> From<Table> for CreateTarget<T> {
    fn from(table: Table) -> Self {
        let error = (!TableSpec::of::<T>().accepts(table.as_str())).then(|| {
            format!(
                "table `{}` is not a table of the model `{}`",
                table.as_str(),
                T::TABLE
            )
        });
        Self::of(TargetKind::Table(table, error))
    }
}

/// A checked member of `T`'s family.
impl<T: Model> From<TableOf<T>> for CreateTarget<T> {
    fn from(table: TableOf<T>) -> Self {
        Self::of(TargetKind::Table(table.into(), None))
    }
}

impl<T: Model> From<TableOf<T>> for CreateTargets<T> {
    fn from(table: TableOf<T>) -> Self {
        Self(vec![table.into()])
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

// On a new record, `MERGE` and `PATCH` apply to an empty one: a `PATCH` of a path that does not
// exist yet, such as a `replace`, fails the statement.
data_clause_methods!(Node, CreateStatement, "record");

// `RETURN BEFORE` is not offered: SurrealDB documents it as `NONE` for `CREATE`.
return_methods!([T: Node] CreateStatement<T> { none, null, after, diff, value, fields, links });

impl<T: Node, R: ReturnMode, C: Cardinality> Statement for CreateStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        let body = &self.body;
        out.push_str("CREATE ");
        if C::ONLY {
            out.push_str("ONLY ");
        }
        out.push_joined_with(&body.targets, ", ", |out, target| target.render(out));
        if body.targets.is_empty() {
            out.push_error("a CREATE statement needs at least one target");
        }
        if let Some(ids) = DataClause::content_ids(body.data.as_ref()) {
            for error in body
                .targets
                .iter()
                .filter_map(|target| target.content_error(&ids.id))
            {
                out.push_error(error);
            }
        }
        out.push(&body.data);
        self.mode.render(out);
        if let Some(timeout) = &body.timeout {
            out.push_str(" TIMEOUT ").push(timeout);
        }
    }
}

impl_statement!([T: Node, R: ReturnMode, C: Cardinality] CreateStatement<T, R, C>);

impl<T: Node, R: ReturnMode, C: Cardinality> Returning for CreateStatement<T, R, C> {
    type Mode = R;
    type Cardinality = C;
}
