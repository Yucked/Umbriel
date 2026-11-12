//! Bytes functions, `bytes::*`.
//!
//! ```rust
//! use umbriel::{*, functions::bytes};
//! let payload = Field::new("payload");
//! assert_eq!(bytes::len!(payload.clone()).to_debug_sql(), "bytes::len(payload)");
//! assert_eq!(BytesLike::from(payload).len().to_debug_sql(), "payload.len()");
//! ```

use crate::kinds;

functions! { $ "bytes" "::", bytes, bytes;
    /// The number of bytes.
    len(bytes: BytesLike) -> kinds::Int [BytesLike.len];
}
