//! `#[derive(Models)]`: an enum with one variant per model, each holding that model.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, spanned::Spanned};

use crate::helpers::get_crate_name;

/// Implements `SurrealValue`, decoding a record into the variant whose table family holds its id,
/// and `Models`, with a `const` assertion that no two variants' families overlap.
pub(crate) fn derive_models(input: &DeriveInput) -> syn::Result<TokenStream> {
    let crate_name = get_crate_name(false);
    let ident = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "a `Models` union cannot be generic: its variants' tables must be known statically",
        ));
    }
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "`Models` derives for an enum with one variant per model",
        ));
    };
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            ident,
            "a `Models` union needs at least one variant",
        ));
    }

    let mut variants = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        let model: Option<&Type> = match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                fields.unnamed.first().map(|field| &field.ty)
            }
            _ => None,
        };
        let Some(model) = model else {
            return Err(syn::Error::new_spanned(
                variant,
                "each variant of a `Models` union holds exactly one model, e.g. `User(User)`",
            ));
        };
        variants.push((&variant.ident, model));
    }

    let types = quote!(#crate_name::types);
    let members = variants
        .iter()
        .map(|(_, model)| quote!(#crate_name::TableSpec::of::<#model>()));
    let into_value = variants.iter().map(
        |(variant, _)| quote!(Self::#variant(record) => #types::SurrealValue::into_value(record)),
    );
    let is_value = variants
        .iter()
        .map(|(_, model)| quote!(<#model as #types::SurrealValue>::is_value(value)));
    let from_value = variants.iter().map(|(variant, model)| {
        quote!(
            if #crate_name::TableSpec::of::<#model>().accepts(table) {
                return <#model as #types::SurrealValue>::from_value(value).map(Self::#variant);
            }
        )
    });
    let overlap_message = format!("two variants of `{ident}` have overlapping table families");

    Ok(quote!(
        impl #types::SurrealValue for #ident {
            fn kind_of() -> #types::Kind {
                #types::Kind::Object
            }

            fn is_value(value: &#types::Value) -> bool {
                #( #is_value )||*
            }

            fn into_value(self) -> #types::Value {
                match self {
                    #( #into_value, )*
                }
            }

            fn from_value(value: #types::Value) -> ::std::result::Result<Self, #types::Error> {
                let table = match &value {
                    #types::Value::Object(object) => match object.get("id") {
                        ::std::option::Option::Some(#types::Value::RecordId(id)) => {
                            ::std::option::Option::Some(id.table.as_str().to_owned())
                        }
                        _ => ::std::option::Option::None,
                    },
                    _ => ::std::option::Option::None,
                };
                if let ::std::option::Option::Some(table) = &table {
                    let table = table.as_str();
                    #( #from_value )*
                }
                ::std::result::Result::Err(
                    #types::ConversionError::from_value(
                        <Self as #types::SurrealValue>::kind_of(),
                        &value,
                    )
                    .with_context("the record's id is not in a table of any variant")
                    .into(),
                )
            }
        }

        impl #crate_name::Models for #ident {
            const MEMBERS: &'static [#crate_name::TableSpec] = &[#( #members ),*];
        }

        const _: () = ::core::assert!(
            #crate_name::__private::overlapping_members(<#ident as #crate_name::Models>::MEMBERS)
                .is_none(),
            #overlap_message
        );
    ))
}
