// Statement syntax
// DELETE [ FROM | ONLY ] @targets
//     [ WITH NOINDEX | WITH INDEX @indexes ]
//     [ WHERE @condition ]
//     [ RETURN NONE | RETURN BEFORE | RETURN AFTER | RETURN DIFF | RETURN @statement_param, ... ]
//     [ TIMEOUT @duration ]
//     [ EXPLAIN [ FULL ]]
// ;
use std::{marker::PhantomData, ops::Deref};

use crate::{
    Conditional, DurationLike, Edge, Field, Filter, Model, Renderer, SchemaGetter, ValueLike,
    render::Checked,
};

use super::{
    Cardinality, Explained, Many, One, ReturnDefaultNone, ReturnMode, Returning, Statement,
    impl_statement, index_hint::IndexHint, returning::return_methods, select::IndexNames,
    update::TargettablesForUpdate,
};

/// Creates a new DELETE statement over the model's table or one of its records. Deleting a
/// record also deletes the edges connected to it.
///
/// It returns nothing unless a return method is chosen. Only `RETURN BEFORE` and `RETURN DIFF`
/// are offered: the deleted records have no state after the statement, so their fields, `VALUE`
/// projections included, read as `NONE`.
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::delete};
/// # #[model(node, table = "user")]
/// # #[derive(Debug, Clone, Default)]
/// # pub struct User { pub id: RecordId<Self>, pub age: u8 }
///
/// let UserSchema { age, .. } = User::schema();
/// let statement = delete::<User>(User::table())
///     .where_(age.less_than(18))
///     .return_before();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DELETE user WHERE age < 18 RETURN BEFORE;"
/// );
/// ```
pub fn delete<T: Model>(targettables: impl Into<TargettablesForUpdate>) -> DeleteStatement<T> {
    DeleteStatement::new(targettables.into().checked::<T>())
}

/// Creates a new DELETE ONLY statement over a single record.
pub fn delete_only<T: Model>(
    targettables: impl Into<TargettablesForUpdate>,
) -> DeleteStatement<T, ReturnDefaultNone, One> {
    DeleteStatement::new(targettables.into().checked::<T>())
}

/// Creates a DELETE statement over the edges a graph path reaches, such as
/// `Student::with(student).writes__(Empty)` for `DELETE student:…->writes`: only those edges are
/// deleted, and the path's edge must be `E`.
pub fn delete_edges<E, P>(path: &P) -> DeleteStatement<E>
where
    E: Edge,
    P: Deref<Target = <E as SchemaGetter>::Schema>,
    for<'a> &'a <E as SchemaGetter>::Schema: Into<Field>,
{
    let path: Field = (&**path).into();
    DeleteStatement::new(Checked::new(ValueLike::from(path), None))
}

#[derive(Debug, Clone)]
struct DeleteBody {
    target: Checked<ValueLike>,
    index_hint: Option<IndexHint>,
    where_: Option<Filter>,
    timeout: Option<DurationLike>,
}

/// A DELETE statement. `R` is its return mode and `C` whether it is written with `ONLY`; both
/// decide the type [`Returning::execute`] returns.
#[derive(Debug, Clone)]
pub struct DeleteStatement<T, R = ReturnDefaultNone, C = Many> {
    body: DeleteBody,
    mode: R,
    cardinality: PhantomData<(fn() -> T, C)>,
}

impl<T: Model, C: Cardinality> DeleteStatement<T, ReturnDefaultNone, C> {
    fn new(target: Checked<ValueLike>) -> Self {
        Self {
            body: DeleteBody {
                target,
                index_hint: None,
                where_: None,
                timeout: None,
            },
            mode: ReturnDefaultNone,
            cardinality: PhantomData,
        }
    }
}

impl<T: Model, R: ReturnMode, C: Cardinality> DeleteStatement<T, R, C> {
    /// Adds a condition to the delete statement.
    ///
    /// # Examples
    ///
    /// ```
    /// use umbriel::{*, statements::delete};
    /// # #[model(node, table = "user")]
    /// # pub struct User { pub id: RecordId<Self>, pub age: u8 }
    ///
    /// let age = User::schema().age;
    /// let statement = delete::<User>(User::table())
    ///     .where_(cond(age.greater_than(18)).and(age.less_than(80)));
    /// assert_eq!(
    ///     statement.to_debug_sql(),
    ///     "DELETE user WHERE (age > 18) AND (age < 80);"
    /// );
    /// ```
    pub fn where_(mut self, condition: impl Conditional) -> Self {
        self.body.where_ = Some(Filter::new(condition));
        self
    }

    /// `WITH NOINDEX`: the planner finds the records to delete through the table iterator and
    /// uses no index.
    pub fn with_no_index(mut self) -> Self {
        self.body.index_hint = Some(IndexHint::NoIndex);
        self
    }

    /// `WITH INDEX …`: the planner may use only the named indexes to find the records to delete.
    pub fn with_index(mut self, indexes: impl Into<IndexNames>) -> Self {
        self.body.index_hint = Some(IndexHint::indexes(indexes));
        self
    }

    /// Sets the timeout duration for the query, bound as a query parameter.
    pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
        self.body.timeout = Some(duration.into());
        self
    }

    /// `EXPLAIN`: returns the query plan instead of running the statement, so nothing is deleted.
    pub fn explain(self) -> Explained {
        Explained::new(self, false)
    }

    /// `EXPLAIN FULL`: the plan with the number of records each step processes. SurrealDB runs
    /// the statement to count them, so the records **are deleted**.
    pub fn explain_full(self) -> Explained {
        Explained::new(self, true)
    }

    fn with_mode<M: ReturnMode>(self, mode: M) -> DeleteStatement<T, M, C> {
        DeleteStatement {
            body: self.body,
            mode,
            cardinality: PhantomData,
        }
    }
}

return_methods!([T: Model] DeleteStatement<T> { null, before, diff });

impl<T: Model, R: ReturnMode, C: Cardinality> Statement for DeleteStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DELETE ");
        if C::ONLY {
            out.push_str("ONLY ");
        }
        out.push(&self.body.target).push(&self.body.index_hint);

        if let Some(condition) = &self.body.where_ {
            out.push_str(" WHERE ").push(condition);
        }

        self.mode.render(out);

        if let Some(timeout) = &self.body.timeout {
            out.push_str(" TIMEOUT ").push(timeout);
        }
    }
}

impl_statement!([T: Model, R: ReturnMode, C: Cardinality] DeleteStatement<T, R, C>);

impl<T: Model, R: ReturnMode, C: Cardinality> Returning for DeleteStatement<T, R, C> {
    type Mode = R;
    type Cardinality = C;
}
