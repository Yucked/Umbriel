//! Codebase schemas the migration tests generate migrations from: the evolving `Resources`
//! versions and their rename cases, and dedicated fixtures for secrets, statement order,
//! formatting and field names.
//!
//! Fixtures that must generate successfully define nothing whose secrets the database redacts:
//! no users and no record access. Those live in their own fixtures, whose generation fails.

use std::time::Duration;

use chrono::{DateTime, Utc};
use umbriel::{
    DbResources, Field, JwtAlgorithm, LinkOne, Model, Node, Operatable, Param, Raw, RecordId,
    Render, Table, TableResources, chain, cond, create_param_name_fn, create_table_resources,
    model,
    statements::{
        AnalyzerFilter, DefineAccessStatement, FunctionArgument, SnowballLanguage, Tokenizer,
        UserRole, define_access, define_analyzer, define_event, define_function, define_index,
        define_param, define_user, fulltext_analyzer, jwt_access, record_access, return_, select,
    },
    types::Kind,
};

/// A definition as the complete statement `DbResources` takes.
pub fn raw(definition: impl Render) -> Raw {
    Raw::new(definition.to_debug_sql())
}

/// `fn::<name>($id: int)`, returning `$id`.
pub fn identity_function(name: &str) -> Raw {
    let id = Param::new("id");
    raw(define_function(name)
        .arguments(vec![FunctionArgument {
            name: id.clone(),
            type_: Kind::Int,
        }])
        .body(chain(return_(Field::from(&id)))))
}

create_param_name_fn!(__some_test_param1);
create_param_name_fn!(__some_test_param2);
create_param_name_fn!(__some_test_param3);

/// JWT access verified with a public key and issuing no tokens: nothing in it is secret, so it
/// round-trips through generated migrations.
pub fn partner_api() -> DefineAccessStatement {
    define_access("partner_api").on_database().jwt(
        jwt_access()
            .algorithm(JwtAlgorithm::RS256)
            .key("example-public-key"),
    )
}

#[derive(Debug, Clone)]
pub struct Resources;

impl DbResources for Resources {
    create_table_resources!(
        Animal,
        AnimalSnakeCase,
        Crop,
        AnimalEatsCrop,
        AnimalSnakeCaseEatsCrop,
        Student,
        Planet
    );

    fn analyzers(&self) -> Vec<Raw> {
        vec![raw(define_analyzer("ascii")
            .tokenizers([Tokenizer::Class])
            .filters([
                AnalyzerFilter::Lowercase,
                AnalyzerFilter::Ascii,
                AnalyzerFilter::Edgengram(2, 15),
                AnalyzerFilter::Snowball(SnowballLanguage::English),
            ]))]
    }

    fn functions(&self) -> Vec<Raw> {
        vec![
            identity_function("get_animal_by_id"),
            identity_function("get_animal_by_id2"),
        ]
    }

    fn params(&self) -> Vec<Raw> {
        vec![
            raw(define_param(__some_test_param1()).value("test param one")),
            raw(define_param(__some_test_param2()).value("test param two")),
            raw(define_param(__some_test_param3()).value("test param three")),
        ]
    }

    fn accesses(&self) -> Vec<Raw> {
        vec![raw(partner_api())]
    }
}

#[derive(Debug, Clone)]
pub struct ResourcesV2;

impl DbResources for ResourcesV2 {
    create_table_resources!(AnimalV2, Crop, AnimalEatsCropV2, PlanetV2, NewStuff);
}

/// `ResourcesV2` with `AnimalV3` in place of `AnimalV2`.
#[derive(Debug, Clone)]
pub struct ResourcesV2Changed;

impl DbResources for ResourcesV2Changed {
    create_table_resources!(AnimalV3, Crop, AnimalEatsCropV2, PlanetV2, NewStuff);
}

#[derive(Debug, Clone)]
pub struct ResourcesV3;

impl DbResources for ResourcesV3 {
    create_table_resources!(Crop, Student);
}

#[derive(Debug, Clone)]
pub struct ResourcesV4;

impl DbResources for ResourcesV4 {
    create_table_resources!(Crop, Student);
}

#[derive(Debug, Clone)]
pub struct ResourcesV5;

impl DbResources for ResourcesV5 {
    create_table_resources!(Student);
}

#[derive(Debug, Clone)]
pub struct ResourcesV6;

impl DbResources for ResourcesV6 {}

#[derive(Debug, Clone)]
pub struct ResourcesV7;

impl DbResources for ResourcesV7 {
    create_table_resources!(Animal, Crop, AnimalEatsCrop, Planet);
}

#[derive(Debug, Clone)]
pub struct ResourcesV8;

