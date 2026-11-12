//! Hashing and password functions, `crypto::*`.
//!
//! ```rust
//! use umbriel::{*, functions::crypto};
//! assert_eq!(crypto::sha256!("text").to_debug_sql(), "crypto::sha256('text')");
//! ```

use crate::kinds;

functions! { $ "crypto" "::", crypto, crypto;
    /// The BLAKE3 hash, in hexadecimal.
    blake3(value: StrandLike) -> kinds::String;
    /// The Jenkins one-at-a-time hash, as an integer.
    joaat(value: StrandLike) -> kinds::Int;
    /// The MD5 hash, in hexadecimal.
    md5(value: StrandLike) -> kinds::String;
    /// The SHA-1 hash, in hexadecimal.
    sha1(value: StrandLike) -> kinds::String;
    /// The SHA-256 hash, in hexadecimal.
    sha256(value: StrandLike) -> kinds::String;
    /// The SHA-512 hash, in hexadecimal.
    sha512(value: StrandLike) -> kinds::String;
}

/// Argon2 password hashing, `crypto::argon2::*`.
pub mod argon2 {
    use crate::kinds;

    functions! { $ "crypto::argon2" "::", crypto_argon2, crypto::argon2;
        /// Whether `password` matches the hash `hash`.
        compare(hash: StrandLike, password: StrandLike) -> kinds::Bool;
        /// The hash of `password`.
        generate(password: StrandLike) -> kinds::String;
    }
}

/// bcrypt password hashing, `crypto::bcrypt::*`.
pub mod bcrypt {
    use crate::kinds;

    functions! { $ "crypto::bcrypt" "::", crypto_bcrypt, crypto::bcrypt;
        /// Whether `password` matches the hash `hash`.
        compare(hash: StrandLike, password: StrandLike) -> kinds::Bool;
        /// The hash of `password`.
        generate(password: StrandLike) -> kinds::String;
    }
}

/// PBKDF2 password hashing, `crypto::pbkdf2::*`.
pub mod pbkdf2 {
    use crate::kinds;

    functions! { $ "crypto::pbkdf2" "::", crypto_pbkdf2, crypto::pbkdf2;
        /// Whether `password` matches the hash `hash`.
        compare(hash: StrandLike, password: StrandLike) -> kinds::Bool;
        /// The hash of `password`.
        generate(password: StrandLike) -> kinds::String;
    }
}

/// scrypt password hashing, `crypto::scrypt::*`.
pub mod scrypt {
    use crate::kinds;

    functions! { $ "crypto::scrypt" "::", crypto_scrypt, crypto::scrypt;
        /// Whether `password` matches the hash `hash`.
        compare(hash: StrandLike, password: StrandLike) -> kinds::Bool;
        /// The hash of `password`.
        generate(password: StrandLike) -> kinds::String;
    }
}
