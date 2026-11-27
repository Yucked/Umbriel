//! `RecordAccessDefinition` (`orm/src/traits/access.rs`): credentials typed by the access method
//! they sign in through.

use umbriel::{
    OnDatabase, Raw, RecordAccessDefinition, Runnable,
    statements::{DefineAccessStatement, define_access, record_access},
    types::SurrealValue,
};
use umbriel_tests::*;

#[derive(SurrealValue)]
#[surreal(crate = "umbriel::types")]
struct NewAccount {
    name: String,
    pass: String,
    plan: String,
}

#[derive(SurrealValue)]
#[surreal(crate = "umbriel::types")]
struct Login {
    name: String,
    pass: String,
}

struct Account;

impl RecordAccessDefinition for Account {
    const NAME: &'static str = "account";
    type Signup = NewAccount;
    type Signin = Login;

    fn define() -> DefineAccessStatement<OnDatabase> {
        define_access(Self::NAME).on_database().record(
            record_access()
                .signup(Raw::new(
                    "CREATE person SET name = $name, plan = $plan, \
                     pass = crypto::argon2::generate($pass)",
                ))
                .signin(Raw::new(
                    "SELECT * FROM person WHERE name = $name \
                     AND crypto::argon2::compare(pass, $pass)",
                )),
        )
    }
}

#[test]
fn credentials_name_the_method_and_carry_its_parameters() {
    let credentials = Account::signin(
        "acme",
        "app",
        Login {
            name: "Alice".into(),
            pass: "example-password".into(),
        },
    );
    assert_eq!(credentials.namespace, "acme");
    assert_eq!(credentials.database, "app");
    assert_eq!(credentials.access, Account::NAME);
    assert_eq!(credentials.params.name, "Alice");
}

/// Sign-up and sign-in through the trait: the typed parameters reach the expressions.
#[tokio::test]
async fn typed_credentials_sign_up_and_in() {
    let (db, guard) = db().await;
    Account::define().run(&db).await.expect("defined");
    // A record user reads nothing of a table without permissions, its own record included.
    Raw::new("DEFINE TABLE person PERMISSIONS FOR select WHERE id = $auth.id;")
        .run(&db)
        .await
        .expect("defined");

    let session = guest(Endpoint::Main).await;
    session
        .signup(Account::signup(
            guard.namespace(),
            guard.database(),
            NewAccount {
                name: "Alice".into(),
                pass: "example-password".into(),
                plan: "pro".into(),
            },
        ))
        .await
        .expect("signed up");

    let session = guest(Endpoint::Main).await;
    session
        .signin(Account::signin(
            guard.namespace(),
            guard.database(),
            Login {
                name: "Alice".into(),
                pass: "example-password".into(),
            },
        ))
        .await
        .expect("signed in");
    bounded("selecting the test database", async {
        session
            .use_ns(guard.namespace())
            .use_db(guard.database())
            .await
    })
    .await;
    let plan: String = read(&session, "RETURN $auth.plan;").await;
    assert_eq!(plan, "pro");
}
