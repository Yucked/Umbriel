//! `DEFINE ACCESS`. Syntax: <https://surrealdb.com/docs/surrealql/statements/define/access>,
//! checked against `parse_define_access` and `parse_jwt` in `surrealdb-syn` 3.3.4.

use std::marker::PhantomData;

use crate::{
    Access, DurationLike, JwtAlgorithm, Name, OnDatabase, OnLevel, OnNamespace, OnRoot, Raw,
    Render, Renderer, StrandLike,
};

use super::{DefineOptions, Statement, Subquery, impl_define_options, impl_definition};

/// An expression of an access method (`SIGNUP`, `SIGNIN`, `AUTHENTICATE` or `CONTEXT`): a
/// statement, or SurrealQL text.
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

/// How long a grant, token or session lasts: a duration, or `NONE` for no expiry.
pub enum Expiry {
    /// Expires after the duration.
    After(DurationLike),
    /// `NONE`: never expires.
    Never,
}

impl<T: Into<DurationLike>> From<T> for Expiry {
    fn from(duration: T) -> Self {
        Self::After(duration.into())
    }
}

impl Render for Expiry {
    fn render(&self, out: &mut Renderer) {
        match self {
            Self::After(duration) => {
                out.push(duration);
            }
            Self::Never => {
                out.push_str("NONE");
            }
        }
    }
}

/// The `DURATION FOR GRANT | TOKEN | SESSION` clause of an access method or a user, written in
/// that order and separated by commas.
#[derive(Default)]
pub(crate) struct Durations {
    pub(crate) grant: Option<Expiry>,
    pub(crate) token: Option<Expiry>,
    pub(crate) session: Option<Expiry>,
}

impl Render for Durations {
    fn render(&self, out: &mut Renderer) {
        let parts = [
            ("GRANT", &self.grant),
            ("TOKEN", &self.token),
            ("SESSION", &self.session),
        ];
        let mut set = parts
            .iter()
            .filter_map(|(name, expiry)| expiry.as_ref().map(|expiry| (*name, expiry)))
            .peekable();
        if set.peek().is_none() {
            return;
        }
        out.push_str(" DURATION ");
        out.push_joined_with(set, ", ", |out, (name, expiry)| {
            out.push_str("FOR ")
                .push_str(name)
                .push_str(" ")
                .push(expiry);
        });
    }
}

/// Defines an access method: record access with sign-up and sign-in, verification of JWTs issued
/// elsewhere, or bearer keys granted to users or records.
///
/// The level comes first, and decides which types are offered: record access, and bearer access
/// for records, exist on a database only, so they're methods of
/// `DefineAccessStatement<OnDatabase>` and don't compile on another level.
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
/// let account = define_access("account")
///     .on_database()
///     .record(
///         record_access()
///             .signup(signup)
///             .signin(signin)
///             .with_jwt(jwt_access().algorithm(JwtAlgorithm::HS512).key("example-key"))
///             .with_refresh(),
///     )
///     .duration_for_token(std::time::Duration::from_secs(15 * 60))
///     .duration_for_session(std::time::Duration::from_secs(12 * 60 * 60));
/// assert_eq!(
///     account.to_debug_sql(),
///     "DEFINE ACCESS account ON DATABASE TYPE RECORD \
///      SIGNUP (CREATE user SET email = $email, pass = crypto::argon2::generate($pass)) \
///      SIGNIN (SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)) \
///      WITH JWT ALGORITHM HS512 KEY 'example-key', WITH REFRESH \
///      DURATION FOR TOKEN 15m, FOR SESSION 12h;"
/// );
///
/// let api = define_access("api").on_namespace().jwt(
///     jwt_access()
///         .algorithm(JwtAlgorithm::RS256)
///         .key("example-public-key")
///         .audience(["acme-app"]),
/// );
/// assert_eq!(
///     api.to_debug_sql(),
///     "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key' \
///      AUDIENCE 'acme-app';"
/// );
/// ```
pub fn define_access(name: impl Into<Name<Access>>) -> DefineAccessInit {
    DefineAccessInit { name: name.into() }
}

/// A `DEFINE ACCESS` awaiting its level.
#[must_use]
pub struct DefineAccessInit {
    name: Name<Access>,
}

impl DefineAccessInit {
    fn on<L: OnLevel>(self) -> DefineAccessStatement<L> {
        DefineAccessStatement {
            name: self.name,
            options: DefineOptions::default(),
            kind: None,
            authenticate: None,
            context: None,
            durations: Durations::default(),
            level: PhantomData,
        }
    }

    /// Defines the access method on the root.
    pub fn on_root(self) -> DefineAccessStatement<OnRoot> {
        self.on()
    }

    /// Defines the access method on the current namespace.
    pub fn on_namespace(self) -> DefineAccessStatement<OnNamespace> {
        self.on()
    }

    /// Defines the access method on the current database.
    pub fn on_database(self) -> DefineAccessStatement<OnDatabase> {
        self.on()
    }
}

