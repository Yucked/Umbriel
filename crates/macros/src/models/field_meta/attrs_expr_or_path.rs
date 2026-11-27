use crate::helpers::get_crate_name;
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use surrealdb_types::Kind;
use syn::{Expr, Lit, Meta, MetaNameValue};

#[derive(Clone, Debug)]
pub enum ExprOrPath {
    Expr(Expr),
    Path(syn::Path),
}

impl FromMeta for ExprOrPath {
    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Lit(lit) => Self::from_value(&lit.lit),
            _ => Ok(ExprOrPath::Expr(expr.clone())),
        }
    }

    fn from_meta(item: &syn::Meta) -> darling::Result<Self> {
        match item {
            Meta::Path(path) => Ok(ExprOrPath::Path(path.clone())),
            Meta::NameValue(MetaNameValue { value, .. }) => match value {
                Expr::Path(expr_path) => Ok(ExprOrPath::Path(expr_path.path.clone())),
                Expr::Lit(lit) => Self::from_value(&lit.lit),
                _ => Ok(ExprOrPath::Expr(value.clone())),
            },
            Meta::List(_) => Err(darling::Error::unsupported_shape(
                "Expected a path or a name-value pair",
            )),
        }
    }

    fn from_value(value: &Lit) -> darling::Result<Self> {
        match value {
            Lit::Byte(_)
            | Lit::Char(_)
            | Lit::Int(_)
            | Lit::Float(_)
            | Lit::Bool(_)
            | Lit::Str(_) => Ok(ExprOrPath::Expr(syn::parse2(quote!(#value))?)),
            _ => Err(darling::Error::unexpected_lit_type(value)),
        }
    }
}

impl ToTokens for ExprOrPath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ExprOrPath::Expr(expr) => expr.to_tokens(tokens),
            ExprOrPath::Path(path) => {
                quote!(#path()).to_tokens(tokens);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct AttributeValue(ExprOrPath);
#[derive(Clone, Debug)]
pub struct AttributeAssert(ExprOrPath);
#[derive(Clone, Debug)]
pub struct AttributeItemAssert(ExprOrPath);
#[derive(Clone, Debug)]
pub struct AttributeAs(ExprOrPath);
#[derive(Clone, Debug)]
pub struct AttributeDefine(ExprOrPath);

macro_rules! impl_from_expr_or_path {
    ($ty:ty) => {
        impl FromMeta for $ty {
            fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
                Ok(Self(ExprOrPath::Expr(expr.clone())))
            }

            fn from_meta(item: &syn::Meta) -> darling::Result<Self> {
                Ok(Self(ExprOrPath::from_meta(item)?))
            }

            fn from_value(value: &Lit) -> darling::Result<Self> {
                Ok(Self(ExprOrPath::from_value(value)?))
            }
        }

        impl ToTokens for $ty {
            fn to_tokens(&self, tokens: &mut TokenStream) {
                self.0.to_tokens(tokens)
            }
        }
    };
}

impl_from_expr_or_path!(AttributeValue);
impl_from_expr_or_path!(AttributeAssert);
impl_from_expr_or_path!(AttributeItemAssert);
impl_from_expr_or_path!(AttributeAs);
impl_from_expr_or_path!(AttributeDefine);

impl AttributeValue {
    /// Asserts that the `value` expression can hold a value of the field's database type.
    pub fn get_default_value_static_assertion(&self, db_field_type: &Kind) -> TokenStream {
        let crate_name = get_crate_name();
        let assertion = match db_field_type {
            Kind::Bytes => quote!(assert_value_is_bytes),
            Kind::Null => quote!(assert_value_is_null),
            Kind::Uuid => quote!(assert_value_is_uuid),
            Kind::Duration => quote!(assert_value_is_duration),
            Kind::String => quote!(assert_value_is_string),
            Kind::Int => quote!(assert_value_is_int),
            Kind::Float => quote!(assert_value_is_float),
            Kind::Bool => quote!(assert_value_is_bool),
            Kind::Array(..) => quote!(assert_value_is_array),
            Kind::Set(..) => quote!(assert_value_is_set),
            Kind::Datetime => quote!(assert_value_is_datetime),
            Kind::Decimal | Kind::Number => quote!(assert_value_is_number),
            Kind::Record(_) => quote!(assert_value_is_record_id),
            Kind::Geometry(_) => quote!(assert_value_is_geometry),
            Kind::Any => quote!(assert_value_is_any),
            // `option<T>` is `none | T`.
            Kind::Either(variants) if variants.contains(&Kind::None) => {
                quote!(assert_value_is_option)
            }
            Kind::Object
            | Kind::Either(_)
            | Kind::None
            | Kind::Regex
            | Kind::Table(_)
            | Kind::Function(..)
            | Kind::Range
            | Kind::Literal(_)
            | Kind::File(_) => return TokenStream::new(),
        };

        quote!(let _ = #crate_name::validators::#assertion(#self);)
    }
}
