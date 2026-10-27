use quote::{ToTokens, quote};
use surrealdb_types::{Kind, Table};
use syn::{self, Expr, spanned::Spanned};

use crate::helpers::get_crate_name;
use crate::models::*;

use super::{field_name_serialized::DbFieldName, *};

pub struct FieldTypeInference<'a> {
    pub db_field_name: &'a DbFieldName,
    pub field_ty: &'a CustomType,
    pub relation_type: &'a RelationType,
    pub model_attrs: &'a ModelAttributes<'a>,
}

impl<'a> FieldTypeInference<'a> {
    /// The stored type of a field without `ty = "…"`; an error when it cannot be inferred, so a
    /// definition is never emitted without a type.
    pub fn infer_type(&self) -> ExtractorResult<Option<DbFieldTypeAstMeta>> {
        match self.infer_type_by_priority()? {
            Some(meta) => Ok(Some(meta)),
            None => Err(syn::Error::new(
                self.field_ty.span(),
                "could not infer the field's database type; specify it, e.g. `#[orm(ty = \"array<string>\")]`",
            )
            .into()),
        }
    }

    pub fn infer_type_by_priority(&self) -> ExtractorResult<Option<DbFieldTypeAstMeta>> {
        let relation_type = self.relation_type;
        let model_attrs = self.model_attrs;
        let ty = self.field_ty;
        let field_name = self.db_field_name;

        // NOTE: The order of the inference approach is important. Type path should always be first
        let priority1 = || self.based_on_type_path_token_structure(ty, relation_type.clone());
        let priority2 = || self.based_on_db_field_name(ty, field_name, model_attrs);
        let priority3 = || self.based_on_field_link_type(ty, relation_type);

        let db_type = if let Ok(db_ty) = priority1() {
            Some(db_ty)
        } else if let Ok(db_ty) = priority2() {
            Some(db_ty)
        } else if let Ok(Some(db_ty)) = priority3() {
            Some(db_ty)
        } else {
            None
        };

        Ok(db_type)
    }

