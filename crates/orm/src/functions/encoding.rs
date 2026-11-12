//! Encoding functions, `encoding::*`.
//!
//! ```rust
//! use umbriel::{*, functions::encoding};
//! assert_eq!(
//!     encoding::base64::decode!("aGk=").to_debug_sql(),
//!     "encoding::base64::decode('aGk=')"
//! );
//! ```

/// Base64, `encoding::base64::*`.
pub mod base64 {
    use crate::kinds;

    functions! { $ "encoding::base64" "::", encoding_base64, encoding::base64;
        /// The bytes a base64 string encodes.
        decode(text: StrandLike) -> kinds::Bytes;
        /// The bytes in base64, padded with `=` when `padded` is true.
        encode(bytes: BytesLike; padded: BoolLike) -> kinds::String;
    }
}

/// CBOR, `encoding::cbor::*`.
pub mod cbor {
    use crate::kinds;

    functions! { $ "encoding::cbor" "::", encoding_cbor, encoding::cbor;
        /// The value CBOR bytes encode.
        decode(bytes: BytesLike) -> kinds::Any;
        /// The value as CBOR bytes.
        encode(value: ValueLike) -> kinds::Bytes;
    }
}

/// JSON, `encoding::json::*`.
pub mod json {
    use crate::kinds;

    functions! { $ "encoding::json" "::", encoding_json, encoding::json;
        /// The value a JSON string encodes.
        decode(text: StrandLike) -> kinds::Any;
        /// The value as a JSON string.
        encode(value: ValueLike) -> kinds::String;
    }
}
