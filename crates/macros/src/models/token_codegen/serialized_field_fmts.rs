use crate::helpers::get_crate_name;
use quote::quote;

use crate::models::*;

use super::Codegen;

impl Codegen<'_> {
    pub fn create_db_fields_for_links_and_loaders(&mut self) -> ExtractorResult<()> {
        let crate_name = get_crate_name(false);
        let table_derive_attrs = self.table_derive_attributes();
        let field_receiver = self.field_receiver();
        let db_field_name = field_receiver.db_field_name(&table_derive_attrs.casing()?)?;
        let relation_type = field_receiver.to_relation_type(table_derive_attrs);

        // Relations are read through graph traversals and never stored on the record.
        if relation_type.is_relate_graph() {
            return Ok(());
        }

        self.serialized_fmt_db_field_names_instance
            .push(quote!(#db_field_name).into());
        // A computed value is never written, so `object!` does not require or take one.
        if !field_receiver.is_computed() {
            self.object_field_names.push(quote!(#db_field_name).into());
        }

        let field = || quote!(#crate_name::Field::new(#db_field_name));

        match relation_type {
            RelationType::LinkOne(target) => {
                self.link_one_fields.push(field().into());
                self.link_one_and_self_fields.push(field().into());
                self.linked_fields.push(field().into());
                self.push_link_path(&target)?;
            }
            RelationType::LinkSelf(link_self) => {
                self.link_self_fields.push(field().into());
                self.link_one_and_self_fields.push(field().into());
                self.linked_fields.push(field().into());
                self.push_link_path(&link_self.to_linkone_attr_type(table_derive_attrs)?)?;
            }
            RelationType::LinkMany(target) => {
                self.link_many_fields.push(field().into());
                self.linked_fields.push(field().into());
                self.push_link_path(&target)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// The field's typed path in `<Model>Links`: named after the Rust field, with the field's own
    /// visibility, rendering the stored name.
    fn push_link_path(&mut self, target: &impl quote::ToTokens) -> ExtractorResult<()> {
        let crate_name = get_crate_name(false);
        let model_attributes = self.table_derive_attributes();
        let model = model_attributes.ident();
        let (_, ty_generics, _) = model_attributes.generics().split_for_impl();
        let field_receiver = self.field_receiver();
        let ident = field_receiver.ident()?;
        let vis = &field_receiver.vis;
        let db_field_name = field_receiver.db_field_name(&model_attributes.casing()?)?;

        let name: &syn::Ident = &ident;
        let doc = format!("The path over `{name}`, stored as `{db_field_name}`.");
        self.link_path_fields.push(quote!(
            #[doc = #doc]
            #vis #ident: #crate_name::LinkPath<#model #ty_generics, #target>
        ));
        self.link_path_inits
            .push(quote!(#ident: #crate_name::__private::link_path(#db_field_name)));
        Ok(())
    }
}
