use crate::helpers::get_crate_name;
use quote::{ToTokens, quote};

use crate::models::*;

use super::Codegen;

impl Codegen<'_> {
    /// One `Patch<T>` field of `<Model>Patch` per stored field, and the stored names `object!`
    /// checks keys against.
    pub fn create_patch_metadata(&mut self) -> ExtractorResult<()> {
        let crate_name = get_crate_name(false);
        let model_attributes = self.table_derive_attributes();
        let field_receiver = self.field_receiver();
        let casing = model_attributes.casing()?;

        if field_receiver
            .to_relation_type(model_attributes)
            .is_relate_graph()
        {
            return Ok(());
        }

        let field_ident_normalized = field_receiver.field_ident_normalized(&casing)?;
        self.renamed_serialized_fields_kv
            .push(quote!(pub #field_ident_normalized: &'static str).into());

        let db_field_name = field_receiver.db_field_name(&casing)?;
        // A record's id is its identity, not a value to update.
        if db_field_name.is_id() {
            return Ok(());
        }

        let vis = &model_attributes.receiver().vis;
        let field_ident = field_receiver.ident()?;
        let field_type = field_receiver
            .ty()
            .replace_self_with_current_struct_concrete_type(model_attributes)?;

        self.patch_fields
            .push(quote!(#vis #field_ident: #crate_name::Patch<#field_type>).into());
        self.patch_defaults
            .push(quote!(#field_ident: #crate_name::Patch::Keep).into());
        self.patch_setters.push(
            quote!(
                setters.extend(self.#field_ident.into_setter(#crate_name::Field::ident(#db_field_name)));
            )
            .into(),
        );

        // An embedded object merges field by field through its own `<Object>Merge`; another value
        // through the `MergeFields` type its `merge = "…"` names.
        let merge_type = match (
            field_receiver.to_relation_type(model_attributes),
            &field_receiver.merge,
        ) {
            (RelationType::NestObject(_), Some(merge)) => {
                return Err(syn::Error::new_spanned(
                    merge.to_token_stream(),
                    "`merge` names the merge of a value that is not a `#[model(object)]`; a \
                     `nest_object` field already merges through the object's own merge",
                )
                .into());
            }
            (RelationType::NestObject(nest_object), None) => quote!(
                #crate_name::Merge<#field_type, <#nest_object as #crate_name::Object>::Merge>
            ),
            (_, Some(merge)) => quote!(#crate_name::Merge<#field_type, #merge>),
            (_, None) => quote!(#crate_name::Merge<#field_type>),
        };
        self.merge_fields
            .push(quote!(#vis #field_ident: #merge_type));
        self.merge_defaults
            .push(quote!(#field_ident: #crate_name::Merge::Keep));
        self.merge_entries
            .push(quote!(self.#field_ident.push_entry(#db_field_name, object);));

        Ok(())
    }
}
