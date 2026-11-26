use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream, TokenTree};

use syn::Ident;

/// The path generated code names `umbriel` by: its name in the caller's manifest, which differs
/// when the caller renames the dependency.
pub fn get_crate_name() -> TokenStream {
    let name = match crate_name("umbriel") {
        Ok(FoundCrate::Name(name)) => name,
        Ok(FoundCrate::Itself) | Err(_) => "umbriel".to_string(),
    };
    TokenTree::from(Ident::new(&name, Span::call_site())).into()
}
