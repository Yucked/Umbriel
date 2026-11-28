use syn::{self, GenericArgument, Path, PathArguments, Type, parse_quote};

use super::{StructIdent, table::TableNameIdent};

use crate::models::*;

create_tokenstream_wrapper!(=>ExplicitFullyQualifiedGenericsPath);

/// A model being generated: the parsed struct and which kind of model it is.
#[derive(Clone, Debug)]
pub struct ModelAttributes<'a> {
    receiver: &'a ModelReceiver,
    data_type: DataType,
}

impl<'a> ModelAttributes<'a> {
    pub fn new(receiver: &'a ModelReceiver) -> ExtractorResult<Self> {
        // Validated before any field: type inference reads the table and only reports that it
        // could not infer a type, hiding the table's own error.
        receiver.table()?;
        Ok(Self {
            data_type: receiver.data_type()?,
            receiver,
        })
    }

    pub fn receiver(&self) -> &'a ModelReceiver {
        self.receiver
    }

    pub fn fields(&self) -> ExtractorResult<Vec<&'a MyFieldReceiver>> {
        Ok(self
            .receiver
            .data
            .as_ref()
            .take_struct()
            .ok_or(darling::Error::custom("Expected a struct"))?
            .fields)
    }

    pub fn ident(&self) -> StructIdent {
        self.receiver.ident()
    }

    pub fn generics(&self) -> &StructGenerics {
        self.receiver.generics()
    }

    pub fn table(&self) -> ExtractorResult<Option<&TableNameIdent>> {
        self.receiver.table()
    }

    pub fn explicit_fully_qualified_generics_path(&self) -> ExplicitFullyQualifiedGenericsPath {
        let (_struct_impl_generics, struct_ty_generics, _struct_where_clause) =
            &self.generics().split_for_impl();
        let explicit_generics = if struct_ty_generics.into_token_stream().is_empty() {
            quote!()
        } else {
            quote!(::#struct_ty_generics)
        };
        explicit_generics.into()
    }

    pub fn to_data_type(&self) -> DataType {
        self.data_type
    }

    pub fn casing(&self) -> ExtractorResult<CaseString> {
        self.receiver.casing()
    }

    /// The struct's type with its generic parameters as arguments and no bounds: `Struct<'a, T>`.
    pub fn construct_struct_type_without_bounds(&self) -> CustomType {
        let mut path = Path::from(self.ident());
        let generics = self.generics().to_basic_generics_ref();

        // Process generics, excluding bounds
        if !generics.params.is_empty() {
            let args = generics
                .params
                .iter()
                .map(|param| match param {
                    syn::GenericParam::Type(type_param) => {
                        GenericArgument::Type(parse_quote!(#type_param))
                    }
                    syn::GenericParam::Lifetime(lifetime_def) => {
                        GenericArgument::Lifetime(lifetime_def.lifetime.clone())
                    }
                    // The struct's own type is written with the parameter, not its default value.
                    syn::GenericParam::Const(const_param) => {
                        let ident = &const_param.ident;
                        GenericArgument::Const(parse_quote!(#ident))
                    }
                })
                .collect();

            // invariant: the path is the struct's identifier, and generics with parameters have
            // their angle brackets.
            path.segments
                .last_mut()
                .expect("Problem getting last segment of path. Path potentially empty.")
                .arguments = PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                colon2_token: None,
                lt_token: generics.lt_token.expect("Missing lt token"),
                args,
                gt_token: generics.gt_token.expect("Missing gt token"),
            });
        }

        Type::Path(syn::TypePath {
            attrs: Vec::new(),
            qself: None,
            path,
        })
        .into()
    }
}
