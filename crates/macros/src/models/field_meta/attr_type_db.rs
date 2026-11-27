use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use surrealdb_types::Kind;
use syn::spanned::Spanned;

use crate::{helpers::get_crate_name, kind::kind_tokens, models::*, surql};

create_tokenstream_wrapper!(=> FieldTypeDbToken);

impl From<&Kind> for FieldTypeDbToken {
    fn from(kind: &Kind) -> Self {
        Self(kind_tokens(kind, &get_crate_name()))
    }
}

/// A field's database type, the tokens building it, and the checks that the field's Rust type
/// can hold it.
#[derive(Debug, Clone)]
pub struct DbFieldTypeAstMeta {
    /// The type as the macro knows it. Record tables only known at runtime (another model's
    /// `table()`) are absent here and present in the tokens.
    pub field_type_db_original: Kind,
    pub field_type_db_token: FieldTypeDbToken,
    pub static_assertion_token: TokenStream,
}

impl DbFieldTypeAstMeta {
    /// A type whose tokens are exactly `kind`.
    pub fn new(kind: Kind, static_assertion_token: TokenStream) -> Self {
        Self {
            field_type_db_token: (&kind).into(),
            field_type_db_original: kind,
            static_assertion_token,
        }
    }
}

/// A field type written in `#[orm(ty = "…")]`, parsed with SurrealDB's own parser.
#[derive(Debug, Clone)]
pub struct FieldTypeDb(pub Kind);

impl FieldTypeDb {
    pub fn inner(&self) -> &Kind {
        &self.0
    }

    /// Asserts that `field_type` can hold a value of this type.
    pub fn generate_static_assertion(&self, field_type: &CustomType) -> TokenStream {
        let crate_name = get_crate_name();
        let assertion = match &self.0 {
            Kind::Any => quote!(assert_type_is_any),
            Kind::Null => quote!(assert_type_is_null),
            Kind::Uuid => quote!(assert_type_is_uuid),
            Kind::Bytes => quote!(assert_type_is_bytes),
            Kind::String => quote!(assert_type_is_string),
            Kind::Int => quote!(assert_type_is_int),
            Kind::Float => quote!(assert_type_is_float),
            Kind::Bool => quote!(assert_type_is_bool),
            Kind::Array(..) => quote!(assert_type_is_array),
            Kind::Set(..) => quote!(assert_type_is_set),
            Kind::Datetime => quote!(assert_type_is_datetime),
            Kind::Decimal => quote!(assert_type_is_decimal),
            Kind::Duration => quote!(assert_type_is_duration),
            Kind::Number => quote!(assert_type_is_number),
            Kind::Record(_) => quote!(assert_type_is_record_id),
            Kind::Geometry(_) => quote!(assert_type_is_geometry),
            // `option<T>` is `none | T`.
            Kind::Either(variants) if variants.contains(&Kind::None) => {
                quote!(assert_type_is_option)
            }
            // Objects, unions, literals and the remaining kinds have no single Rust shape to
            // check; the value conversion still has to succeed at runtime.
            Kind::Object
            | Kind::Either(_)
            | Kind::None
            | Kind::Regex
            | Kind::Table(_)
            | Kind::Function(..)
            | Kind::Range
            | Kind::Literal(_)
            | Kind::File(_) => return quote!(),
        };
        quote!(#crate_name::validators::#assertion::<#field_type>();)
    }
}

impl FromMeta for FieldTypeDb {
    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        let field_type = match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) => lit_str.value(),
            syn::Expr::Path(expr_path) => expr_path
                .path
                .get_ident()
                .map(ToString::to_string)
                .ok_or_else(|| {
                    darling::Error::custom("Expected an identifier for ty").with_span(&expr.span())
                })?,
            _ => {
                return Err(darling::Error::custom("Expected a string literal for ty")
                    .with_span(&expr.span()));
            }
        };
        surql::parse_kind(&field_type, expr.span())
            .map(Self)
            .map_err(darling::Error::from)
    }
}
