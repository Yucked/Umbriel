use heck::ToSnakeCase;
use syn::ext::IdentExt;

use crate::models::*;

create_ident_wrapper!(TableNameIdent);

impl TableNameIdent {
    pub(crate) fn validate_and_return(
        &self,
        struct_name_ident: &StructIdent,
        relax_table: &Option<bool>,
    ) -> ExtractorResult<&Self> {
        let table = self;
        let expected_table = struct_name_ident.unraw().to_string().to_snake_case();
        if !relax_table.unwrap_or(false) && self.unraw() != expected_table {
            return Err(syn::Error::new(
                table.span(),
                format!(
                    "the table must be the struct name in snake case, `{expected_table}`; \
                     set `#[orm(relax_table)]` to name it freely"
                ),
            )
            .into());
        };

        Ok(self)
    }

    /// The table's name as stored: a keyword table such as `type` is `r#type` as an identifier.
    pub(crate) fn as_string(&self) -> TableNameAsString {
        TableNameAsString(self.unraw().to_string())
    }
}

pub struct TableNameAsString(String);

impl ToTokens for TableNameAsString {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table = self.0.to_string();
        let table = quote!(#table);
        table.to_tokens(tokens);
    }
}
