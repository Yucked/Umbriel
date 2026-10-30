// The only module that names `surrealdb_syn`: it is SurrealDB's internal parser with no SemVer
// guarantee, so its version moves with `surrealdb` and its API stays contained here.

use proc_macro2::Span;
use surrealdb_syn::{
    lexer::Lexer,
    token::{Operator, Token, TokenKind},
};

use crate::models::EdgeDirection;

/// Parses a relation's `connection` as SurrealQL and returns its direction, edge table and node
/// table. The parser checks the syntax; the lexer then requires the whole text to be exactly two
/// graph steps in one direction over plain table names, so trailing or extra parts, filters,
/// parameters, `<->` and mixed arrows are rejected.
pub(crate) fn graph_connection(
    connection: &str,
    span: Span,
) -> syn::Result<(EdgeDirection, &str, &str)> {
    surrealdb_syn::idiom(connection).map_err(|err| syn::Error::new(span, err))?;

    let mut lexer = Lexer::new(connection.as_bytes());
    let tokens: Vec<Token> = lexer.by_ref().collect();
    let text = |token: &Token| lexer.span_str(token.span);
    // A table name lexes as an identifier, or as a keyword when it spells one (such as `user`).
    let is_name =
        |token: &Token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword(_));
    let is_out = |token: &Token| token.kind == TokenKind::ArrowRight;
    let is_in = |chevron: &Token, dash: &Token| {
        chevron.kind == TokenKind::LeftChefron
            && dash.kind == TokenKind::Operator(Operator::Subtract)
    };
    match tokens.as_slice() {
        [a, edge, b, node] if is_out(a) && is_out(b) && is_name(edge) && is_name(node) => {
            Ok((EdgeDirection::Out, text(edge), text(node)))
        }
        [a1, a2, edge, b1, b2, node]
            if is_in(a1, a2) && is_in(b1, b2) && is_name(edge) && is_name(node) =>
        {
            Ok((EdgeDirection::In, text(edge), text(node)))
        }
        _ => Err(syn::Error::new(
            span,
            format!(
                "`{connection}` is not a relation connection: write exactly `->edge->node` or \
                 `<-edge<-node`, with table names"
            ),
        )),
    }
}

/// Checks that `query` is syntactically valid SurrealQL, reporting the parser's message at `span`.
///
/// Syntax only: tables, fields, permissions and parameter types are not checked.
pub(crate) fn validate(query: &str, span: Span) -> syn::Result<()> {
    surrealdb_syn::parse(query)
        .map(drop)
        .map_err(|err| syn::Error::new(span, err))
}

/// Parses a field type as a definition writes it, e.g. `option<array<record<user>>>`, into the
/// SDK's `Kind`, reporting the parser's message at `span`.
pub(crate) fn parse_kind(kind: &str, span: Span) -> syn::Result<surrealdb_types::Kind> {
    surrealdb_syn::kind_for_definition(kind)
        .map(Into::into)
        .map_err(|err| syn::Error::new(span, err))
}
