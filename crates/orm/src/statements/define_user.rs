use std::fmt::{self, Display};

use surrealdb::types::Value;

use crate::{Level, Name, Renderer, User};

use super::{
    DefineOptions, Statement,
    define_access::{Durations, Expiry},
    impl_define_options, impl_definition,
};

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
/// let statement = define_user(alice)
///     .on_root()
///     .password("example-password")
///     .role(UserRole::Owner)
///     .duration_for_session(std::time::Duration::from_secs(60 * 60));
///
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE USER alice ON ROOT PASSWORD 'example-password' ROLES OWNER DURATION FOR SESSION 1h;"
/// );
/// ```
pub fn define_user(name: impl Into<Name<User>>) -> DefineUserInit {
    DefineUserInit(name.into())
}

/// A `DEFINE USER` awaiting its level.
#[must_use]
pub struct DefineUserInit(Name<User>);

impl DefineUserInit {
    fn on(self, level: Level) -> DefineUserStatement {
        DefineUserStatement {
            name: self.0,
            options: DefineOptions::default(),
            level,
            credential: None,
            roles: Vec::new(),
            durations: Durations::default(),
        }
    }

    /// Defines the user on the root.
    pub fn on_root(self) -> DefineUserStatement {
        self.on(Level::Root)
    }

    /// Defines the user on the current namespace.
    pub fn on_namespace(self) -> DefineUserStatement {
        self.on(Level::Namespace)
    }

    /// Defines the user on the current database.
    pub fn on_database(self) -> DefineUserStatement {
        self.on(Level::Database)
    }
}

/// A system user's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// An already-hashed password.
pub struct Passhash(pub(crate) String);

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

impl UserCredential {
    /// Writes ` PASSWORD '…'` or ` PASSHASH '…'`. SurrealQL takes the credential as a string
    /// literal, so it's inlined.
    pub(crate) fn render(&self, out: &mut Renderer) {
        let (keyword, text) = match self {
            UserCredential::Password(password) => (" PASSWORD ", &password.0),
            UserCredential::Passhash(hash) => (" PASSHASH ", &hash.0),
        };
        out.push_str(keyword)
            .push_value(&Value::String(text.clone()));
    }
}

/// Writes ` ROLES a, b`, or nothing for no roles.
pub(crate) fn push_roles(out: &mut Renderer, roles: &[UserRole]) {
    if roles.is_empty() {
        return;
    }
    out.push_str(" ROLES ")
        .push_joined_with(roles, ", ", |out, role| {
            out.push_display(role);
        });
}

/// A `DEFINE USER` statement.
#[must_use]
pub struct DefineUserStatement {
    name: Name<User>,
    options: DefineOptions,
    level: Level,
    credential: Option<UserCredential>,
    roles: Vec<UserRole>,
    durations: Durations,
}

impl DefineUserStatement {
    /// `PASSWORD`: a plain-text password, which SurrealDB hashes.
    pub fn password(mut self, password: impl Into<Password>) -> Self {
        self.credential = Some(UserCredential::Password(password.into()));
        self
    }

    /// `PASSHASH`: an already-hashed password.
    pub fn passhash(mut self, passhash: impl Into<Passhash>) -> Self {
        self.credential = Some(UserCredential::Passhash(passhash.into()));
        self
    }

    /// `ROLES`: adds a role; repeated calls add roles.
    pub fn role(mut self, role: UserRole) -> Self {
        self.roles.push(role);
        self
    }

    /// `DURATION FOR TOKEN`: how long the user's tokens are valid, or [`Expiry::Never`].
    pub fn duration_for_token(mut self, expiry: impl Into<Expiry>) -> Self {
        self.durations.token = Some(expiry.into());
        self
    }

    /// `DURATION FOR SESSION`: how long the user's sessions last, or [`Expiry::Never`].
    pub fn duration_for_session(mut self, expiry: impl Into<Expiry>) -> Self {
        self.durations.session = Some(expiry.into());
        self
    }
}

impl Statement for DefineUserStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE USER");
        self.options.render_kind(out);
        out.push_str(" ")
            .push(&self.name)
            .push_str(" ON ")
            .push(&self.level);
        if let Some(credential) = &self.credential {
            credential.render(out);
        }
        push_roles(out, &self.roles);
        out.push(&self.durations);
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineUserStatement);
impl_definition!(DefineUserStatement);
