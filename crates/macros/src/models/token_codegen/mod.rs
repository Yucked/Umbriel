mod aliases;
mod define_statement;
mod field_connection_build;
mod field_metadata;
mod field_type_assertions;
mod field_value_setter;
mod flatten;
mod link_methods;
mod patch;
mod relate;
mod serialized_field_fmts;
mod simple;

use quote::format_ident;

use crate::models::*;

use self::{
    field_value_setter::FieldSetterImplTokens, link_methods::LinkFieldTraversalMethodToken,
    relate::NodeEdgeMetadataLookupTable,
};

create_ident_wrapper!(AliasesStructNameIdent);
create_ident_wrapper!(TestFunctionNameIdent);
create_ident_wrapper!(StructWithRenamedSerializedFields);

/// The names of the items generated next to a model. None of them is inside a module, so a model
/// declared in a function can name them.
pub(crate) struct CommonIdents {
    /// `<Model>Schema`
    pub schema: syn::Ident,
    /// The model's stored names, a constant that never names the model's generic parameters.
    pub stored_fields: syn::Ident,
    /// The struct `assert_fields!` checks relations against: one field named after the table.
    pub table_name_checker: syn::Ident,
    pub aliases_struct_name: AliasesStructNameIdent,
    pub test_function_name: TestFunctionNameIdent,
    pub struct_with_renamed_serialized_fields: StructWithRenamedSerializedFields,
}

#[derive(Default, Clone)]
pub struct Codegen<'a> {
    /// list of fields names that are actually serialized and not skipped as string.
    /// Usually, this is every field except the many-to-many relation fields which
    /// are not serialized because they are not directly part of the struct.
    /// They are readonly fields that are accessed through the relation field.
    pub serialized_fmt_db_field_names_instance: Vec<SerializableField>,
    /// The stored names `object!` takes: the stored fields without the computed ones, which are
    /// never written.
    pub object_field_names: Vec<SerializableField>,
    /// The name of the all fields that are linked i.e line_one, line_many, or line_self.
    pub linked_fields: Vec<LinkedField>,
    /// The names of link_one fields
    pub link_one_fields: Vec<LinkOneField>,
    /// The names of link_self fields
    pub link_self_fields: Vec<LinkSelfField>,
    /// The names of link_one and link_self fields
    pub link_one_and_self_fields: Vec<LinkOneAndSelfField>,
    /// The names of link_many fields
    pub link_many_fields: Vec<LinkManyField>,
    /// Generated example: pub timeWritten: Field,
    /// key(normalized_field_name)-value(Field) e.g pub out: Field, of field name and Field type
    /// to build up struct for generating fields of a Schema of the Edge
    /// The full thing can look like, next to the model (no module):
    /// ```rust,ignore
    /// pub struct __6Writes2idField<In, Out>(pub Field, pub PhantomData<fn() -> (In, Out,)>);
    /// pub struct __6Writes11timeWrittenField<In, Out>(pub Field, pub PhantomData<fn() -> (In, Out,)>);
    ///
    /// pub struct WritesSchema<In, Out> {
    ///     pub id: __6Writes2idField<In, Out>,
    ///     pub timeWritten: __6Writes11timeWrittenField<In, Out>,
    /// }
    /// ```
    pub schema_struct_fields_types_kv: Vec<SchemaStructFieldsTypesKv>,

    pub field_wrapper_type_custom_implementations: Vec<FieldSetterImplTokens>,

    /// Generated example: `timeWritten: ___________path.clone().with_field("timeWritten").into(),`
    /// Each schema field is the schema's path followed by the field's stored name.
    pub schema_struct_fields_names_kv: Vec<SchemaStructFieldsNamesKv>,
    /// The schema's field idents, for its fieldwise `Clone`.
    pub schema_field_idents: Vec<TokenStream>,

    /// Generated example: pub writtenBooks: AliasName,
    /// This is used when you have a relate attribute signaling a graph with e.g node->edge->node
    /// The full thing can look like:
    /// ```rust,ignore
    ///     #[derive(Debug, Default)]
    ///     pub struct Writes<Model: ::serde::Serialize + Default> {
    ///                pub writtenBooks: AliasName,
    ///          }
    /// ```
    pub aliases_struct_fields_types_kv: Vec<AliasesStructFieldsTypesKv>,

    /// Generated example: pub writtenBooks: "writtenBooks".into(),
    /// This is used to build the actual instance of the struct with aliases
    /// The full thing can look like and the fields should be in normalized form:
    /// i.e writtenBooks => writtenBooks if serde camelizes
    /// ```rust, ignore
    /// Self {
    ///                pub writtenBooks: AliasName,
    /// }
    /// ```
    pub aliases_struct_fields_names_kv: Vec<AliasesStructFieldsNamesKv>,

    /// Field names after taking into consideration
    /// serde serialized renaming or casings
    /// i.e time_written => timeWritten if serde camelizes
    pub serialized_field_names_normalised: Vec<DbFieldName>,

    /// Generated example:
    /// ```rust,ignore
    /// // For relate field
    /// type StudentWritesBlogTableName = <StudentWritesBlog as Edge>::TableNameChecker;
    /// #crate_name::validators::assert_fields!(StudentWritesBlogTableName: Writes);
    ///
    /// type StudentWritesBlogInNode = <StudentWritesBlog as Edge>::In;
    /// #crate_name::validators::assert_type_eq_all!(StudentWritesBlogInNode, Student);
    ///
    /// type StudentWritesBlogOutNode = <StudentWritesBlog as Edge>::Out;
    /// #crate_name::validators::assert_type_eq_all!(StudentWritesBlogOutNode, Blog);
    ///
    ///
    /// #crate_name::validators::assert_impl_one!(StudentWritesBlog: Edge);
    /// #crate_name::validators::assert_impl_one!(Student: Node);
    /// #crate_name::validators::assert_impl_one!(Blog: Node);
    /// #crate_name::validators::assert_type_eq_all!(LinkOne<Book>, LinkOne<Book>);
    /// ```
    /// Perform all necessary static checks
    pub static_assertions: Vec<StaticAssertionToken>,

    /// When a field references another model as Link, we want to generate a method for that
    /// to be able to access the foreign fields
    /// Generated Example for e.g field with best_student: <Student>
    /// ```rust, ignore
    /// pub fn best_student(&self, filter: Filter) -> Student {
    ///     Student::__________connect_to_graph_traversal_string(&self.___________graph_traversal_string, filter)
    /// }
    /// ```
    pub record_link_fields_methods: Vec<LinkFieldTraversalMethodToken>,
    /// Each stored field's name and definitions, written for the table `_____table`.
    pub field_metadata: Vec<FieldMetadataToken>,
    /// The types of flattened objects, whose fields are stored as this model's own.
    pub flattened_objects: Vec<TokenStream>,
    /// The Rust names of flattened fields: `object!` requires them, and rejects them since their
    /// schemas are objects rather than assignable fields.
    pub flattened_field_keys: Vec<TokenStream>,
    pub node_edge_metadata: NodeEdgeMetadataLookupTable<'a>,
    pub fields_relations_aliased: Vec<FieldsRelationsAliased>,
    /// e.g `pub time: umbriel::Patch<Time>` in `<Model>Patch`.
    pub patch_fields: Vec<PatchField>,
    /// e.g `time: umbriel::Patch::Keep`.
    pub patch_defaults: Vec<PatchDefault>,
    /// Adds the assignment of a patch field that is not `Keep`.
    pub patch_setters: Vec<PatchSetter>,
    /// e.g `pub time: umbriel::Merge<Time, <Time as umbriel::Object>::Merge>` in `<Model>Merge`.
    pub merge_fields: Vec<TokenStream>,
    /// e.g `time: umbriel::Merge::Keep`.
    pub merge_defaults: Vec<TokenStream>,
    /// Adds the entry of a merge field that is not `Keep`.
    pub merge_entries: Vec<TokenStream>,
    pub renamed_serialized_fields_kv: Vec<RenamedSerializedFields>,
    /// The Rust field holding the record's `id`.
    pub record_id_field: Option<syn::Ident>,

    struct_attributes_data: StructAttributesData<'a>,
}

