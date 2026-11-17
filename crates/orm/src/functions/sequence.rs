//! Sequence functions, `sequence::*`.
//!
//! ```rust
//! use umbriel::{*, functions::sequence};
//! assert_eq!(
//!     sequence::nextval!("order_number").to_debug_sql(),
//!     "sequence::nextval('order_number')"
//! );
//! ```

use crate::kinds;

functions! { $ "sequence" "::", sequence, sequence;
    /// The next value of the sequence named `sequence`. Values are unique and increasing, but a
    /// value drawn in a transaction that fails is not issued again, so they may have gaps.
    nextval(sequence: StrandLike) -> kinds::Int;
}
