use surrealdb::{
    Connection, Surreal,
    types::{self, SurrealValue, Table, ToSql},
};

use crate::{
    Alias, All, Conditional, Error, Field, IntoNodeClause, ModelMerge, ModelPatch, Raw, RecordId,
    RecordKey, RecordRange, Render, Result, StoredFields, TableOf,
    __private::ModelField,
    execution::{classify, execute},
    query::table_of::select_range,
    statements::{DeleteStatement, SelectCount, SelectStatement, select, update::update},
};

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct FieldMetadata {
    pub name: Field,
    pub old_name: Option<Field>,
    // A single field can have multiple definitions. e.g item assetions and whatnot
    pub definition: Vec<Raw>,
}

/// A record model: a node or an edge, stored in its own table and identified by its
/// `id: RecordId<Self>` field.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an Umbriel model",
    label = "not a model",
    note = "declare `{Self}` with `#[umbriel::model(node)]` or `#[umbriel::model(edge)]`"
)]
// `'static`: a model is an owned record type, so queries can hold statements over it as shared
// nodes.
pub trait Model: Sized + SurrealValue + 'static {
    /// For checking renamed struct field names
    #[doc(hidden)]
    type StructRenamedCreator;

    /// The model's default table, from `table = "…"`. Static, so a `Models` union can check its
    /// members' tables at compile time.
    const TABLE: &'static str;

    /// The prefix of the tables chosen at run time, from `table_prefix = "…"`; `None` when the
    /// model has only its default table. See [`TableSpec`].
    const TABLE_PREFIX: Option<&'static str>;

    /// The model's default table.
    fn table() -> Table {
        Table::new(Self::TABLE)
    }

    /// The table `name` of this model's family, chosen at run time.
    ///
    /// # Errors
    ///
    /// [`Error::TableNotInFamily`] when `name` is neither the default table nor a longer name
    /// starting with the model's `table_prefix`.
    fn table_of(name: &str) -> Result<TableOf<Self>> {
        TableOf::new(name)
    }

    /// The record's id.
    fn record_id(&self) -> &RecordId<Self>;

    /// The id with the given key in this model's default table; [`TableOf::id`] builds one in
    /// another member.
    fn id(key: impl RecordKey) -> RecordId<Self> {
        RecordId::new(key)
    }

    /// The name of the all fields that are serializable
    /// and can potentially be written to the database.
    fn get_serializable_fields() -> Vec<Field>;

    /// The name of the all fields that are linked i.e line_one, line_many, or line_self.
    fn get_linked_fields() -> Vec<Field>;

    /// The names of link_one fields
    fn get_link_one_fields() -> Vec<Field>;

    /// The names of link_self fields
    fn get_link_self_fields() -> Vec<Field>;

    /// The names of link_one and link_self fields
    fn get_link_one_and_self_fields() -> Vec<Field>;

    /// The names of link_many fields
    fn get_link_many_fields() -> Vec<Field>;

    /// Get model's table definition statement
    fn define_table() -> Raw;
    /// Get model's fields definitions statements as a list
    fn define_fields() -> Vec<Raw>;

    /// Get old name of field
    fn get_field_meta() -> Vec<FieldMetadata>;
}

/// Record operations, run through the SDK's typed methods, plus statement builders for queries
/// over the model's table.
pub trait SurrealCrud: Model + Send {
    /// Fetches a record by id.
    fn get(
        db: &Surreal<impl Connection>,
        id: &RecordId<Self>,
    ) -> impl Future<Output = Result<Option<Self>>> + Send {
        async move {
            db.select(types::RecordId::from(id))
                .await
                .map_err(|error| classify(error, None))
        }
    }

    /// Creates or replaces the record with this model's id (`UPSERT … CONTENT`), returning the
    /// stored record.
    fn save(self, db: &Surreal<impl Connection>) -> impl Future<Output = Result<Self>> + Send {
        async move {
            let id = types::RecordId::from(self.record_id());
            let what = id.to_sql();
            db.upsert(id)
                .content(self)
                .await
                .map_err(|error| classify(error, None))?
                .ok_or(Error::RecordNotFound(what))
        }
    }

    /// Deletes this record, returning it as it was before deletion.
    fn delete(
        &self,
        db: &Surreal<impl Connection>,
    ) -> impl Future<Output = Result<Option<Self>>> + Send
    where
        Self: Sync,
    {
        Self::delete_by_id(db, self.record_id())
    }

    /// Deletes a record by id, returning it as it was before deletion.
    fn delete_by_id(
        db: &Surreal<impl Connection>,
        id: &RecordId<Self>,
    ) -> impl Future<Output = Result<Option<Self>>> + Send {
        async move {
            db.delete(types::RecordId::from(id))
                .await
                .map_err(|error| classify(error, None))
        }
    }