/// A `DEFINE ACCESS` statement on the level `L`.
#[must_use]
pub struct DefineAccessStatement<L> {
    name: Name<Access>,
    options: DefineOptions,
    kind: Option<AccessKind>,
    authenticate: Option<AccessExpression>,
    context: Option<AccessExpression>,
    durations: Durations,
    level: PhantomData<L>,
}

enum AccessKind {
    Record(RecordAccess),
    Jwt(JwtAccess),
    Bearer {
        for_record: bool,
        access: BearerAccess,
    },
}

impl<L: OnLevel> DefineAccessStatement<L> {
    /// JWT access (`TYPE JWT`): verifies tokens issued elsewhere.
    pub fn jwt(mut self, access: JwtAccess) -> Self {
        self.kind = Some(AccessKind::Jwt(access));
        self
    }

    /// Bearer access for system users (`TYPE BEARER FOR USER`): keys granted with
    /// [`access`](super::access)`(name).grant_for_user(…)`.
    pub fn bearer_for_user(mut self, access: BearerAccess) -> Self {
        self.kind = Some(AccessKind::Bearer {
            for_record: false,
            access,
        });
        self
    }

    /// `AUTHENTICATE @expression`: runs after the user is identified, and may change or refuse
    /// the identity. A `THROW` in it refuses the sign-in with its message.
    pub fn authenticate(mut self, expression: impl Into<AccessExpression>) -> Self {
        self.authenticate = Some(expression.into());
        self
    }

    /// `CONTEXT @expression` (SurrealDB 3.3): a read-only expression evaluated once when the
    /// session is authenticated, whose value the session reads as `$session.data`. SurrealDB
    /// refuses a definition whose expression could write.
    pub fn context(mut self, expression: impl Into<AccessExpression>) -> Self {
        self.context = Some(expression.into());
        self
    }

    /// `DURATION FOR GRANT`: how long a bearer grant is valid.
    pub fn duration_for_grant(mut self, expiry: impl Into<Expiry>) -> Self {
        self.durations.grant = Some(expiry.into());
        self
    }

    /// `DURATION FOR TOKEN`: how long an issued token is valid. Record access refuses `NONE`.
    pub fn duration_for_token(mut self, expiry: impl Into<Expiry>) -> Self {
        self.durations.token = Some(expiry.into());
        self
    }

    /// `DURATION FOR SESSION`: how long an authenticated session lasts.
    pub fn duration_for_session(mut self, expiry: impl Into<Expiry>) -> Self {
        self.durations.session = Some(expiry.into());
        self
    }
}

impl DefineAccessStatement<OnDatabase> {
    /// Record access (`TYPE RECORD`): record users sign up and sign in with the database's own
    /// records.
    pub fn record(mut self, access: RecordAccess) -> Self {
        self.kind = Some(AccessKind::Record(access));
        self
    }

    /// Bearer access for record users (`TYPE BEARER FOR RECORD`): keys granted with
    /// [`access`](super::access)`(name).grant_for_record(…)`.
    pub fn bearer_for_record(mut self, access: BearerAccess) -> Self {
        self.kind = Some(AccessKind::Bearer {
            for_record: true,
            access,
        });
        self
    }
}

impl<L: OnLevel> Statement for DefineAccessStatement<L> {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE ACCESS");
        self.options.render_kind(out);
        out.push_str(" ")
            .push(&self.name)
            .push_str(" ON ")
            .push(&L::LEVEL);

        match &self.kind {
            Some(AccessKind::Record(record)) => {
                out.push(record);
            }
            Some(AccessKind::Jwt(jwt)) => {
                out.push_str(" TYPE JWT").push(jwt);
            }
            Some(AccessKind::Bearer { for_record, access }) => {
                out.push_str(if *for_record {
                    " TYPE BEARER FOR RECORD"
                } else {
                    " TYPE BEARER FOR USER"
                })
                .push(access);
            }
            None => {
                out.push_error(
                    "DEFINE ACCESS needs a type: `record`, `jwt`, `bearer_for_user` or \
                     `bearer_for_record`",
                );
            }
        }

        if let Some(authenticate) = &self.authenticate {
            out.push_str(" AUTHENTICATE ").push(authenticate);
        }
        if let Some(context) = &self.context {
            out.push_str(" CONTEXT ").push(context);
        }
        out.push(&self.durations);
        self.options.render_comment(out);
    }
}

impl_define_options!([L: OnLevel] DefineAccessStatement<L>);
impl_definition!([L: OnLevel] DefineAccessStatement<L>);

/// Starts record access options: sign-up, sign-in, token signing and refresh tokens.
pub fn record_access() -> RecordAccess {
    RecordAccess {
        signup: None,
        signin: None,
        jwt: None,
        refresh: false,
    }
}

/// The options of record access (`TYPE RECORD`).
#[must_use]
pub struct RecordAccess {
    signup: Option<AccessExpression>,
    signin: Option<AccessExpression>,
    jwt: Option<JwtAccess>,
    refresh: bool,
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

