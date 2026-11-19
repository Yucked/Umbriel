//! `session::*` (`orm/src/functions/session.rs`): every function rendered and the values
//! SurrealDB gives in the test session.

use umbriel::functions::session;
use umbriel_tests::*;

#[test]
fn rendering() {
    calls!(
        (session::ac!(), "session::ac()"),
        (session::db!(), "session::db()"),
        (session::id!(), "session::id()"),
        (session::ip!(), "session::ip()"),
        (session::ns!(), "session::ns()"),
        (session::origin!(), "session::origin()"),
        (session::rd!(), "session::rd()"),
        (session::token!(), "session::token()"),
    );
}

#[tokio::test]
async fn results() {
    let (db, guard) = db().await;
    holds_all!(
        &db,
        [
            (session::ns!(), &format!("'{}'", guard.namespace())),
            (session::db!(), &format!("'{}'", guard.database())),
            (session::rd!(), "NONE"),
        ]
    );
}
