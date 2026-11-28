//! The migration CLI (`orm/src/migrations/cli/mod.rs`, `config.rs`): commands parsed from
//! arguments, as a migrator binary receives them, connecting to a server as a database user.

mod down;
mod generate;
mod init;
mod list;
mod prune;
mod reset;
mod unlock;
mod up;

use clap::Parser;
use pretty_assertions::assert_eq;
use umbriel::migrations::{Init, MigrationFlag, Migrator, Mode};
use umbriel_tests::{
    Endpoint,
    migrations::{Resources, helpers::*},
    succeeds,
};

fn migrator(args: &[&str]) -> Migrator {
    Migrator::try_parse_from(std::iter::once("migrator").chain(args.iter().copied()))
        .expect("the arguments parse")
}

/// `up` connects with `--url`, `--ns`, `--db`, `--user` and `--pass` as a database user, and
/// applies what `init` wrote.
#[tokio::test]
async fn up_connects_as_a_database_user_and_applies_the_migrations() {
    let project = Project::new(MigrationFlag::TwoWay, Mode::Strict).await;
    project
        .run(
            Init::new("test migration 1".into(), true, false),
            Resources,
            allow_empty(),
        )
        .await;
    assert!(
        succeeds(
            &project.db,
            "DEFINE USER migrator ON DATABASE PASSWORD 'example-password' ROLES OWNER;"
        )
        .await,
        "the database user is defined"
    );
    let database: String = umbriel_tests::read(&project.db, "RETURN session::db();").await;

    let url = Endpoint::Main.url();
    let dir = project.dir.to_str().expect("a UTF-8 path");
    let mut up = migrator(&[
        "up",
        "--dir",
        dir,
        "--url",
        &url,
        "--ns",
        umbriel_tests::NAMESPACE,
        "--db",
        &database,
        "--user",
        "migrator",
        "--pass",
        "example-password",
    ]);
    up.run_fn(Resources, allow_empty())
        .await
        .expect("up applies the migration");
    assert_eq!(applied(up.db().expect("connected")).await.len(), 1);
    project
        .assert_state(&["test_migration_1"], &["test_migration_1"])
        .await;
}

/// `--yes` is taken by the commands that ask before deleting or replacing, and only by them;
/// `--replace-history` only by `up`.
#[test]
fn confirmation_flags_parse_where_they_apply() {
    for args in [
        &["prune", "--yes"][..],
        &["down", "--prune", "--yes"],
        &["reset", "--name", "init", "--yes"],
        &["unlock", "--yes"],
        &["unlock"],
        &["up", "--replace-history"],
    ] {
        let parsed =
            Migrator::try_parse_from(std::iter::once("migrator").chain(args.iter().copied()));
        assert!(parsed.is_ok(), "{args:?}: {parsed:?}");
    }
    for args in [&["up", "--yes"][..], &["down", "--replace-history"]] {
        let parsed =
            Migrator::try_parse_from(std::iter::once("migrator").chain(args.iter().copied()));
        assert!(parsed.is_err(), "{args:?}: {parsed:?}");
    }
}

/// Regression (UMB-037.3): the whole URL was lowercased, which breaks a case-sensitive path.
#[test]
fn only_the_url_scheme_is_lowercased() {
    use umbriel::migrations::config::UrlDb;
    let parsed = |url: &str| url.parse::<UrlDb>().expect("a URL").to_string();
    assert_eq!(parsed("FILE:///Tmp/MyDB"), "file:///Tmp/MyDB");
    assert_eq!(parsed("ws://Example.com:8000"), "ws://Example.com:8000");
    for memory in ["mem://", "MEM://", "memory", "Memory"] {
        assert!(
            matches!(memory.parse::<UrlDb>(), Ok(UrlDb::Memory)),
            "{memory}"
        );
    }
}

/// Regression (UMB-038): `-v` started at 3.
#[test]
fn verbosity_counts_up_from_zero() {
    assert_eq!(migrator(&["up"]).verbosity(), 0);
    assert_eq!(migrator(&["up", "-v"]).verbosity(), 1);
    assert_eq!(migrator(&["up", "-vvv"]).verbosity(), 3);
}

/// The environment a child run of this test binary reads its case from.
const CHILD: &str = "UMBRIEL_TEST_CLI_CHILD";

/// Runs `test`, a test of this module, in a child process with `environment`, and returns what
/// it printed. Credentials come from the environment, so each case runs where it can set its own
/// without touching this process's.
fn child_output(test: &str, case: &str, environment: &[(&str, &str)]) -> String {
    let module = module_path!()
        .split_once("::")
        .map_or("", |(_, rest)| rest);
    let mut command = std::process::Command::new(std::env::current_exe().expect("the test binary"));
    command
        .args(["--exact", &format!("{module}::{test}"), "--nocapture"])
        .env(CHILD, case)
        .env_remove(umbriel::migrations::USER_VARIABLE)
        .env_remove(umbriel::migrations::PASSWORD_VARIABLE);
    for (name, value) in environment {
        command.env(name, value);
    }
    let output = command.output().expect("the child runs");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

/// What the child prints for its case: the help, or the outcome of connecting without
/// credentials.
fn child_case(case: &str) {
    match case {
        "help" => {
            let error = Migrator::try_parse_from(["umbriel", "up", "--help"])
                .expect_err("help is shown as an error");
            println!("{}", error.render());
        }
        "missing" => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime");
            let error = runtime.block_on(async {
                migrator(&["up", "--url", "ws://127.0.0.1:1"])
                    .run_fn(Resources, allow_empty())
                    .await
                    .expect_err("refused")
            });
            println!("outcome: {error:?}");
        }
        other => panic!("unknown case {other}"),
    }
}

/// Regression (UMB-037.1): credentials came only as arguments, defaulting to `root`/`root`.
/// They are read from the environment, and the password's value never appears in help.
#[test]
fn help_names_the_credential_variables_but_hides_the_password() {
    if let Ok(case) = std::env::var(CHILD) {
        return child_case(&case);
    }
    let help = child_output(
        "help_names_the_credential_variables_but_hides_the_password",
        "help",
        &[
            ("UMBRIEL_USER", "example-user"),
            ("UMBRIEL_PASS", "example-secret-password"),
        ],
    );
    assert!(help.contains("UMBRIEL_PASS"), "{help}");
    assert!(help.contains("example-user"), "the user is read from the environment: {help}");
    assert!(!help.contains("example-secret-password"), "{help}");
}

/// Without credentials, a command fails before connecting.
#[test]
fn missing_credentials_are_refused() {
    if let Ok(case) = std::env::var(CHILD) {
        return child_case(&case);
    }
    let without_user = child_output("missing_credentials_are_refused", "missing", &[]);
    assert!(
        without_user.contains(r#"MissingArgument { argument: "--user" }"#),
        "{without_user}"
    );
    let without_password = child_output(
        "missing_credentials_are_refused",
        "missing",
        &[("UMBRIEL_USER", "example-user")],
    );
    assert!(
        without_password.contains(r#"MissingArgument { argument: "--pass" }"#),
        "{without_password}"
    );
}

#[test]
fn an_unknown_experimental_feature_is_refused() {
    let parsed = Migrator::try_parse_from(["migrator", "up", "--allow-experimental", "graphql"]);
    assert!(parsed.is_err(), "{parsed:?}");
}
