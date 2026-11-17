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
    /// `BACKEND @url`: where the files are stored, such as `memory`, `file:/data/avatars` or an
    /// object store URL.
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
