use std::cell::Cell;

use proc_macro2::Span;
use syn::Ident;

thread_local! {
    /// The index of the next local the current expansion generates. Each query macro resets it
    /// before it parses its input, so the names depend only on that input.
    static NEXT_LOCAL: Cell<usize> = const { Cell::new(0) };
}

/// Starts the numbering of generated locals over, for a new expansion.
pub fn reset_variable_names() {
    NEXT_LOCAL.set(0);
}

/// A fresh name for a local that a query macro generates. It resolves with mixed-site hygiene, so
/// it can't collide with a name the caller writes, nor with the locals of another expansion
/// (nested `chain!`s included, which each have their own hygiene context). The index keeps the
/// locals of one expansion apart, and the same input always expands to the same names.
pub fn generate_variable_name() -> Ident {
    let index = NEXT_LOCAL.replace(NEXT_LOCAL.get() + 1);
    Ident::new(&format!("__umbriel_local_{index}"), Span::mixed_site())
}
