use std::fmt::{self, Display};

use crate::{Render, Renderer};

/// A JWT signature algorithm.
#[expect(missing_docs, reason = "each variant is the algorithm's own name")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JwtAlgorithm {
    EDDSA,
    ES256,
    ES384,
    ES512,
    HS256,
    HS384,
    HS512,
    PS256,
    PS384,
    PS512,
    RS256,
    RS384,
    RS512,
}

impl JwtAlgorithm {
    /// The algorithm's SurrealQL keyword.
    pub fn as_str(self) -> &'static str {
        match self {
            JwtAlgorithm::EDDSA => "EDDSA",
            JwtAlgorithm::ES256 => "ES256",
            JwtAlgorithm::ES384 => "ES384",
            JwtAlgorithm::ES512 => "ES512",
            JwtAlgorithm::HS256 => "HS256",
            JwtAlgorithm::HS384 => "HS384",
            JwtAlgorithm::HS512 => "HS512",
            JwtAlgorithm::PS256 => "PS256",
            JwtAlgorithm::PS384 => "PS384",
            JwtAlgorithm::PS512 => "PS512",
            JwtAlgorithm::RS256 => "RS256",
            JwtAlgorithm::RS384 => "RS384",
            JwtAlgorithm::RS512 => "RS512",
        }
    }
}

impl Display for JwtAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Render for JwtAlgorithm {
    fn render(&self, out: &mut Renderer) {
        out.push_str(self.as_str());
    }
}

/// The level an access method, a user, a removal or an `INFO` applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// `ROOT`
    Root,
    /// `NAMESPACE`: the current namespace.
    Namespace,
    /// `DATABASE`: the current database.
    Database,
}

impl Level {
    /// The level's SurrealQL keyword.
    pub const fn as_str(self) -> &'static str {
        match self {
            Level::Root => "ROOT",
            Level::Namespace => "NAMESPACE",
            Level::Database => "DATABASE",
        }
    }
}

impl Render for Level {
    fn render(&self, out: &mut Renderer) {
        out.push_str(self.as_str());
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
