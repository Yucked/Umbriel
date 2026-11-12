//! Parsing functions, `parse::*`.
//!
//! ```rust
//! use umbriel::{*, functions::parse};
//! assert_eq!(
//!     parse::url::host!("https://example.com").to_debug_sql(),
//!     "parse::url::host('https://example.com')"
//! );
//! ```

/// Email address parsing, `parse::email::*`.
pub mod email {
    use crate::kinds;

    functions! { $ "parse::email" "::", parse_email, parse::email;
        /// The host of an email address.
        host(email: StrandLike) -> kinds::String;
        /// The user of an email address.
        user(email: StrandLike) -> kinds::String;
    }
}

/// URL parsing, `parse::url::*`.
pub mod url {
    use crate::kinds;

    functions! { $ "parse::url" "::", parse_url, parse::url;
        /// The domain of a URL.
        domain(url: StrandLike) -> kinds::String;
        /// The fragment of a URL; `NONE` without one.
        fragment(url: StrandLike) -> kinds::Optional<kinds::String>;
        /// The host of a URL.
        host(url: StrandLike) -> kinds::String;
        /// The path of a URL.
        path(url: StrandLike) -> kinds::String;
        /// The port of a URL; `NONE` without one.
        port(url: StrandLike) -> kinds::Optional<kinds::Int>;
        /// The query string of a URL; `NONE` without one.
        query(url: StrandLike) -> kinds::Optional<kinds::String>;
        /// The scheme of a URL.
        scheme(url: StrandLike) -> kinds::String;
    }
}