impl DbResources for ResourcesV8 {}

#[derive(Debug, Clone)]
pub struct ResourcesV9;

impl DbResources for ResourcesV9 {}

#[derive(Debug, Clone)]
pub struct ResourcesV10;

impl DbResources for ResourcesV10 {
    create_table_resources!(Animal, Crop, Planet);
}

pub mod invalid_cases {
    use super::*;

    #[model(
        node,
        table = "user_renaming_from_currently_used_field_name_disallowed"
    )]
    #[orm(schemafull)]
    #[surreal(rename_all = "camelCase")]
    #[derive(TableResources, Debug, Clone, Default)]
    pub struct UserRenamingFromCurrentlyUsedFieldNameDisallowed {
        pub id: RecordId<Self>,
        #[orm(old_name = "firstName")]
        pub username: String,
        pub first_name: String,
        pub created_at: DateTime<Utc>,
    }

    pub struct ResourcesVRenamingFromCurrentlyUsedFieldNameDisallowed;

    impl DbResources for ResourcesVRenamingFromCurrentlyUsedFieldNameDisallowed {
        create_table_resources!(UserRenamingFromCurrentlyUsedFieldNameDisallowed);
    }

    #[model(node, table = "user_renaming_with_same_old_field_name_disallowed")]
    #[orm(schemafull)]
    #[surreal(rename_all = "camelCase")]
    #[derive(TableResources, Debug, Clone, Default)]
    pub struct UserRenamingWithSameOldFieldNameDisallowed {
        pub id: RecordId<Self>,
        #[orm(old_name = "firstName")]
        pub first_name: String,
        pub another_stuff: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone)]
    pub struct ResourcesVRenamingWithSameOldFieldNameDisallowed;

    impl DbResources for ResourcesVRenamingWithSameOldFieldNameDisallowed {
        create_table_resources!(UserRenamingWithSameOldFieldNameDisallowed);
    }
}

#[model(node, table = "new_stuff")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct NewStuff {
    pub id: RecordId<Self>,
    pub first_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[model(node, table = "planet")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct Planet {
    pub id: RecordId<Self>,
    pub first_name: String,
    pub population: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub labels: Vec<String>,
}

/// `planet` with `firstName` replaced by `newName` and no `old_name`: the prompter decides
/// between renaming and deleting.
#[model(node, table = "planet")]
#[orm(schemafull, relax_table)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct PlanetV2 {
    pub id: RecordId<Self>,
    pub new_name: String,
    pub population: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub labels: Vec<String>,
}

#[model(node, table = "student")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct Student {
    pub id: RecordId<Self>,
    pub university: String,
    pub age: u8,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub mod snake_cases {
    use super::*;

    #[model(node, table = "animal_snake_case")]
    #[orm(schemafull)]
    #[derive(TableResources, Debug, Clone, Default)]
    pub struct AnimalSnakeCase {
        pub id: RecordId<Self>,
        pub species: String,
        pub attributes: Vec<String>,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
        pub velocity: u64,
    }

    /// The second version of `AnimalSnakeCase`, renaming `attributes` explicitly.
    #[model(node, table = "animal_snake_case")]
    #[orm(schemafull, relax_table)]
    #[derive(TableResources, Debug, Clone, Default)]
    pub struct AnimalSnakeCaseV2 {
        pub id: RecordId<Self>,
        pub species: String,
        #[orm(old_name = "attributes")]
        pub characteristics: Vec<String>,
        pub velocity: u64,
    }

    #[model(edge, table = "eats_snake_case")]
    #[orm(schemafull)]
    #[derive(TableResources, Debug, Clone, Default)]
    pub struct EatsSnakeCase<In: Node, Out: Node> {
        pub id: RecordId<Self>,
        #[orm(link_one = "In")]
        pub r#in: LinkOne<In>,
        #[orm(link_one = "Out")]
        pub out: LinkOne<Out>,
        pub place: String,
        pub created_at: DateTime<Utc>,
    }

    pub type AnimalSnakeCaseEatsCrop = EatsSnakeCase<AnimalSnakeCase, Crop>;
}

pub use snake_cases::{AnimalSnakeCase, AnimalSnakeCaseEatsCrop};

#[model(node, table = "animal")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone, Default)]
pub struct Animal {
    pub id: RecordId<Self>,
    pub species: String,
    pub attributes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub velocity: u64,
}

