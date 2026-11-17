//! API functions, `api::*`. `api::invoke` calls a defined API from a query; the middleware
//! functions (`api::timeout`, `api::req::*`, `api::res::*`) take a request SurrealDB supplies, so
//! they are written through [`ApiConfig`](crate::statements::ApiConfig)'s methods instead.
//!
//! ```rust
//! use umbriel::{*, functions::api};
//! assert_eq!(api::invoke!("/status").to_debug_sql(), "api::invoke('/status')");
//! ```

use crate::kinds;

functions! { $ "api" "::", api, api;
    /// Calls the API defined at `path` (starting with `/`) and returns its response object
    /// (`status`, `headers`, `body`). `request` may give `method`, `headers`, `query` and `body`;
    /// without it, the call is a `GET`.
    invoke(path: StrandLike; request: ObjectLike) -> kinds::Object;
}
