use std::{fmt::Display, ops::Deref};

use darling::FromMeta;
use quote::ToTokens;
use surrealdb_types::GeometryKind;
use syn::{
    self, Expr, GenericArgument, Ident, PathArguments, Token, Type, parse_quote, spanned::Spanned,
};

use crate::models::*;

use super::{custom_type_self_replacement::ReplaceSelfVisitor, *};

#[derive(Debug, Clone)]
pub struct CustomTypeNoSelf(CustomType);

impl CustomTypeNoSelf {
    pub fn new(ty: Type) -> Self {
        Self(CustomType(ty))
    }

    pub fn into_inner(self) -> CustomType {
        self.0
    }

    pub fn inner(&self) -> &CustomType {
        &self.0
    }
}

impl ToTokens for CustomTypeNoSelf {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

// takes type as path and stringified also e.g User<'a, T, u32> or "User<'a, T, u32>"
#[derive(Debug, Clone)]
pub struct CustomType(Type);

create_custom_type_wrapper!(CustomTypeInnerAngleBracket);

impl FromMeta for CustomType {
    fn from_meta(item: &syn::Meta) -> darling::Result<Self> {
        // Type::from_meta(item).map(Self)
        let ty = match item {
            syn::Meta::Path(path) => Type::Path(syn::TypePath {
                attrs: Vec::new(),
                qself: None,
                path: path.clone(),
            }),
            syn::Meta::NameValue(name_value) => match &name_value.value {
                syn::Expr::Lit(lit_str) => match lit_str.lit {
                    syn::Lit::Str(ref lit_str) => syn::parse_str::<Type>(&lit_str.value())?,
                    _ => {
                        return Err(darling::Error::custom(
                            "Unable to parse stringified type. Expected a valid Rust path or a stringified type",
                        ));
                    }
                },
                syn::Expr::Path(path) => Type::Path(syn::TypePath {
                    attrs: Vec::new(),
                    qself: None,
                    path: path.path.clone(),
                }),
                _ => {
                    return Err(darling::Error::custom(
                        "Expected a valid Rust path or a stringified type",
                    ));
                }
            },
            syn::Meta::List(_) => {
                return Err(darling::Error::unsupported_shape(
                    "Expected a path or a name-value pair",
                ));
            }
        };
        Ok(Self(ty))
    }
}

// impl Parse for CustomType {
//     // TODO: Handle type parsing if frommeta does not work or manually implement fromMeta
//     fn parse(input: ParseStream) -> syn::Result<Self> {
//         todo!()
//     }
// }

impl From<Type> for CustomType {
    fn from(ty: Type) -> Self {
        Self(ty)
    }
}

impl ToTokens for CustomType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

#[derive(Debug, Clone)]
pub struct CustomTypeTurboFished(Type);

impl ToTokens for CustomTypeTurboFished {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens)
    }
}

impl CustomType {
    pub fn new(ty: Type) -> Self {
        Self(ty)
    }

    pub fn inner(&self) -> &Type {
        &self.0
    }

    pub fn type_ident(&self) -> ExtractorResult<Ident> {
        match self.inner() {
            Type::Path(type_path) => {
                let last_segment = type_path
                    .path
                    .segments
                    .last()
                    .ok_or_else(|| darling::Error::custom("Expected a type. Make sure there are no typos and you are using a proper struct as the linked Node."))?;
                Ok(last_segment.ident.clone())
            }
            _ => {
                Err(syn::Error::new(self.to_token_stream().span(), "Expected a struct type").into())
            }
        }
    }

    // e.g Option<User> or ::std::option::Option<User> to User
    // Option<&'a str> to &'a str
    pub fn inner_angle_bracket_type(&self) -> ExtractorResult<Option<CustomTypeInnerAngleBracket>> {
        match self.inner() {
            Type::Path(type_path) => {
                let last_segment = type_path
                    .path
                    .segments
                    .last()
                    .ok_or_else(|| darling::Error::custom("Expected a type. Make sure there are no typos and you are using a proper struct as the linked Node."))?;
                if let PathArguments::AngleBracketed(angle_bracketed) = &last_segment.arguments {
                    let first_arg = angle_bracketed.args.first();
                    match first_arg {
                        Some(GenericArgument::Type(ty)) => Ok(Some(ty.clone().into())),
                        _ => Ok(None),
                    }
                } else {
                    Ok(None)
                }
            }
            Type::Array(array) => {
                let elem = array.elem.as_ref();
                Ok(Some(elem.clone().into()))
            }
            Type::Reference(r) => {
                let elem = r.elem.as_ref();
                Ok(Some(elem.clone().into()))
            }
            _ => {
                Err(syn::Error::new(self.to_token_stream().span(), "Expected a struct type").into())
            }
        }
    }

