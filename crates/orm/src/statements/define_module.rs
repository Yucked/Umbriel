use std::str::FromStr;

use surrealdb::types::File;

use crate::{Error, Render, Renderer, Result, render::is_identifier};

use super::{
    DefineOptions, Statement,
    for_permission::{PermissionValue, impl_permissions},
    impl_define_options, impl_definition,
};

/// The name of a module, as `ALTER MODULE` and `REMOVE MODULE` take it, and as `INFO` keys it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleName {
    /// `mod::<name>`: a module named when it was defined.
    Module(String),
    /// `silo::<organisation>::<package>::<major.minor.patch>`: an unnamed module from a silo
    /// package.
    Silo {
        /// The package's organisation.
        organisation: String,
        /// The package.
        package: String,
        /// Its version: major, minor and patch.
        version: (u32, u32, u32),
    },
}

/// Writes `silo::<organisation>::<package>::<major.minor.patch>`, failing when either name is not
/// an identifier.
fn push_silo(out: &mut Renderer, organisation: &str, package: &str, version: (u32, u32, u32)) {
    if !is_identifier(organisation) || !is_identifier(package) {
        out.push_error(format!(
            "invalid silo package `{organisation}::{package}`: each part must be an identifier \
             ([A-Za-z_][A-Za-z0-9_]*)"
        ));
        return;
    }
    let (major, minor, patch) = version;
    out.push_str("silo::")
        .push_str(organisation)
        .push_str("::")
        .push_str(package)
        .push_str("::<")
        .push_display(&major)
        .push_str(".")
        .push_display(&minor)
        .push_str(".")
        .push_display(&patch)
        .push_str(">");
}

/// Writes `mod::<name>`, failing when the name is not an identifier.
fn push_module(out: &mut Renderer, name: &str) -> bool {
    if !is_identifier(name) {
        out.push_error(format!(
            "invalid module name `{name}`: it must be an identifier ([A-Za-z_][A-Za-z0-9_]*)"
        ));
        return false;
    }
    out.push_str("mod::").push_str(name);
    true
}

impl Render for ModuleName {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::Module(name) => {
                push_module(out, name);
            }
            Self::Silo {
                organisation,
                package,
                version,
            } => push_silo(out, organisation, package, *version),
        }
    }
}

/// Parses the name `INFO` keys a module by: `mod::<name>` or
/// `silo::<organisation>::<package>::<major.minor.patch>`.
impl FromStr for ModuleName {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        let invalid = || Error::QueryBuilder(format!("`{name}` is not a module name"));
        if let Some(module) = name.strip_prefix("mod::") {
            return Ok(Self::Module(module.to_owned()));
        }
        let silo = name.strip_prefix("silo::").ok_or_else(invalid)?;
        let (package, version) = silo.split_once("::<").ok_or_else(invalid)?;
        let (organisation, package) = package.split_once("::").ok_or_else(invalid)?;
        let mut parts = version
            .strip_suffix('>')
            .ok_or_else(invalid)?
            .split('.')
            .map(|part| part.parse::<u32>().map_err(|_| invalid()));
        let version = (
            parts.next().ok_or_else(invalid)??,
            parts.next().ok_or_else(invalid)??,
            parts.next().ok_or_else(invalid)??,
        );
        if parts.next().is_some() {
            return Err(invalid());
        }
        Ok(Self::Silo {
            organisation: organisation.to_owned(),
            package: package.to_owned(),
            version,
        })
    }
}

/// What a module is loaded from.
enum Executable {
    /// A file in a bucket.
    File(File),
    /// A silo package.
    Silo {
        organisation: String,
        package: String,
        version: (u32, u32, u32),
    },
}

