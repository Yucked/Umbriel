// ---
use super::Codegen;
use crate::models::*;

impl Codegen<'_> {
    pub fn create_field_type_static_assertion_token(&mut self) -> ExtractorResult<()> {
        let table_derive_attrs = self.table_derive_attributes();
        let field_receiver = self.field_receiver();

        let field_type_meta =
            field_receiver.field_type_db_with_static_assertions(table_derive_attrs)?;
        if let Some(meta) = field_type_meta {
            self.static_assertions.push(meta.static_assertion_token);
        }

        Ok(())
    }
}
