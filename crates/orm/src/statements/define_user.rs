use std::fmt::{self, Display};

use surrealdb::types::Value;

use crate::{Name, Renderer, User};

use super::{DefineOptions, Statement, impl_define_options, impl_definition};

/// A user's plain-text password, hashed by SurrealDB.
pub struct Password(pub(crate) String);

impl Password {
    /// The password text.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for Password {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Password {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl Display for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Define a new database user.
/// Requirements
/// You must be authenticated with a user that has enough permissions. Only the OWNER built-in role grants permissions to create users.
/// You must be authenticated with a user that has permissions on the level where you are creating the user:
/// Root users can create Root, Namespace and Database users.
/// Namespace users can create Namespace and Database users
/// Database user can create Database users.
///
/// Examples:
///
/// ```rust
/// use umbriel::{*, statements::{define_user, UserRole}};
///
/// let statement = define_user("alice")
///     .on_database()
///     .passhash("$argon2id$v=19$m=19456,t=2,p=1$u1CPdtdC0Ek5GE1gvidj/g$fjFa7PZM+4hp4hlUJN1fz/FaDAf7KY1Qu48F5m5P0V8")
///     .role(UserRole::Viewer);
///
/// let alice = User::new("alice");
/// let statement = define_user(alice).on_root().password("example-password").role(UserRole::Owner);
///
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE USER alice ON ROOT PASSWORD 'example-password' ROLES OWNER;"
/// );
/// ```
pub fn define_user(name: impl Into<Name<User>>) -> DefineUserStatement {
    DefineUserStatement {
        name: name.into(),
        options: DefineOptions::default(),
        user_type: None,
        credential: None,
        role: None,
    }
}

/// User Role
#[derive(Clone, Copy)]
pub enum UserRole {
    /// Owner
    Owner,
    /// Editor
    Editor,
    /// Viewer
    Viewer,
}

impl Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UserRole::Owner => "OWNER",
            UserRole::Editor => "EDITOR",
            UserRole::Viewer => "VIEWER",
        })
    }
}

/// The level a user is defined on.
#[derive(Clone, Copy)]
pub enum UserType {
    /// The root level
    Root,
    /// The current namespace
    Namespace,
    /// The current database
    Database,
}

impl Display for UserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UserType::Root => "ROOT",
            UserType::Namespace => "NAMESPACE",
            UserType::Database => "DATABASE",
        })
    }
}

/// An already-hashed password.
pub struct Passhash(String);

impl From<String> for Passhash {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Passhash {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl Display for Passhash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A user's login credential.
pub enum UserCredential {
    /// A plain-text password
    Password(Password),
    /// A password hash
    Passhash(Passhash),
}

/// Define a new database user.
pub struct DefineUserStatement {
    name: Name<User>,
    options: DefineOptions,
    user_type: Option<UserType>,
    credential: Option<UserCredential>,
    role: Option<UserRole>,
}

impl DefineUserStatement {
    /// Set user on root
    pub fn on_root(mut self) -> Self {
        self.user_type = Some(UserType::Root);
        self
    }

    /// Set user on namespace
    pub fn on_namespace(mut self) -> Self {
        self.user_type = Some(UserType::Namespace);
        self
    }

    /// Set user on database
    pub fn on_database(mut self) -> Self {
        self.user_type = Some(UserType::Database);
        self
    }

    /// Set the password
    pub fn password(mut self, password: impl Into<Password>) -> Self {
        self.credential = Some(UserCredential::Password(password.into()));
        self
    }

    /// Set the passhash credential
    pub fn passhash(mut self, passhash: impl Into<Passhash>) -> Self {
        self.credential = Some(UserCredential::Passhash(passhash.into()));
        self
    }

    /// Set the user role
    pub fn role(mut self, role: UserRole) -> Self {
        self.role = Some(role);
        self
    }
}

impl Statement for DefineUserStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE USER");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.name);

        if let Some(user_type) = &self.user_type {
            out.push_str(" ON ").push_display(user_type);
        }

        // SurrealQL takes the credential as a string literal; the definition inlines it.
        match &self.credential {
            Some(UserCredential::Password(password)) => {
                out.push_str(" PASSWORD ")
                    .push_value(&Value::String(password.0.clone()));
            }
            Some(UserCredential::Passhash(hash)) => {
                out.push_str(" PASSHASH ")
                    .push_value(&Value::String(hash.0.clone()));
            }
            None => {}
        }

        if let Some(role) = &self.role {
            out.push_str(" ROLES ").push_display(role);
        }

        self.options.render_comment(out);
    }
}

impl_define_options!(DefineUserStatement);
impl_definition!(DefineUserStatement);