    /// This is done directly by morphologically analysing the field type path tokenstream
    /// using certain heuristics and is cross-referenced with other stuff in case our guess
    /// is wrong. Users can usually specify the exact db type as a field attribute for explicitness
    fn based_on_type_path_token_structure(
        &self,
        field_ty: &CustomType,
        relation_type: RelationType,
    ) -> ExtractorResult<DbFieldTypeAstMeta> {
        let crate_name = get_crate_name(false);
        let binding = field_ty.replace_self_with_current_struct_concrete_type(self.model_attrs)?;
        let ty = &binding.inner();
        let assert = |assertion: proc_macro2::TokenStream| quote!(#crate_name::validators::#assertion::<#ty>(););

        let meta = if field_ty.raw_type_is_bool() {
            DbFieldTypeAstMeta::new(Kind::Bool, assert(quote!(assert_type_is_bool)))
        } else if field_ty.raw_type_is_float() {
            DbFieldTypeAstMeta::new(Kind::Float, assert(quote!(assert_type_is_float)))
        } else if field_ty.raw_type_is_integer() {
            DbFieldTypeAstMeta::new(Kind::Int, assert(quote!(assert_type_is_int)))
        } else if field_ty.raw_type_is_string() {
            DbFieldTypeAstMeta::new(Kind::String, assert(quote!(assert_type_is_string)))
        } else if field_ty.raw_type_is_optional() {
            let item = field_ty
                .inner_angle_bracket_type()?
                .map(|item| self.based_on_type_path_token_structure(&item, relation_type))
                .ok_or(syn::Error::new(
                    ty.span(),
                    "Could not infer type for the optional field",
                ))??;

            let item_token = item.field_type_db_token;
            let item_static_assertion = item.static_assertion_token;
            let option_assertion = assert(quote!(assert_type_is_option));

            DbFieldTypeAstMeta {
                field_type_db_original: Kind::option(item.field_type_db_original),
                field_type_db_token: quote!(#crate_name::types::Kind::option(#item_token)).into(),
                static_assertion_token: quote!(
                    #option_assertion
                    #item_static_assertion
                )
                .into(),
            }
        } else if field_ty.type_ident().is_ok_and(|ident| ident == "Nullable") {
            // `Nullable<T>` holds `NULL` or a value: `null | T`.
            let item = field_ty
                .inner_angle_bracket_type()?
                .map(|item| self.based_on_type_path_token_structure(&item, relation_type))
                .ok_or(syn::Error::new(
                    ty.span(),
                    "Could not infer type for the nullable field",
                ))??;

            let item_token = item.field_type_db_token;
            let item_static_assertion = item.static_assertion_token;
            let null_assertion = assert(quote!(assert_type_is_null));

            DbFieldTypeAstMeta {
                field_type_db_original: Kind::either(vec![Kind::Null, item.field_type_db_original]),
                field_type_db_token: quote!(#crate_name::types::Kind::either(
                    ::std::vec![#crate_name::types::Kind::Null, #item_token]
                ))
                .into(),
                static_assertion_token: quote!(
                    #null_assertion
                    #item_static_assertion
                )
                .into(),
            }
        } else if field_ty.is_array() {
            let array_length = field_ty
                .get_array_const_length()
                .filter(|length| !length.to_token_stream().is_empty());
            let array_length_value = array_length.as_ref().and_then(|length| match length {
                Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(int),
                    ..
                }) => int.base10_parse::<u64>().ok(),
                _ => None,
            });
            // The length may be a const generic or other constant expression only known when
            // the generated code compiles.
            let array_length_token = match &array_length {
                Some(length) => quote!(::std::option::Option::Some((#length) as u64)),
                None => quote!(::std::option::Option::None),
            };

            let item = field_ty
                .inner_angle_bracket_type()?
                .map(|item| self.based_on_type_path_token_structure(&item, relation_type))
                .ok_or(syn::Error::new(
                    ty.span(),
                    "Could not infer type for the array inner type",
                ))??;

            let item_token = item.field_type_db_token;
            let item_static_assertion = item.static_assertion_token;
            let array_assertion = assert(quote!(assert_type_is_array));
            DbFieldTypeAstMeta {
                field_type_db_original: Kind::Array(
                    Box::new(item.field_type_db_original),
                    array_length_value,
                ),
                field_type_db_token: quote!(#crate_name::types::Kind::Array(
                    ::std::boxed::Box::new(#item_token),
                    #array_length_token
                ))
                .into(),
                static_assertion_token: quote!(
                    #array_assertion
                    #item_static_assertion
                )
                .into(),
            }
        } else if field_ty.raw_type_is_set() {
            let item = field_ty
                .inner_angle_bracket_type()?
                .map(|item| self.based_on_type_path_token_structure(&item, relation_type))
                .ok_or(syn::Error::new(
                    ty.span(),
                    "Could not infer type for the set inner type",
                ))??;
            let item_token = item.field_type_db_token;
            let item_static_assertion = item.static_assertion_token;
            let set_assertion = assert(quote!(assert_type_is_set));

            // The SDK writes and reads `HashSet`/`BTreeSet` as arrays (`kind_of` is `array<T>`);
            // a `set<T>` field would store a set value they cannot decode.
            DbFieldTypeAstMeta {
                field_type_db_original: Kind::Array(Box::new(item.field_type_db_original), None),
                field_type_db_token: quote!(#crate_name::types::Kind::Array(
                    ::std::boxed::Box::new(#item_token),
                    ::std::option::Option::None
                ))
                .into(),
                static_assertion_token: quote!(
                    #set_assertion
                    #item_static_assertion
                )
                .into(),
            }
        } else if field_ty.raw_type_is_object() {
            // A map has no marker to check beyond converting to and from a value.
            DbFieldTypeAstMeta::new(Kind::Object, assert(quote!(assert_type_is_any)))
        } else if field_ty.raw_type_is_duration() {
            DbFieldTypeAstMeta::new(Kind::Duration, assert(quote!(assert_type_is_duration)))
        } else if field_ty.raw_type_is_datetime() {
            DbFieldTypeAstMeta::new(Kind::Datetime, assert(quote!(assert_type_is_datetime)))
        } else if let Some(geometry_kinds) = field_ty.raw_type_geometry_kind() {
            DbFieldTypeAstMeta::new(
                Kind::Geometry(geometry_kinds),
                assert(quote!(assert_type_is_geometry)),
            )
        } else if let Some(foreign_type) = self.refereces_a_nested_object(&relation_type)? {
            // Guess if its a foreign Table type by comparing the type path segment with the foreign
            // rust field type ident
            let current_segment_is_likely_foreign_type =
                field_ty.type_ident()? == foreign_type.type_ident()?;
            if current_segment_is_likely_foreign_type {
                DbFieldTypeAstMeta::new(Kind::Object, assert(quote!(assert_type_is_object)))
            } else {
                return Err(syn::Error::new(
                    ty.span(),
                    "Could not infer the database type for the field based on the field type in rust provided. Specify by using e.g ty = \"array\"",
                ).into());
            }
        } else {
            return Err(syn::Error::new(
                ty.span(),
                "Could not infer the database type for the field based on the field type in rust provided. Specify by using e.g ty = \"array\"",
            )
            .into());
        };

        Ok(meta)
    }

    /// Gets out the foreign/nested object type if the field references a foreign struct
    fn refereces_a_nested_object(
        &self,
        relation_type: &'a RelationType,
    ) -> ExtractorResult<Option<&'a CustomType>> {
        // NOTE 1:
        // Links are excluded from this because we are handling them separately in the inference
        // based on link type.
        // Nested objects are inluded here when we infer based on the field type path because
        // nested/embedded objects are just normal rust struct types with no relation nor special treatment
        // within the context of the database. But we are able to infer that because of the
        // nest_array/ nest_object field attributes we provide
        //
        // NOTE 2:
        // We are mainly interested in getting the foreign struct itself whether or not its a
        // single top-level struct, or one or deep lelve nested struct.
        let foreign_type = match relation_type {
            RelationType::NestObject(ref_object) => Some(&**ref_object),
            RelationType::NestArray(foreign_array_object) => Some(&**foreign_array_object),
            RelationType::Relate(_)
            | RelationType::List(_)
            | RelationType::None
            | RelationType::LinkOne(_)
            | RelationType::LinkSelf(_)
            | RelationType::LinkManyInAndOutEdgeNodesInert(_)
            | RelationType::LinkMany(_) => None,
        };

        Ok(foreign_type)
    }

    /// based on field metadata such as reserved field names e.g `id`, `in` and `out`
    /// With these metadata, we can usually reliably derive/guess the field type as a whole
    fn based_on_db_field_name(
        &self,
        field_ty: &CustomType,
        db_field_name: &DbFieldName,
        model_attrs: &ModelAttributes,
    ) -> ExtractorResult<DbFieldTypeAstMeta> {
        let crate_name = get_crate_name(false);
        let model_type = model_attrs.to_data_type();
        let ty = &field_ty.inner();

        let meta = if db_field_name.is_id() {
            let tables = model_attrs
                .table()?
                .map(|table| vec![Table::new(table.to_string())])
                .unwrap_or_default();
            DbFieldTypeAstMeta::new(
                Kind::Record(tables),
                quote!(#crate_name::validators::assert_type_is_record_id::<#ty>();),
            )
        } else if db_field_name.is_in_or_out_edge_node(&model_type) {
            // An edge might be shared by multiple In/Out nodes. So, default to any type of
            // record for edge in and out e.g Student->Plays->Football, Student->Plays->Instrument,
            // Teacher->Plays->Football
            DbFieldTypeAstMeta::new(
                Kind::Record(vec![]),
                quote!(#crate_name::validators::assert_type_is_link_one::<#ty>();),
            )
        } else {
            return Err(syn::Error::new(
                ty.span(),
                format!("Could not infer type for the field - {db_field_name}. Specify by using e.g ty = \"record<any>\"")
            )
            .into());
        };
        Ok(meta)
    }

    /// This is usually shallow and just infers the type as a whole
    /// based on field attributes such as relation type e.g link_one, link_fieldm link_many etc
    /// With these metadata, we can usually reliably derive/guess the field type as a whole
    fn based_on_field_link_type(
        &self,
        field_ty: &CustomType,
        relation_type: &RelationType,
    ) -> ExtractorResult<Option<DbFieldTypeAstMeta>> {
        let crate_name = get_crate_name(false);
        let ty = &field_ty.inner();
        // The linked model's table is only known once the generated code compiles, so the
        // macro's own copy of the type leaves the record's tables empty. The id's own kind is
        // `record<table>`, or `record` for a model with a `table_prefix`.
        let record_of = |model: proc_macro2::TokenStream| quote!(<#crate_name::RecordId<#model> as #crate_name::types::SurrealValue>::kind_of());
        // An unset single link is written as NONE, so its stored type must allow it.
        let optional_record_of = |model: proc_macro2::TokenStream| {
            let record = record_of(model);
            quote!(#crate_name::types::Kind::option(#record))
        };

        let meta = match relation_type {
            RelationType::Relate(_ref_node) => {
                // Relations are readonly and are not stored on nodes, but
                // on edges. Just used on nodes for convenience
                // during deserialization
                return Ok(None);
            }

            RelationType::LinkOne(ref_node) => {
                let ref_node = ref_node.turbo_fishize()?;

                DbFieldTypeAstMeta {
                    field_type_db_original: Kind::option(Kind::Record(vec![])),
                    field_type_db_token: optional_record_of(quote!(#ref_node)).into(),
                    static_assertion_token: quote!(#crate_name::validators::assert_type_eq_all!(#field_ty, #crate_name::LinkOne<#ref_node>);).into(),
                }
            }
            RelationType::LinkSelf(self_node) => {
                let current_struct_type = self.model_attrs.struct_no_bounds()?;
                let self_node = self_node.turbo_fishize()?;

                DbFieldTypeAstMeta {
                    field_type_db_original: Kind::option(Kind::Record(vec![])),
                    field_type_db_token: optional_record_of(quote!(Self)).into(),
                    static_assertion_token: quote!(
                        #crate_name::validators::assert_type_eq_all!(#crate_name::LinkSelf<#current_struct_type>, #crate_name::LinkSelf<#self_node>);
                        #crate_name::validators::assert_type_eq_all!(#field_ty, #crate_name::LinkSelf<#self_node>);
                    )
                    .into(),
                }
            }
            RelationType::LinkMany(ref_node)
            | RelationType::LinkManyInAndOutEdgeNodesInert(ref_node) => {
                let ref_node = ref_node.turbo_fishize()?;
                let record = record_of(quote!(#ref_node));

                DbFieldTypeAstMeta {
                    field_type_db_original: Kind::Array(Box::new(Kind::Record(vec![])), None),
                    field_type_db_token: quote!(#crate_name::types::Kind::Array(
                        ::std::boxed::Box::new(#record),
                        ::std::option::Option::None
                    ))
                    .into(),
                    static_assertion_token: quote!(
                        #crate_name::validators::assert_type_eq_all!(#field_ty, #crate_name::LinkMany<#ref_node>);
                    )
                    .into(),
                }
            }
            // NOTE: Nested objects and nested arrays of objects are handled in the type path token
            RelationType::NestArray(_)
            | RelationType::NestObject(_)
            | RelationType::List(_)
            | RelationType::None => {
                return Err(syn::Error::new(
                    ty.span(),
                    "Could not infer type for the field. Specify it explicitly, e.g. `ty = \"array<string>\"`",
                )
                .into());
            }
        };

        Ok(Some(meta))
    }
}
