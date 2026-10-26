use surrealdb::{
    Connection, Surreal,
    types::{self, SurrealValue, Table, ToSql},
};

use crate::{
    Alias, All, Conditional, Error, Field, Filter, ModelPatch, NodeClause, Raw, RecordId,
    RecordKey, Render, Result, StoredFields, count,
    execution::{classify, execute},
    statements::{
        SelectStatementMini,
        delete::{DeleteStatementMini, delete},
        select::{SelectStatementCount, select},
        select_value,
        update::update,
    },
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

    /// The name of the model/table
    fn table() -> Table;

    /// The record's id.
    fn record_id(&self) -> &RecordId<Self>;

    /// The id with the given key in this model's table.
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

    /// Finds records by filtering.
    fn find_where(filter: impl Conditional + Clone) -> SelectStatementMini<Self> {
        select(All).from(Self::table()).where_(filter).into()
    }

    /// Count filtered records.
    fn count_where(filter: impl Conditional + Clone) -> SelectStatementCount {
        let selection = select_value(Field::new("count")).from(
            select(count!(Filter::new(filter)))
                .from(Self::table())
                .group_all(),
        );
        selection.into()
    }

    /// Count all records.
    fn count_all() -> SelectStatementCount {
        let selection = select_value(Field::new("count"))
            .from(select(count!()).from(Self::table()).group_all());
        selection.into()
    }

    /// Deletes records by filtering.
    fn delete_where(filter: impl Conditional + Clone) -> DeleteStatementMini<Self> {
        delete::<Self>(Self::table()).where_(filter).into()
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

    /// Useful in relate statement for attaching id or statement to a node
    /// Example:
    /// ```rust, ignore
    /// Student::with(Empty).writes__(Empty).book(Empty);
    ///
    /// relate(Student::with(student_id).writes__(Empty).book(book_id)).content(write);
    /// ```
    fn with(clause: impl Into<NodeClause>) -> Self::Schema;

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
