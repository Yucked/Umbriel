use crate::models::*;

use super::{FieldIdentNormalized, FieldWrapperIdent, MyFieldReceiver};

impl MyFieldReceiver {
    pub fn ident(&self) -> ExtractorResult<FieldIdentOriginal> {
        Ok(self
            .ident
            .as_ref()
            .ok_or(darling::Error::custom("Field must have an identifier"))?
            .clone()
            .into())
    }

    pub fn field_ident_normalized(
        &self,
        struct_casing: &CaseString,
    ) -> ExtractorResult<FieldIdentNormalized> {
        Ok(self.ident_meta(struct_casing)?.0)
    }

    pub fn db_field_name(&self, struct_casing: &CaseString) -> ExtractorResult<DbFieldName> {
        Ok(self.ident_meta(struct_casing)?.1)
    }

    /// The type of this field in the model's schema, `__<len><Model><len><field>Field`, named
    /// after the schema's field verbatim. Both names are length-prefixed, so it is distinct from
    /// every other model's and field's: case conversion would merge `foo_bar` and `fooBar`.
    pub fn field_wrapper_ident(
        &self,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<FieldWrapperIdent> {
        let field_name = self
            .field_ident_normalized(&model_attributes.casing()?)?
            .to_string();
        let field_name = field_name.trim_start_matches("r#");

        Ok(model_attributes
            .ident()
            .hidden_ident(format_args!("{}{field_name}Field", field_name.len()))
            .into())
    }

    fn ident_meta(
        &self,
        struct_casing: &CaseString,
    ) -> ExtractorResult<(FieldIdentNormalized, DbFieldName)> {
        let field_ident_original = self.ident()?;
        let span = field_ident_original.span();
        // A raw identifier is stored without its `r#`, as the SDK's derive does.
        let unraw = field_ident_original.to_string();
        let unraw = unraw.trim_start_matches("r#");
        let db_field_name = match surreal_setting(&self.attrs, "rename")? {
            Some(rename) => rename.value(),
            None => struct_casing.apply(unraw),
        };

        // The schema names a field after its stored name, e.g. `firstName` under camelCase,
        // unless that name cannot be a Rust identifier (e.g. kebab-case).
        let schema_name = if is_identifier(&db_field_name) {
            db_field_name.as_str()
        } else {
            unraw
        };
        let field_ident_normalized = if RustReservedKeyword::is_keyword(schema_name) {
            syn::Ident::new_raw(schema_name, span)
        } else {
            syn::Ident::new(schema_name, span)
        };

        Ok((field_ident_normalized.into(), db_field_name.into()))
    }
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    name != "_"
        && chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric())
}
