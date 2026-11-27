pub(crate) mod casing;
pub(crate) mod field_meta;
pub(crate) mod keywords;
pub(crate) mod model;
pub(crate) mod table_meta;
pub(crate) mod token_codegen;
pub(crate) mod utils;
pub(crate) mod variables;

pub use casing::*;
pub use field_meta::*;
pub use keywords::*;
pub use model::*;
pub use table_meta::*;
pub use token_codegen::*;
pub use utils::*;
pub use variables::*;

/// Parsing errors of the model derives. `darling::Error` converts from `syn::Error` with its span,
/// accumulates several errors and writes them as compile errors.
pub type ExtractorResult<T> = darling::Result<T>;

#[derive(Debug, Copy, Clone)]
pub enum DataType {
    Node,
    Edge,
    Object,
}

impl DataType {
    pub fn is_node_or_edge(&self) -> bool {
        matches!(self, Self::Node | Self::Edge)
    }

    pub fn is_edge(&self) -> bool {
        matches!(self, Self::Edge)
    }
}
