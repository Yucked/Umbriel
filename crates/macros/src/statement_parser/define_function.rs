use crate::helpers::get_crate_name;
use crate::kind::kind_tokens;
use crate::surql;
use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::{format_ident, quote, quote_spanned};
use surrealdb_types::Kind;
use syn::{
    self, Ident, Token,
    parse::{Parse, ParseStream},
};

use super::if_else::Body;

#[derive(Clone, Debug)]
struct FieldTypeParser(Kind);

impl Parse for FieldTypeParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        let type_string = parse_until_top_level_comma(input)?;
        surql::parse_kind(&type_string, span).map(FieldTypeParser)
    }
}

fn parse_until_top_level_comma(input: ParseStream) -> syn::Result<String> {
    let mut depth = 0;
    let mut type_string = String::new();

    while !input.is_empty() {
        if input.peek(Token![<]) {
            depth += 1;
            type_string.push('<');
            input.parse::<Token![<]>()?;
        } else if input.peek(Token![>]) && depth > 0 {
            depth -= 1;
            type_string.push('>');
            input.parse::<Token![>]>()?;
        } else if depth == 0 && input.peek(Token![,]) {
            break;
        } else {
            let ch = input.parse::<TokenTree>()?;
            type_string.push_str(&ch.to_string());
        }
    }

    Ok(type_string)
}

impl FieldTypeParser {
    fn is_option(&self) -> bool {
        // `option<T>` is `none | T`.
        matches!(&self.0, Kind::Either(variants) if variants.contains(&Kind::None))
    }

    /// The argument type the generated call helper accepts.
    pub fn to_lib_type(&self) -> proc_macro2::TokenStream {
        let crate_name = get_crate_name(false);
        if self.is_option() {
            return quote!(::std::option::Option<#crate_name::ValueLike>);
        }
        match &self.0 {
            Kind::Bool => quote!(#crate_name::BoolLike),
            Kind::Datetime => quote!(#crate_name::DatetimeLike),
            Kind::String => quote!(#crate_name::StrandLike),
            Kind::Number | Kind::Int | Kind::Float | Kind::Decimal => {
                quote!(#crate_name::NumberLike)
            }
            Kind::Duration => quote!(#crate_name::DurationLike),
            Kind::Object => quote!(#crate_name::ObjectLike),
            Kind::Record(_) => quote!(#crate_name::RecordIdLike),
            Kind::Array(..) => quote!(#crate_name::ArrayLike),
            Kind::Set(..) => quote!(#crate_name::SetLike),
            Kind::Bytes => quote!(#crate_name::BytesLike),
            Kind::Geometry(_) => quote!(#crate_name::GeometryLike),
            Kind::Table(_) => quote!(#crate_name::TableLike),
            _ => quote!(#crate_name::ValueLike),
        }
    }
}

#[derive(Clone, Debug)]
struct Argument {
    name: Ident,
    type_: FieldTypeParser,
}

impl Parse for Argument {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _arg_name: Ident = input.parse()?;
        let _ = input.parse::<Token![:]>()?; // Parsing the colon
        let type_ = input.parse::<FieldTypeParser>()?;

        Ok(Self {
            name: _arg_name,
            type_,
        })
    }
}

struct DefineFunctionStatementParser {
    function_name: Ident,
    args: Vec<Argument>,
    body: Body,
}

impl Parse for DefineFunctionStatementParser {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let function_name = input.parse::<Ident>()?;

        let args_content;
        let _ = syn::parenthesized!(args_content in input);
        let parsed_args = args_content.parse_terminated(Argument::parse, Token![,])?;

        let body = input.parse::<Body>()?;

        Ok(Self {
            function_name,
            args: parsed_args.into_iter().collect::<Vec<_>>(),
            body,
        })
    }
}

impl DefineFunctionStatementParser {
    pub fn tokenize(&self) -> TokenStream {
        let Self {
            function_name,
            args,
            body,
        } = self;
        let crate_name = get_crate_name(false);

        // Each argument is a parameter named by the caller: an invalid name, including one using
        // the prefix reserved for bound values, fails compilation at the name.
        let params_checked = args.iter().map(|param| {
            let name = &param.name;
            quote_spanned!(name.span()=>
                const _: () = #crate_name::Param::assert_valid(stringify!(#name));
            )
        });

        let params_rendered = args.iter().map(|param| {
            let name = &param.name;
            quote!(
                let #name = &#crate_name::Param::new(stringify!(#name));
            )
        });

        let args_used = args
            .iter()
            .map(|param| {
                let name = &param.name;
                let type_ = kind_tokens(&param.type_.0, &crate_name);
                quote!(
                    #crate_name::statements::FunctionArgument {
                         name: #crate_name::Param::new(stringify!(#name)),
                         type_: #type_,
                    }
                )
            })
            .collect::<Vec<_>>();

        let function_stmt_name = format_ident!("{function_name}_statement");

        let define_function_statement: proc_macro2::TokenStream = quote!(
            #( #params_checked )*

            pub fn #function_stmt_name() -> #crate_name::statements::DefineFunctionStatement{
                #( #params_rendered )*

                #crate_name::statements::define_function(stringify!(#function_name))
                .arguments(::std::vec![#(#args_used),*])
                .body(#body)
            }
        );

        let function_params = args.iter().map(|param| {
            let name = &param.name;
            let type_ = &param.type_.to_lib_type();
            quote!(
                #name: impl ::std::convert::Into<#type_>
            )
        });

        let args_converted = args.iter().map(|param| {
            let name = &param.name;
            let type_ = &param.type_.to_lib_type();
            if param.type_.is_option() {
                // An omitted optional argument is passed as `NONE`.
                quote!(
                    let #name: #type_ = #name.into();
                    let #name = #name.unwrap_or_else(|| #crate_name::ValueLike::from(#crate_name::NONE));
                )
            } else {
                quote!(let #name: #type_ = #name.into();)
            }
        });
        let arg_names = args.iter().map(|param| &param.name);

        let exported_function_name = format_ident!("{function_name}_fn");
        let generated_function_def = quote!(
            pub fn #exported_function_name(#( #function_params ), *) -> #crate_name::Function {
                #( #args_converted )*

                #crate_name::Function::__custom(
                    stringify!(#function_name),
                    ::std::vec![#( #crate_name::ValueLike::from(#arg_names) ),*],
                )
            }
        );

        let generated_func_macro = quote!(
                #[macro_use]
                macro_rules! #function_name {
                    ($($param:expr),*) => {
                        {
                            #generated_function_def
                            #exported_function_name($($param),*)
                        }
                    }
                 }
        );
        quote!(
            #define_function_statement
            #generated_function_def
            #generated_func_macro
        )
        .into()
    }
}

pub fn define_function(input: TokenStream) -> TokenStream {
    let function = syn::parse_macro_input!(input as DefineFunctionStatementParser);
    function.tokenize()
}