impl TableResources for Animal {
    fn events_definitions() -> Vec<Raw> {
        let event1 = define_event("event1")
            .on_table(Self::table())
            .when(
                cond(Field::new("species").equal("Homo Erectus"))
                    .and(Field::new("velocity").gt(545)),
            )
            .then(select(umbriel::All).from(Table::new("crop")));

        let event2 = define_event("event2")
            .on_table(Self::table())
            .when(
                cond(Field::new("species").equal("Homo Sapien")).and(Field::new("velocity").lt(10)),
            )
            .then(select(umbriel::All).from(Table::new("eats")));
        vec![raw(event1), raw(event2)]
    }

    fn indexes_definitions() -> Vec<Raw> {
        vec![raw(define_index("species_speed_idx")
            .on_table(Self::table())
            .fields([Field::new("species"), Field::new("velocity")])
            .unique())]
    }
}

/// The second version of `Animal`, renaming `attributes` explicitly and changing its event and
/// index.
#[model(node, table = "animal")]
#[orm(schemafull, relax_table)]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone, Default)]
pub struct AnimalV2 {
    pub id: RecordId<Self>,
    pub species: String,
    #[orm(old_name = "attributes")]
    pub characteristics: Vec<String>,
    pub velocity: u64,
}

impl TableResources for AnimalV2 {
    fn events_definitions() -> Vec<Raw> {
        let event1 = define_event("event1")
            .on_table(Self::table())
            .when(
                cond(Field::new("species").equal("Homo Habillis"))
                    .and(Field::new("velocity").gt(545)),
            )
            .then(select(umbriel::All).from(Table::new("crop")));
        vec![raw(event1)]
    }

    fn indexes_definitions() -> Vec<Raw> {
        vec![raw(define_index("species_speed_idx")
            .on_table(Self::table())
            .fields([Field::new("velocity"), Field::new("characteristics")])
            .unique())]
    }
}

/// `AnimalV2` with its renamed field's definition changed afterwards and the `old_name` left in
/// place.
#[model(node, table = "animal")]
#[orm(schemafull, relax_table)]
#[surreal(rename_all = "camelCase")]
#[derive(Debug, Clone, Default)]
pub struct AnimalV3 {
    pub id: RecordId<Self>,
    pub species: String,
    #[orm(old_name = "attributes")]
    pub characteristics: Vec<u64>,
    pub velocity: u64,
}

impl TableResources for AnimalV3 {
    fn events_definitions() -> Vec<Raw> {
        AnimalV2::events_definitions()
    }

    fn indexes_definitions() -> Vec<Raw> {
        AnimalV2::indexes_definitions()
    }
}

#[model(edge, table = "eats")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct Eats<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[orm(link_one = "In")]
    pub r#in: LinkOne<In>,
    #[orm(link_one = "Out")]
    pub out: LinkOne<Out>,
    pub place: String,
    pub created_at: DateTime<Utc>,
}

pub type AnimalEatsCrop = Eats<Animal, Crop>;

#[model(edge, table = "eats_v2")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct EatsV2<In: Node, Out: Node> {
    pub id: RecordId<Self>,
    #[surreal(rename = "in")]
    #[orm(link_one = "In", ty = "record")]
    pub in_: LinkOne<In>,
    #[orm(link_one = "Out", ty = "record")]
    pub out: LinkOne<Out>,
    pub location: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub type AnimalEatsCropV2 = EatsV2<AnimalV2, Crop>;

#[model(node, table = "crop")]
#[orm(schemafull)]
#[surreal(rename_all = "camelCase")]
#[derive(TableResources, Debug, Clone, Default)]
pub struct Crop {
    pub id: RecordId<Self>,
    pub color: String,
}

/// No definitions at all.
#[derive(Debug, Clone)]
pub struct EmptyResources;

impl DbResources for EmptyResources {}

create_param_name_fn!(__placeholder);

/// The redaction placeholder as ordinary text, a param's value and a public-key access's
/// comment, where the database redacts nothing.
#[derive(Debug, Clone)]
pub struct PlaceholderResources;

impl DbResources for PlaceholderResources {
    fn params(&self) -> Vec<Raw> {
        vec![raw(define_param(__placeholder()).value("[REDACTED]"))]
    }

    fn accesses(&self) -> Vec<Raw> {
        vec![Raw::new(
            "DEFINE ACCESS partner_notes ON DATABASE TYPE JWT ALGORITHM RS256 \
             KEY 'example-public-key' COMMENT '[REDACTED]';",
        )]
    }
}

/// Record access: SurrealDB generates and redacts its token-signing key, so it cannot be
/// written to a migration.
pub fn customer_access(session: Duration) -> Raw {
    raw(define_access("customer").on_database().record(
        record_access()
            .signin(select(umbriel::All).from(Table::new("customer")))
            .duration_for_session(session),
    ))
}