#[derive(Clone, Default)]
struct StructAttributesData<'a> {
    field_receiver: Option<&'a MyFieldReceiver>,
    struct_basic_model_attributes: Option<&'a ModelAttributes<'a>>,
}

impl<'a> Codegen<'a> {
    fn new(model_attributes: &'a ModelAttributes) -> Self {
        let struct_attributes_data = StructAttributesData {
            struct_basic_model_attributes: Some(model_attributes),
            field_receiver: None,
        };

        Self {
            struct_attributes_data,
            ..Default::default()
        }
    }

    pub(crate) fn common_idents(&self) -> CommonIdents {
        let model = self.table_derive_attributes().ident();
        let struct_name_ident = model.to_string();

        CommonIdents {
            schema: model.schema_ident(),
            stored_fields: model.hidden_ident("StoredFields"),
            table_name_checker: model.hidden_ident("TableNameChecker"),
            test_function_name: model.hidden_ident("StaticAssertions").into(),
            aliases_struct_name: format_ident!("{struct_name_ident}Aliases").into(),
            struct_with_renamed_serialized_fields: model.hidden_ident("RenamedCreator").into(),
        }
    }

    fn set_field_receiver(&mut self, field_receiver: &'a MyFieldReceiver) {
        self.struct_attributes_data.field_receiver = Some(field_receiver);
    }

    pub(crate) fn field_receiver(&self) -> &'a MyFieldReceiver {
        self.struct_attributes_data.field_receiver
            .expect("Field receiver has not been set. Make sure it has been set by calling set_field_receiver")
    }

    pub(crate) fn table_derive_attributes(&self) -> &'a ModelAttributes<'a> {
        self.struct_attributes_data
            .struct_basic_model_attributes
            .expect("Model attributes has not been set. Make sure it has been set by calling set_field_receiver")
    }

    pub(crate) fn data_type(&self) -> DataType {
        self.table_derive_attributes().to_data_type()
    }

    /// Derive the schema properties for a struct
    pub(crate) fn parse_fields(model_attributes: &'a ModelAttributes<'a>) -> ExtractorResult<Self> {
        let mut tokens_generator = Self::new(model_attributes);

        for field_receiver in model_attributes.fields()? {
            tokens_generator.set_field_receiver(field_receiver);

            if let Some(flatten) = surreal_flag(&field_receiver.attrs, "flatten")? {
                tokens_generator.create_flattened_field(&flatten)?;
                continue;
            }

            tokens_generator.create_table_id_type_token()?;
            tokens_generator.create_field_definition_checks()?;
            tokens_generator.create_db_field_names_token()?;
            tokens_generator.create_field_type_static_assertion_token()?;
            tokens_generator.create_field_setter_impl()?;
            tokens_generator.create_field_metadata_token()?;
            tokens_generator.create_field_connection_builder_token()?;
            tokens_generator.create_relation_connection_tokenstream()?;
            tokens_generator.create_db_fields_for_links_and_loaders()?;
            tokens_generator.create_link_methods()?;
            tokens_generator.create_relation_aliases_struct_fields_types_kv()?;
            tokens_generator.create_patch_metadata()?;
        }
        Ok(tokens_generator)
    }
}
