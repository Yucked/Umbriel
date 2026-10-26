use crate::{
    Error, Field, Model, Projections, Queryable, Render, Result, ReturnType, execution::execute,
};
use surrealdb::{Connection, IndexedResults, Surreal, types::SurrealValue};

// Create, Update, Relate, Delete
// [ RETURN [ NONE | BEFORE | AFTER | DIFF | @projections ... ]

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
    /// Runs the query against the database. Succeeds only when every statement succeeded;
    /// otherwise returns the first failure, classified as [`Error::RecordExists`],
    /// [`Error::TableNotFound`] or [`Error::Statement`].
    fn run(
        &self,
        db: Surreal<impl surrealdb::Connection>,
    ) -> impl Future<Output = Result<IndexedResults>> + Send
    where
        Self: Sync,
    {
        async move { execute(&db, self.to_query()?).await }
    }

    /// Runs the query, checks the complete response as [`run`](Self::run) does, and decodes the
    /// result of its first statement.
    fn get_data<T>(
        &self,
        db: Surreal<impl surrealdb::Connection>,
    ) -> impl Future<Output = Result<Option<T>>> + Send
    where
        Self: Sync,
        T: SurrealValue,
    {
        async move {
            let mut response = self.run(db).await?;

            response.take::<Option<T>>(0).map_err(Error::Conversion)
        }
    }
}

impl<Q> Runnable for Q where Q: Queryable {}