/// A database user with a password hash, which SurrealDB redacts.
pub fn reporting_user(role: UserRole) -> Raw {
    raw(define_user("reporting")
        .on_database()
        .passhash("$argon2id$v=19$m=19456,t=2,p=1$ZXhhbXBsZXNhbHQ$ZXhhbXBsZWhhc2g")
        .role(role))
}

/// Record access with a one-hour session.
#[derive(Debug, Clone)]
pub struct RecordAccessResources;

impl DbResources for RecordAccessResources {
    fn accesses(&self) -> Vec<Raw> {
        vec![customer_access(Duration::from_secs(60 * 60))]
    }
}

/// The record access with a two-hour session.
#[derive(Debug, Clone)]
pub struct RecordAccessResourcesChanged;

impl DbResources for RecordAccessResourcesChanged {
    fn accesses(&self) -> Vec<Raw> {
        vec![customer_access(Duration::from_secs(2 * 60 * 60))]
    }
}

/// A viewer database user.
#[derive(Debug, Clone)]
pub struct UserResources;

impl DbResources for UserResources {
    fn users(&self) -> Vec<Raw> {
        vec![reporting_user(UserRole::Viewer)]
    }
}

/// The user as an editor.
#[derive(Debug, Clone)]
pub struct UserResourcesChanged;

impl DbResources for UserResourcesChanged {
    fn users(&self) -> Vec<Raw> {
        vec![reporting_user(UserRole::Editor)]
    }
}

/// Notes searched through an analyzer that uses a function: the chain function → analyzer →
/// index must be created in that order and removed in the reverse.
#[model(node, table = "note")]
#[orm(schemafull)]
#[derive(Debug, Clone, Default)]
pub struct Note {
    pub id: RecordId<Self>,
    pub body: String,
}

impl TableResources for Note {}

/// The function and analyzer of `NoteSearch`, as SurrealQL: the analyzer builder has no
/// `FUNCTION` clause.
pub fn normalize_function(name: &str) -> Raw {
    Raw::new(format!(
        "DEFINE FUNCTION fn::{name}($text: string) -> string {{ RETURN string::lowercase($text); }};"
    ))
}

pub fn normalized_analyzer(name: &str, function: &str) -> Raw {
    Raw::new(format!(
        "DEFINE ANALYZER {name} FUNCTION fn::{function} TOKENIZERS blank FILTERS lowercase;"
    ))
}

pub fn body_index(analyzer: &str) -> Raw {
    raw(define_index("note_body")
        .on_table(Table::new("note"))
        .fields(Field::new("body"))
        .fulltext(fulltext_analyzer(analyzer).bm25(1.2, 0.75)))
}

/// Notes indexed through `normalized`, which uses `fn::normalize`.
#[derive(Debug, Clone)]
pub struct NoteSearch;

impl DbResources for NoteSearch {
    fn functions(&self) -> Vec<Raw> {
        vec![normalize_function("normalize")]
    }

    fn analyzers(&self) -> Vec<Raw> {
        vec![normalized_analyzer("normalized", "normalize")]
    }

    fn tables(&self) -> Vec<Raw> {
        let mut tables = <Note as TableResources>::fields_definitions();
        tables.insert(0, <Note as TableResources>::table_definition());
        tables.push(body_index("normalized"));
        tables
    }
}

/// The index moved to `normalized_v2`, which uses `fn::normalize_v2`; the first analyzer and
/// function are removed.
#[derive(Debug, Clone)]
pub struct NoteSearchV2;

impl DbResources for NoteSearchV2 {
    fn functions(&self) -> Vec<Raw> {
        vec![normalize_function("normalize_v2")]
    }

    fn analyzers(&self) -> Vec<Raw> {
        vec![normalized_analyzer("normalized_v2", "normalize_v2")]
    }

    fn tables(&self) -> Vec<Raw> {
        let mut tables = <Note as TableResources>::fields_definitions();
        tables.insert(0, <Note as TableResources>::table_definition());
        tables.push(body_index("normalized_v2"));
        tables
    }
}

/// Only the note table: the index, analyzer and function are removed together.
#[derive(Debug, Clone)]
pub struct NoteOnly;

impl DbResources for NoteOnly {
    create_table_resources!(Note);
}

/// A table whose fields test joining structured field names to their `DEFINE FIELD` text: a
/// reserved word, nested paths, array items, an escaped name and a name that prefixes another.
#[derive(Debug, Clone)]
pub struct Catalog;

