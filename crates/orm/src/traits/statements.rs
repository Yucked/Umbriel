use crate::{
    Error, Executor, Field, Model, Projections, Queryable, Render, Result, ValueLike,
    execution::{execute, execute_with_stats, take_one},
};
use surrealdb::{IndexedResults, method::WithStats, types::SurrealValue};

// The execution methods return `impl Future + Send` (with `Self: Sync` for `&self` receivers) so
// their futures stay `Send` for multi-threaded runtimes, as they were under `async_trait`.

/// A trait that represents a statement that can be run against the database.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be executed",
    label = "not a runnable statement",
    note = "only complete statements can run; build one with `umbriel::statements`"
)]
pub trait Runnable
where
    Self: Queryable,
{
    /// Runs the query on a client or an open SDK transaction. Succeeds only when every statement
    /// succeeded; otherwise returns the first failure, classified as [`Error::RecordExists`],
    /// [`Error::TableNotFound`], [`Error::Conflict`], [`Error::NotAllowed`],
    /// [`Error::Connection`] or [`Error::Statement`].
    fn run(&self, db: &impl Executor) -> impl Future<Output = Result<IndexedResults>> + Send
    where
        Self: Sync,
    {
        async move { execute(db, self.to_query()?).await }
    }

    /// [`run`](Self::run), with the SDK's statistics for each statement (its execution time).
    /// The response is checked exactly as `run` checks it.
    ///
    /// # Errors
    ///
    /// The errors of [`run`](Self::run).
    fn run_with_stats(
        &self,
        db: &impl Executor,
    ) -> impl Future<Output = Result<WithStats<IndexedResults>>> + Send
    where
        Self: Sync,
    {
        async move { execute_with_stats(db, self.to_query()?).await }
    }

    /// Runs the query, checks the complete response as [`run`](Self::run) does, and decodes the
    /// result of its first statement as `T`: `None` when it is `NONE`. The result is decoded as
    /// it is, so a statement without `ONLY` decodes as a list, such as `Vec<T>`, even when it
    /// returns one record. A one-element list is not unwrapped into its element, as it was before
    /// Umbriel issue #11.
    ///
    /// # Errors
    ///
    /// The errors of [`run`](Self::run), or [`Error::Conversion`] when the result does not decode
    /// as `T`, such as a single model asked for from a statement without `ONLY`.
    fn get_data<T>(&self, db: &impl Executor) -> impl Future<Output = Result<Option<T>>> + Send
    where
        Self: Sync,
        T: SurrealValue,
    {
        async move { take_one(self.run(db).await?) }
    }
}

impl<Q> Runnable for Q where Q: Queryable {}

/// Checks that every field is one of `T`'s record links, comparing rendered paths, so a caller's
/// `Field::new("author")`, a `Field::raw("author")` and the generated schema field all match.
fn validate_fields_to_fetch<T: Model>(linked_fields_to_fetch: &[Field]) -> Result<()> {
    let linked_fields = T::get_linked_fields()
        .iter()
        .map(Render::to_debug_sql)
        .collect::<Vec<_>>();
    let unfetchable = linked_fields_to_fetch
        .iter()
        .map(Render::to_debug_sql)
        .filter(|field| !linked_fields.contains(field))
        .collect::<Vec<_>>();

    if !unfetchable.is_empty() {
        return Err(Error::FieldsUnfetchableNotARecordLink(
            unfetchable.join(", "),
        ));
    }
    Ok(())
}

/// `*` and each link fetched: one `.*` fetches a single link and, since SurrealDB 3, every link
/// of an array too (2.x needed `.*.*` for arrays). A link to a missing record becomes `NULL`.
pub(crate) fn link_projections(linked_fields: Vec<Field>) -> Projections {
    std::iter::once(crate::All.into())
        .chain(linked_fields.iter().map(|field| field.all().into()))
        .collect::<Vec<ValueLike>>()
        .into()
}

/// [`link_projections`] for fields that must be `T`'s record links.
pub(crate) fn fetch_projections<T: Model>(linked_fields: Vec<Field>) -> Result<Projections> {
    validate_fields_to_fetch::<T>(&linked_fields)?;
    Ok(link_projections(linked_fields))
}
