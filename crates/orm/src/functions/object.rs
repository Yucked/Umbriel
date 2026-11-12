//! Object functions, `object::*`.
//!
//! ```rust
//! use umbriel::{*, functions::object};
//! let settings = Field::new("settings");
//! assert_eq!(object::keys!(settings.clone()).to_debug_sql(), "object::keys(settings)");
//! assert_eq!(ObjectLike::from(settings).len().to_debug_sql(), "settings.len()");
//! ```

use crate::kinds;

functions! { $ "object" "::", object, object;
    /// The `[key, value]` pairs of the object.
    entries(object: ObjectLike) -> kinds::Array<kinds::Array> [ObjectLike.entries];
    /// The object with the fields of `other` added, replacing any it has.
    extend(object: ObjectLike, other: ObjectLike) -> kinds::Object [ObjectLike.extend];
    /// The object of the `[key, value]` pairs.
    from_entries(entries: ArrayLike) -> kinds::Object;
    /// Whether the object has no fields.
    is_empty(object: ObjectLike) -> kinds::Bool [ObjectLike.is_empty];
    /// The keys of the object.
    keys(object: ObjectLike) -> kinds::Array<kinds::String> [ObjectLike.keys];
    /// The number of fields.
    len(object: ObjectLike) -> kinds::Int [ObjectLike.len];
    /// The object without the field, or the fields, `keys`.
    remove(object: ObjectLike, keys: ValueLike) -> kinds::Object [ObjectLike.remove];
    /// The values of the object.
    values(object: ObjectLike) -> kinds::Array [ObjectLike.values];
}
