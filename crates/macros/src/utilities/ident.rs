use crate::models::*;
use darling::FromField;
use syn::{Ident, Type};

create_ident_wrapper!(FieldIdentNormalizedDeserialized);

#[derive(Clone, Debug, FromField)]
#[darling(attributes(pick), forward_attrs(surreal))]
pub struct FieldAttribute {
    /// Get the ident of the field. For fields in tuple or newtype structs or
    /// enum bodies, this can be `None`.
    pub ident: Option<Ident>,
    /// This magic field name pulls the type from the input.
    pub ty: Type,
    /// The field's `#[surreal(…)]` attributes, read for its stored name.
    pub attrs: Vec<syn::Attribute>,
}

impl FieldAttribute {
    pub fn ident(&self) -> ExtractorResult<FieldIdentOriginal> {
        Ok(self
            .ident
            .as_ref()
            .ok_or(darling::Error::custom("Field must have an identifier"))?
            .clone()
            .into())
    }

    /// The field's stored name as an identifier, raw when it is a keyword.
    pub fn field_ident_normalized_deserialized_rawable(
        &self,
        struct_casing: &CaseString,
    ) -> ExtractorResult<FieldIdentNormalizedDeserialized> {
        let field_ident_original = self.ident()?;
        let span = field_ident_original.span();
        let unraw = field_ident_original.to_string();
        let unraw = unraw.trim_start_matches("r#");
        let name = match surreal_setting(&self.attrs, "rename")? {
            Some(rename) => rename.value(),
            None => struct_casing.apply(unraw),
        };

        let ident = if RustReservedKeyword::is_keyword(&name) {
            syn::Ident::new_raw(&name, span)
        } else {
            syn::parse_str::<syn::Ident>(&name)
                .map_err(|_| syn::Error::new(span, format!("`{name}` is not a Rust identifier")))?
        };
        Ok(ident.into())
    }
}