/// A trait that represents a statement that can be run against the database and return a single
pub trait ReturnableStandard<T>
where
    Self: Sized + Send + Sync + Runnable,
    T: Model,
{
    // /// Runs the statement against the database and returns the first result before the change.
    // async fn return_first_before(self, db: Surreal<Db>) -> Result<Option<T>> {
    //     let query = self.set_return_type(ReturnType::Before);
    //     query.return_first(db).await
    // }
    //
    // /// Runs the statement against the database and returns the first result after the change.
    // async fn return_first_after(self, db: Surreal<Db>) -> Result<Option<T>> {
    //     let query = self.set_return_type(ReturnType::After);
    //     query.return_first(db).await
    // }
    //
    // /// Runs the statement against the database and returns the first result of the change.
    // async fn return_first_diff(self, db: Surreal<Db>) -> Result<Option<T>> {
    //     let query = self.set_return_type(ReturnType::Diff);
    //     query.return_first(db).await
    // }

    /// Runs the statement against the database and returns the first result of the change with the
    /// specified projections or list of fields.
    fn return_first_projections<P>(
        self,
        db: Surreal<impl Connection>,
        projections: impl Send + Into<Projections>,
    ) -> impl Future<Output = Result<Option<P>>> + Send
    where
        P: SurrealValue,
    {
        async move {
            let mut query = self;
            let projections: Projections = projections.into();
            query = query.set_return_type(ReturnType::Projections(projections));

            let response = query.run(db).await?;
            get_first::<P>(response)
        }
    }

    /// Runs the statement against the database and returns the one result of the change with the
    /// specified projections or list of fields.
    fn return_one_projections<P>(
        self,
        db: Surreal<impl Connection>,
        projections: impl Send + Into<Projections>,
    ) -> impl Future<Output = Result<Option<P>>> + Send
    where
        P: SurrealValue,
    {
        async move {
            let mut query = self;
            let projections: Projections = projections.into();
            query = query.set_return_type(ReturnType::Projections(projections));

            let response = query.run(db).await?;
            get_one::<P>(response)
        }
    }

    /// Runs the statement against the database and returns the many result of the change with the
    /// specified projections or list of fields.
    fn return_many_projections<P>(
        self,
        db: Surreal<impl Connection>,
        projections: impl Send + Into<Projections>,
    ) -> impl Future<Output = Result<Vec<P>>> + Send
    where
        P: SurrealValue,
    {
        async move {
            let mut query = self;
            let projections: Projections = projections.into();
            query = query.set_return_type(ReturnType::Projections(projections));

            let response = query.run(db).await?;
            get_many::<P>(response)
        }
    }

    #[doc(hidden)]
    fn validate_fields_to_fetch(linked_fields_to_fetch: &[Field]) -> Result<()> {
        // Compared as rendered paths: a caller's `Field::new("author")` and the generated
        // schema field for `author` are built from different parts.
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

    /// Sets the return type to projections and fetches all records links.
    /// Defaults values to null for referenced records that do not exist.
    fn load_links(self, linked_fields_to_fetch: Vec<impl Into<Field>>) -> Result<Self> {
        let linked_fields_to_fetch = linked_fields_to_fetch
            .into_iter()
            .map(Into::into)
            .collect::<Vec<Field>>();
        Self::validate_fields_to_fetch(&linked_fields_to_fetch)?;

        let projections = ReturnType::Projections(
            vec![Field::new("*")]
                .into_iter()
                .chain(
                    linked_fields_to_fetch
                        .into_iter()
                        // One `.*` fetches a single link and, since SurrealDB 3, every link of an
                        // array too (2.x needed `.*.*` for arrays).
                        .map(|field| field.all())
                        .collect::<Vec<_>>(),
                )
                .collect::<Vec<_>>()
                .into(),
        );

        Ok(self.set_return_type(projections))
    }

    /// load links values and filter out null references.
    // fn load_links_non_null(self, linked_fields_to_fetch: Vec<Field>) -> Result<Self> {
    //     Self::validate_fields_to_fetch(&linked_fields_to_fetch)?;
    //     let projections = ReturnType::Projections(
    //         vec![Field::new("*")]
    //             .into_iter()
    //             .chain(
    //                 linked_fields_to_fetch
    //                     .into_iter()
    //                     // Fetch only where the link is not null.
    //                     .map(|field| {
    //                         // format!("{}[WHERE type::thing(id) IS NOT NONE].*", field).into()
    //                         format!("{}[WHERE id].*", field).into()
    //                     })
    //                     .collect::<Vec<_>>(),
    //             )
    //             .collect::<Vec<_>>()
    //             .into(),
    //     );
    //
    //     Ok(self.set_return_type(projections))
    // }
    //
    /// Sets the return type to projections and fetches the all record links values.
    /// For link_one, link_self, it returns null if the link is null or the reference
    /// does not exist. For link_many, it returns None for items that are null or the reference
    /// do not exist.
    fn load_all_links(self) -> Result<Self> {
        self.load_links(T::get_linked_fields())
    }

    // /// Sets the return type to projections and fetches the all record links values.
    // /// For link_one, link_self, it returns null if the link is null or the reference
    // /// does not exist. For link_many, it returns filters out links that are null or that
    // /// point to reference that does not yet exist
    // fn load_all_links_non_null(self) -> Result<Self> {
    //     self.load_links_non_null(T::get_linked_fields())
    // }

    /// Sets the return type to projections and fetches the all record links values
    /// for link_many fields including the null record links.
    fn load_link_manys(self) -> Result<Self> {
        self.load_links(T::get_link_many_fields())
    }

    // /// Sets the return type to projections and fetches the all record links values
    // /// for link_many fields excluding the null record links.
    // fn load_link_manys_non_null(self) -> Result<Self> {
    //     self.load_links_non_null(T::get_link_many_fields())
    // }

    /// Sets the return type to projections and fetches the all record links values
    /// for link_one fields. Defaults to null if the reference does not exist.
    fn load_link_ones(self) -> Result<Self> {
        self.load_links(T::get_link_one_fields())
    }

    /// Sets the return type to projections and fetches the all record links values
    /// for link_self fields. Defaults to null if the reference does not exist.
    fn load_link_selfs(self) -> Result<Self> {
        self.load_links(T::get_link_self_fields())
    }

    /// Runs the statement against the database and returns the one result.
    fn return_one(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Option<T>>> + Send {
        async move {
            let response = self.run(db).await?;
            get_one::<T>(response)
        }
    }

    /// Runs the statement against the database and returns the one result.
    /// Returns an error if the result is not exactly one.
    fn get_one(self, db: Surreal<impl Connection>) -> impl Future<Output = Result<T>> + Send {
        async move {
            let response = self.run(db).await?;
            get_one::<T>(response)?.ok_or_else(|| Error::RecordNotFound(self.to_debug_sql()))
        }
    }

    /// Runs the statement against the database and returns the many results.
    fn return_many(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Vec<T>>> + Send {
        async move {
            let response = self.run(db).await?;
            get_many::<T>(response)
        }
    }

    /// Runs the statement against the database and returns no result.
    fn return_none(&self, db: Surreal<impl Connection>) -> impl Future<Output = Result<()>> + Send {
        async move {
            self.run(db).await?;
            Ok(())
        }
    }

    /// Runs the statement against the database and returns the first result.
    fn return_first(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Option<T>>> + Send {
        async move {
            let response = self.run(db).await?;
            get_first::<T>(response)
        }
    }

    /// Runs the statement against the database and returns the many results before the change.
    fn return_many_before(
        self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Vec<T>>> + Send {
        async move {
            let query = self.set_return_type(ReturnType::Before);
            query.return_many(db).await
        }
    }

    /// Internal method to set the surrealdb return type of the statement.
    fn set_return_type(self, return_type: ReturnType) -> Self;

    /// Internal method to get the surrealdb return type of the statement.
    fn get_return_type(&self) -> ReturnType;
}

/// A trait that represents a statement that can be run against the database
pub trait ReturnableDefault<T>
where
    Self: Runnable,
    T: SurrealValue,
{
    /// Runs the statement against the database and returns the one result with custom specified
    /// return type.
    fn return_one_explicit<V>(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Option<V>>> + Send
    where
        Self: Sync,
        V: SurrealValue,
    {
        async move {
            let response = self.run(db).await?;
            get_one::<V>(response)
        }
    }

    /// Runs the statement against the database and returns the many results with custom
    /// specified.
    fn return_many_explicit<V>(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Vec<V>>> + Send
    where
        Self: Sync,
        V: SurrealValue,
    {
        async move {
            let response = self.run(db).await?;
            get_many::<V>(response)
        }
    }
}

/// A trait that represents a statement that can be run against the database. Specifically for
/// selct statements.
pub trait ReturnableSelect: Runnable {
    /// Runs the statement against the database and returns no result.
    fn return_none(&self, db: Surreal<impl Connection>) -> impl Future<Output = Result<()>> + Send
    where
        Self: Sync,
    {
        async move {
            self.run(db).await?;
            Ok(())
        }
    }

    /// Runs the statement against the database and returns the first result.
    fn return_first<T>(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Option<T>>> + Send
    where
        Self: Sync,
        T: SurrealValue,
    {
        async move {
            let response = self.run(db).await?;
            get_first::<T>(response)
        }
    }

    /// Runs the statement against the database and returns the one result.
    fn return_one<T>(
        &self,
        db: Surreal<impl surrealdb::Connection>,
    ) -> impl Future<Output = Result<Option<T>>> + Send
    where
        Self: Sync,
        T: SurrealValue,
    {
        async move {
            let response = self.run(db).await?;
            get_one::<T>(response)
        }
    }

    /// Runs the statement against the database and returns the one result with result unchecked.
    fn return_one_unchecked<T>(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = T> + Send
    where
        Self: Sync,
        T: SurrealValue,
    {
        async move {
            let response = self.run(db).await.unwrap();
            get_last::<T>(response).unwrap().unwrap()
        }
    }

    /// Runs the statement against the database and returns the many results.
    fn return_many<T>(
        &self,
        db: Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Vec<T>>> + Send
    where
        Self: Sync,
        T: SurrealValue,
    {
        async move {
            let response = self.run(db).await?;
            get_many::<T>(response)
        }
    }
}

fn get_one<T>(mut response: IndexedResults) -> Result<Option<T>>
where
    T: SurrealValue,
{
    let mut value = response.take::<Vec<T>>(0).map_err(Error::Conversion)?;
    if value.len() > 1 {
        return Err(Error::TooManyItemsReturned { expected: 1 });
    }
    Ok(value.pop())
}

fn get_many<T>(mut response: IndexedResults) -> Result<Vec<T>>
where
    T: SurrealValue,
{
    let value = response.take::<Vec<T>>(0).map_err(Error::Conversion)?;

    Ok(value)
}

fn get_first<T>(mut response: IndexedResults) -> Result<Option<T>>
where
    T: SurrealValue,
{
    let mut value = response.take::<Vec<T>>(0).map_err(Error::Conversion)?;

    let value = if !value.is_empty() {
        Some(value.swap_remove(0))
    } else {
        None
    };

    Ok(value)
}

fn get_last<T>(mut response: IndexedResults) -> Result<Option<T>>
where
    T: SurrealValue,
{
    let mut value = response.take::<Vec<T>>(0).map_err(Error::Conversion)?;

    let value = if !value.is_empty() { value.pop() } else { None };

    Ok(value)
}
