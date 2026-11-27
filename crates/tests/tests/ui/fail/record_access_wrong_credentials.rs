use umbriel::{
    OnDatabase, RecordAccessDefinition,
    statements::{DefineAccessStatement, define_access, record_access},
    types::SurrealValue,
};

#[derive(SurrealValue)]
#[surreal(crate = "umbriel::types")]
struct Login {
    name: String,
}

#[derive(SurrealValue)]
#[surreal(crate = "umbriel::types")]
struct Other {
    code: u32,
}

struct Account;

impl RecordAccessDefinition for Account {
    const NAME: &'static str = "account";
    type Signup = Login;
    type Signin = Login;

    fn define() -> DefineAccessStatement<OnDatabase> {
        define_access(Self::NAME).on_database().record(record_access())
    }
}

// Sign-in credentials take the method's own parameter type.
fn main() {
    let _ = Account::signin("acme", "app", Other { code: 1 });
}
