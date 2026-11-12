//! `orm/src/kinds.rs`: every marker's SurrealQL kind, as `DEFINE FIELD … TYPE` renders it and
//! SurrealDB accepts it.

use umbriel::{
    Render, Table,
    kinds::{self, KindOf},
    statements::define_field,
};
use umbriel_tests::*;

/// The `DEFINE FIELD` text of a field typed by `K`.
fn defined_as<K: KindOf>() -> String {
    define_field("amount")
        .on_table(Table::from("thing"))
        .type_(K::kind())
        .to_debug_sql()
}

#[track_caller]
fn renders<K: KindOf>(expected: &str) {
    assert_eq!(
        defined_as::<K>(),
        format!("DEFINE FIELD amount ON TABLE thing TYPE {expected};")
    );
}

#[tokio::test]
async fn every_marker_renders_its_kind() {
    renders::<kinds::Any>("any");
    renders::<kinds::Bool>("bool");
    renders::<kinds::Int>("int");
    renders::<kinds::Float>("float");
    renders::<kinds::Decimal>("decimal");
    renders::<kinds::Number>("number");
    renders::<kinds::String>("string");
    renders::<kinds::Datetime>("datetime");
    renders::<kinds::Duration>("duration");
    renders::<kinds::Uuid>("uuid");
    renders::<kinds::Bytes>("bytes");
    renders::<kinds::Object>("object");
    renders::<kinds::Geometry>("geometry");
    renders::<kinds::Point>("geometry<point>");
    renders::<kinds::Table>("table");
    renders::<kinds::Regex>("regex");
    renders::<kinds::Range>("range");
    renders::<kinds::Record>("record");
    renders::<kinds::Record<User>>("record<user>");
    renders::<kinds::Array>("array<any>");
    renders::<kinds::Array<kinds::Int>>("array<int>");
    renders::<kinds::Array<kinds::Record<User>>>("array<record<user>>");
    renders::<kinds::Set<kinds::String>>("set<string>");
    renders::<kinds::Optional<kinds::Int>>("none | int");

    let db = db().await;
    assert!(succeeds(&db, "DEFINE TABLE thing SCHEMAFULL;").await);
    for definition in [
        defined_as::<kinds::Point>(),
        defined_as::<kinds::Record<User>>(),
        defined_as::<kinds::Array<kinds::Record<User>>>(),
        defined_as::<kinds::Set<kinds::String>>(),
        defined_as::<kinds::Optional<kinds::Int>>(),
    ] {
        let definition = definition.replace("DEFINE FIELD", "DEFINE FIELD OVERWRITE");
        assert!(
            succeeds(&db, &definition).await,
            "`{definition}` is accepted"
        );
    }
}
