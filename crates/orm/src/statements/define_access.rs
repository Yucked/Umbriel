// Statement syntax (the subset Umbriel covers so far)
// DEFINE ACCESS @name ON [ ROOT | NAMESPACE | DATABASE ]
// 	TYPE RECORD
// 		[ SIGNUP @expression ]
// 		[ SIGNIN @expression ]
// 		[ WITH JWT ALGORITHM @algorithm KEY @key ]
// 		[ DURATION [ FOR TOKEN @duration ] [, FOR SESSION @duration ] ]
// 	| TYPE JWT [ ALGORITHM @algorithm KEY @key | URL @url ]
// 		[ DURATION FOR SESSION @duration ]
//
// Record access is defined on a database only.
use crate::{
    Access, AccessLevel, DurationLike, JwtAlgorithm, Name, Raw, Render, Renderer, StrandLike,
};

use super::{DefineOptions, Statement, Subquery, impl_define_options, impl_definition};

/// A `SIGNUP` or `SIGNIN` expression of record access: a statement, or SurrealQL text.
pub enum AccessExpression {
    /// A statement, rendered as a subquery.
    Subquery(Subquery),
    /// SurrealQL text, rendered in parentheses.
    Raw(Raw),
}

impl<T: Into<Subquery>> From<T> for AccessExpression {
    fn from(value: T) -> Self {
        Self::Subquery(value.into())
    }
}

impl From<Raw> for AccessExpression {
    fn from(value: Raw) -> Self {
        Self::Raw(value)
    }
}

impl Render for AccessExpression {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::Subquery(subquery) => {
                out.push(subquery);
            }
            Self::Raw(raw) => {
                out.push_str("(").push(raw).push_str(")");
            }
        }
    }
}

/// Defines an access method: record access with sign-up and sign-in, or verification of JWTs
/// issued elsewhere.
///
/// Record access replaces `DEFINE SCOPE` and the scope tokens of `DEFINE TOKEN … ON SCOPE`; JWT
/// access replaces namespace and database tokens.
///
/// # Example
///
/// ```rust
/// use umbriel::{*, statements::{define_access, jwt_access, record_access}};
///
/// let signup = Raw::new("CREATE user SET email = $email, pass = crypto::argon2::generate($pass)");
/// let signin = Raw::new(
///     "SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)",
/// );
/// let account = define_access("account").on_database().record(
///     record_access()
///         .signup(signup)
///         .signin(signin)
///         .with_jwt(JwtAlgorithm::HS512, "example-key")
///         .duration_for_token(std::time::Duration::from_secs(15 * 60))
///         .duration_for_session(std::time::Duration::from_secs(12 * 60 * 60)),
/// );
/// assert_eq!(
///     account.to_debug_sql(),
///     "DEFINE ACCESS account ON DATABASE TYPE RECORD \
///      SIGNUP (CREATE user SET email = $email, pass = crypto::argon2::generate($pass)) \
///      SIGNIN (SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)) \
///      WITH JWT ALGORITHM HS512 KEY 'example-key' \
///      DURATION FOR TOKEN 15m, FOR SESSION 12h;"
/// );
///
/// let api = define_access("api")
///     .on_namespace()
///     .jwt(jwt_access().algorithm(JwtAlgorithm::RS256).key("example-public-key"));
/// assert_eq!(
///     api.to_debug_sql(),
///     "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key';"
/// );
///
/// // Record access exists on databases only.
/// let invalid = define_access("account").on_root().record(record_access());
/// assert!(invalid.to_query().is_err());
/// ```
pub fn define_access(name: impl Into<Name<Access>>) -> DefineAccessStatement {
    DefineAccessStatement {
        name: name.into(),
        options: DefineOptions::default(),
        level: None,
        kind: None,
    }
}

/// A `DEFINE ACCESS` statement.
pub struct DefineAccessStatement {
    name: Name<Access>,
    options: DefineOptions,
    level: Option<AccessLevel>,
    kind: Option<AccessKind>,
}

enum AccessKind {
    Record(RecordAccess),
    Jwt(JwtAccess),
}

impl DefineAccessStatement {
    /// Defines the access method on the root.
    pub fn on_root(mut self) -> Self {
        self.level = Some(AccessLevel::Root);
        self
    }

    /// Defines the access method on the current namespace.
    pub fn on_namespace(mut self) -> Self {
        self.level = Some(AccessLevel::Namespace);
        self
    }

    /// Defines the access method on the current database.
    pub fn on_database(mut self) -> Self {
        self.level = Some(AccessLevel::Database);
        self
    }

    /// Record access (`TYPE RECORD`); valid on a database only.
    pub fn record(mut self, access: RecordAccess) -> Self {
        self.kind = Some(AccessKind::Record(access));
        self
    }

    /// JWT access (`TYPE JWT`).
    pub fn jwt(mut self, access: JwtAccess) -> Self {
        self.kind = Some(AccessKind::Jwt(access));
        self
    }
}

impl Statement for DefineAccessStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE ACCESS");
        self.options.render_kind(out);
        out.push_str(" ").push(&self.name);

        match self.level {
            Some(level) => {
                out.push_str(" ON ").push(&level);
            }
            None => {
                out.push_error(
                    "DEFINE ACCESS needs a level: `on_root`, `on_namespace` or `on_database`",
                );
            }
        }

        match &self.kind {
            Some(AccessKind::Record(record)) => {
                if self
                    .level
                    .is_some_and(|level| level != AccessLevel::Database)
                {
                    out.push_error("record access can only be defined on a database");
                }
                out.push(record);
            }
            Some(AccessKind::Jwt(jwt)) => {
                out.push(jwt);
            }
            None => {
                out.push_error("DEFINE ACCESS needs a type: `record` or `jwt`");
            }
        }

        self.options.render_comment(out);
    }
}

