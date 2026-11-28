use std::{fmt::Display, ops::Deref};

use darling::FromMeta;
use quote::ToTokens;
use surrealdb_types::GeometryKind;
use syn::{
    self, Expr, GenericArgument, Ident, PathArguments, Token, Type, spanned::Spanned,
};

use crate::models::*;

use super::{custom_type_self_replacement::ReplaceSelfVisitor, *};

/// The name a type path gives a standard type: the last segment of a bare name (`String`) or of
/// the standard-library path the prelude takes it from (`std::string::String`,
/// `::core::primitive::u8`), so a qualified type infers as the bare one does (UMB-034). `None`
/// for any other path, or one with generic arguments before its last segment.
fn std_type_name(path: &syn::Path) -> Option<String> {
    const PREFIXES: [&[&str]; 5] = [
        &["std", "string"],
        &["alloc", "string"],
        &["std", "primitive"],
        &["core", "primitive"],
        &["alloc", "primitive"],
    ];
    let segments = path.segments.iter().collect::<Vec<_>>();
    let (last, prefix) = segments.split_last()?;
    if prefix.iter().any(|segment| !segment.arguments.is_none()) {
        return None;
    }
    let prefix = prefix
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let bare = prefix.is_empty() && path.leading_colon.is_none();
    let standard = PREFIXES.iter().any(|known| *known == prefix.as_slice());
    (bare || standard).then(|| last.ident.to_string())
}

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
        const INTEGERS: [&str; 12] = [
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        ];
        match self.inner() {
            syn::Type::Path(p) => std_type_name(&p.path).is_some_and(|name| INTEGERS.contains(&name.as_str())),
            _ => false,
        }
    }

    pub fn raw_type_is_string(&self) -> bool {
        let is_string = |path: &syn::Path| {
            std_type_name(path).is_some_and(|name| ["String", "str"].contains(&name.as_str()))
        };
        match &self.inner() {
            syn::Type::Path(p) => is_string(&p.path),
            syn::Type::Reference(r) => matches!(&*r.elem, syn::Type::Path(p) if is_string(&p.path)),
            _ => false,
        }
    }

    pub fn raw_type_is_bool(&self) -> bool {
        match self.inner() {
            syn::Type::Path(p) => std_type_name(&p.path).is_some_and(|name| name == "bool"),
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
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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

    pub fn raw_type_is_optional(&self) -> bool {
        let ty = self.inner();
        match ty {
            syn::Type::Path(path) => {
                let last_seg = path
                    .path
                    .segments
                    .last()
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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
                    // invariant: a type path parsed from source has at least one segment.
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
        f.write_str(match self {
            RustType::Vec => "Vec",
            RustType::Option => "Option",
            RustType::HashSet => "HashSet",
            RustType::BTreeSet => "BTreeSet",
            RustType::HashMap => "HashMap",
            RustType::BTreeMap => "BTreeMap",
            RustType::DateTime => "DateTime",
        })
    }
}
