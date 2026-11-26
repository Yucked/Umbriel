use std::str::FromStr;

use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
    ToUpperCamelCase,
};
use syn::Attribute;

use super::{ExtractorResult, surreal_setting};

/// A struct's casing from its `#[surreal(rename_all = "…")]` attribute.
pub fn struct_casing(attrs: &[Attribute]) -> ExtractorResult<CaseString> {
    match surreal_setting(attrs, "rename_all")? {
        Some(casing) => CaseString::from_str(&casing.value())
            .map_err(|()| syn::Error::new(casing.span(), "unknown `rename_all` casing").into()),
        None => Ok(CaseString::None),
    }
}

/// A struct's `#[surreal(rename_all = "…")]`, with the values the SDK's derive accepts.
#[derive(Debug, Clone, Copy, Default)]
pub enum CaseString {
    #[default]
    None,
    Camel,
    Snake,
    Pascal,
    Lower,
    Upper,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl FromStr for CaseString {
    type Err = ();

    /// The names the SDK's `rename_all` takes.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "camelCase" => Self::Camel,
            "snake_case" => Self::Snake,
            "PascalCase" => Self::Pascal,
            "lowercase" => Self::Lower,
            "UPPERCASE" => Self::Upper,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnake,
            "kebab-case" => Self::Kebab,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebab,
            _ => return Err(()),
        })
    }
}

impl CaseString {
    /// The stored name of a field, cased with the same `heck` conversions as the SDK's derive, so
    /// the schema names exactly the keys `SurrealValue` writes.
    pub fn apply(self, ident: &str) -> String {
        match self {
            CaseString::None => ident.to_owned(),
            CaseString::Lower => ident.to_lowercase(),
            CaseString::Upper => ident.to_uppercase(),
            CaseString::Camel => ident.to_lower_camel_case(),
            CaseString::Snake => ident.to_snake_case(),
            CaseString::Pascal => ident.to_upper_camel_case(),
            CaseString::ScreamingSnake => ident.to_shouty_snake_case(),
            CaseString::Kebab => ident.to_kebab_case(),
            CaseString::ScreamingKebab => ident.to_shouty_kebab_case(),
        }
    }
}