    /// Applies a partial update in one statement and returns the record afterwards.
    ///
    /// Every field that is not [`Patch::Keep`](crate::Patch::Keep) becomes an assignment in a
    /// single `UPDATE … SET`; `Set` replaces the field's value, nested objects included. A patch
    /// with only `Keep` fields reads the record with `SELECT` instead, so it causes no write or
    /// update-side effects. `None` means no record was returned: the record does not exist or is
    /// not visible.
    fn patch<P>(
        db: &Surreal<impl Connection>,
        id: &RecordId<Self>,
        patch: P,
    ) -> impl Future<Output = Result<Option<Self>>> + Send
    where
        P: ModelPatch<Model = Self> + Send,
    {
        async move {
            let setters = patch.into_setters();
            let query = if setters.is_empty() {
                select(All).from(id).to_query()?
            } else {
                update::<Self>(id).set(setters).to_query()?
            };
            execute(db, query).await?.take(0).map_err(Error::Conversion)
        }
    }

    /// Merges a partial record into the record (`UPDATE … MERGE`) and returns it afterwards.
    ///
    /// Unlike [`patch`](Self::patch), whose `Set` replaces a field whole, a merge combines nested
    /// objects at every level: [`Merge::Fields`](crate::Merge::Fields) changes only the named
    /// fields of an embedded object, and stored fields the merge does not mention survive. Arrays
    /// are replaced whole. A merge with only `Keep` fields reads the record with `SELECT` instead,
    /// so it causes no write or update-side effects; [`update(..).merge(..)`](crate::statements::update)
    /// writes `MERGE {}` as built. `None` means no record was returned.
    fn merge<M>(
        db: &Surreal<impl Connection>,
        id: &RecordId<Self>,
        merge: M,
    ) -> impl Future<Output = Result<Option<Self>>> + Send
    where
        M: ModelMerge<Model = Self> + Send,
    {
        async move {
            let object = merge.into_object();
            let query = if object.is_empty() {
                select(All).from(id).to_query()?
            } else {
                update::<Self>(id).merge_object(object).to_query()?
            };
            execute(db, query).await?.take(0).map_err(Error::Conversion)
        }
    }

    /// Inserts several records into the default table in one SDK call
    /// (`insert(table).content(records)`), returning the stored records;
    /// [`TableOf::insert_many`] inserts into another member.
    ///
    /// # Errors
    ///
    /// [`Error::QueryBuilder`] when a record's id names another table, and
    /// [`Error::RecordExists`] when a record's id already exists, with the database error kept.
    fn insert_many(
        db: &Surreal<impl Connection>,
        records: Vec<Self>,
    ) -> impl Future<Output = Result<Vec<Self>>> + Send {
        async move { TableOf::<Self>::default().insert_many(db, records).await }
    }

    /// Fetches the records whose keys fall in `range`, e.g. `User::get_range(&db, a..b)`, in the
    /// range's own table.
    ///
    /// # Errors
    ///
    /// [`Error::QueryBuilder`] when the range's bounds name different tables.
    fn get_range(
        db: &Surreal<impl Connection>,
        range: impl Into<RecordRange<Self>>,
    ) -> impl Future<Output = Result<Vec<Self>>> + Send {
        let parts = range.into().into_parts();
        async move {
            let (table, keys) = parts?;
            select_range(db, &table, keys).await
        }
    }

    /// Fetches the records with the given ids that exist, in one SDK call. Missing ids return
    /// nothing.
    fn get_many(
        db: &Surreal<impl Connection>,
        ids: &[RecordId<Self>],
    ) -> impl Future<Output = Result<Vec<Self>>> + Send {
        let ids = ids
            .iter()
            .map(|id| types::Value::RecordId(id.into()))
            .collect::<Vec<_>>();
        async move {
            db.select::<Vec<Self>>(ids)
                .await
                .map_err(|error| classify(error, None))
        }
    }

    /// `SELECT * FROM table` over the default table, decoded as the model;
    /// [`TableOf::select`] reads another member.
    fn select() -> SelectStatement<Self> {
        TableOf::<Self>::default().select()
    }

    /// `SELECT VALUE field` over the default table, decoded as the field's own type `V`. The
    /// field must be one of this model's generated schema fields.
    fn select_value<F, V>(field: F) -> SelectStatement<V>
    where
        F: ModelField<Self, V>,
        V: SurrealValue,
    {
        TableOf::<Self>::default().select_value(field)
    }

    /// Finds records of the default table by filtering; [`TableOf::find_where`] searches
    /// another member.
    fn find_where(filter: impl Conditional + Clone) -> SelectStatement<Self> {
        TableOf::<Self>::default().find_where(filter)
    }

    /// Counts filtered records of the default table.
    fn count_where(filter: impl Conditional + Clone) -> SelectCount {
        TableOf::<Self>::default().count_where(filter)
    }