    // e.g from User<T> to User::<T>
    pub fn turbo_fishize(&self) -> ExtractorResult<CustomTypeTurboFished> {
        match self.inner() {
            Type::Path(type_path) => {
                let mut path = type_path.path.clone();

                if let Some(last) = path.segments.last_mut() {
                    let arguments = std::mem::replace(&mut last.arguments, PathArguments::None);
                    match arguments {
                        PathArguments::AngleBracketed(angle_bracketed) => {
                            let colon2_token = Some(Token![::](angle_bracketed.span()));
                            last.arguments = PathArguments::AngleBracketed(
                                syn::AngleBracketedGenericArguments {
                                    colon2_token,
                                    ..angle_bracketed
                                },
                            );
                        }
                        _ => last.arguments = arguments,
                    }
                }

                let ty = Type::Path(syn::TypePath {
                    attrs: Vec::new(),
                    qself: type_path.qself.clone(),
                    path,
                });
                Ok(CustomTypeTurboFished(ty))
            }
            _ => Err(syn::Error::new(
                self.to_token_stream().span(),
                "Unsupported type for turbofishing",
            )
            .into()),
        }
    }

    pub fn type_name(&self) -> ExtractorResult<Ident> {
        match self.inner() {
            Type::Path(type_path) => {
                let last_segment = type_path
                    .path
                    .segments
                    .last()
                    .ok_or_else(|| darling::Error::custom("Expected a type. Make sure there are no typos and you are using a proper struct as the linked Node."))?;
                Ok(last_segment.ident.clone())
            }
            _ => {
                Err(syn::Error::new(self.to_token_stream().span(), "Expected a struct type").into())
            }
        }
    }

