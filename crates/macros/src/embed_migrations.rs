use std::{env, fs, path::PathBuf};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitStr;

use crate::helpers::get_crate_name;

/// Default migration directory, relative to the consumer's `Cargo.toml`.
const DEFAULT_DIR: &str = "migrations";

/// Expands `embed_migrations!()` or `embed_migrations!("path")`.
///
/// Only lists the directory and includes each file; parsing, pairing and validation of the
/// filenames happen in `umbriel::migrations::EmbeddedMigrations`. `include_str!` makes edits
/// rebuild the consumer; newly added files need `cargo::rerun-if-changed` in a build script.
pub fn generate_embedded_migrations(input: TokenStream) -> TokenStream {
    match expand(input) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    }
}

fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let path = if input.is_empty() {
        None
    } else {
        Some(syn::parse2::<LitStr>(input)?)
    };
    let span = path.as_ref().map_or_else(Span::call_site, LitStr::span);
    let relative = path.map_or_else(|| DEFAULT_DIR.to_string(), |path| path.value());

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|err| syn::Error::new(span, format!("CARGO_MANIFEST_DIR is not set: {err}")))?;
    let dir = PathBuf::from(manifest_dir).join(&relative);

    let mut files = fs::read_dir(&dir)
        .map_err(|err| {
            syn::Error::new(
                span,
                format!("cannot read migration directory {}: {err}", dir.display()),
            )
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| syn::Error::new(span, err.to_string()))?;
    files.retain(|path| path.is_file());
    files.sort();

    let crate_name = get_crate_name();
    let entries = files.iter().map(|path| {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path = path.to_string_lossy();
        quote!(#crate_name::migrations::FileMetadataStatic {
            name: #name,
            content: ::core::include_str!(#path),
        })
    });

    Ok(quote!(#crate_name::migrations::EmbeddedMigrations::new(&[#(#entries),*])))
}