    /// Counts every record of the default table.
    fn count_all() -> SelectCount {
        TableOf::<Self>::default().count_all()
    }

    /// Deletes records of the default table by filtering.
    fn delete_where(filter: impl Conditional + Clone) -> DeleteStatement<Self> {
        TableOf::<Self>::default().delete_where(filter)
    }
}

impl<T> SurrealCrud for T where T: Model + Send {}

/// Record operations for nodes.
pub trait SurrealCrudNode: Node + Send {
    /// Creates the record with this model's id (`CREATE … CONTENT`), returning the stored
    /// record. Fails if a record with the same id already exists.
    fn create(self, db: &Surreal<impl Connection>) -> impl Future<Output = Result<Self>> + Send {
        async move {
            let id = types::RecordId::from(self.record_id());
            let what = id.to_sql();
            db.create(id)
                .content(self)
                .await
                .map_err(|error| classify(error, None))?
                .ok_or(Error::RecordNotFound(what))
        }
    }
}

impl<T> SurrealCrudNode for T where T: Node + Send {}

/// Node is a trait signifying a node in the graph
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a node",
    label = "not a node",
    note = "declare `{Self}` with `#[umbriel::model(node)]`"
)]
pub trait Node: Model + SchemaGetter {
    /// The type of the aliases
    type Aliases;
    #[doc(hidden)]
    type TableNameChecker;

    /// returns the alias names of the relation graph strings of the model
    /// e.g for relation - `->edge->graph AS alias`, the alias would be alias.
    fn aliases() -> Self::Aliases;

    /// returns the table name of the node
    fn get_table() -> Table;

    /// Starts a graph path at this node: a record id of this model (or a list of them), a filter,
    /// a parameter or a subquery. A record id of another model does not compile; see
    /// [`IntoNodeClause`] for what the database checks instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use umbriel::*;
    /// # #[model(node, table = "student")]
    /// # pub struct Student {
    /// #     pub id: RecordId<Self>,
    /// #     #[orm(relate(model = "StudentWritesBook", connection = "->writes->book"))]
    /// #     pub books: Relate<Book>,
    /// # }
    /// # #[model(node, table = "book")]
    /// # pub struct Book { pub id: RecordId<Self> }
    /// # #[model(edge, table = "writes")]
    /// # pub struct Writes<In: Node, Out: Node> {
    /// #     pub id: RecordId<Self>,
    /// #     #[orm(link_one = "In")]
    /// #     pub r#in: LinkOne<In>,
    /// #     #[orm(link_one = "Out")]
    /// #     pub out: LinkOne<Out>,
    /// # }
    /// # pub type StudentWritesBook = Writes<Student, Book>;
    ///
    /// let books = Student::with(Student::id("alice")).writes__(E).book(E);
    /// assert_eq!(Field::from(books).to_debug_sql(), "student:alice->writes->book");
    /// ```
    fn with(clause: impl IntoNodeClause<Self>) -> Self::Schema;

    /// returns the relations aliases of the model in the format `->edge->graph AS alias`.
    fn get_fields_relations_aliased() -> Vec<Alias>;
}

/// Edge is a trait signifying an edge in the graph
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an edge",
    label = "not an edge",
    note = "declare `{Self}` with `#[umbriel::model(edge)]`"
)]
pub trait Edge: Model + SchemaGetter {
    /// The Origin node
    type In;
    /// The Destination node
    type Out;
    #[doc(hidden)]
    type TableNameChecker;

    /// returns the table name of the edge
    fn get_table() -> Table;
}

/// Object is a trait signifying a nested object in the graph
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an embedded object",
    label = "not an object",
    note = "declare `{Self}` with `#[umbriel::model(object)]`"
)]
pub trait Object: SurrealValue + SchemaGetter {
    /// The names this object stores, including those of the objects it flattens. A model that
    /// flattens this object stores them as its own fields.
    #[doc(hidden)]
    const STORED_FIELDS: StoredFields;

    /// The generated `<Object>Merge`, which merges a subset of this object's fields when it is
    /// nested in a model.
    #[doc(hidden)]
    type Merge: crate::MergeFields + Default;

    /// The metadata and definitions of this object's fields as top-level fields of `table`, for
    /// a model that flattens this object.
    #[doc(hidden)]
    fn field_meta_on(table: Table) -> Vec<FieldMetadata>;
}

/// Trait for getting the schema of a node for generating graph strings
pub trait SchemaGetter {
    /// The type of the schema
    type Schema;
    /// returns the schema of the node for generating graph strings e.g
    ///  let UserSchema { id, age, name, email, writes__ } = User::schema();
    fn schema() -> Self::Schema;
    /// Same as schema but prefixed. Useful for traversing the graph for e.g aliases.
    fn schema_prefixed(prefix: impl Into<Field>) -> Self::Schema;
}