/// Defines a module loaded from a file in a bucket: `DEFINE MODULE [OVERWRITE | IF NOT EXISTS]
/// mod::<name> FROM f"<bucket>:<key>" UNSIGNED [COMMENT @string] [PERMISSIONS …]`.
///
/// Modules are experimental in SurrealDB 3.3.4: the database must allow the `surrealism`
/// feature, or the statement fails to parse. No module can be signature-verified yet, so
/// `UNSIGNED` is always written, as 3.3.4 requires. An embedded engine stores the definition
/// but cannot load the module: the SDK builds without SurrealDB's own `surrealism` feature.
///
/// SurrealDB parses no escapes in a file pointer, so a bucket may only contain ASCII letters,
/// digits, `_`, `-` and `.`, and a key those and `/`; any other character is a render error.
///
/// ```rust
/// use umbriel::{*, statements::define_module_file};
///
/// let statement = define_module_file("geo", "modules", "/geo.wasm").permissions_full();
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE MODULE mod::geo FROM f\"modules:/geo.wasm\" UNSIGNED PERMISSIONS FULL;"
/// );
/// ```
pub fn define_module_file(
    name: impl Into<String>,
    bucket: impl Into<String>,
    key: impl Into<String>,
) -> DefineModuleStatement {
    DefineModuleStatement {
        name: Some(name.into()),
        executable: Executable::File(File::new(bucket, key)),
        options: DefineOptions::default(),
        permissions: None,
    }
}

/// Defines a module from a silo package: `DEFINE MODULE [OVERWRITE | IF NOT EXISTS]
/// [mod::<name> FROM] silo::<organisation>::<package>::<major.minor.patch> UNSIGNED …`. Without
/// [`name`](DefineModuleStatement::name), the module is named after the package and version.
///
/// ```rust
/// use umbriel::{*, statements::define_module_silo};
///
/// assert_eq!(
///     define_module_silo("acme", "geo", (1, 2, 3)).to_debug_sql(),
///     "DEFINE MODULE silo::acme::geo::<1.2.3> UNSIGNED;"
/// );
/// ```
pub fn define_module_silo(
    organisation: impl Into<String>,
    package: impl Into<String>,
    version: (u32, u32, u32),
) -> DefineModuleStatement {
    DefineModuleStatement {
        name: None,
        executable: Executable::Silo {
            organisation: organisation.into(),
            package: package.into(),
            version,
        },
        options: DefineOptions::default(),
        permissions: None,
    }
}

/// A `DEFINE MODULE` statement.
pub struct DefineModuleStatement {
    name: Option<String>,
    executable: Executable,
    options: DefineOptions,
    permissions: Option<PermissionValue>,
}

impl DefineModuleStatement {
    /// `mod::<name> FROM`: names a silo module. A file module is named when it is created.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl_permissions!(single: DefineModuleStatement);

impl Statement for DefineModuleStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE MODULE");
        self.options.render_kind(out);
        out.push_str(" ");
        if let Some(name) = &self.name {
            if !push_module(out, name) {
                return;
            }
            out.push_str(" FROM ");
        }
        match &self.executable {
            // SurrealDB 3.3.4 parses no escapes in a file pointer, so a bucket or key outside
            // its character set is refused rather than escaped.
            Executable::File(file) => {
                if !is_file_part(file.bucket(), false) || !is_file_part(file.key(), true) {
                    out.push_error(format!(
                        "invalid file pointer for module `{name}`: a bucket may only contain \
                         letters, digits, `_`, `-` and `.`, and a key also `/`",
                        name = self.name.as_deref().unwrap_or_default()
                    ));
                    return;
                }
                out.push_syntax(file);
            }
            Executable::Silo {
                organisation,
                package,
                version,
            } => push_silo(out, organisation, package, *version),
        }
        out.push_str(" UNSIGNED");
        self.options.render_comment(out);
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
    }
}

/// Whether `part` is a bucket name (non-empty) or, with `key`, a key, in the characters the
/// SurrealDB 3.3.4 lexer accepts in a file pointer.
fn is_file_part(part: &str, key: bool) -> bool {
    (key || !part.is_empty())
        && part.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'-' | b'.')
                || (key && byte == b'/')
        })
}

impl_define_options!(DefineModuleStatement);
impl_definition!(DefineModuleStatement);
