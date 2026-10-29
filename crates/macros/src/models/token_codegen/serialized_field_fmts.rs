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

        let field = || quote!(#crate_name::Field::new(#db_field_name));

        match relation_type {
            RelationType::LinkOne(_) => {
                self.link_one_fields.push(field().into());
                self.link_one_and_self_fields.push(field().into());
                self.linked_fields.push(field().into());
            }
            RelationType::LinkSelf(_) => {
                self.link_self_fields.push(field().into());
                self.link_one_and_self_fields.push(field().into());
                self.linked_fields.push(field().into());
            }
            RelationType::LinkMany(_) => {
                self.link_many_fields.push(field().into());
                self.linked_fields.push(field().into());
            }
            _ => {}
        }
        Ok(())
    }
}
