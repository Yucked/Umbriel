use crate::models::*;

use super::Codegen;

impl Codegen<'_> {
    pub fn create_relation_aliases_struct_fields_types_kv(&mut self) -> ExtractorResult<()> {
        let crate_name = get_crate_name();
        let table_derive_attributes = &self.table_derive_attributes();
        let casing = table_derive_attributes.casing()?;
        let field_receiver = self.field_receiver();
        let db_field_name = field_receiver.db_field_name(&casing)?;
        let field_ident_normalized = field_receiver.field_ident_normalized(&casing)?;

        if let RelationType::Relate(_) = field_receiver.to_relation_type(table_derive_attributes) {
            self.aliases_struct_fields_types_kv
                .push(quote!(pub #field_ident_normalized: #crate_name::AliasName, ));

            self.aliases_struct_fields_names_kv.push(
                quote!(#field_ident_normalized: #crate_name::AliasName::new(#db_field_name),),
            );
        }
        Ok(())
    }
}
