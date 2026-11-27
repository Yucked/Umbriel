use surrealdb::{opt::auth::Record, types::SurrealValue};

use crate::{OnDatabase, statements::DefineAccessStatement};

/// A record access method, with the parameters its sign-up and sign-in take as types.
///
/// [`signup`](Self::signup) and [`signin`](Self::signin) build the SDK's
/// [`Record`](surrealdb::opt::auth::Record) credentials for this method, so a call names the
/// method once, where it's defined, and passes parameters of the declared types. Sign up and
/// sign in with the SDK's own `Surreal::signup` and `Surreal::signin`.
///
/// Boundary: nothing checks that the `$params` the `SIGNUP` and `SIGNIN` expressions read are
/// the fields of [`Signup`](Self::Signup) and [`Signin`](Self::Signin); a missing one is `NONE`
/// in the expression.
///
/// ```rust
/// use umbriel::{*, statements::{DefineAccessStatement, define_access, jwt_access, record_access}};
/// use umbriel::types::SurrealValue;
///
/// #[derive(SurrealValue)]
/// #[surreal(crate = "umbriel::types")]
/// struct Credentials {
///     email: String,
///     pass: String,
/// }
///
/// struct Account;
///
/// impl RecordAccessDefinition for Account {
///     const NAME: &'static str = "account";
///     type Signup = Credentials;
///     type Signin = Credentials;
///
///     fn define() -> DefineAccessStatement<OnDatabase> {
///         define_access(Self::NAME).on_database().record(
///             record_access()
///                 .signup(Raw::new("CREATE user SET email = $email, pass = crypto::argon2::generate($pass)"))
///                 .signin(Raw::new(
///                     "SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)",
///                 ))
///                 .with_jwt(jwt_access().algorithm(JwtAlgorithm::HS512).key("example-key")),
///         )
///     }
/// }
///
/// let credentials = Account::signin(
///     "acme",
///     "app",
///     Credentials { email: "alice@example.com".into(), pass: "example-password".into() },
/// );
/// assert_eq!(credentials.access, "account");
/// ```
pub trait RecordAccessDefinition {
    /// The access method's name.
    const NAME: &'static str;

    /// The parameters sign-up takes, flattened into the credentials.
    type Signup: SurrealValue;

    /// The parameters sign-in takes, flattened into the credentials.
    type Signin: SurrealValue;

    /// The method's `DEFINE ACCESS … TYPE RECORD`, named [`NAME`](Self::NAME).
    fn define() -> DefineAccessStatement<OnDatabase>;

    /// Sign-up credentials for this method on `namespace` and `database`.
    fn signup(namespace: &str, database: &str, params: Self::Signup) -> Record<Self::Signup> {
        Record {
            namespace: namespace.to_owned(),
            database: database.to_owned(),
            access: Self::NAME.to_owned(),
            params,
        }
    }

    /// Sign-in credentials for this method on `namespace` and `database`.
    fn signin(namespace: &str, database: &str, params: Self::Signin) -> Record<Self::Signin> {
        Record {
            namespace: namespace.to_owned(),
            database: database.to_owned(),
            access: Self::NAME.to_owned(),
            params,
        }
    }
}
