/// Parsing errors of the model derives. `darling::Error` converts from `syn::Error` with its span,
/// accumulates several errors and writes them as compile errors.
pub type ExtractorResult<T> = darling::Result<T>;
