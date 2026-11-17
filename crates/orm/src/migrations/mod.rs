/// The migration command line, run from a binary that names the codebase's resources.
pub mod cli;
pub(crate) mod database;
mod error;
pub(crate) mod resources;
pub(crate) mod schema;

pub use cli::*;
pub use database::*;
pub use error::*;
pub use resources::*;
pub use schema::{Definition, ResourceKey, ResourceKind, Schema};

pub use umbriel_macros::embed_migrations;
