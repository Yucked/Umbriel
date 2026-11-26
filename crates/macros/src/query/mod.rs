use proc_macro2::{Ident, TokenStream};
use surrealdb_syn::{
    lexer::Lexer,
    token::{Delim, Keyword, Token, TokenKind},
};
use syn::parse::{Parse, ParseStream};
use syn::parse2;
use syn::punctuated::Punctuated;
use syn::token::{Brace, Bracket, Comma};
use syn::{Expr, LitStr, Result, Token};

struct Binding {
    key: Ident,
    value: Expr,
}

struct Bindings {
    pairs: Punctuated<Binding, Comma>,
}

impl Parse for Binding {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let value: Expr = input.parse()?;
        Ok(Binding { key, value })
    }
}

impl Parse for Bindings {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let _brace_token: Brace = syn::braced!(content in input);
        let pairs = Punctuated::parse_terminated(&content)?;
        Ok(Bindings { pairs })
    }
}

/// `query!(db, "…" | ["…", …] [, { name: value, … }])`.
struct QueryArgs {
    db: Expr,
    queries: Vec<LitStr>,
    bindings: Vec<Binding>,
}

impl Parse for QueryArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let db = input.parse()?;
        input.parse::<Token![,]>()?;

        let queries = if input.peek(Bracket) {
            let content;
            syn::bracketed!(content in input);
            Punctuated::<LitStr, Comma>::parse_terminated(&content)?
                .into_iter()
                .collect()
        } else {
            vec![input.parse()?]
        };

        let mut bindings = Vec::new();
        if input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
            let Bindings { pairs } = input.parse()?;
            bindings = pairs.into_iter().collect();
            input.parse::<Option<Token![,]>>()?;
        }

        Ok(Self {
            db,
            queries,
            bindings,
        })
    }
}

/// Parameters SurrealDB sets itself, which a query uses without a binding
/// (<https://surrealdb.com/docs/surrealql/parameters>).
const PREDEFINED_PARAMS: &[&str] = &[
    "access",
    "action",
    "after",
    "auth",
    "before",
    "event",
    "file",
    "input",
    "parent",
    "reference",
    "request",
    "session",
    "target",
    "this",
    "token",
    "value",
];

/// Names declared for part of a query: a block's (`{ … }`), ending with its closing brace, or a
/// closure body's, ending at the first `,`, `;` or closing delimiter at the closure's depth.
struct Scope<'a> {
    depth: usize,
    closure: bool,
    names: Vec<&'a str>,
}

/// The parameters a query takes from outside: those it uses where no enclosing scope has
/// declared them yet, and that SurrealDB does not set. Scopes follow SurrealQL: a `LET` declares
/// its parameter after its statement, until the end of the enclosing block; `FOR` and
/// `DEFINE FUNCTION` parameters belong to their body block, closure parameters to the closure's
/// body, and `DEFINE PARAM` to the database. `root` holds the top level's declarations, which
/// later queries of the same request see. SurrealDB's own lexer reads strings and comments as
/// single tokens, so a `$` inside them is not a parameter.
fn external_params<'a>(sql: &'a str, root: &mut Vec<&'a str>) -> Vec<&'a str> {
    let mut lexer = Lexer::new(sql.as_bytes());
    let tokens: Vec<Token> = lexer.by_ref().collect();
    let name = |token: &Token| -> &'a str {
        lexer.span_str(token.span)[1..].trim_matches(|c: char| matches!(c, '`' | '⟨' | '⟩'))
    };
    let is = |index: usize, kind: TokenKind| tokens.get(index).is_some_and(|t| t.kind == kind);
    let parameter_at = |index: usize| is(index, TokenKind::Parameter).then(|| name(&tokens[index]));
    let after_define =
        |index: usize| index >= 2 && is(index - 2, TokenKind::Keyword(Keyword::Define));

    let mut depth = 0;
    let mut scopes: Vec<Scope<'a>> = Vec::new();
    // `LET` parameters, declared when the statement at their depth ends.
    let mut lets: Vec<(usize, &'a str)> = Vec::new();
    // `FOR` or function parameters, declared in the next block opened at their depth.
    let mut body: Option<(usize, Vec<&'a str>)> = None;
    let mut external = Vec::new();

    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        index += 1;
        match token.kind {
            TokenKind::Keyword(keyword @ (Keyword::Let | Keyword::For | Keyword::Param)) => {
                let Some(name) = parameter_at(index) else {
                    continue;
                };
                match keyword {
                    Keyword::Let => lets.push((depth, name)),
                    Keyword::For => body = Some((depth, vec![name])),
                    _ if after_define(index) => root.push(name),
                    // `REMOVE PARAM $x` names the parameter without using it.
                    _ => {}
                }
                index += 1;
            }
            TokenKind::Keyword(Keyword::Function) if after_define(index) => {
                // The arguments are the parameters in the first, balanced, delimiters.
                let mut arguments = Vec::new();
                let mut inner = 0usize;
                while let Some(token) = tokens.get(index) {
                    index += 1;
                    match token.kind {
                        TokenKind::OpenDelim(_) => inner += 1,
                        TokenKind::CloseDelim(_) => {
                            inner = inner.saturating_sub(1);
                            if inner == 0 {
                                break;
                            }
                        }
                        TokenKind::Parameter if inner > 0 => arguments.push(name(token)),
                        _ => {}
                    }
                }
                body = Some((depth, arguments));
            }
            TokenKind::Vert if is(index, TokenKind::Parameter) => {
                // A closure, `|$a, $b: int| …`: its parameters run to the next `|`.
                let mut names = Vec::new();
                while let Some(token) = tokens.get(index) {
                    index += 1;
                    match token.kind {
                        TokenKind::Vert => break,
                        TokenKind::Parameter => names.push(name(token)),
                        _ => {}
                    }
                }
                scopes.push(Scope {
                    depth,
                    closure: true,
                    names,
                });
            }
            TokenKind::OpenDelim(delim) => {
                let names = match body.take() {
                    Some((at, names)) if at == depth && delim == Delim::Brace => names,
                    pending => {
                        body = pending;
                        Vec::new()
                    }
                };
                depth += 1;
                if delim == Delim::Brace {
                    scopes.push(Scope {
                        depth,
                        closure: false,
                        names,
                    });
                }
            }
            TokenKind::CloseDelim(_) => {
                // Ends the block it closes and the closures inside it.
                while scopes.last().is_some_and(|scope| scope.depth >= depth) {
                    scopes.pop();
                }
                lets.retain(|(at, _)| *at < depth);
                depth = depth.saturating_sub(1);
            }
            TokenKind::Comma | TokenKind::SemiColon => {
                while scopes
                    .last()
                    .is_some_and(|scope| scope.closure && scope.depth >= depth)
                {
                    scopes.pop();
                }
                if token.kind == TokenKind::SemiColon {
                    let names = match scopes.last_mut() {
                        Some(scope) => &mut scope.names,
                        None => &mut *root,
                    };
                    names.extend(lets.extract_if(.., |(at, _)| *at == depth).map(|(_, n)| n));
                }
            }
            TokenKind::Parameter => {
                let name = name(token);
                let declared =
                    scopes.iter().any(|scope| scope.names.contains(&name)) || root.contains(&name);
                if !declared && !PREDEFINED_PARAMS.contains(&name) && !external.contains(&name) {
                    external.push(name);
                }
            }
            _ => {}
        }
    }
    // A final statement without a `;` ends with the query.
    root.extend(
        lets.into_iter()
            .filter(|(at, _)| *at == 0)
            .map(|(_, name)| name),
    );
    external
}

