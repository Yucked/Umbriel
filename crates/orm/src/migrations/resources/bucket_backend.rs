//! Bucket backends that may be written to a migration.
//!
//! SurrealDB does not redact a bucket's backend URL, so credentials in it would be copied into
//! migration files. In 3.3.4 (`surrealdb-core` `buc/store/cloud.rs`) they can be the URL's user
//! info or the `access_key`, `secret_key`, `service_account_key` and `sas_token` query
//! parameters. The check is an allowlist: only the parameters known to carry no secret pass,
//! so a parameter it does not know, however it is spelt or encoded, is refused.

/// Query parameters a backend may carry: an S3 region, a key prefix, the path of a service
/// account file (not its contents), anonymous access, the storage emulator switch and a file
/// backend's key folding (`buc/store/file.rs`).
const ALLOWED: [&str; 6] = [
    "region",
    "prefix",
    "service_account",
    "anonymous",
    "use_emulator",
    "lowercase_paths",
];

/// Query parameters that carry a secret, named in the refusal.
const CREDENTIALS: [&str; 4] = [
    "access_key",
    "secret_key",
    "service_account_key",
    "sas_token",
];

/// Why `backend` may not be written to a migration, or `None` when it may: it carries user
/// info (anything before an `@`), a fragment, or a query parameter other than the allowed ones.
/// The reason names a known credential parameter, and never repeats a value or an unknown
/// parameter, which could itself be a secret.
#[doc(hidden)]
pub fn credential_in_backend(backend: &str) -> Option<String> {
    if backend.contains('#') {
        return Some("a URL fragment".to_owned());
    }
    let (location, query) = backend.split_once('?').unwrap_or((backend, ""));
    if location.contains('@') {
        return Some("user info".to_owned());
    }
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| pair.split_once('=').map_or(pair, |(key, _)| key))
        .find(|key| !ALLOWED.contains(key))
        .map(|key| {
            if CREDENTIALS.contains(&key) {
                format!("query parameter {key}")
            } else {
                "an unrecognised query parameter".to_owned()
            }
        })
}
