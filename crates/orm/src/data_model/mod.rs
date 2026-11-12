/// Casts, `<kind> value`.
mod casting;
pub use casting::{Cast, cast, cast_dynamic};

/// SurrealQL constants, such as `math::PI`.
mod constant;
pub use constant::Constant;