impl DbResources for Catalog {
    fn tables(&self) -> Vec<Raw> {
        [
            "DEFINE TABLE catalog SCHEMAFULL;",
            "DEFINE FIELD `value` ON TABLE catalog TYPE string;",
            "DEFINE FIELD address ON TABLE catalog TYPE object;",
            "DEFINE FIELD address.city ON TABLE catalog TYPE string;",
            // An untyped array: a typed one defines its `[*]` field itself.
            "DEFINE FIELD tags ON TABLE catalog TYPE array;",
            "DEFINE FIELD tags[*] ON TABLE catalog TYPE string;",
            "DEFINE FIELD `display-name` ON TABLE catalog TYPE string;",
            "DEFINE FIELD foo ON TABLE catalog TYPE object;",
            "DEFINE FIELD foo.bar ON TABLE catalog TYPE string;",
            "DEFINE FIELD foobar ON TABLE catalog TYPE string;",
        ]
        .into_iter()
        .map(Raw::new)
        .collect()
    }
}

/// The schema a hand-formatted migration file defines differently written: see
/// `FORMATTED_NOTES`.
#[derive(Debug, Clone)]
pub struct FormattedNotes;

impl DbResources for FormattedNotes {
    fn tables(&self) -> Vec<Raw> {
        [
            "DEFINE TABLE memo SCHEMAFULL;",
            "DEFINE FIELD title ON TABLE memo TYPE string DEFAULT 'Untitled' ASSERT string::len($value) > 0;",
            "DEFINE FIELD views ON TABLE memo TYPE int DEFAULT 0;",
            "DEFINE INDEX memo_title ON TABLE memo FIELDS title UNIQUE;",
            "DEFINE EVENT memo_created ON TABLE memo WHEN $event = 'CREATE' THEN (UPDATE $after SET views = 0);",
        ]
        .into_iter()
        .map(Raw::new)
        .collect()
    }
}

/// `FormattedNotes`, written differently: lowercase keywords, line breaks, `ON` without `TABLE`,
/// clauses in another order, double quotes, and defaults stated explicitly (`SCHEMAFULL` alone
/// implies `TYPE NORMAL`).
pub const FORMATTED_NOTES: &str = r#"define table memo
    type normal schemafull permissions none;
define field title on memo
    type string
    assert string::len($value) > 0
    default "Untitled"
    permissions full;
define field views on memo type int default 0;
define index memo_title on memo fields title unique;
define event memo_created on memo
    when $event = "CREATE"
    then (update $after set views = 0);"#;

/// Every definition clause #13 added: table kinds and comments, `READONLY`, `DEFAULT ALWAYS`,
/// `FLEXIBLE`, `COMPUTED`, a float default on a decimal field, an `ASYNC` event, an enforced
/// relation, a view and param permissions. Each table is defined after the tables it reads, and
/// the view, `sale_large`, sorts after its source `sale`, so replaying the definitions in
/// `Schema::iter` order (kind, then table name) is also dependency-safe.
pub const NEW_CLAUSES_TABLES: &[&str] = &[
    "DEFINE TABLE customer SCHEMALESS;",
    "DEFINE TABLE sale SCHEMAFULL TYPE NORMAL COMMENT 'Acme Corp sales';",
    "DEFINE FIELD amount ON TABLE sale TYPE int READONLY COMMENT 'in cents';",
    "DEFINE FIELD refunds ON TABLE sale TYPE int DEFAULT ALWAYS 0;",
    "DEFINE FIELD meta ON TABLE sale TYPE object FLEXIBLE;",
    "DEFINE FIELD doubled ON TABLE sale COMPUTED amount * 2;",
    "DEFINE FIELD price ON TABLE sale TYPE decimal DEFAULT 1.5f;",
    "DEFINE EVENT audit ON TABLE sale THEN (CREATE log) ASYNC RETRY 2 MAXDEPTH 1;",
    "DEFINE TABLE likes TYPE RELATION IN customer OUT sale ENFORCED;",
    "DEFINE TABLE sale_large AS SELECT * FROM sale WHERE amount >= 10;",
];

/// The param of [`NewClauses`], with a permission.
pub const NEW_CLAUSES_PARAM: &str = "DEFINE PARAM $greeting VALUE 'Hello' PERMISSIONS NONE;";

/// The schema of [`NEW_CLAUSES_TABLES`] and [`NEW_CLAUSES_PARAM`].
#[derive(Debug, Clone)]
pub struct NewClauses;

impl DbResources for NewClauses {
    fn tables(&self) -> Vec<Raw> {
        NEW_CLAUSES_TABLES.iter().copied().map(Raw::new).collect()
    }

    fn params(&self) -> Vec<Raw> {
        vec![Raw::new(NEW_CLAUSES_PARAM)]
    }
}
