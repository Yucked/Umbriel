use super::ident::{FieldAttribute, FieldIdentNormalizedDeserialized};
use crate::models::{ExtractorResult, StructGenerics, CaseString, struct_casing};

use darling::{FromDeriveInput, ast::Data, util};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Ident, Type};

#[derive(Clone, Debug, FromDeriveInput)]
#[darling(attributes(pick), forward_attrs(surreal))]
pub struct TableDeriveAttributesPickable {
    pub(crate) ident: Ident,
    /// The struct's `#[surreal(…)]` attributes, read for `rename_all`.
    pub(crate) attrs: Vec<syn::Attribute>,
    pub(crate) generics: StructGenerics,
    /// Receives the body of the struct or enum. We don't care about
    /// struct fields because we previously told darling we only accept structs.
    pub data: Data<util::Ignored, FieldAttribute>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PickableMetadata<'a> {
    pub(crate) field_name_normalized_deserialized: Vec<FieldIdentNormalizedDeserialized>,
    pub(crate) field_type: Vec<&'a Type>,
}

impl TableDeriveAttributesPickable {
    pub fn casing(&self) -> ExtractorResult<CaseString> {
        struct_casing(&self.attrs)
    }

    pub(crate) fn get_meta(&self) -> ExtractorResult<PickableMetadata<'_>> {
        let fields =
            self.data.as_ref().take_struct().ok_or(
                darling::Error::custom("Only structs are supported").with_span(&self.ident),
            )?;
        let struct_casing_de = self.casing()?;

        let mut meta = PickableMetadata::default();

        for field_attr in fields {
            let f = field_attr.field_ident_normalized_deserialized_rawable(&struct_casing_de)?;
            meta.field_name_normalized_deserialized.push(f);
            meta.field_type.push(&field_attr.ty);
        }

        Ok(meta)
    }
}

impl ToTokens for TableDeriveAttributesPickable {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table_derive_attributes = self;
        let struct_name_ident = &table_derive_attributes.ident;
        let (struct_impl_generics, struct_ty_generics, struct_where_clause) =
            &table_derive_attributes.generics.split_for_impl();
        let meta = match table_derive_attributes.get_meta() {
            Ok(meta) => meta,
            Err(err) => return tokens.extend(err.write_errors()),
        };
        let PickableMetadata {
            field_name_normalized_deserialized,
            field_type,
        } = meta;

        let pickable_name = format_ident!("{struct_name_ident}Pickable");
        tokens.extend(quote!(
            #[allow(non_camel_case_types, unused)]
            pub trait #pickable_name {
                #(
                    type #field_name_normalized_deserialized ;
                ) *
            }

            impl #struct_impl_generics #pickable_name for #struct_name_ident #struct_ty_generics #struct_where_clause {
                #(
                    type #field_name_normalized_deserialized = #field_type ;
                ) *
            }

        ));
    }
}
