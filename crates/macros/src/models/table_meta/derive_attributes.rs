use crate::helpers::get_crate_name;
use darling::{FromDeriveInput, ast::Data, util};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Ident, ext::IdentExt};

use super::table::TableNameIdent;
use crate::models::*;

create_ident_wrapper!(StructIdent);
create_ident_wrapper!(StructPatchIdent);

impl StructIdent {
    pub fn patch_ident(&self) -> StructPatchIdent {
        StructPatchIdent(format_ident!("{}Patch", self.to_string()))
    }

    /// The model's merge type, `<Model>Merge`, generated next to the model.
    pub fn merge_ident(&self) -> Ident {
        format_ident!("{}Merge", self.to_string())
    }

    /// The model's schema type, `<Model>Schema`, generated next to the model.
    pub fn schema_ident(&self) -> Ident {
        format_ident!("{}Schema", self.to_string())
    }

    /// An item the expansion names but users do not: `__<len><Model><suffix>`. Generated items
    /// are siblings of the model, never inside a module, so models declared in functions work.
    /// The length of the model's name ends the prefix unambiguously: without it, `User`'s
    /// `ProfileNameField` and `UserProfile`'s `NameField` would share one name.
    pub fn hidden_ident(&self, suffix: impl std::fmt::Display) -> Ident {
        let model = self.0.unraw().to_string();
        format_ident!("__{}{model}{suffix}", model.len())
    }
}

/// A struct marked with `#[umbriel::model(..)]`. The attribute macro forwards its arguments as
/// `#[orm(node | edge | object, table = ..)]`; the other `#[orm(..)]` settings are the user's.
#[derive(Clone, Debug, FromDeriveInput)]
#[darling(attributes(orm), forward_attrs(surreal), supports(struct_named))]
pub struct ModelReceiver {
    pub(crate) ident: Ident,
    pub(crate) vis: syn::Visibility,
    /// The struct's `#[surreal(…)]` attributes, read for `rename_all`.
    pub(crate) attrs: Vec<syn::Attribute>,
    pub(crate) generics: StructGenerics,
    pub data: Data<util::Ignored, MyFieldReceiver>,

    #[darling(default)]
    pub(crate) node: bool,

    #[darling(default)]
    pub(crate) edge: bool,

    #[darling(default)]
    pub(crate) object: bool,

    #[darling(default)]
    pub(crate) table: Option<TableNameIdent>,

    #[darling(default)]
    pub(crate) relax_table: Option<bool>,

    /// `table_prefix = "log_"`: the model is also stored in tables chosen at run time, each the
    /// prefix followed by at least one more character.
    #[darling(default)]
    pub(crate) table_prefix: Option<String>,

    #[darling(default)]
    pub(crate) schemafull: Option<bool>,

    #[darling(default)]
    pub(crate) drop: Option<bool>,

    pub(crate) as_: Option<AttributeAs>,

    pub(crate) permissions: Option<Permissions>,

    #[darling(default)]
    pub(crate) define: Option<AttributeDefine>,
}

impl ModelReceiver {
    pub fn ident(&self) -> StructIdent {
        StructIdent(self.ident.clone())
    }

    pub fn generics(&self) -> &StructGenerics {
        &self.generics
    }

    pub fn data_type(&self) -> ExtractorResult<DataType> {
        match (self.node, self.edge, self.object) {
            (true, false, false) => Ok(DataType::Node),
            (false, true, false) => Ok(DataType::Edge),
            (false, false, true) => Ok(DataType::Object),
            _ => Err(syn::Error::new_spanned(
                &self.ident,
                "a model is exactly one of `node`, `edge` or `object`",
            )
            .into()),
        }
    }

    /// The table of a node or edge; objects are stored inside other records and have none.
    pub fn table(&self) -> ExtractorResult<Option<&TableNameIdent>> {
        match (self.data_type()?, &self.table) {
            (DataType::Object, None) => Ok(None),
            (DataType::Object, Some(table)) => Err(syn::Error::new_spanned(
                table,
                "an embedded `object` model has no table",
            )
            .into()),
            (_, Some(table)) => Ok(Some(
                table.validate_and_return(&self.ident(), &self.relax_table)?,
            )),
            (_, None) => Err(syn::Error::new_spanned(
                &self.ident,
                "a `node` or `edge` model needs `table = \"…\"`",
            )
            .into()),
        }
    }

    /// The `table_prefix`, checked: only a node or edge has one, and it is not empty. Table names
    /// are escaped when rendered, so it may hold any characters.
    pub fn table_prefix(&self) -> ExtractorResult<Option<&str>> {
        let Some(prefix) = &self.table_prefix else {
            return Ok(None);
        };
        if matches!(self.data_type()?, DataType::Object) {
            return Err(syn::Error::new_spanned(
                &self.ident,
                "an embedded `object` model has no table, so no `table_prefix`",
            )
            .into());
        }
        if prefix.is_empty() {
            return Err(syn::Error::new_spanned(
                &self.ident,
                "`table_prefix` cannot be empty: every table would be a member",
            )
            .into());
        }
        Ok(Some(prefix))
    }

    /// The struct's `#[surreal(rename_all = "…")]`.
    pub fn casing(&self) -> ExtractorResult<StructLevelCasing> {
        struct_casing(&self.attrs)
    }

    pub fn get_table_definition_token(&self) -> ExtractorResult<TableDefinitions> {
        let ModelReceiver {
            ref drop,
            ref schemafull,
            ref as_,
            ref permissions,
            ref define,
            ..
        } = *self;

        let crate_name = get_crate_name(false);

        if (define.is_some())
            && (drop.is_some() || as_.is_some() || schemafull.is_some() || permissions.is_some())
        {
            return Err(syn::Error::new_spanned(
                &self.ident,
                "`define` gives the whole definition, so it cannot be combined with `drop`, `as`, \
                 `schemafull` or `permissions`",
            )
            .into());
        }

        let definition = match define {
            Some(define) => define.to_token_stream(),
            None => {
                let mut define_table_methods: Vec<TokenStream> = vec![];

                if drop.is_some() {
                    define_table_methods.push(quote!(.drop()))
                }

                if let Some(as_) = as_ {
                    define_table_methods.push(quote!(.as_(#as_)))
                }

                if schemafull.is_some() {
                    define_table_methods.push(quote!(.schemafull()))
                }

                if let Some(permissions) = permissions {
                    define_table_methods.push(permissions.to_token_stream());
                }

                quote!(
                    #crate_name::statements::define_table(<Self as #crate_name::Model>::table())
                    #( #define_table_methods) *
                )
            }
        };

        // Definitions write their values inline; `inline` keeps the builder's errors.
        Ok(quote!(#crate_name::Raw::inline(&#definition)).into())
    }
}