    // This extracts generics metadata for field and from struct generics metadata.
    // This could come from the concrete rust field type or
    // as an attribute on the field from links which link to
    // other tables structs models i.e Edge, Node and Objects.
    // These are usually specified using the link_one, link_self
    // and link_many and relate attributes.
    // e.g
    // #[orm(link_one = User<'a, T, u32>)]
    // student: LinkOne<User<'a, T, u32>
    pub fn get_generics_from_current_struct(
        &self,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<CustomGenerics> {
        let custom_type = self.replace_self_with_current_struct_concrete_type(model_attributes)?;
        Ok(
            GenericTypeExtractor::sync_field_type_to_current_struct_generics(
                model_attributes,
                custom_type.inner(),
            ),
        )
    }

    // pub fn get_generics_meta<'a>(
    //     &self,
    //     model_attributes: &'a ModelAttributes,
    // ) -> FieldGenericsMeta<'a> {
    //     let binding =
    //         GenericTypeExtractor::extract_generics_for_complex_type(model_attributes, &self);
    //     let (field_impl_generics, field_ty_generics, field_where_clause) =
    //         &binding.split_for_impl();
    //     FieldGenericsMeta {
    //         field_impl_generics,
    //         field_ty_generics,
    //         field_where_clause: field_where_clause.cloned(),
    //     }
    // }

    pub fn replace_self_with_current_struct_concrete_type(
        &self,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<CustomTypeNoSelf> {
        let mut replacer = ReplaceSelfVisitor {
            struct_ident: model_attributes.ident().into_inner(),
            generics: model_attributes.generics().to_angle_bracketed(),
        };
        Ok(replacer.replace_self(self))
    }

    fn _strip_bounds_from_generics(&self) -> Self {
        let stripped_ty = match self.inner() {
            Type::Path(type_path) => {
                let mut new_type_path = type_path.clone();

                // Iterate through the path segments
                for segment in &mut new_type_path.path.segments {
                    if let PathArguments::AngleBracketed(angle_bracketed) = &mut segment.arguments {
                        // Collect only the generic identifiers, dropping bounds
                        let modified_args = angle_bracketed
                            .args
                            .iter()
                            .map(|arg| {
                                match arg {
                                    GenericArgument::Type(Type::Path(tp)) => {
                                        // Keep only the type identifier
                                        let ident = &tp
                                            .path
                                            .get_ident()
                                            .expect("Problem getting type path as ident.");
                                        parse_quote!(#ident)
                                    }
                                    GenericArgument::Lifetime(lifetime) => {
                                        // Keep only the lifetime identifier
                                        parse_quote!(#lifetime)
                                    }
                                    GenericArgument::Const(const_param) => {
                                        // Keep only the const parameter
                                        parse_quote!(#const_param)
                                    }
                                    _ => arg.clone(), // Other types of arguments are left as is
                                }
                            })
                            .collect();

                        // Replace the arguments with the modified ones
                        angle_bracketed.args = modified_args;
                    }
                }

                Type::Path(new_type_path)
            }
            _ => self.inner().clone(),
        };
        Self(stripped_ty)
    }

    pub fn is_numeric(&self) -> bool {
        let ty = &self.inner();
        match ty {
            syn::Type::Path(p) => {
                let path = &p.path;
                path.leading_colon.is_none() && path.segments.len() == 1 && {
                    let ident = &path.segments[0].ident.to_string();
                    [
                        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
                        "i128", "isize", "f32", "f64",
                    ]
                    .iter()
                    .any(|&x| x == ident)
                }
            }
            _ => false,
        }
    }

    pub fn raw_type_is_float(&self) -> bool {
        match self.inner() {
            syn::Type::Path(p) => {
                let path = &p.path;
                path.leading_colon.is_none() && path.segments.len() == 1 && {
                    let ident = &path.segments[0].ident.to_string();
                    ["f32", "f64"].iter().any(|&x| x == ident)
                }
            }
            _ => false,
        }
    }

    pub fn raw_type_is_integer(&self) -> bool {
        match self.inner() {
            syn::Type::Path(p) => {
                let path = &p.path;
                path.leading_colon.is_none() && path.segments.len() == 1 && {
                    let ident = &path.segments[0].ident.to_string();
                    [
                        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
                        "i128", "isize",
                    ]
                    .iter()
                    .any(|&x| x == ident)
                }
            }
            _ => false,
        }
    }

    pub fn raw_type_is_string(&self) -> bool {
        match &self.inner() {
            syn::Type::Path(p) => {
                let path = &p.path;
                path.leading_colon.is_none() && path.segments.len() == 1 && {
                    let ident = &path.segments[0].ident.to_string();
                    ["String", "str"].contains(&ident.as_str())
                }
            }
            syn::Type::Reference(r) => {
                if let syn::Type::Path(ref p) = *r.elem {
                    let path = &p.path;
                    path.leading_colon.is_none() && path.segments.len() == 1 && {
                        let ident = &path.segments[0].ident.to_string();
                        ["String", "str"].contains(&ident.as_str())
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn raw_type_is_bool(&self) -> bool {
        match self.inner() {
            syn::Type::Path(p) => {
                let path = &p.path;
                path.leading_colon.is_none() && path.segments.len() == 1 && {
                    let ident = &path.segments[0].ident.to_string();
                    ["bool"].iter().any(|&x| x == ident)
                }
            }
            _ => false,
        }
    }

    pub fn is_set(&self) -> bool {
        let ty = &self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::Infer(_))) = args.args.first()
                    {
                        return false;
                    }
                    last_seg.ident.to_string().to_lowercase() == "hashset"
                        || last_seg.ident.to_string().to_lowercase() == "btreeset"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn is_vec(&self) -> bool {
        let ty = &self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::Infer(_))) = args.args.first()
                    {
                        return false;
                    }
                    last_seg.ident == "Vec"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn is_array_const(&self) -> bool {
        let ty = &self.inner();
        matches!(ty, Type::Array(_type_array))
    }

    pub fn is_array(&self) -> bool {
        self.is_array_const() || self.is_vec()
    }

    // TODO: Remove this?
    // pub fn is_list(&self) -> bool {
    //     self.is_array() || self.is_set()
    // }

    pub fn raw_type_is_optional(&self) -> bool {
        let ty = self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::Infer(_))) = args.args.first()
                    {
                        return false;
                    }
                    last_seg.ident == RustType::Option.to_string()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn raw_type_is_set(&self) -> bool {
        let ty = self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::Infer(_))) = args.args.first()
                    {
                        return false;
                    }
                    last_seg.ident == RustType::HashSet.to_string()
                        || last_seg.ident == RustType::BTreeSet.to_string()
                } else {
                    false
                }
            }
            syn::Type::Array(_) => false,
            _ => false,
        }
    }

    pub fn raw_type_is_object(&self) -> bool {
        let ty = self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(syn::Type::Infer(_))) = args.args.first()
                    {
                        return false;
                    }
                    last_seg.ident == RustType::HashMap.to_string()
                        || last_seg.ident == RustType::BTreeMap.to_string()
                } else {
                    false
                }
            }
            syn::Type::Array(_) => false,
            _ => false,
        }
    }

    pub fn raw_type_is_datetime(&self) -> bool {
        let ty = self.inner();
        match ty {
            syn::Type::Path(type_path) => {
                let last_segment = type_path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                last_segment.ident.to_string().to_lowercase()
                    == RustType::DateTime.to_string().to_lowercase()
            }
            _ => false,
        }
    }

    pub fn raw_type_is_duration(&self) -> bool {
        let ty = self.inner();
        match ty {
            syn::Type::Path(type_path) => {
                let last_segment = type_path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                last_segment.ident == "Duration"
            }
            _ => false,
        }
    }

    /// The last path segment's name when it has no generic arguments, e.g. `Uuid` for
    /// `types::Uuid`; `None` for other types, so `std::ops::Range<i64>` is not `Range`.
    pub fn plain_type_ident(&self) -> Option<String> {
        let syn::Type::Path(path) = self.inner() else {
            return None;
        };
        let last_segment = path.path.segments.last()?;
        last_segment
            .arguments
            .is_none()
            .then(|| last_segment.ident.to_string())
    }

    /// The geometry kinds a geometry type holds; empty for `Geometry`, which holds any.
    pub fn raw_type_geometry_kind(&self) -> Option<Vec<GeometryKind>> {
        let ty = &self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");
                let kind = last_seg.ident.to_string().to_lowercase();

                let kind = match kind.as_str() {
                    "point" => GeometryKind::Point,
                    "linestring" => GeometryKind::Line,
                    "polygon" => GeometryKind::Polygon,
                    "multipoint" => GeometryKind::MultiPoint,
                    "multilinestring" => GeometryKind::MultiLine,
                    "multipolygon" => GeometryKind::MultiPolygon,
                    "geometrycollection" => GeometryKind::Collection,
                    "geometry" => return Some(vec![]),
                    _ => return None,
                };
                Some(vec![kind])
            }
            _ => None,
        }
    }

    fn get_type_inner_type(&self, potential_type_idents: &[RustType]) -> Option<CustomType> {
        let ty = &self.inner();

        let item_ty = match ty {
            syn::Type::Path(type_path) => {
                let last_segment = type_path
                    .path
                    .segments
                    .last()
                    .expect("Must have at least one segment");

                if !potential_type_idents
                    .iter()
                    .any(|type_ident| last_segment.ident == type_ident.to_string())
                {
                    return None;
                }
                let item_ty = match last_segment.arguments {
                    syn::PathArguments::AngleBracketed(ref args) => args.args.first(),
                    _ => None,
                };

                match item_ty {
                    Some(syn::GenericArgument::Type(ty)) => ty,
                    _ => return None,
                }
            }
            // syn:Type::Array(array) => {
            //     array.elem.as_ref()
            // },
            _ => return None,
        };

        Some(item_ty.clone().into())
    }

    pub fn get_set_inner_type(&self) -> Option<CustomType> {
        self.get_type_inner_type(&[RustType::HashSet, RustType::BTreeSet])
    }

    pub fn get_array_inner_type(&self) -> Option<CustomType> {
        self.get_vec_inner_type()
            .or_else(|| self.get_array_const_inner_type())
    }

    pub fn get_array_const_length(&self) -> Option<Expr> {
        let ty = &self.inner();
        match ty {
            syn::Type::Array(array) => Some(array.len.clone()),
            _ => None,
        }
    }

    pub fn get_vec_inner_type(&self) -> Option<CustomType> {
        self.get_type_inner_type(&[RustType::Vec])
    }

    pub fn get_array_const_inner_type(&self) -> Option<CustomType> {
        let ty = &self.inner();
        match ty {
            syn::Type::Array(array) => Some(array.elem.deref().clone().into()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum RustType {
    Vec,
    Option,
    HashSet,
    BTreeSet,
    HashMap,
    BTreeMap,
    DateTime,
}

impl Display for RustType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ty = match self {
            RustType::Vec => "Vec",
            RustType::Option => "Option",
            RustType::HashSet => "HashSet",
            RustType::BTreeSet => "BTreeSet",
            RustType::HashMap => "HashMap",
            RustType::BTreeMap => "BTreeMap",
            RustType::DateTime => "DateTime",
        };
        write!(f, "{ty}")
    }
}
