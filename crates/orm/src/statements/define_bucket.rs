use surrealdb::types::Value;

use crate::{Conditional, Filter, Ident, Name, Renderer};

use super::{
    DefineOptions, Statement, for_permission::PermissionValue, impl_define_options, impl_definition,
};

/// Defines a bucket, a store of files: `DEFINE BUCKET [OVERWRITE | IF NOT EXISTS] @name
/// [BACKEND @url] [PERMISSIONS NONE | FULL | WHERE @condition] [READONLY] [COMMENT @string]`.
///
/// Buckets are experimental in SurrealDB 3.3.4: the database must allow the `files` feature, or
/// the statement fails to parse. Without a backend, the bucket uses the server's global bucket.
///
/// SurrealDB connects to the backend when the bucket is defined. Redefining a bucket whose
/// backend is `memory` connects a new, empty store, so the files it held are lost; generated
/// migrations therefore define a bucket without `OVERWRITE` and refuse to change one. `INFO` shows
/// the backend URL unredacted, so a URL carrying credentials is visible to anyone who can read
/// the schema, and generated migrations refuse to write one.
///
/// ```rust
/// use umbriel::{*, statements::define_bucket};
///
/// let statement = define_bucket("avatars").backend("memory").readonly().permissions_full();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE BUCKET avatars BACKEND 'memory' PERMISSIONS FULL READONLY;"
/// );
/// ```
pub fn define_bucket(name: impl Into<Name<Ident>>) -> DefineBucketStatement {
    DefineBucketStatement {
        name: name.into(),
        options: DefineOptions::default(),
        backend: None,
        readonly: false,
        permissions: None,
    }
}

/// A `DEFINE BUCKET` statement.
pub struct DefineBucketStatement {
    name: Name<Ident>,
    options: DefineOptions,
    backend: Option<String>,
    readonly: bool,
    permissions: Option<PermissionValue>,
}

impl DefineBucketStatement {
    /// `BACKEND @url`: where the files are stored. SurrealDB 3.3.4 connects to it when the bucket
    /// is defined, and fails the statement for a URL no backend accepts. The backends
    /// (`surrealdb-core` `buc/mod.rs`, `buc/store/*.rs`):
    ///
    /// | URL | Store | Restrictions |
    /// |---|---|---|
    /// | `memory`, `memory://` | In the server's memory | Lost on restart and on redefinition; a `READONLY` memory bucket starts empty and stays so |
    /// | `file:/path` | A server directory | The path must be absolute and inside the server's `bucket_folder_allowlist`, which is empty by default, so every path is refused until it is set. Defining the bucket creates the directory and its parents when missing, and fails when the path is not a directory. A key may not escape the directory. `?lowercase_paths` folds keys to lower case |
    /// | `s3:/bucket`, `s3://endpoint/bucket`, `s3+http://…`, `s3+https://…` | S3 or an S3-compatible service | A host is a custom endpoint (MinIO, R2, …), and `s3+http`/`s3+https` choose its protocol; the first path segment is the bucket. `region`, `prefix`; credentials as `access_key` and `secret_key` |
    /// | `gs://bucket`, `gcs://bucket` | Google Cloud Storage | `prefix`; `service_account` (a file path) or `service_account_key`; `anonymous` |
    /// | `az://account/container`, `azure://…` | Azure Blob Storage | `prefix`; credentials as `access_key` or `sas_token`; `anonymous`, `use_emulator` |
    /// | none | The server's global bucket, under a prefix of the namespace and database ids and the bucket name | Needs the server's `global_bucket` setting. With `global_bucket_enforced`, every bucket must use it and a `BACKEND` fails |
    ///
    /// Only `memory` is available on `wasm32`. Credentials in the URL are stored and shown by
    /// `INFO` as written, so generated migrations refuse a backend carrying user info, a
    /// fragment, `access_key`, `secret_key`, `service_account_key`, `sas_token` or any parameter
    /// not listed above.
    pub fn backend(mut self, url: impl Into<String>) -> Self {
        self.backend = Some(url.into());
        self
    }

    /// `READONLY`: files can be read but not written.
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// `PERMISSIONS NONE`.
    pub fn permissions_none(mut self) -> Self {
        self.permissions = Some(PermissionValue::None);
        self
    }

    /// `PERMISSIONS FULL`.
    pub fn permissions_full(mut self) -> Self {
        self.permissions = Some(PermissionValue::Full);
        self
    }

    /// `PERMISSIONS WHERE @condition`.
    pub fn permissions_where(mut self, condition: impl Conditional) -> Self {
        self.permissions = Some(PermissionValue::Where(Filter::new(condition)));
        self
    }
}

impl Statement for DefineBucketStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE BUCKET");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.name);
        if let Some(backend) = &self.backend {
            out.push_str(" BACKEND ")
                .push_value(&Value::String(backend.clone()));
        }
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
        if self.readonly {
            out.push_str(" READONLY");
        }
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineBucketStatement);
impl_definition!(DefineBucketStatement);
