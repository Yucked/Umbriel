use std::fmt::{self, Display};

use surrealdb::types::Value;

use crate::{
    Conditional, DurationLike, Filter, ObjectLike, Render, Renderer, StrandLike, ValueLike,
    render::{is_identifier, push_path},
};

use super::{
    DefineOptions, Statement, for_permission::PermissionValue, impl_define_options, impl_definition,
};

/// An HTTP method an API action answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiMethod {
    /// `DELETE`
    Delete,
    /// `GET`
    Get,
    /// `PATCH`
    Patch,
    /// `POST`
    Post,
    /// `PUT`
    Put,
    /// `TRACE`
    Trace,
}

impl Display for ApiMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Delete => "DELETE",
            Self::Get => "GET",
            Self::Patch => "PATCH",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Trace => "TRACE",
        })
    }
}

/// A middleware call: a function name and its arguments.
#[derive(Debug, Clone)]
struct Middleware {
    name: String,
    args: Vec<ValueLike>,
}

impl Render for Middleware {
    fn render(&self, out: &mut Renderer) {
        match self.name.split_once("::") {
            Some((prefix, path)) if is_identifier(prefix) => {
                if !push_path(out, prefix, path) {
                    return;
                }
            }
            _ => {
                out.push_error(format!(
                    "invalid middleware name `{}`: it must be a function path such as \
                     `api::timeout` or `fn::check`",
                    self.name
                ));
                return;
            }
        }
        out.push_str("(").push(&self.args).push_str(")");
    }
}

/// How an API handles requests: `[PERMISSIONS NONE | FULL | WHERE @condition]
/// [MIDDLEWARE @function(@args), …]`. Used by [`define_api`]'s `FOR ANY` and actions, by
/// [`alter_api`](super::alter_api()), and by [`define_config_api`](super::define_config_api()),
/// which applies it to every API of the database.
///
/// ```rust
/// use std::time::Duration;
/// use umbriel::{*, statements::api_config};
///
/// let config = api_config().middleware("api::timeout", [Duration::from_secs(1)]).permissions_full();
/// # let _ = config;
/// ```
#[derive(Debug, Clone, Default)]
pub struct ApiConfig {
    middleware: Vec<Middleware>,
    permissions: Option<PermissionValue>,
}

/// An empty [`ApiConfig`].
pub fn api_config() -> ApiConfig {
    ApiConfig::default()
}

