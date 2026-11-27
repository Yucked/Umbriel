use crate::keywords::keywords;

keywords! {
    /// A JWT signature algorithm.
    #[expect(missing_docs, reason = "each variant is the algorithm's own name")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum JwtAlgorithm: render {
        EDDSA => "EDDSA",
        ES256 => "ES256",
        ES384 => "ES384",
        ES512 => "ES512",
        HS256 => "HS256",
        HS384 => "HS384",
        HS512 => "HS512",
        PS256 => "PS256",
        PS384 => "PS384",
        PS512 => "PS512",
        RS256 => "RS256",
        RS384 => "RS384",
        RS512 => "RS512",
    }
}

keywords! {
    /// The level an access method, a user, a removal or an `INFO` applies to.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Level: render {
        /// `ROOT`
        Root => "ROOT",
        /// `NAMESPACE`: the current namespace.
        Namespace => "NAMESPACE",
        /// `DATABASE`: the current database.
        Database => "DATABASE",
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A [`Level`] as a type, for builders whose clauses depend on the level: record access and
/// bearer access for records exist on a database only, so only
/// [`DefineAccessStatement<OnDatabase>`](crate::statements::DefineAccessStatement) offers them.
/// Sealed: [`OnRoot`], [`OnNamespace`] and [`OnDatabase`] are its only implementors.
pub trait OnLevel: sealed::Sealed + Send + Sync + 'static {
    /// The level the type stands for.
    const LEVEL: Level;
}

/// The root level, as a type.
#[derive(Debug, Clone, Copy)]
pub struct OnRoot;

/// The current namespace, as a type.
#[derive(Debug, Clone, Copy)]
pub struct OnNamespace;

/// The current database, as a type.
#[derive(Debug, Clone, Copy)]
pub struct OnDatabase;

impl sealed::Sealed for OnRoot {}
impl sealed::Sealed for OnNamespace {}
impl sealed::Sealed for OnDatabase {}

impl OnLevel for OnRoot {
    const LEVEL: Level = Level::Root;
}

impl OnLevel for OnNamespace {
    const LEVEL: Level = Level::Namespace;
}

impl OnLevel for OnDatabase {
    const LEVEL: Level = Level::Database;
}
