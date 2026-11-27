use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, parse_macro_input};

mod embed_migrations;
mod helpers;
mod kind;
mod models;
mod models_union;
mod query;
mod statement_parser;
mod surql;
mod utilities;

use models::ModelReceiver;
use statement_parser::{helpers::reset_variable_names, query_chain};
use utilities::{PickedMeta, TableDeriveAttributesPickable};

/// Expands a darling-parsed derive input, reporting attribute errors at their spans.
fn derive_with<T: FromDeriveInput + quote::ToTokens>(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match T::from_derive_input(&input) {
        Ok(output) => quote!(#output).into(),
        Err(err) => err.write_errors().into(),
    }
}

/// Declares a model; documented on its re-export, `umbriel::model`.
#[proc_macro_attribute]
pub fn model(args: TokenStream, item: TokenStream) -> TokenStream {
    models::model_attribute(args.into(), item.into()).into()
}

/// Generates a model's code; added by `#[umbriel::model]`, which passes the model kind and
/// table as `#[orm(..)]`. `surreal` is declared too so `value = manual` models keep their
/// `#[surreal(rename …)]` attributes.
#[doc(hidden)]
#[proc_macro_derive(ModelCodegen, attributes(orm, surreal))]
pub fn model_codegen(input: TokenStream) -> TokenStream {
    derive_with::<ModelReceiver>(input)
}

/// Declares a union of models for mixed-model writes; documented on its re-export,
/// `umbriel::Models`.
#[proc_macro_derive(Models, attributes(models))]
pub fn models_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    models_union::derive_models(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Pickable)]
pub fn pickable_derive(input: TokenStream) -> TokenStream {
    derive_with::<TableDeriveAttributesPickable>(input)
}

#[proc_macro]
pub fn pick(input: TokenStream) -> TokenStream {
    match syn::parse::<PickedMeta>(input) {
        Ok(output) => quote!(#output).into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn embed_migrations(input: TokenStream) -> TokenStream {
    embed_migrations::generate_embedded_migrations(input.into()).into()
}

/// Checks a query at compile time and returns the query as a string.
/// Unlike, query! macro, this macro does not allow variable interpolation.
///
/// # Validation
/// Syntax only, using SurrealDB's own parser at the version Umbriel depends on: tables, fields,
/// permissions and parameter types are not checked, and syntax newer than that version is rejected.
///
/// # Arguments
/// * `query` - The query to be checked at compile time.
///
/// The example is on `umbriel::query_raw`: this crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn query_raw(raw_input: TokenStream) -> TokenStream {
    let r_input = raw_input.clone();
    let literal = parse_macro_input!(r_input as LitStr);
    match surql::validate(&literal.value(), literal.span()) {
        Ok(()) => raw_input,
        Err(err) => err.to_compile_error().into(),
    }
}

/// Checks one or multiple quer(ies) at compile time.
/// It also allows interpolations of variables and can
/// be run.
///
/// # Validation
/// Syntax only, using SurrealDB's own parser at the version Umbriel depends on: tables, fields,
/// permissions and parameter types are not checked, and syntax newer than that version is rejected.
/// Every `$placeholder` must have a binding and every binding a placeholder.
///
/// # Arguments
/// * `db` - The database connection to be used.
/// * `quer(ies)` - A single or list of queries to be checked at compile time.
/// * `params` - The parameters to be used for the query.
///
/// Several queries can share one set of bindings. The examples are on `umbriel::query`: this
/// crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn query(raw_input: TokenStream) -> TokenStream {
    query::query(raw_input.into()).into()
}

/// A novel approach to writing SQL queries in Rust. It allows you to write SQL queries
/// in a native-like Rust Syntax. It supports mixing query builder functions with native looking
/// syntax such as if, for, let, return statements and standard Rust expressions which can be
/// a normal umbriel query builder function statement.
///
/// It dynamically returns a query chain, transaction or a block depending on the usage.
/// By default, it returns a query chain. If you want to return a transaction, you start
/// with a `begin transaction;` and end with a `commit transaction;` or `cancel transaction;`
/// keywords.
/// To return a block, you have to return typically at the end.
///
/// # Arguments
/// * `queries` - Series of queries to be checked at compile time.
///
/// The examples are on `umbriel::chain`: this crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn chain(input: TokenStream) -> TokenStream {
    query_chain::chain(input)
}

/// A block is a series of queries that are executed as a single
/// unit, surrounded by curly braces and typically returns a value. It can be assigned to a field or variable.
/// It supports mixing query builder functions with native looking
/// syntax such as if, for, let, return statements and standard Rust expressions which can be
/// a normal umbriel query builder function statement.
///
/// # Arguments
/// * `queries` - Series of queries to be checked at compile time.
///
/// The example is on `umbriel::block`: this crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn block(input: TokenStream) -> TokenStream {
    query_chain::block(input)
}

/// A transaction is a series of queries that are executed as a single
/// unit. It supports mixing query builder functions with native looking
/// syntax such as if, for, let, return statements and standard Rust expressions which can be
/// a normal umbriel query builder function statement.
///
/// It must start with a `begin transaction;` and end with a `commit transaction;` or `cancel transaction;`
/// keywords.
///
/// # Arguments
/// * `queries` - Series of queries to be checked at compile time.
///
/// The example is on `umbriel::transaction`: this crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn transaction(input: TokenStream) -> TokenStream {
    query_chain::transaction(input)
}

/// A macro to generate a function definition statement and the corresponding helper function.
/// e.g. `define_function!(get_it(first: bool, last: string, birthday: string) { let person = "43"; return person; });`
/// generates a `get_it_statement` itself and `get_it` helper function created by the macro.
///
/// # Arguments
/// * `function definition` - The function definition
///
/// The example is on `umbriel::define_function`: this crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn define_function(input: TokenStream) -> TokenStream {
    reset_variable_names();
    statement_parser::define_function(input)
}

/// A helper function to create a standalone for loop if you don't want to define within
/// chain! or block! or transaction! macro. This is almost never necessary. Just use
/// chain! or block! or transaction! macro and define your for loop within it,
///
/// The loop parameter is declared by the macro; refer to it by reference inside the body.
///
/// The example is on `umbriel::for_`: this crate's doctests cannot use `umbriel`.
#[proc_macro]
pub fn for_(input: TokenStream) -> TokenStream {
    reset_variable_names();
    statement_parser::for_::for_loop_without_for_keyword(input)
}