impl ApiConfig {
    /// `MIDDLEWARE @name(@args)`: a function each request passes through, such as
    /// `api::timeout` or a custom `fn::…`. The name is a `::`-separated path of identifiers, or
    /// the query fails when it is rendered. Further calls add middleware after the earlier ones.
    pub fn middleware<A: Into<ValueLike>>(
        mut self,
        name: impl Into<String>,
        args: impl IntoIterator<Item = A>,
    ) -> Self {
        self.middleware.push(Middleware {
            name: name.into(),
            args: args.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// `MIDDLEWARE api::timeout(@timeout)`: fails a request that takes longer.
    pub fn timeout(self, timeout: impl Into<DurationLike>) -> Self {
        self.middleware("api::timeout", [ValueLike::from(timeout.into())])
    }

    /// `MIDDLEWARE api::req::body(@strategy)`: parses the request body into `$request.body`.
    pub fn req_body(self, strategy: BodyStrategy) -> Self {
        self.middleware("api::req::body", [strategy.value()])
    }

    /// `MIDDLEWARE api::req::max_body(@bytes)`: refuses a request body larger than `bytes`.
    pub fn req_max_body(self, bytes: u32) -> Self {
        self.middleware("api::req::max_body", [ValueLike::value(i64::from(bytes))])
    }

    /// `MIDDLEWARE api::res::body(@strategy)`: writes the response body in this format.
    pub fn res_body(self, strategy: BodyStrategy) -> Self {
        self.middleware("api::res::body", [strategy.value()])
    }

    /// `MIDDLEWARE api::res::status(@status)`: sets the response status.
    pub fn res_status(self, status: u16) -> Self {
        self.middleware("api::res::status", [ValueLike::value(i64::from(status))])
    }

    /// `MIDDLEWARE api::res::header(@name, @value)`: sets a response header.
    pub fn res_header(self, name: impl Into<StrandLike>, value: impl Into<StrandLike>) -> Self {
        self.middleware(
            "api::res::header",
            [ValueLike::from(name.into()), ValueLike::from(value.into())],
        )
    }

    /// `MIDDLEWARE api::res::header(@name)`: removes a response header.
    pub fn drop_res_header(self, name: impl Into<StrandLike>) -> Self {
        self.middleware("api::res::header", [ValueLike::from(name.into())])
    }

    /// `MIDDLEWARE api::res::headers(@headers)`: sets several response headers; a header whose
    /// value is `NONE` is removed.
    pub fn res_headers(self, headers: impl Into<ObjectLike>) -> Self {
        self.middleware("api::res::headers", [ValueLike::from(headers.into())])
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

/// The format `api::req::body` parses a request body from, or `api::res::body` writes a
/// response body in. `Auto` follows the request's `Content-Type` or `Accept` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BodyStrategy {
    /// `auto`
    Auto,
    /// `json`
    Json,
    /// `cbor`
    Cbor,
    /// `flatbuffers`
    Flatbuffers,
    /// `plain`: UTF-8 text.
    Plain,
    /// `bytes`
    Bytes,
    /// `native`: SurrealDB's own format.
    Native,
}

impl BodyStrategy {
    /// The name SurrealDB reads, as a string value.
    fn value(self) -> ValueLike {
        ValueLike::value(
            match self {
                Self::Auto => "auto",
                Self::Json => "json",
                Self::Cbor => "cbor",
                Self::Flatbuffers => "flatbuffers",
                Self::Plain => "plain",
                Self::Bytes => "bytes",
                Self::Native => "native",
            }
            .to_owned(),
        )
    }
}

impl Render for ApiConfig {
    fn render(&self, out: &mut Renderer) {
        if let Some(permissions) = &self.permissions {
            out.push(permissions);
        }
        if !self.middleware.is_empty() {
            out.push_str(" MIDDLEWARE ").push(&self.middleware);
        }
    }
}

/// What an API does for some methods: `FOR @methods [@config] THEN @action`.
///
/// ```rust
/// use umbriel::{*, statements::{ApiMethod, api_action, api_config}};
///
/// let action = api_action([ApiMethod::Get, ApiMethod::Post], "ok")
///     .config(api_config().permissions_full());
/// # let _ = action;
/// ```
pub fn api_action(
    methods: impl IntoIterator<Item = ApiMethod>,
    action: impl Into<ValueLike>,
) -> ApiAction {
    ApiAction {
        methods: methods.into_iter().collect(),
        config: ApiConfig::default(),
        action: action.into(),
    }
}

/// An action of [`define_api`] or [`alter_api`](super::alter_api()). See [`api_action`].
#[derive(Debug, Clone)]
pub struct ApiAction {
    pub(crate) methods: Vec<ApiMethod>,
    config: ApiConfig,
    action: ValueLike,
}

impl ApiAction {
    /// The middleware and permissions of the action's requests.
    pub fn config(mut self, config: ApiConfig) -> Self {
        self.config = config;
        self
    }
}

/// Writes ` FOR @methods`, failing when there are none.
pub(crate) fn push_methods(out: &mut Renderer, methods: &[ApiMethod]) {
    if methods.is_empty() {
        out.push_error("an API action needs at least one method");
        return;
    }
    out.push_str(" FOR ");
    for (index, method) in methods.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_display(method);
    }
}

impl Render for ApiAction {
    fn render(&self, out: &mut Renderer) {
        push_methods(out, &self.methods);
        out.push(&self.config).push_str(" THEN ").push(&self.action);
    }
}

/// Writes an API's path, a string literal.
pub(crate) fn push_api_path(out: &mut Renderer, path: &str) {
    out.push_value(&Value::String(path.to_owned()));
}

/// Defines an API endpoint: `DEFINE API [OVERWRITE | IF NOT EXISTS] @path [FOR ANY [@config]
/// [THEN @fallback]] [FOR @methods [@config] THEN @action …] [COMMENT @string]`.
///
/// [`for_any`](DefineApiStatement::for_any) configures every method, and
/// [`fallback`](DefineApiStatement::fallback) answers the methods no action names.
///
/// ```rust
/// use umbriel::{*, statements::{ApiMethod, api_action, api_config, define_api}};
///
/// let statement = define_api("/status")
///     .for_any(api_config().permissions_full())
///     .action(api_action([ApiMethod::Get], "ok"));
/// assert_eq!(
///     statement.to_debug_sql(),
///     "DEFINE API '/status' FOR ANY PERMISSIONS FULL FOR GET THEN 'ok';"
/// );
/// ```
pub fn define_api(path: impl Into<String>) -> DefineApiStatement {
    DefineApiStatement {
        path: path.into(),
        options: DefineOptions::default(),
        any: None,
        fallback: None,
        actions: Vec::new(),
    }
}

/// A `DEFINE API` statement.
pub struct DefineApiStatement {
    path: String,
    options: DefineOptions,
    any: Option<ApiConfig>,
    fallback: Option<ValueLike>,
    actions: Vec<ApiAction>,
}

impl DefineApiStatement {
    /// `FOR ANY @config`: the middleware and permissions of every request.
    pub fn for_any(mut self, config: ApiConfig) -> Self {
        self.any = Some(config);
        self
    }

    /// `FOR ANY … THEN @fallback`: the answer to a method no action names.
    pub fn fallback(mut self, fallback: impl Into<ValueLike>) -> Self {
        self.fallback = Some(fallback.into());
        self
    }

    /// `FOR @methods … THEN @action`. Further calls add actions after the earlier ones.
    pub fn action(mut self, action: ApiAction) -> Self {
        self.actions.push(action);
        self
    }
}

impl Statement for DefineApiStatement {
    fn render_body(&self, out: &mut Renderer) {
        out.push_str("DEFINE API");
        self.options.render_kind(out);
        out.push_str(" ");
        push_api_path(out, &self.path);
        if self.any.is_some() || self.fallback.is_some() {
            out.push_str(" FOR ANY").push(&self.any);
            if let Some(fallback) = &self.fallback {
                out.push_str(" THEN ").push(fallback);
            }
        }
        for action in &self.actions {
            out.push(action);
        }
        self.options.render_comment(out);
    }
}

impl_define_options!(DefineApiStatement);
impl_definition!(DefineApiStatement);
