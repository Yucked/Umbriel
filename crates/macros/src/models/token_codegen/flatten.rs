use crate::helpers::get_crate_name;
use quote::quote;

use crate::models::*;

use super::Codegen;

impl Codegen<'_> {
    /// A `#[surreal(flatten)]` field: its object's fields are stored as this model's own. The
    /// object's schema continues at this model's path, its definitions and stored names are this
    /// model's, and the model's patch assigns each of its fields. The object must be a
    /// `#[model(object)]`, whose stored names are known statically; maps and other shapes are
    /// rejected because they are not `Object`s.
    pub fn create_flattened_field(&mut self, flatten: &syn::Path) -> ExtractorResult<()> {
        let crate_name = get_crate_name(false);
        let model_attributes = self.table_derive_attributes();
        let field_receiver = self.field_receiver();
        let MyFieldReceiver {
            array_item_ty_specified,
            old_name,
            relate,
            link_one,
            link_self,
            link_many,
            nest_array,
            nest_object,
            field_type_db,
            assert,
            define,
            value,
            permissions,
            item_assert,
            ..
        } = field_receiver;
        let has_settings = array_item_ty_specified.is_some()
            || old_name.is_some()
            || relate.is_some()
            || link_one.is_some()
            || link_self.is_some()
            || link_many.is_some()
            || nest_array.is_some()
            || nest_object.is_some()
            || field_type_db.is_some()
            || assert.is_some()
            || define.is_some()
            || value.is_some()
            || permissions.is_some()
            || item_assert.is_some();
        if has_settings {
            return Err(syn::Error::new_spanned(
                flatten,
                "a flattened field takes no `#[orm(..)]` settings: the object's own fields define \
                 how they are stored",
            )
            .into());
        }

        let VariablesModelMacro {
            ___________path, ..
        } = VariablesModelMacro::new();
        let ident = field_receiver.ident()?;
        // The stored names are checked by a constant that cannot name the model's generic
        // parameters, so the object's type must not depend on them; its fields would not be
        // known when the model is compiled anyway.
        if !field_receiver
            .ty()
            .get_generics_from_current_struct(model_attributes)?
            .params()
            .is_empty()
        {
            return Err(syn::Error::new_spanned(
                &field_receiver.ty,
                "a flattened object's type cannot depend on the model's generic parameters: its \
                 stored fields must be known when the model is compiled",
            )
            .into());
        }
        let ty = field_receiver
            .ty()
            .replace_self_with_current_struct_concrete_type(model_attributes)?;

        self.static_assertions.push(
            quote!(#crate_name::validators::assert_impl_one!(#ty: #crate_name::Object);).into(),
        );

        // The object's schema starts at this model's path: its fields are this model's.
        self.schema_struct_fields_types_kv
            .push(quote!(pub #ident: <#ty as #crate_name::SchemaGetter>::Schema,).into());
        self.schema_struct_fields_names_kv.push(
            quote!(#ident: <#ty as #crate_name::SchemaGetter>::schema_prefixed(#___________path.clone()),)
                .into(),
        );
        self.schema_field_idents.push(quote!(#ident));

        self.flattened_objects.push(quote!(#ty));
        self.flattened_field_keys.push(quote!(stringify!(#ident)));
        self.renamed_serialized_fields_kv
            .push(quote!(pub #ident: &'static str).into());

        let vis = &model_attributes.receiver().vis;
        self.patch_fields
            .push(quote!(#vis #ident: #crate_name::FlatPatch<#ty>).into());
        self.patch_defaults
            .push(quote!(#ident: #crate_name::FlatPatch::Keep).into());
        self.patch_setters
            .push(quote!(setters.extend(self.#ident.into_setters());).into());
        self.merge_fields.push(quote!(
            #vis #ident: #crate_name::FlatMerge<#ty, <#ty as #crate_name::Object>::Merge>
        ));
        self.merge_defaults
            .push(quote!(#ident: #crate_name::FlatMerge::Keep));
        self.merge_entries
            .push(quote!(self.#ident.push_entries(object);));

        Ok(())
    }
}
