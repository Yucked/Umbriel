//! Modules (`orm/src/statements/define_module.rs`, and `alter_module`/`remove_module`):
//! rendering, checked with SurrealDB's parser, names, and their effect on a database that allows
//! the experimental `surrealism` feature, and its refusal on a server without it. No module here
//! is called.

use umbriel::{
    Render, Runnable,
    statements::{
        ModuleName, alter_module, define_bucket, define_module_file, define_module_silo,
        remove_module,
    },
};
use umbriel_tests::*;

fn geo() -> ModuleName {
    ModuleName::Module("geo".to_owned())
}

fn silo() -> ModuleName {
    ModuleName::Silo {
        organisation: "acme".to_owned(),
        package: "geo".to_owned(),
        version: (1, 2, 3),
    }
}

#[test]
fn definitions_always_write_unsigned() {
    assert_sql(
        &define_module_file("geo", "modules", "geo.wasm")
            .overwrite()
            .comment("Acme Corp maps")
            .permissions_none(),
        "DEFINE MODULE OVERWRITE mod::geo FROM f\"modules:/geo.wasm\" UNSIGNED \
         COMMENT 'Acme Corp maps' PERMISSIONS NONE;",
    );
    assert_sql(
        &define_module_silo("acme", "geo", (1, 2, 3)),
        "DEFINE MODULE silo::acme::geo::<1.2.3> UNSIGNED;",
    );
    assert_sql(
        &define_module_silo("acme", "geo", (1, 2, 3)).name("maps"),
        "DEFINE MODULE mod::maps FROM silo::acme::geo::<1.2.3> UNSIGNED;",
    );
}

/// Regression: SurrealDB 3.3.4 parses no escapes in a file pointer, so the SDK's escaped
/// rendering of other characters was unparseable.
#[test]
fn a_file_pointer_outside_the_parsers_characters_fails() {
    for (bucket, key) in [
        ("acme files", "geo.wasm"),
        ("", "geo.wasm"),
        ("modules", "a\"b.wasm"),
        ("modules", "geo.wasm\" UNSIGNED; REMOVE TABLE user; --"),
        ("modules", "geo:wasm"),
    ] {
        assert_render_error(
            &define_module_file("geo", bucket, key),
            "invalid file pointer",
        );
    }
    let sql = define_module_file("geo", "acme-files.v2", "maps/geo_1.wasm").to_debug_sql();
    assert_parses(&sql);
    assert!(
        sql.contains(r#"f"acme-files.v2:/maps/geo_1.wasm""#),
        "{sql}"
    );
}

#[test]
fn hostile_names_fail_without_being_written() {
    for hostile in ["geo; REMOVE TABLE user", "1geo", "", "geo::maps"] {
        let statement = define_module_file(hostile, "modules", "geo.wasm");
        assert_render_error(&statement, "invalid module name");
        assert_render_error(
            &remove_module(ModuleName::Module(hostile.to_owned())),
            "invalid module name",
        );
    }
    assert_render_error(
        &define_module_silo("acme corp", "geo", (1, 0, 0)),
        "invalid silo package",
    );
}

#[test]
fn names_parse_from_their_storage_form() {
    assert_eq!("mod::geo".parse::<ModuleName>().expect("parses"), geo());
    assert_eq!(
        "silo::acme::geo::<1.2.3>"
            .parse::<ModuleName>()
            .expect("parses"),
        silo()
    );
    for invalid in [
        "geo",
        "silo::acme::geo",
        "silo::acme::<1.2.3>",
        "silo::acme::geo::<1.2>",
        "silo::acme::geo::<1.2.3.4>",
        "silo::acme::geo::<1.x.3>",
    ] {
        assert!(invalid.parse::<ModuleName>().is_err(), "{invalid}");
    }
}

#[test]
fn alter_and_remove_name_either_form() {
    assert_sql(
        &alter_module(silo())
            .if_exists()
            .drop_comment()
            .permissions_full(),
        "ALTER MODULE IF EXISTS silo::acme::geo::<1.2.3> DROP COMMENT PERMISSIONS FULL;",
    );
    assert_sql(
        &remove_module(geo()).if_exists(),
        "REMOVE MODULE IF EXISTS mod::geo;",
    );
}

#[tokio::test]
async fn a_module_is_defined_altered_and_removed() {
    let (db, _guard) = db().await;
    define_bucket("modules")
        .backend("memory")
        .run(&db)
        .await
        .expect("bucket defined");
    define_module_file("geo", "modules", "geo.wasm")
        .run(&db)
        .await
        .expect("module defined");
    let module = stored_definition(&db, "INFO FOR DB;", "modules", "mod::geo").await;
    assert!(module.contains("UNSIGNED"), "{module}");

    alter_module(geo())
        .comment("Acme Corp maps")
        .run(&db)
        .await
        .expect("altered");
    let module = stored_definition(&db, "INFO FOR DB;", "modules", "mod::geo").await;
    assert!(module.contains("COMMENT 'Acme Corp maps'"), "{module}");

    remove_module(geo()).run(&db).await.expect("removed");
    assert!(remove_module(geo()).run(&db).await.is_err());
}

#[tokio::test]
async fn a_module_needs_the_surrealism_feature() {
    let (db, _guard) = restricted_db().await;
    let error = define_module_silo("acme", "geo", (1, 2, 3))
        .run(&db)
        .await
        .expect_err("modules are experimental")
        .to_string();
    assert!(error.contains("Surrealism"), "{error}");
}
