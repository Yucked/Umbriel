use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, Meta, Token, punctuated::Punctuated};

use crate::models::*;

/// Expands `#[umbriel::model(node | edge | object, table = "…", value = manual)]`.
///
/// The struct gains the SDK's `SurrealValue` derive (unless `value = manual`) and the hidden
/// `ModelCodegen` derive, which receives the model kind and table as `#[orm(..)]`.
pub fn model_attribute(args: TokenStream, item: TokenStream) -> TokenStream {
    match expand_model_attribute(args, item.clone()) {
        Ok(tokens) => tokens,
        Err(error) => {
            let error = error.to_compile_error();
            quote!(#error #item)
        }
    }
}

fn expand_model_attribute(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let crate_name = get_crate_name(false);
    let args = syn::parse::Parser::parse2(Punctuated::<Meta, Token![,]>::parse_terminated, args)?;

    let mut kind = None;
    let mut table = None;
    let mut table_prefix = None;
    let mut manual_value = false;
    for arg in &args {
        match arg {
            Meta::Path(path)
                if path.is_ident("node") || path.is_ident("edge") || path.is_ident("object") =>
            {
                if kind.replace(path.clone()).is_some() {
                    return Err(syn::Error::new_spanned(
                        path,
                        "a model is exactly one of `node`, `edge` or `object`",
                    ));
                }
            }
            Meta::NameValue(name_value) if name_value.path.is_ident("table") => {
                table = Some(name_value.value.clone());
            }
            Meta::NameValue(name_value) if name_value.path.is_ident("table_prefix") => {
                table_prefix = Some(name_value.value.clone());
            }
            Meta::NameValue(name_value) if name_value.path.is_ident("value") => {
                match &name_value.value {
                    Expr::Path(path) if path.path.is_ident("manual") => manual_value = true,
                    value => {
                        return Err(syn::Error::new_spanned(
                            value,
                            "the only `value` setting is `manual`, for a hand-written `SurrealValue`",
                        ));
                    }
                }
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected `node`, `edge` or `object`, `table = \"…\"`, `table_prefix = \"…\"` \
                     or `value = manual`",
                ));
            }
        }
    }

    let kind = kind.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected the model kind: `node`, `edge` or `object`",
        )
    })?;
    let table = table.map(|table| quote!(, table = #table));
    let table_prefix = table_prefix.map(|prefix| quote!(, table_prefix = #prefix));

    // `SurrealValue` names its runtime through the consumer's own path to Umbriel, so a
    // renamed dependency works without a direct `surrealdb` dependency. The derive writes
    // `impl SurrealValue for …` unqualified, so the trait is brought into scope through a glob:
    // an explicit import of it shadows the glob, and repeated globs name the same trait.
    let (surreal_value_scope, surreal_value) = if manual_value {
        (None, None)
    } else {
        let types_path = quote!(#crate_name::types).to_string();
        (
            Some(quote!(
                #[allow(unused_imports)]
                use #crate_name::__private::surreal_value::*;
            )),
            Some(quote!(
                #[derive(#crate_name::types::SurrealValue)]
                #[surreal(crate = #types_path)]
            )),
        )
    };

    Ok(quote!(
        #surreal_value_scope
        #surreal_value
        #[derive(#crate_name::__private::ModelCodegen)]
        #[orm(#kind #table #table_prefix)]
        #item
    ))
}

impl ToTokens for ModelReceiver {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self.expand() {
            Ok(expanded) => tokens.extend(expanded),
            Err(error) => tokens.extend(error.write_errors()),
        }
    }
}

impl ModelReceiver {
    fn expand(&self) -> ExtractorResult<TokenStream> {
        let crate_name = get_crate_name(false);
        let model_attributes = ModelAttributes::new(self)?;
        let data_type = model_attributes.to_data_type();
        let struct_name_ident = &self.ident();
        let (impl_generics, ty_generics, where_clause) = self.generics().split_for_impl();
        let struct_marker = self.generics().phantom_marker_type();
        let explicit_generics = model_attributes.explicit_fully_qualified_generics_path();
        let VariablesModelMacro {
            ___________path,
            __________connect,
            _____struct_marker_ident,
            _____table,
        } = VariablesModelMacro::new();

        let code_gen = Codegen::parse_fields(&model_attributes)?;
        let Codegen {
            schema_struct_fields_types_kv,
            schema_struct_fields_names_kv,
            field_wrapper_type_custom_implementations,
            static_assertions,
            record_link_fields_methods,
            serialized_fmt_db_field_names_instance: own_stored_fields,
            flattened_objects,
            field_metadata,
            schema_field_idents,
            ..
        } = &code_gen;

        let CommonIdents {
            schema,
            stored_fields,
            table_name_checker,
            test_function_name,
            ..
        } = code_gen.common_idents();

        let record_model = match data_type {
            DataType::Node | DataType::Edge => Some(self.record_model_tokens(&code_gen)?),
            DataType::Object => None,
        };

        let merge_ident = struct_name_ident.merge_ident();
        let merge_struct = self.merge_struct_tokens(&code_gen)?;

        let model_kind_impl = match data_type {
            DataType::Node => self.node_tokens(&code_gen)?,
            DataType::Edge => self.edge_tokens(&code_gen)?,
            DataType::Object => quote!(
                impl #impl_generics #crate_name::Object for #struct_name_ident #ty_generics #where_clause {
                    const STORED_FIELDS: #crate_name::StoredFields = #stored_fields;

                    type Merge = #merge_ident #ty_generics;

                    fn field_meta_on(#_____table: #crate_name::Table) -> ::std::vec::Vec<#crate_name::FieldMetadata> {
                        ::std::iter::IntoIterator::into_iter([#( #field_metadata ),*])
                            #( .chain(<#flattened_objects as #crate_name::Object>::field_meta_on(#_____table.clone())) )*
                            .collect()
                    }
                }
            ),
        };

        // The stored names are string literals and the flattened objects are concrete types, so
        // these constants never name the model's generic parameters and the check is evaluated
        // for every model, used or not.
        let stored_fields_definition = quote!(
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            const #stored_fields: #crate_name::StoredFields = #crate_name::StoredFields::new(
                &[#( #own_stored_fields ),*],
                &[#( &<#flattened_objects as #crate_name::Object>::STORED_FIELDS ),*],
            );
            const _: () = #stored_fields.assert_valid();
        );

        // Lets `assert_fields!` check that relations name a real node or edge table.
        let table_name_checker_definition = model_attributes.table()?.map(|table| {
            quote!(
                // The field is the table's name as written, e.g. `assignedTo`.
                #[doc(hidden)]
                #[allow(non_snake_case)]
                pub struct #table_name_checker {
                    pub #table: ::std::string::String,
                }
            )
        });

        let node_only = matches!(data_type, DataType::Node).then(|| {
            let Codegen {
                aliases_struct_fields_types_kv,
                aliases_struct_fields_names_kv,
                node_edge_metadata,
                ..
            } = &code_gen;
            let aliases_struct_name = &code_gen.common_idents().aliases_struct_name;
            quote!(
                #[derive(Debug, Clone)]
                #[allow(non_snake_case)]
                pub struct #aliases_struct_name {
                   #( #aliases_struct_fields_types_kv) *
                }

                impl #aliases_struct_name {
                    pub fn new() -> Self {
                        Self {
                           #( #aliases_struct_fields_names_kv) *
                        }
                    }
                }

                #node_edge_metadata
            )
        });

        // Everything is generated next to the model, outside any module: a module cannot see
        // items declared in a function, so a model declared there could not be named from one.
        Ok(quote!(
            impl #impl_generics #crate_name::SchemaGetter for #struct_name_ident #ty_generics #where_clause {
                type Schema = #schema #ty_generics;

                fn schema() -> Self::Schema {
                    #schema #explicit_generics ::new()
                }

                fn schema_prefixed(prefix: impl ::std::convert::Into<#crate_name::Field>) -> Self::Schema {
                    #schema #explicit_generics ::new_prefixed(prefix)
                }
            }

            #model_kind_impl

            #record_model

            #merge_struct

            #stored_fields_definition

            #table_name_checker_definition

            #( #field_wrapper_type_custom_implementations) *

            /// Paths to the model's fields, from the query root or a graph traversal.
            #[allow(non_snake_case)]
            pub struct #schema #ty_generics #where_clause {
               #( #schema_struct_fields_types_kv) *
                #___________path: #crate_name::Field,
                #_____struct_marker_ident: #struct_marker,
            }

            // Fieldwise and hand-written: derives would require the model's generics, and the
            // type arguments of flattened objects, to be `Debug`/`Clone`. Every field is a field
            // wrapper or another schema, which implement both without bounds.
            impl #impl_generics ::std::fmt::Debug for #schema #ty_generics #where_clause {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    f.debug_struct(::std::stringify!(#schema))
                        #( .field(stringify!(#schema_field_idents), &self.#schema_field_idents) )*
                        .field("path", &self.#___________path)
                        .finish()
                }
            }

            // A clone keeps fields the caller reassigned.
            impl #impl_generics ::std::clone::Clone for #schema #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    Self {
                        #( #schema_field_idents: ::std::clone::Clone::clone(&self.#schema_field_idents), )*
                        #___________path: ::std::clone::Clone::clone(&self.#___________path),
                        #_____struct_marker_ident: ::std::marker::PhantomData,
                    }
                }
            }

            impl #impl_generics #crate_name::Render for #schema #ty_generics #where_clause {
                fn render(&self, out: &mut #crate_name::Renderer) {
                    out.push(&self.#___________path);
                }
            }

            impl #impl_generics #crate_name::Aliasable for #schema #ty_generics #where_clause {}

            impl #impl_generics ::std::convert::From<&#schema #ty_generics> for #crate_name::Field #where_clause {
                fn from(schema: &#schema #ty_generics) -> Self {
                    schema.#___________path.clone()
                }
            }

            impl #impl_generics ::std::convert::From<#schema #ty_generics> for #crate_name::Field #where_clause {
                fn from(schema: #schema #ty_generics) -> Self {
                    schema.#___________path
                }
            }

            impl #impl_generics ::std::convert::From<#schema #ty_generics> for #crate_name::ValueLike #where_clause {
                fn from(schema: #schema #ty_generics) -> Self {
                    #crate_name::ValueLike::from(schema.#___________path)
                }
            }

            // Link methods are named after the stored field, like the schema's fields.
            #[allow(non_snake_case)]
            impl #impl_generics #schema #ty_generics #where_clause {
                pub fn new() -> Self {
                    Self::___________with_path(#crate_name::Field::default())
                }

                pub fn new_prefixed(prefix: impl ::std::convert::Into<#crate_name::Field>) -> Self {
                    Self::___________with_path(prefix.into())
                }

                /// The schema at the end of `path` extended by `clause`.
                #[doc(hidden)]
                pub fn #__________connect(
                    path: impl ::std::convert::Into<#crate_name::Field>,
                    clause: #crate_name::Clause,
                ) -> Self {
                    let mut path = path.into();
                    path.push_clause(clause);
                    Self::___________with_path(path)
                }

                fn ___________with_path(#___________path: #crate_name::Field) -> Self {
                    Self {
                       #( #schema_struct_fields_names_kv) *
                        #___________path,
                        #_____struct_marker_ident: ::std::marker::PhantomData,
                    }
                }

                #( #record_link_fields_methods) *
            }

            #node_only

            #[allow(non_snake_case, unreachable_code, dead_code)]
            fn #test_function_name #impl_generics() #where_clause {
                #( #static_assertions) *
            }
        ))
    }

    /// `Model`, its patch type and the helpers `object!` checks keys with.
    fn record_model_tokens(&self, code_gen: &Codegen) -> ExtractorResult<TokenStream> {
        let crate_name = get_crate_name(false);
        let struct_name_ident = &self.ident();
        let (impl_generics, ty_generics, where_clause) = self.generics().split_for_impl();
        let struct_marker = self.generics().phantom_marker_type();
        let vis = &self.vis;
        let table = self
            .table()?
            .expect("a node or edge has a table")
            .as_string();
        let table_prefix = match self.table_prefix()? {
            Some(prefix) => quote!(::std::option::Option::Some(#prefix)),
            None => quote!(::std::option::Option::None),
        };
        let table_definitions = self.get_table_definition_token()?;
        let patch_ident = struct_name_ident.patch_ident();
        let VariablesModelMacro {
            _____struct_marker_ident,
            _____table,
            ..
        } = VariablesModelMacro::new();
        let Codegen {
            serialized_fmt_db_field_names_instance: serializable_fields,
            linked_fields,
            link_one_fields,
            link_self_fields,
            link_one_and_self_fields,
            link_many_fields,
            field_metadata,
            flattened_objects,
            flattened_field_keys,
            renamed_serialized_fields_kv,
            patch_fields,
            patch_defaults,
            patch_setters,
            record_id_field,
            ..
        } = code_gen;
        let CommonIdents {
            struct_with_renamed_serialized_fields,
            stored_fields,
            ..
        } = code_gen.common_idents();
        let object_keys_count = serializable_fields.len() + flattened_field_keys.len();
        let record_id_field = record_id_field.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(
                &self.ident,
                "a node or edge needs its record id: `id: RecordId<Self>`",
            )
        })?;
        let patch_doc = format!(
            "A partial update of [`{}`]: each field left as `Patch::Keep` is unchanged.",
            self.ident
        );

        Ok(quote!(
            impl #impl_generics #struct_name_ident #ty_generics #where_clause {
                /// The keys `object!` requires: the stored fields, and each flattened field, which
                /// it then rejects since a flattened object is not one assignable field.
                #[doc(hidden)]
                pub const fn __get_serializable_field_names() -> [&'static str; #object_keys_count] {
                    [#( #serializable_fields, )* #( #flattened_field_keys, )*]
                }
            }

            #[doc(hidden)]
            #[allow(non_snake_case)]
            pub struct #struct_with_renamed_serialized_fields {
               #(#renamed_serialized_fields_kv), *
            }

            impl #impl_generics #crate_name::Model for #struct_name_ident #ty_generics #where_clause {
                type StructRenamedCreator = #struct_with_renamed_serialized_fields;

                const TABLE: &'static str = #table;

                const TABLE_PREFIX: ::std::option::Option<&'static str> = #table_prefix;

                fn record_id(&self) -> &#crate_name::RecordId<Self> {
                    &self.#record_id_field
                }

                fn get_serializable_fields() -> ::std::vec::Vec<#crate_name::Field> {
                    let mut fields = ::std::vec::Vec::new();
                    #stored_fields.for_each(&mut |name| fields.push(#crate_name::Field::new(name)));
                    fields
                }

                fn get_linked_fields() -> ::std::vec::Vec<#crate_name::Field> {
                    ::std::vec![#( #linked_fields ), *]
                }

                fn get_link_one_fields() -> ::std::vec::Vec<#crate_name::Field> {
                    ::std::vec![#( #link_one_fields ), *]
                }

                fn get_link_self_fields() -> ::std::vec::Vec<#crate_name::Field> {
                    ::std::vec![#( #link_self_fields ), *]
                }

                fn get_link_one_and_self_fields() -> ::std::vec::Vec<#crate_name::Field> {
                    ::std::vec![#( #link_one_and_self_fields ), *]
                }

                fn get_link_many_fields() -> ::std::vec::Vec<#crate_name::Field> {
                    ::std::vec![#( #link_many_fields ), *]
                }

                fn define_table() -> #crate_name::Raw {
                    #table_definitions
                }

                fn define_fields() -> ::std::vec::Vec<#crate_name::Raw> {
                    <Self as #crate_name::Model>::get_field_meta()
                        .into_iter()
                        .flat_map(|meta| meta.definition)
                        .collect()
                }

                // Flattened objects' fields are defined as this table's own.
                fn get_field_meta() -> ::std::vec::Vec<#crate_name::FieldMetadata> {
                    let #_____table = <Self as #crate_name::Model>::table();
                    ::std::iter::IntoIterator::into_iter([#( #field_metadata ),*])
                        #( .chain(<#flattened_objects as #crate_name::Object>::field_meta_on(#_____table.clone())) )*
                        .collect()
                }
            }

            #[doc = #patch_doc]
            #[allow(non_snake_case)]
            #vis struct #patch_ident #impl_generics #where_clause {
                #( #patch_fields, )*
                #[doc(hidden)]
                #vis #_____struct_marker_ident: #struct_marker,
            }

            impl #impl_generics ::std::default::Default for #patch_ident #ty_generics #where_clause {
                fn default() -> Self {
                    Self {
                        #( #patch_defaults, )*
                        #_____struct_marker_ident: ::std::marker::PhantomData,
                    }
                }
            }

            impl #impl_generics #crate_name::ModelPatch for #patch_ident #ty_generics #where_clause {
                type Model = #struct_name_ident #ty_generics;

                fn into_setters(self) -> ::std::vec::Vec<#crate_name::Setter> {
                    let mut setters = ::std::vec::Vec::new();
                    #( #patch_setters )*
                    setters
                }
            }
        ))
    }

    /// `<Model>Merge`: one `Merge` per stored field, for every kind of model. An object's merge is
    /// nested in the merge of a model that embeds it; a node's or edge's also implements
    /// `ModelMerge`.
    fn merge_struct_tokens(&self, code_gen: &Codegen) -> ExtractorResult<TokenStream> {
        let crate_name = get_crate_name(false);
        let struct_name_ident = &self.ident();
        let (impl_generics, ty_generics, where_clause) = self.generics().split_for_impl();
        let struct_marker = self.generics().phantom_marker_type();
        let vis = &self.vis;
        let merge_ident = struct_name_ident.merge_ident();
        let VariablesModelMacro {
            _____struct_marker_ident,
            ..
        } = VariablesModelMacro::new();
        let Codegen {
            merge_fields,
            merge_defaults,
            merge_entries,
            ..
        } = code_gen;
        let merge_doc = format!(
            "A merge into [`{}`] (`MERGE`): each field left as `Merge::Keep` is unchanged, and \
             nested objects combine with the stored ones.",
            self.ident
        );
        let model_merge = matches!(self.data_type()?, DataType::Node | DataType::Edge).then(|| {
            quote!(
                impl #impl_generics #crate_name::ModelMerge for #merge_ident #ty_generics #where_clause {
                    type Model = #struct_name_ident #ty_generics;
                }
            )
        });

        Ok(quote!(
            #[doc = #merge_doc]
            #[allow(non_snake_case)]
            #vis struct #merge_ident #impl_generics #where_clause {
                #( #merge_fields, )*
                #[doc(hidden)]
                #vis #_____struct_marker_ident: #struct_marker,
            }

            impl #impl_generics ::std::default::Default for #merge_ident #ty_generics #where_clause {
                fn default() -> Self {
                    Self {
                        #( #merge_defaults, )*
                        #_____struct_marker_ident: ::std::marker::PhantomData,
                    }
                }
            }

            impl #impl_generics #crate_name::MergeFields for #merge_ident #ty_generics #where_clause {
                fn push_entries(self, object: &mut #crate_name::MergeObject) {
                    #( #merge_entries )*
                }
            }

            #model_merge
        ))
    }

    fn node_tokens(&self, code_gen: &Codegen) -> ExtractorResult<TokenStream> {
        let crate_name = get_crate_name(false);
        let struct_name_ident = &self.ident();
        let (impl_generics, ty_generics, where_clause) = self.generics().split_for_impl();
        let table = self.table()?.expect("a node has a table").as_string();
        let explicit_generics =
            ModelAttributes::new(self)?.explicit_fully_qualified_generics_path();
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let CommonIdents {
            schema,
            table_name_checker,
            aliases_struct_name,
            ..
        } = code_gen.common_idents();
        let fields_relations_aliased = &code_gen.fields_relations_aliased;

        Ok(quote!(
            impl #impl_generics #crate_name::Node for #struct_name_ident #ty_generics #where_clause {
                type TableNameChecker = #table_name_checker;
                type Aliases = #aliases_struct_name;

                fn with(clause: impl #crate_name::IntoNodeClause<Self>) -> <Self as #crate_name::SchemaGetter>::Schema {
                    let clause: #crate_name::NodeClause = #crate_name::IntoNodeClause::<Self>::into_node_clause(clause);
                    #schema #explicit_generics ::#__________connect(
                        #crate_name::Field::default(),
                        clause.with_family(#crate_name::TableSpec::of::<Self>()).into_inner(),
                    )
                }

                fn aliases() -> Self::Aliases {
                    #aliases_struct_name::new()
                }

                fn get_table() -> #crate_name::Table {
                    #crate_name::Table::new(#table)
                }

                fn get_fields_relations_aliased() -> ::std::vec::Vec<#crate_name::Alias> {
                    ::std::vec![#( #fields_relations_aliased ), *]
                }
            }
        ))
    }

    fn edge_tokens(&self, code_gen: &Codegen) -> ExtractorResult<TokenStream> {
        let crate_name = get_crate_name(false);
        let struct_name_ident = &self.ident();
        let (impl_generics, ty_generics, where_clause) = self.generics().split_for_impl();
        let table = self.table()?.expect("an edge has a table").as_string();
        let CommonIdents {
            table_name_checker, ..
        } = code_gen.common_idents();

        let has_in_and_out_fields = code_gen
            .serialized_field_names_normalised
            .iter()
            .filter(|name| name.is_in_or_out_edge_node(&DataType::Edge))
            .count()
            == 2;
        if !has_in_and_out_fields {
            return Err(syn::Error::new_spanned(
                &self.ident,
                "an edge model needs `in` and `out` fields",
            )
            .into());
        }

        Ok(quote!(
            impl #impl_generics #crate_name::Edge for #struct_name_ident #ty_generics #where_clause {
                type In = In;
                type Out = Out;
                type TableNameChecker = #table_name_checker;

                fn get_table() -> #crate_name::Table {
                    #crate_name::Table::new(#table)
                }
            }
        ))
    }
}
