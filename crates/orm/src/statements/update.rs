// Statement syntax
// UPDATE [ ONLY ] @targets
// 	[ WITH NOINDEX | WITH INDEX @indexes ]
// 	[ CONTENT @value
// 	  | MERGE @value
// 	  | PATCH @value
// 	  | REPLACE @value
// 	  | [ SET @field = @value, ... | UNSET @field, ... ]
// 	]
// 	[ WHERE @condition ]
// 	[ RETURN NONE | RETURN BEFORE | RETURN AFTER | RETURN DIFF | RETURN @statement_param, ... | RETURN VALUE @statement_param ]
// 	[ TIMEOUT @duration ]
// 	[ EXPLAIN [ FULL ]]
// ;
use std::marker::PhantomData;

use surrealdb::types::{self, SurrealValue, Table, ToSql};

use crate::{
    Conditional, DurationLike, Filter, Model, ModelMerge, PatchOp, RecordId, Renderer, Setter,
    TableOf, TableSpec, ValueLike, Writable, render::Checked,
};

use super::{
    Cardinality, Explained, Many, One, ReturnAfter, ReturnMode, Returning, Statement,
    data::{DataClause, Unsettables},
    impl_statement,
    index_hint::IndexHint,
    returning::return_methods,
};

/// Creates a new UPDATE statement.
/// The UPDATE statement can be used to update or modify records in the database. It never creates
/// records: a missing target is left missing.
///
/// Since SurrealDB 3.3.0 the `WHERE` condition is checked before the data clause, and both read
/// the record as it was before the statement.
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::update};
/// # #[model(node, table = "user")]
/// # #[derive(Debug, Clone, Default)]
/// # pub struct User { pub id: RecordId<Self>, pub name: String, pub score: u32 }
///
/// let UserSchema { score, .. } = User::schema();
/// let statement = update::<User>(User::id("alice"))
///     .set(score.increment_by(5u32))
///     .return_value(score);
/// assert_eq!(
///     statement.to_debug_sql(),
///     "UPDATE user:alice SET score += 5 RETURN VALUE score;"
/// );
/// ```
pub fn update<T: Writable>(targettables: impl Into<TargettablesForUpdate>) -> UpdateStatement<T> {
    UpdateStatement {
        body: UpdateBody::new::<T>(targettables.into()),
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// Creates a new UPDATE ONLY statement, returning a single object.
pub fn update_only<T: Writable>(
    targettables: impl Into<TargettablesForUpdate>,
) -> UpdateStatement<T, ReturnAfter<T>, One> {
    UpdateStatement {
        body: UpdateBody::new::<T>(targettables.into()),
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// The target of an `UPDATE`, `UPSERT` or `DELETE`: the model's table or one of its records.
#[derive(Debug, Clone)]
pub enum TargettablesForUpdate {
    /// A table, rendered as its name.
    Table(Table),
    /// A record, bound as a value.
    Record(types::RecordId),
}

impl TargettablesForUpdate {
    /// The target, with an error when it is not in a table of `T`'s family.
    pub(crate) fn checked<T: Model>(self) -> Checked<ValueLike> {
        let family = TableSpec::of::<T>();
        match self {
            Self::Table(target) => {
                let error = (!family.accepts(target.as_str())).then(|| {
                    format!(
                        "table `{}` is not a table of the model `{}`",
                        target.as_str(),
                        T::TABLE
                    )
                });
                Checked::new(ValueLike::table(target), error)
            }
            Self::Record(id) => {
                let error = (!family.accepts(id.table.as_str())).then(|| {
                    format!(
                        "record `{}` does not belong to a table of the model `{}`",
                        id.to_sql(),
                        T::TABLE
                    )
                });
                Checked::new(ValueLike::value(id), error)
            }
        }
    }
}

impl<T: Model> From<TableOf<T>> for TargettablesForUpdate {
    fn from(table: TableOf<T>) -> Self {
        Self::Table(table.into())
    }
}

impl<T: Model> From<&TableOf<T>> for TargettablesForUpdate {
    fn from(table: &TableOf<T>) -> Self {
        Self::Table(table.table().clone())
    }
}

impl<T: Model> From<T> for TargettablesForUpdate {
    fn from(value: T) -> Self {
        Self::Record(value.record_id().into())
    }
}

impl From<Table> for TargettablesForUpdate {
    fn from(value: Table) -> Self {
        Self::Table(value)
    }
}

impl From<&Table> for TargettablesForUpdate {
    fn from(value: &Table) -> Self {
        Self::Table(value.clone())
    }
}

impl From<types::RecordId> for TargettablesForUpdate {
    fn from(value: types::RecordId) -> Self {
        Self::Record(value)
    }
}

impl From<&types::RecordId> for TargettablesForUpdate {
    fn from(value: &types::RecordId) -> Self {
        Self::Record(value.clone())
    }
}

impl<T: Model> From<RecordId<T>> for TargettablesForUpdate {
    fn from(value: RecordId<T>) -> Self {
        Self::Record(value.into())
    }
}

impl<T: Model> From<&RecordId<T>> for TargettablesForUpdate {
    fn from(value: &RecordId<T>) -> Self {
        Self::Record(value.into())
    }
}

/// The clauses an `UPDATE` and an `UPSERT` share, in the order both write them.
#[derive(Debug, Clone)]
pub(crate) struct UpdateBody {
    /// The target as given, to check a `CONTENT` record id against.
    pub(crate) target_kind: TargettablesForUpdate,
    pub(crate) target: Checked<ValueLike>,
    pub(crate) index_hint: Option<IndexHint>,
    pub(crate) data: Option<DataClause>,
    pub(crate) where_: Option<Filter>,
    pub(crate) timeout: Option<DurationLike>,
}

impl UpdateBody {
    pub(crate) fn new<T: Model>(target: TargettablesForUpdate) -> Self {
        Self {
            target_kind: target.clone(),
            target: target.checked::<T>(),
            index_hint: None,
            data: None,
            where_: None,
            timeout: None,
        }
    }

    /// The error when the `CONTENT` record id `id` is not in the target table, or is not the
    /// target record.
    fn content_error(&self, id: &types::RecordId) -> Option<String> {
        match &self.target_kind {
            TargettablesForUpdate::Table(table) if id.table != *table => Some(format!(
                "the content's record `{}` is not in the target table `{}`",
                id.to_sql(),
                table.as_str()
            )),
            TargettablesForUpdate::Record(target) if id != target => Some(format!(
                "the content's record `{}` is not the target record `{}`",
                id.to_sql(),
                target.to_sql()
            )),
            _ => None,
        }
    }

    /// Writes `KEYWORD [ONLY] target [WITH …] data [WHERE …]`; the return clause follows.
    pub(crate) fn render_head(&self, keyword: &str, only: bool, out: &mut Renderer) {
        out.push_str(keyword).push_str(" ");
        if only {
            out.push_str("ONLY ");
        }
        if let Some(error) =
            DataClause::content_ids(self.data.as_ref()).and_then(|ids| self.content_error(&ids.id))
        {
            out.push_error(error);
        }
        out.push(&self.target)
            .push(&self.index_hint)
            .push(&self.data);
        if let Some(condition) = &self.where_ {
            out.push_str(" WHERE ").push(condition);
        }
    }

    /// Writes `[TIMEOUT …]`, after the return clause.
    pub(crate) fn render_tail(&self, out: &mut Renderer) {
        if let Some(timeout) = &self.timeout {
            out.push_str(" TIMEOUT ").push(timeout);
        }
    }
}

/// Implements the index hint, data clauses, `WHERE`, `TIMEOUT` and `EXPLAIN` that `UPDATE` and
/// `UPSERT` share.
macro_rules! update_clauses {
    ($statement:ident) => {
        impl<T: Model, R: ReturnMode, C: Cardinality> $statement<T, R, C> {
            /// Caution! Replaces the whole record with the model (`CONTENT`). `CONTENT` bypasses
            /// `READONLY` fields; use [`replace`](Self::replace) to respect them. The model's
            /// record id must be in a table target's table, or equal a record target; otherwise
            /// the statement fails when it is rendered.
            pub fn content(mut self, content: T) -> Self {
                self.body.data = Some(DataClause::content(content));
                self
            }

            /// Merges the non-`Keep` fields of a generated `<Model>Merge` into the record
            /// (`MERGE`). Nested objects combine at every level; arrays are replaced whole.
            ///
            /// An empty merge, with every field `Keep`, still writes `MERGE {}`: the statement is
            /// a real update, so events fire and its `WHERE`, `ONLY`, return mode and `EXPLAIN`
            /// apply as written.
            pub fn merge(self, merge: impl ModelMerge<Model = T>) -> Self {
                self.merge_object(merge.into_object())
            }

            /// `MERGE` with an object already built from a `<Model>Merge`.
            pub(crate) fn merge_object(mut self, object: $crate::MergeObject) -> Self {
                self.body.data = Some(DataClause::Merge(ValueLike::new(object)));
                self
            }

            /// Caution! Replaces the whole record with the given object, removing every field it
            /// does not contain (`REPLACE`). Unlike `CONTENT`, it respects `READONLY` fields.
            pub fn replace(mut self, replacement: impl SurrealValue) -> Self {
                self.body.data = Some(DataClause::Replace(ValueLike::value(replacement)));
                self
            }

            /// When specifying fields to update using the SET clause,
            /// it is possible to increment and decrement numeric values,
            /// and add or remove values from arrays.
            /// To increment a numeric value, or to add an item to an array,
            /// use the `append` or incremenet (i.e +=) operator. To decrement a numeric value,
            /// or to remove an value from an array, use the `decrement` or `remove` (i.e -=)
            /// operator. Repeated calls add to the assignments.
            ///
            /// # Examples
            ///
            /// ```
            /// use umbriel::{*, statements::update};
            /// # #[model(node, table = "weapon")]
            /// # pub struct Weapon {
            /// #     pub id: RecordId<Self>,
            /// #     pub name: String,
            /// #     pub damage: u32,
            /// #     pub owners: Vec<String>,
            /// # }
            ///
            /// let WeaponSchema { name, damage, owners, .. } = Weapon::schema();
            /// let laser = Weapon::id("laser");
            ///
            /// // Several assignments; `increment_by` writes `+=`.
            /// let statement = update::<Weapon>(&laser)
            ///     .set([name.equal_to("Laser"), damage.increment_by(100u32)]);
            /// assert_eq!(
            ///     statement.to_debug_sql(),
            ///     "UPDATE weapon:laser SET name = 'Laser', damage += 100;"
            /// );
            ///
            /// // `append` adds to an array; `at` assigns one element.
            /// let statement = update::<Weapon>(&laser)
            ///     .set([owners.append("Alice"), owners.at(0).equal_to("Bob")]);
            /// assert_eq!(
            ///     statement.to_debug_sql(),
            ///     "UPDATE weapon:laser SET owners += 'Alice', owners[0] = 'Bob';"
            /// );
            /// ```
            pub fn set(mut self, settables: impl Into<Vec<Setter>>) -> Self {
                self.body.data = Some(DataClause::set(self.body.data.take(), settables.into()));
                self
            }

            /// Removes fields from the record (`UNSET`). Repeated calls add to the fields.
            pub fn unset(mut self, fields: impl Into<Unsettables>) -> Self {
                self.body.data = Some(DataClause::unset(self.body.data.take(), fields.into().0));
                self
            }

            /// Applies JSON Patch operations to the record (`PATCH`). Repeated calls add to the
            /// operations; if one fails, such as a failed `test`, the statement fails.
            ///
            /// # Example
            ///
            /// ```
            /// use umbriel::{*, statements::update};
            /// # #[model(node, table = "user")]
            /// # pub struct User { pub id: RecordId<Self>, pub name: String }
            ///
            /// let name = User::schema().name;
            /// let statement = update::<User>(User::id("alice"))
            ///     .patch([name.patch_test("Alice"), name.patch_replace("Bob")]);
            /// assert_eq!(
            ///     statement.to_debug_sql(),
            ///     "UPDATE user:alice PATCH [{ op: 'test', path: '/name', value: 'Alice' }, \
            ///      { op: 'replace', path: '/name', value: 'Bob' }];"
            /// );
            /// ```
            pub fn patch(mut self, patch_op: impl Into<Vec<PatchOp>>) -> Self {
                self.body.data = Some(DataClause::patch(self.body.data.take(), patch_op.into()));
                self
            }

            /// Adds a condition to the `WHERE` clause of the query.
            pub fn where_(mut self, condition: impl Conditional) -> Self {
                self.body.where_ = Some(Filter::new(condition));
                self
            }

            /// `WITH NOINDEX`: the planner finds the records to change through the table
            /// iterator and uses no index.
            pub fn with_no_index(mut self) -> Self {
                self.body.index_hint = Some($crate::statements::index_hint::IndexHint::NoIndex);
                self
            }

            /// `WITH INDEX …`: the planner may use only the named indexes to find the records
            /// to change.
            pub fn with_index(
                mut self,
                indexes: impl Into<$crate::statements::select::IndexNames>,
            ) -> Self {
                self.body.index_hint =
                    Some($crate::statements::index_hint::IndexHint::indexes(indexes));
                self
            }

            /// Sets the timeout duration for the query, bound as a query parameter.
            pub fn timeout(mut self, duration: impl Into<DurationLike>) -> Self {
                self.body.timeout = Some(duration.into());
                self
            }

            /// `EXPLAIN`: returns the query plan instead of running the statement, so nothing is
            /// written.
            pub fn explain(self) -> Explained {
                Explained::new(self, false)
            }

            /// `EXPLAIN FULL`: the plan with the number of records each step processes. SurrealDB
            /// runs the statement to count them, so the records **are written**.
            pub fn explain_full(self) -> Explained {
                Explained::new(self, true)
            }

            fn with_mode<M: ReturnMode>(self, mode: M) -> $statement<T, M, C> {
                $statement {
                    body: self.body,
                    mode,
                    cardinality: PhantomData,
                }
            }
        }
    };
}
pub(crate) use update_clauses;

/// An `UPDATE` statement. `R` is its return mode and `C` whether it is written with `ONLY`; both
/// decide the type [`Returning::execute`] returns.
#[derive(Debug, Clone)]
pub struct UpdateStatement<T, R = ReturnAfter<T>, C = Many> {
    body: UpdateBody,
    mode: R,
    cardinality: PhantomData<(fn() -> T, C)>,
}

update_clauses!(UpdateStatement);

return_methods!([T: Model] UpdateStatement<T> { none, null, before, after, diff, value, fields, links });

impl<T: Model, R: ReturnMode, C: Cardinality> Statement for UpdateStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        self.body.render_head("UPDATE", C::ONLY, out);
        self.mode.render(out);
        self.body.render_tail(out);
    }
}

impl_statement!([T: Model, R: ReturnMode, C: Cardinality] UpdateStatement<T, R, C>);

impl<T: Model, R: ReturnMode, C: Cardinality> Returning for UpdateStatement<T, R, C> {
    type Mode = R;
    type Cardinality = C;
}
