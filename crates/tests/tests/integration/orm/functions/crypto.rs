//! `crypto::*` (`orm/src/functions/crypto.rs`): every function run, the hashes SurrealDB
//! computes and the password schemes round-tripping. None of them has a method form.

use umbriel::{Param, Render, functions::crypto};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    holds_all!(
        &db,
        [
            (crypto::md5("a"), "'0cc175b9c0f1b6a831c399e269772661'"),
            (
                crypto::sha1("a"),
                "'86f7e437faa5a7fce15d1ddcb9eaeaea377667b8'"
            ),
            (
                crypto::sha256("a"),
                "'ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb'",
            ),
            (
                crypto::sha512("a"),
                "'1f40fc92da241694750979ee6cf582f2d5d7d28e18335de05abc54d0560e0f5302860c652bf08d560252aa5e74210546f369fbbbce8c12cfc7957b2652fe9a75'",
            ),
            (
                crypto::argon2::compare(crypto::argon2::generate("pass"), "pass"),
                "true"
            ),
            (
                crypto::argon2::compare(crypto::argon2::generate("pass"), "other"),
                "false"
            ),
            (
                crypto::bcrypt::compare(crypto::bcrypt::generate("pass"), "pass"),
                "true"
            ),
            (
                crypto::pbkdf2::compare(crypto::pbkdf2::generate("pass"), "pass"),
                "true"
            ),
            (
                crypto::scrypt::compare(crypto::scrypt::generate("pass"), "pass"),
                "true"
            ),
        ]
    );
    is_each!(
        &db,
        [(crypto::blake3("a"), "string"), (crypto::joaat("a"), "int"),]
    );
}

#[test]
fn generated_bindings() {
    let nested = crypto::sha256(crypto::md5(Param::new("password")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "crypto::sha256(crypto::md5($password))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = failure(&db, "RETURN crypto::md5([1]);").await;
    assert!(error.contains("crypto::md5"), "{error}");
}
