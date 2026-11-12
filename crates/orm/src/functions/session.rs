//! Session functions, `session::*`. Each is `NONE` when the session does not have the value.
//!
//! ```rust
//! use umbriel::{*, functions::session};
//! assert_eq!(session::db!().to_debug_sql(), "session::db()");
//! ```

use crate::kinds;

functions! { $ "session" "::", session, session;
    /// The access method of the session.
    ac() -> kinds::Optional<kinds::String>;
    /// The selected database.
    db() -> kinds::Optional<kinds::String>;
    /// The session id.
    id() -> kinds::Optional<kinds::String>;
    /// The client's IP address.
    ip() -> kinds::Optional<kinds::String>;
    /// The selected namespace.
    ns() -> kinds::Optional<kinds::String>;
    /// The HTTP origin of the session.
    origin() -> kinds::Optional<kinds::String>;
    /// The record the session is authenticated as.
    rd() -> kinds::Optional<kinds::Record>;
    /// The claims of the session's token.
    token() -> kinds::Optional<kinds::Object>;
}