    /// `WITH JWT …`: how issued tokens are signed and verified. With a symmetric algorithm
    /// (`HS256`, `HS384`, `HS512`) the key both signs and verifies. An asymmetric one (such as
    /// `RS256`) verifies only, and issuing tokens needs
    /// [`JwtAccess::with_issuer_key`]. An [`audience`](JwtAccess::audience) is signed into
    /// issued tokens and required of verified ones.
    pub fn with_jwt(mut self, jwt: JwtAccess) -> Self {
        self.jwt = Some(jwt);
        self
    }

    /// `WITH REFRESH`: sign-in also issues a refresh token, which `authenticate` exchanges for a
    /// new token and refresh token.
    pub fn with_refresh(mut self) -> Self {
        self.refresh = true;
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
        if let Some(jwt) = &self.jwt {
            out.push_str(" WITH JWT").push(jwt);
        }
        if self.refresh {
            out.push_str(if self.jwt.is_some() {
                ", WITH REFRESH"
            } else {
                " WITH REFRESH"
            });
        }
    }
}

/// Starts bearer access options.
pub fn bearer_access() -> BearerAccess {
    BearerAccess { jwt: None }
}

/// The options of bearer access (`TYPE BEARER`).
#[must_use]
pub struct BearerAccess {
    jwt: Option<JwtAccess>,
}

impl BearerAccess {
    /// `WITH JWT …`: how the tokens a bearer key signs in to are signed and verified.
    pub fn with_jwt(mut self, jwt: JwtAccess) -> Self {
        self.jwt = Some(jwt);
        self
    }
}

impl Render for BearerAccess {
    fn render(&self, out: &mut Renderer) {
        if let Some(jwt) = &self.jwt {
            out.push_str(" WITH JWT").push(jwt);
        }
    }
}

/// Starts JWT options: how tokens are verified, which audiences they must name, and how tokens
/// are issued.
pub fn jwt_access() -> JwtAccess {
    JwtAccess {
        algorithm: None,
        key: None,
        url: None,
        audience: Vec::new(),
        issuer: None,
    }
}

/// JWT options, for `TYPE JWT` and the `WITH JWT` of record and bearer access: verification with
/// `ALGORITHM … KEY …` or the keys published at a JWKS `URL`, then `AUDIENCE` and
/// `WITH ISSUER`.
#[must_use]
pub struct JwtAccess {
    algorithm: Option<JwtAlgorithm>,
    key: Option<StrandLike>,
    url: Option<StrandLike>,
    audience: Vec<StrandLike>,
    issuer: Option<(Option<JwtAlgorithm>, StrandLike)>,
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

    /// The JWKS URL publishing the keys that verify the tokens. SurrealDB fetches it only with
    /// the `jwks` capability and network access to the URL.
    pub fn url(mut self, url: impl Into<StrandLike>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// `AUDIENCE @audience, …` (SurrealDB 3.3): a verified token's `aud` claim must name one of
    /// these, and tokens SurrealDB issues carry them. Repeated calls add audiences.
    pub fn audience<A: Into<StrandLike>>(mut self, audience: impl IntoIterator<Item = A>) -> Self {
        self.audience.extend(audience.into_iter().map(Into::into));
        self
    }

    /// `WITH ISSUER KEY @key`: the key SurrealDB signs issued tokens with, under the verifying
    /// algorithm. Needed to issue tokens with an asymmetric algorithm, whose verifying key is
    /// public.
    pub fn with_issuer_key(mut self, key: impl Into<StrandLike>) -> Self {
        self.issuer = Some((None, key.into()));
        self
    }

    /// `WITH ISSUER ALGORITHM @algorithm KEY @key`: as
    /// [`with_issuer_key`](Self::with_issuer_key), naming the algorithm, which tokens verified
    /// through a JWKS `URL` need. Verifying with a key and issuing with another algorithm is
    /// refused when the query is rendered, as SurrealDB refuses it.
    pub fn with_issuer(mut self, algorithm: JwtAlgorithm, key: impl Into<StrandLike>) -> Self {
        self.issuer = Some((Some(algorithm), key.into()));
        self
    }
}

impl Render for JwtAccess {
    fn render(&self, out: &mut Renderer) {
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

        if !self.audience.is_empty() {
            out.push_str(" AUDIENCE ").push_joined(&self.audience, ", ");
        }

        if let Some((algorithm, key)) = &self.issuer {
            out.push_str(" WITH ISSUER");
            if let Some(algorithm) = algorithm {
                if self
                    .algorithm
                    .is_some_and(|verifying| verifying != *algorithm)
                {
                    out.push_error(
                        "the issuer's algorithm must be the verifying algorithm; leave it out \
                         with `with_issuer_key`",
                    );
                }
                out.push_str(" ALGORITHM ").push(algorithm);
            }
            out.push_str(" KEY ").push(key);
        }
    }
}
