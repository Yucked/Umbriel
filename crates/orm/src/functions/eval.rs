//! Query evaluation functions, `eval::*`.
//!
//! SurrealDB runs them only when its capabilities allow both queries and evaluated queries
//! (`--allow-eval-query`). The embedded engine cannot allow them: the SDK's `Capabilities` has no
//! setting for evaluated queries in 3.3.4, so there they fail.
//!
//! ```rust
//! use umbriel::{*, functions::eval};
//! assert_eq!(eval::surql!("RETURN 1").to_debug_sql(), "eval::surql('RETURN 1')");
//! ```

use crate::kinds;

functions! { $ "eval" "::", eval, eval;
    /// The result of the GraphQL query, with `bindings` when given.
    gql(query: StrandLike; bindings: ObjectLike) -> kinds::Any;
    /// The result of the SurrealQL query, with `bindings` when given.
    surql(query: StrandLike; bindings: ObjectLike) -> kinds::Any;
}
