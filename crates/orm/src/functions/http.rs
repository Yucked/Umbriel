//! HTTP request functions, `http::*`. SurrealDB runs them only when its network capability allows
//! the target.
//!
//! ```rust
//! use umbriel::{*, functions::http};
//! assert_eq!(
//!     http::get!("https://example.com").to_debug_sql(),
//!     "http::get('https://example.com')"
//! );
//! ```

use crate::{StrandLike, kinds};

/// A URL: a string.
pub type Url = StrandLike;

functions! { $ "http" "::", http, http;
    /// A `DELETE` request, with the request headers `headers` when given.
    delete(url: StrandLike; headers: ObjectLike) -> kinds::Any;
    /// A `GET` request, with the request headers `headers` when given.
    get(url: StrandLike; headers: ObjectLike) -> kinds::Any;
    /// A `HEAD` request, with the request headers `headers` when given.
    head(url: StrandLike; headers: ObjectLike) -> kinds::Any;
    /// A `PATCH` request with `body`, with the request headers `headers` when given.
    patch(url: StrandLike, body: ValueLike; headers: ObjectLike) -> kinds::Any;
    /// A `POST` request with `body`, with the request headers `headers` when given.
    post(url: StrandLike, body: ValueLike; headers: ObjectLike) -> kinds::Any;
    /// A `PUT` request with `body`, with the request headers `headers` when given.
    put(url: StrandLike, body: ValueLike; headers: ObjectLike) -> kinds::Any;
}
