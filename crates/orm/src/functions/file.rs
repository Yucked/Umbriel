//! File functions, `file::*`, on the files of a bucket. Files are experimental in SurrealDB
//! 3.3.4: the database must allow the `files` feature, or every call but `file::bucket` and
//! `file::key` fails.
//!
//! A file is a pointer, `f"bucket:/key"`: a [`File`](crate::types::File) binds as a
//! [`FileLike`](crate::FileLike). Each operation on one file is also a method of the pointer, as
//! SurrealDB dispatches it: `FileLike::from(file).get()` renders `($_v0).get()`.
//!
//! ```rust
//! use umbriel::{*, functions::file, types::File};
//! let avatar = FileLike::from(File::new("avatars", "alice.png"));
//! assert_eq!(file::get(avatar.clone()).to_debug_sql(), "file::get(f\"avatars:/alice.png\")");
//! assert_eq!(avatar.exists().to_debug_sql(), "(f\"avatars:/alice.png\").exists()");
//! ```

use crate::kinds;

functions! { $ "file" "::", file, file;
    /// The bucket of `file`.
    bucket(file: FileLike) -> kinds::String [FileLike.bucket];
    /// The key of `file`, starting with `/`.
    key(file: FileLike) -> kinds::String [FileLike.key];
    /// Writes `value` (bytes, or a string as its UTF-8 bytes) to `file`, replacing it; `NONE`.
    put(file: FileLike, value: ValueLike) -> kinds::Any [FileLike.put];
    /// Writes `value` to `file` unless it exists, in which case nothing changes; `NONE` either
    /// way.
    put_if_not_exists(file: FileLike, value: ValueLike) -> kinds::Any
        [FileLike.put_if_not_exists];
    /// The contents of `file`, or `NONE` when it does not exist.
    get(file: FileLike) -> kinds::Optional<kinds::Bytes> [FileLike.get];
    /// The metadata of `file` (`file`, `size`, `updated`), or `NONE` when it does not exist.
    head(file: FileLike) -> kinds::Optional<kinds::Object> [FileLike.head];
    /// Deletes `file`; `NONE`, whether or not it existed.
    delete(file: FileLike) -> kinds::Any [FileLike.delete];
    /// Copies `file` to `target`, a key in the same bucket or another file, replacing it; `NONE`.
    copy(file: FileLike, target: ValueLike) -> kinds::Any [FileLike.copy];
    /// Copies `file` to `target` unless it exists, in which case nothing changes; `NONE` either
    /// way.
    copy_if_not_exists(file: FileLike, target: ValueLike) -> kinds::Any
        [FileLike.copy_if_not_exists];
    /// Renames `file` to the key `target` in its bucket, replacing it; `NONE`.
    rename(file: FileLike, target: StrandLike) -> kinds::Any [FileLike.rename];
    /// Renames `file` to `target` unless it exists, in which case nothing changes and `file`
    /// keeps its name; `NONE` either way.
    rename_if_not_exists(file: FileLike, target: StrandLike) -> kinds::Any
        [FileLike.rename_if_not_exists];
    /// Whether `file` exists.
    exists(file: FileLike) -> kinds::Bool [FileLike.exists];
    /// The metadata of the files in the bucket named `bucket`, in key order. `options` may give
    /// `prefix`, `start` (list after this key) and `limit`.
    list(bucket: StrandLike; options: ObjectLike) -> kinds::Array<kinds::Object>;
}