/// Every binding names a parameter the queries take from outside, and every such parameter is
/// bound; reported at the binding's key or the query using the parameter. The queries run in
/// order as one request, so a top-level declaration in one holds in the next.
fn validate_placeholders(queries: &[(LitStr, String)], bindings: &[Binding]) -> Result<()> {
    let mut root = Vec::new();
    let external: Vec<Vec<&str>> = queries
        .iter()
        .map(|(_, sql)| external_params(sql, &mut root))
        .collect();

    for binding in bindings {
        if !external.iter().flatten().any(|name| binding.key == name) {
            return Err(syn::Error::new(
                binding.key.span(),
                format!(
                    "No placeholder found in SQL query for binding '{}'",
                    binding.key
                ),
            ));
        }
    }

    for ((literal, _), used) in queries.iter().zip(&external) {
        if let Some(name) = used
            .iter()
            .find(|name| !bindings.iter().any(|binding| binding.key == name))
        {
            return Err(syn::Error::new(
                literal.span(),
                format!("No binding found for placeholder '${name}'"),
            ));
        }
    }

    Ok(())
}

pub fn query(args: TokenStream) -> TokenStream {
    match expand(args) {
        Ok(output) => output,
        Err(error) => error.to_compile_error(),
    }
}

/// The expansion, with argument and placeholder errors reported as compile errors.
fn expand(args: TokenStream) -> Result<TokenStream> {
    let QueryArgs {
        db: db_con,
        queries,
        bindings,
    } = parse2(args)?;
    let queries = queries
        .into_iter()
        .map(|literal| {
            let sql = literal.value();
            crate::surql::validate(&sql, literal.span())?;
            Ok((literal, sql))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut output = TokenStream::new();

    validate_placeholders(&queries, &bindings)?;

    for (i, (_, query_str)) in queries.iter().enumerate() {
        let is_first = i == 0;
        if is_first {
            output.extend(quote::quote! {
                #db_con.query(#query_str)
            });
        } else {
            output.extend(quote::quote! {
                .query(#query_str)
            });
        }
    }

    // Binding names are parameter names chosen by the caller: an invalid one, including one using
    // the prefix reserved for bound values, fails compilation at the name.
    let crate_name = crate::helpers::get_crate_name();
    let mut name_checks = TokenStream::new();
    for Binding { key, value } in bindings {
        let name = key.to_string();
        name_checks.extend(quote::quote_spanned! {key.span()=>
            const _: () = #crate_name::Param::assert_valid(#name);
        });
        output.extend(quote::quote! {
            .bind((#name, #value))
        });
    }

    Ok(quote::quote!({
        #name_checks
        #output
    }))
}
