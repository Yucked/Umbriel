use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};

use crate::helpers::get_crate_name;

use super::{
    helpers::generate_variable_name, let_::LetStatementParser, query::QueryParser,
    return_::ReturnStatementParser,
};

#[derive(Debug, Clone)]
pub struct QueriesChainParser {
    pub statements: Vec<QueryParser>,
}

impl QueriesChainParser {
    /// The statements as a transaction when they hold exactly one `BEGIN` and end with one
    /// `COMMIT` or `CANCEL`, as a block when one of them is a top-level `return`, and as a chain
    /// otherwise.
    pub fn to_tokenstream(&self) -> TokenStream {
        let GeneratedCode {
            to_render,
            query_chain_var_ident,
        } = self.generate_code();
        let crate_name = get_crate_name();
        let tail = match self.transaction_ending() {
            Some(ending) => quote! {
                #crate_name::statements::begin_transaction()
                    .query(#query_chain_var_ident)
                    #ending
            },
            None if self.is_likely_query_block() => quote!(#query_chain_var_ident.as_block()),
            None => quote!(#query_chain_var_ident),
        };
        quote! {
            {
                #( #to_render )*

                #tail
            }
        }
    }

    /// The builder call that closes a transaction: `.commit_transaction()` or
    /// `.cancel_transaction()`. `None` unless the statements hold exactly one `BEGIN` and one
    /// `COMMIT` or `CANCEL`, and the latter is the last statement.
    fn transaction_ending(&self) -> Option<TokenStream> {
        let mut markers = self.statements.iter().filter(|statement| {
            matches!(
                statement,
                QueryParser::BeginTransaction
                    | QueryParser::CommitTransaction
                    | QueryParser::CancelTransaction
            )
        });
        let last = self.statements.last();
        match (markers.next(), markers.next(), markers.next(), last) {
            (
                Some(QueryParser::BeginTransaction),
                Some(QueryParser::CommitTransaction),
                None,
                Some(QueryParser::CommitTransaction),
            ) => Some(quote!(.commit_transaction())),
            (
                Some(QueryParser::BeginTransaction),
                Some(QueryParser::CancelTransaction),
                None,
                Some(QueryParser::CancelTransaction),
            ) => Some(quote!(.cancel_transaction())),
            _ => None,
        }
    }

    pub fn is_valid_transaction_statement(&self) -> bool {
        self.transaction_ending().is_some()
    }

    pub fn is_likely_query_block(&self) -> bool {
        self.statements.iter().any(QueryParser::is_return_statement)
    }
}

pub struct GeneratedCode {
    pub to_render: Vec<proc_macro2::TokenStream>,
    pub query_chain_var_ident: Ident,
}

struct QueryLine {
    pub rendered: proc_macro2::TokenStream,
    pub var_ident: proc_macro2::TokenStream,
}

impl QueriesChainParser {
    pub fn generate_code(&self) -> GeneratedCode {
        let statements = &self.statements;
        let crate_name = get_crate_name();

        let query_chain_var_name = format_ident!("query_chain_{}", generate_variable_name());
        let lines = statements.iter().map(|stmt_or_expr| match stmt_or_expr {
        QueryParser::LetStatement(var_statement) => {
            let LetStatementParser { ident, expr, .. } = var_statement;
            Some(QueryLine {
                rendered: quote! (
                    const _: () = #crate_name::Param::assert_valid(stringify!(#ident));
                    let ref #ident = #crate_name::statements::let_(stringify!(#ident)).equal_to(#expr);
                ),
                var_ident: quote!(#ident)
            })
        }
        QueryParser::Expr {expr, generated_ident} => Some(
                QueryLine {
                    rendered: quote! (
                        let #generated_ident = #expr;
                    ),
                    var_ident: quote!(#generated_ident)
                }
            ),
        QueryParser::ForLoop(for_loop_parser) => {
            let for_loop_content = &for_loop_parser.meta_content;
            let generated_ident = &for_loop_parser.generated_ident;
            let tokenized = for_loop_content.tokenize();
            let query_chain: proc_macro2::TokenStream = tokenized.query_chain.into();
                Some(QueryLine {
                    rendered: quote! (
                        let #generated_ident = #query_chain;
                    ),
                    var_ident: quote!(#generated_ident)
                })
            },
            QueryParser::IfEsle(if_else_meta) => {
                let tokenized = if_else_meta.meta_content.tokenize();
                let query_chain: proc_macro2::TokenStream = tokenized.query_chain.into();
                let generated_ident = &if_else_meta.meta_content.generated_ident;
                Some(
                    QueryLine {
                        rendered: quote! (
                            let #generated_ident = #query_chain;
                        ),
                        var_ident: quote!(#generated_ident)
                    }
                )
            },
            QueryParser::BeginTransaction | QueryParser::CommitTransaction | QueryParser::CancelTransaction => {
                None
            },
            QueryParser::ReturnStatement(r_stmt) => {
                let ReturnStatementParser { expr, generated_ident, _return, } = r_stmt;
                Some(
                    QueryLine {
                        rendered: quote! (
                            let #generated_ident = #crate_name::statements::return_(#expr);
                        ),
                        var_ident: quote!(#generated_ident)
                    }
                )
            },
            QueryParser::BreakStatement => {
                let generated_ident = generate_variable_name();
                Some(
                    QueryLine {
                        rendered: quote! (
                            let #generated_ident = #crate_name::statements::break_();
                        ),
                        var_ident: quote!(#generated_ident)
                    }
                )
            },
            QueryParser::ContinueStatement => {
                let generated_ident = generate_variable_name();
                Some(
                    QueryLine {
                        rendered: quote! (
                            let #generated_ident = #crate_name::statements::continue_();
                        ),
                        var_ident: quote!(#generated_ident)
                    }
                )
            },
   });

        let code = lines
            .flatten()
            .enumerate()
            .map(|(i, line)| {
                let QueryLine {
                    rendered,
                    var_ident,
                } = line;

                let is_first = i == 0;

                if is_first {
                    quote!(
                        #rendered
                        let #query_chain_var_name  = #crate_name::chain(#var_ident);
                    )
                } else {
                    quote!(
                        #rendered
                        let #query_chain_var_name  = #query_chain_var_name.chain(#var_ident);
                    )
                }
            })
            .collect::<Vec<_>>();

        GeneratedCode {
            to_render: code,
            query_chain_var_ident: query_chain_var_name,
        }
    }
}

impl Parse for QueriesChainParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut statements = Vec::new();

        while !input.is_empty() {
            statements.push(input.parse()?);
        }

        Ok(QueriesChainParser { statements })
    }
}
