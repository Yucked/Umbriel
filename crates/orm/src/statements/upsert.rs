// Statement syntax
// UPSERT [ ONLY ] @targets
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

use surrealdb::types::SurrealValue;

use crate::{
    Conditional, DurationLike, Filter, Model, ModelMerge, PatchOp, Renderer, Setter, ValueLike,
};

use super::{
    Cardinality, Explained, Many, One, ReturnAfter, ReturnMode, Returning, Statement,
    data::{DataClause, Unsettables},
    impl_statement,
    returning::return_methods,
    update::{TargettablesForUpdate, UpdateBody, update_clauses},
};

/// Creates an UPSERT statement: it updates the records it matches and creates the record
/// otherwise.
///
/// What it matches, per SurrealDB 3.3:
/// - a unique index is always checked first, so an `UPSERT` whose data clashes with a unique
///   index updates the record holding that value;
/// - on a table without `WHERE`, it creates a record unless a unique index matches;
/// - with `WHERE`, the condition gates only the update path, and is checked before the data
///   clause (since 3.3.0); creating a record does not evaluate it;
/// - on a record id, it creates the record when it is missing; when it exists and `WHERE` does not
///   match, it writes and returns nothing.
///
/// # Examples
///
/// ```
/// use umbriel::{*, statements::upsert};
/// # #[model(node, table = "user")]
/// # #[derive(Debug, Clone, Default)]
/// # pub struct User { pub id: RecordId<Self>, pub name: String }
///
/// let UserSchema { name, .. } = User::schema();
/// let statement = upsert::<User>(User::id("alice")).set(name.equal_to("Alice".to_owned()));
/// assert_eq!(
///     statement.to_debug_sql(),
///     "UPSERT user:alice SET name = 'Alice';"
/// );
/// ```
pub fn upsert<T: Model>(targettables: impl Into<TargettablesForUpdate>) -> UpsertStatement<T> {
    UpsertStatement {
        body: UpdateBody::new::<T>(targettables.into()),
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// Creates an UPSERT ONLY statement, returning a single object.
pub fn upsert_only<T: Model>(
    targettables: impl Into<TargettablesForUpdate>,
) -> UpsertStatement<T, ReturnAfter<T>, One> {
    UpsertStatement {
        body: UpdateBody::new::<T>(targettables.into()),
        mode: ReturnAfter::default_mode(),
        cardinality: PhantomData,
    }
}

/// An UPSERT statement. `R` is its return mode and `C` whether it is written with `ONLY`; both
/// decide the type [`Returning::execute`] returns.
#[derive(Debug, Clone)]
pub struct UpsertStatement<T, R = ReturnAfter<T>, C = Many> {
    body: UpdateBody,
    mode: R,
    cardinality: PhantomData<(fn() -> T, C)>,
}

update_clauses!(UpsertStatement);

return_methods!([T: Model] UpsertStatement<T> { none, null, before, after, diff, value, fields, links });

impl<T: Model, R: ReturnMode, C: Cardinality> Statement for UpsertStatement<T, R, C> {
    fn render_body(&self, out: &mut Renderer) {
        self.body.render_head("UPSERT", C::ONLY, out);
        self.mode.render(out);
        self.body.render_tail(out);
    }
}

impl_statement!([T: Model, R: ReturnMode, C: Cardinality] UpsertStatement<T, R, C>);

impl<T: Model, R: ReturnMode, C: Cardinality> Returning for UpsertStatement<T, R, C> {
    type Mode = R;
    type Cardinality = C;
}