impl_define_options!(DefineAccessStatement);
impl_definition!(DefineAccessStatement);

/// Starts record access options: sign-up, sign-in, token signing and durations.
pub fn record_access() -> RecordAccess {
    RecordAccess {
        signup: None,
        signin: None,
        jwt: None,
        token_duration: None,
        session_duration: None,
    }
}

/// The options of record access (`TYPE RECORD`).
pub struct RecordAccess {
    signup: Option<AccessExpression>,
    signin: Option<AccessExpression>,
    jwt: Option<(JwtAlgorithm, StrandLike)>,
    token_duration: Option<DurationLike>,
    session_duration: Option<DurationLike>,
}

impl RecordAccess {
    /// The expression that creates a record user when signing up.
    pub fn signup(mut self, expression: impl Into<AccessExpression>) -> Self {
        self.signup = Some(expression.into());
        self
    }

    /// The expression that finds the record user when signing in.
    pub fn signin(mut self, expression: impl Into<AccessExpression>) -> Self {
        self.signin = Some(expression.into());
        self
    }

    /// Verifies tokens with this algorithm and key (`WITH JWT ALGORITHM … KEY …`).
    ///
    /// With a symmetric algorithm (`HS256`, `HS384`, `HS512`) SurrealDB also signs the tokens it
    /// issues on sign-up and sign-in with the same key. An asymmetric algorithm (such as `RS256`)
    /// only configures verification: issuing tokens then needs `WITH ISSUER KEY`, which this
    /// builder does not cover yet.
    pub fn with_jwt(mut self, algorithm: JwtAlgorithm, key: impl Into<StrandLike>) -> Self {
        self.jwt = Some((algorithm, key.into()));
        self
    }

    /// How long issued tokens are valid (`DURATION FOR TOKEN`).
    pub fn duration_for_token(mut self, duration: impl Into<DurationLike>) -> Self {
        self.token_duration = Some(duration.into());
        self
    }

    /// How long sessions last (`DURATION FOR SESSION`).
    pub fn duration_for_session(mut self, duration: impl Into<DurationLike>) -> Self {
        self.session_duration = Some(duration.into());
        self
    }
}

impl Render for RecordAccess {
    fn render(&self, out: &mut Renderer) {
        out.push_str(" TYPE RECORD");

        if let Some(signup) = &self.signup {
            out.push_str(" SIGNUP ").push(signup);
        }

        if let Some(signin) = &self.signin {
            out.push_str(" SIGNIN ").push(signin);
        }

        if let Some((algorithm, key)) = &self.jwt {
            out.push_str(" WITH JWT ALGORITHM ")
                .push(algorithm)
                .push_str(" KEY ")
                .push(key);
        }

        match (&self.token_duration, &self.session_duration) {
            (Some(token), Some(session)) => {
                out.push_str(" DURATION FOR TOKEN ")
                    .push(token)
                    .push_str(", FOR SESSION ")
                    .push(session);
            }
            (Some(token), None) => {
                out.push_str(" DURATION FOR TOKEN ").push(token);
            }
            (None, Some(session)) => {
                out.push_str(" DURATION FOR SESSION ").push(session);
            }
            (None, None) => {}
        }
    }
}

/// Starts JWT access options: how tokens are verified and how long sessions last.
pub fn jwt_access() -> JwtAccess {
    JwtAccess {
        algorithm: None,
        key: None,
        url: None,
        session_duration: None,
    }
}

/// The options of JWT access (`TYPE JWT`): verification with `ALGORITHM … KEY …`, or with the
/// keys published at a JWKS `URL`.
pub struct JwtAccess {
    algorithm: Option<JwtAlgorithm>,
    key: Option<StrandLike>,
    url: Option<StrandLike>,
    session_duration: Option<DurationLike>,
}

impl JwtAccess {
    /// The signature algorithm of the tokens.
    pub fn algorithm(mut self, algorithm: JwtAlgorithm) -> Self {
        self.algorithm = Some(algorithm);
        self
    }

    /// The key that verifies the tokens: a shared secret or a public key.
    pub fn key(mut self, key: impl Into<StrandLike>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// The JWKS URL publishing the keys that verify the tokens.
    pub fn url(mut self, url: impl Into<StrandLike>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// How long sessions last (`DURATION FOR SESSION`).
    pub fn duration_for_session(mut self, duration: impl Into<DurationLike>) -> Self {
        self.session_duration = Some(duration.into());
        self
    }
}

impl Render for JwtAccess {
    fn render(&self, out: &mut Renderer) {
        out.push_str(" TYPE JWT");

        match (&self.algorithm, &self.key, &self.url) {
            (Some(algorithm), Some(key), None) => {
                out.push_str(" ALGORITHM ")
                    .push(algorithm)
                    .push_str(" KEY ")
                    .push(key);
            }
            (None, None, Some(url)) => {
                out.push_str(" URL ").push(url);
            }
            (_, _, Some(_)) => {
                out.push_error("JWT access takes either `algorithm` and `key`, or `url`, not both");
            }
            _ => {
                out.push_error("JWT access needs `algorithm` and `key`, or `url`");
            }
        }

        if let Some(session) = &self.session_duration {
            out.push_str(" DURATION FOR SESSION ").push(session);
        }
    }
}
