//! Models: what `#[model]` generates (definitions, schema, patches, flattened objects) and how
//! models, record ids and links behave against the in-memory engine.

use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use chrono::{DateTime, Utc};
use surrealdb::{Surreal, engine::local::Db};
use umbriel::{
    All,
    CrudType::*,
    E, Edge, Field, FlatPatch, LinkMany, LinkOne, LinkSelf, Model, ModelPatch, NONE, Node,
    Nullable, Operatable, Patch, Patchable, Raw, RecordId, RecordRange, Reference, Relate, Render,
    Runnable, SchemaGetter, SetterArray, SetterAssignable, SetterNumeric, SurrealCrud,
    SurrealCrudNode, cond, model,
    statements::{Returning, define_field, define_table, for_permission, relate, select},
    types::{self, Kind, RecordIdKey, SurrealValue, Value},
    value,
};
use umbriel_tests::*;

/// Definitions joined one per line.
fn joined(definitions: &[Raw]) -> String {
    definitions
        .iter()
        .map(Raw::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The definitions of `field`, in order.
fn definitions_of(definitions: &[Raw], field: &str) -> Vec<String> {
    let prefix = format!("DEFINE FIELD {field} ");
    let item_prefix = format!("DEFINE FIELD {field}.* ");
    definitions
        .iter()
        .map(Raw::as_str)
        .filter(|definition| {
            definition.starts_with(&prefix) || definition.starts_with(&item_prefix)
        })
        .map(str::to_owned)
        .collect()
}

fn sorted(fields: Vec<Field>) -> Vec<String> {
    let mut fields = fields.iter().map(Render::to_debug_sql).collect::<Vec<_>>();
    fields.sort();
    fields
}

fn line_string() -> geo::LineString {
    geo::LineString::from(vec![(0.0, 0.0), (1.0, 1.0)])
}

fn polygon() -> geo::Polygon {
    geo::Polygon::new(
        geo::LineString::from(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]),
        vec![],
    )
}

fn alien(key: &str, name: &str) -> Alien {
    Alien {
        id: Alien::id(key),
        name: name.to_owned(),
        age: 30,
        created: DateTime::<Utc>::UNIX_EPOCH,
        life_expectancy: Duration::from_secs(60),
        line_string: line_string(),
        territory_area: polygon(),
        home: geo::Point::new(1.0, 2.0),
        tags: vec!["friendly".to_owned()],
        ally: LinkSelf::null(),
        weapon: LinkOne::null(),
        space_ships: LinkMany::null(),
        planets_to_visit: Relate::null(),
    }
}

fn space_ship(key: &str, name: &str) -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id(key),
        name: name.to_owned(),
        created: DateTime::<Utc>::UNIX_EPOCH,
    }
}

fn weapon(key: i64, strength: f64) -> Weapon {
    Weapon {
        id: Weapon::id(key),
        name: format!("Weapon {key}"),
        strength,
        ..Default::default()
    }
}

async fn apply_definitions<M: Model>(db: &Surreal<Db>) {
    for definition in std::iter::once(M::define_table()).chain(M::define_fields()) {
        definition
            .run(db.clone())
            .await
            .unwrap_or_else(|error| panic!("`{definition}` is accepted: {error}"));
    }
}

/// A node with optional, nullable, set and map fields.
#[model(node, table = "profile")]
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: RecordId<Self>,
    pub name: String,
    pub nickname: Option<String>,
    pub bio: Option<Nullable<String>>,
    pub scores: HashSet<i64>,
    pub settings: std::collections::BTreeMap<String, i64>,
}

fn profile(key: &str, nickname: Option<&str>, bio: Option<Nullable<String>>) -> Profile {
    Profile {
        id: Profile::id(key),
        name: "Alice".to_owned(),
        nickname: nickname.map(str::to_owned),
        bio,
        scores: HashSet::from([1, 2]),
        settings: Default::default(),
    }
}

/// An object stored as fields of the model that flattens it.
#[model(object)]
#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    pub street: String,
    pub city: String,
}

/// A generic model whose parameter has the name the field generator would otherwise give its
/// arithmetic operand: the generated name must avoid it.
#[model(node, table = "counter")]
#[derive(Debug, Clone)]
pub struct Counter<__UmbrielOperand: SurrealValue + umbriel::validators::Int + 'static> {
    pub id: RecordId<Self>,
    #[orm(ty = "int")]
    pub step: __UmbrielOperand,
    pub total: u32,
}

#[model(node, table = "customer")]
#[derive(Debug, Clone)]
pub struct Customer {
    pub id: RecordId<Self>,
    pub name: String,
    #[surreal(flatten)]
    pub address: Address,
}

/// A node whose `id` has an assertion and a default but no key type.
#[model(node, table = "ticket")]
#[derive(Debug, Clone)]
pub struct Ticket {
    #[orm(assert = value().not_equal(Ticket::id("blocked")), value = "first")]
    pub id: RecordId<Self>,
    pub title: String,
}

/// An embedded object whose fields may be absent.
#[model(object)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Offsets {
    pub x: Option<i64>,
    pub y: Option<i64>,
}

/// A node holding an embedded object, for whole-object patches.
#[model(node, table = "marker")]
#[derive(Debug, Clone)]
pub struct Marker {
    pub id: RecordId<Self>,
    #[orm(nest_object = "Offsets")]
    pub offsets: Offsets,
}

/// A plan's tier, stored as its name.
#[derive(Debug, Clone, Copy, Default, PartialEq, SurrealValue)]
// Separate attributes: the derive's enum options stop at the first key they don't know.
#[surreal(crate = "umbriel::types")]
#[surreal(untagged)]
pub enum Tier {
    // `rename` rather than `value`: the derive writes `value` variants with a hardcoded
    // `surrealdb_types::` path that ignores `crate`.
    #[default]
    #[surreal(rename = "free")]
    Free,
    #[surreal(rename = "pro")]
    Pro,
}

/// A node with a stored enum.
#[model(node, table = "plan")]
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub id: RecordId<Self>,
    #[orm(ty = "'free' | 'pro'")]
    pub tier: Tier,
}

/// A node whose conversion is written by hand (`value = manual`) instead of derived.
#[model(node, table = "thermostat", value = manual)]
#[derive(Debug, Clone, PartialEq)]
pub struct Thermostat {
    pub id: RecordId<Self>,
    pub tenths: i64,
}

impl SurrealValue for Thermostat {
    fn kind_of() -> Kind {
        Kind::Object
    }

    fn into_value(self) -> Value {
        let mut object = types::Object::new();
        object.insert("id", self.id);
        object.insert("tenths", self.tenths);
        Value::Object(object)
    }

    fn from_value(value: Value) -> Result<Self, types::Error> {
        let Value::Object(mut object) = value else {
            return Err(types::Error::internal(format!(
                "expected a thermostat object, got {value:?}"
            )));
        };
        let mut field = |name: &str| object.remove(name).unwrap_or(Value::None);
        Ok(Self {
            id: RecordId::from_value(field("id"))?,
            tenths: i64::from_value(field("tenths"))?,
        })
    }
}

/// A node keyed by arrays: `ty` on `id` is the key's type, not the field's.
#[model(node, table = "reading")]
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    #[orm(ty = "array")]
    pub id: RecordId<Self>,
    pub celsius: f64,
}

mod definitions {
    use super::*;

    #[test]
    fn stored_types_are_inferred_from_rust_types() {
        assert_eq!(Alien::define_table().as_str(), "DEFINE TABLE alien;");
        assert_eq!(
            joined(&Alien::define_fields()),
            "DEFINE FIELD name ON TABLE alien TYPE string;
DEFINE FIELD age ON TABLE alien TYPE int;
DEFINE FIELD created ON TABLE alien TYPE datetime;
DEFINE FIELD lifeExpectancy ON TABLE alien TYPE duration;
DEFINE FIELD lineString ON TABLE alien TYPE geometry<line>;
DEFINE FIELD territoryArea ON TABLE alien TYPE geometry<polygon>;
DEFINE FIELD home ON TABLE alien TYPE geometry<point>;
DEFINE FIELD tags ON TABLE alien TYPE array<string>;
DEFINE FIELD ally ON TABLE alien TYPE none | record<alien>;
DEFINE FIELD weapon ON TABLE alien TYPE none | record<weapon>;
DEFINE FIELD spaceShips ON TABLE alien TYPE array<record<space_ship>>;"
        );
    }

    #[test]
    fn explicit_stored_types_are_kept() {
        assert_eq!(
            AlienWithExplicitAttributes::define_table().as_str(),
            "DEFINE TABLE alien_with_explicit_attributes;"
        );
        assert_eq!(
            joined(&AlienWithExplicitAttributes::define_fields()),
            "DEFINE FIELD id ON TABLE alien_with_explicit_attributes TYPE string;
DEFINE FIELD name ON TABLE alien_with_explicit_attributes TYPE string;
DEFINE FIELD age ON TABLE alien_with_explicit_attributes TYPE int;
DEFINE FIELD created ON TABLE alien_with_explicit_attributes TYPE datetime;
DEFINE FIELD lifeExpectancy ON TABLE alien_with_explicit_attributes TYPE duration;
DEFINE FIELD territoryArea ON TABLE alien_with_explicit_attributes TYPE geometry<polygon>;
DEFINE FIELD home ON TABLE alien_with_explicit_attributes TYPE geometry<point>;
DEFINE FIELD tags ON TABLE alien_with_explicit_attributes TYPE array<string>;
DEFINE FIELD ally ON TABLE alien_with_explicit_attributes TYPE record<alien_with_explicit_attributes>;
DEFINE FIELD weapon ON TABLE alien_with_explicit_attributes TYPE record<weapon>;
DEFINE FIELD spaceShips ON TABLE alien_with_explicit_attributes TYPE array<record<space_ship>>;"
        );
    }

    #[test]
    fn edge_fields_are_inferred_and_in_and_out_link_any_record() {
        assert_eq!(
            AlienVisitsPlanet::define_table().as_str(),
            "DEFINE TABLE visits;"
        );
        assert_eq!(
            joined(&AlienVisitsPlanet::define_fields()),
            "DEFINE FIELD in ON TABLE visits TYPE record;
DEFINE FIELD out ON TABLE visits TYPE record;
DEFINE FIELD hairColor ON TABLE visits TYPE none | string;
DEFINE FIELD timeVisited ON TABLE visits TYPE duration;
DEFINE FIELD mana ON TABLE visits TYPE none | record<planet>;
DEFINE FIELD name ON TABLE visits TYPE string;
DEFINE FIELD age ON TABLE visits TYPE int;
DEFINE FIELD created ON TABLE visits TYPE datetime;
DEFINE FIELD lifeExpectancy ON TABLE visits TYPE duration;
DEFINE FIELD lineString ON TABLE visits TYPE geometry<line>;
DEFINE FIELD multiLineString ON TABLE visits TYPE geometry<multiline>;
DEFINE FIELD polygon ON TABLE visits TYPE geometry<polygon>;
DEFINE FIELD multiPolygon ON TABLE visits TYPE geometry<multipolygon>;
DEFINE FIELD point ON TABLE visits TYPE geometry<point>;
DEFINE FIELD multiPoint ON TABLE visits TYPE geometry<multipoint>;
DEFINE FIELD geometryCollection ON TABLE visits TYPE geometry;
DEFINE FIELD territoryArea ON TABLE visits TYPE geometry<polygon>;
DEFINE FIELD home ON TABLE visits TYPE geometry<point>;
DEFINE FIELD pointExplicit ON TABLE visits TYPE geometry<point>;
DEFINE FIELD multiPointExplicit ON TABLE visits TYPE geometry<multipoint>;
DEFINE FIELD lineStringExplicit ON TABLE visits TYPE geometry<line>;
DEFINE FIELD multiLineStringExplicit ON TABLE visits TYPE geometry<multiline>;
DEFINE FIELD polygonExplicit ON TABLE visits TYPE geometry<polygon>;
DEFINE FIELD multiPolygonExplicit ON TABLE visits TYPE geometry<multipolygon>;
DEFINE FIELD geometryCollectionExplicit ON TABLE visits TYPE geometry<collection>;
DEFINE FIELD tags ON TABLE visits TYPE array<string>;
DEFINE FIELD weapon ON TABLE visits TYPE none | record<weapon>;
DEFINE FIELD spaceShips ON TABLE visits TYPE array<record<space_ship>>;"
        );
    }

    #[test]
    fn edge_fields_with_explicit_types() {
        assert_eq!(
            AlienVisitsPlanetExplicit::define_table().as_str(),
            "DEFINE TABLE visits_explicit;"
        );
        assert_eq!(
            joined(&AlienVisitsPlanetExplicit::define_fields()),
            "DEFINE FIELD id ON TABLE visits_explicit TYPE int;
DEFINE FIELD in ON TABLE visits_explicit TYPE record;
DEFINE FIELD out ON TABLE visits_explicit TYPE record;
DEFINE FIELD timeVisited ON TABLE visits_explicit TYPE duration;"
        );
    }

    #[test]
    fn nested_objects_are_stored_as_objects() {
        assert_eq!(
            joined(&Weapon::define_fields()),
            "DEFINE FIELD name ON TABLE weapon TYPE string;
DEFINE FIELD strength ON TABLE weapon TYPE float;
DEFINE FIELD created ON TABLE weapon TYPE datetime;
DEFINE FIELD rocket ON TABLE weapon TYPE object;"
        );
    }

    #[test]
    fn optional_nullable_set_and_map_types() {
        assert_eq!(
            joined(&Profile::define_fields()),
            "DEFINE FIELD name ON TABLE profile TYPE string;
DEFINE FIELD nickname ON TABLE profile TYPE none | string;
DEFINE FIELD bio ON TABLE profile TYPE none | null | string;
DEFINE FIELD scores ON TABLE profile TYPE array<int>;
DEFINE FIELD settings ON TABLE profile TYPE object;"
        );
    }

    #[test]
    fn generic_fields_take_their_declared_type() {
        assert_eq!(
            joined(&Planet::<u64>::define_fields()),
            "DEFINE FIELD name ON TABLE planet TYPE string;
DEFINE FIELD population ON TABLE planet TYPE int;
DEFINE FIELD created ON TABLE planet TYPE datetime;
DEFINE FIELD tags ON TABLE planet TYPE array<int>;
DEFINE FIELD data ON TABLE planet TYPE int;"
        );
    }

    #[test]
    fn table_settings_given_inline() {
        let expected = define_table(StudentWithGranularAttributes::table())
            .drop()
            .schemafull()
            .as_(select(All).from(Student::table()))
            .permissions(student_permissions());
        assert_eq!(
            StudentWithGranularAttributes::define_table().as_str(),
            expected.to_debug_sql()
        );
    }

    #[test]
    fn table_settings_given_as_paths() {
        let expected = define_table(StudentFnAttrs::table())
            .drop()
            .schemafull()
            .as_(as_fn())
            .permissions(student_permissions());
        assert_eq!(
            StudentFnAttrs::define_table().as_str(),
            expected.to_debug_sql()
        );
    }

    #[test]
    fn field_settings_given_inline_as_paths_and_as_calls() {
        let table = StudentWithGranularAttributes::table();
        let schema = StudentWithGranularAttributes::schema();
        let fields = StudentWithGranularAttributes::define_fields();
        let age = |name: &'static str| {
            define_field(Field::new(name))
                .on_table(table.clone())
                .type_(Kind::Int)
        };

        let inline = age("ageInlineExpr")
            .assert(cond(value().is_not(NONE)).and(value().gte(18)))
            .default(18)
            .permissions(for_permission([Create, Delete]).where_(schema.firstName.is("Alice")));
        assert_eq!(
            definitions_of(&fields, "ageInlineExpr"),
            [inline.to_debug_sql()]
        );

        let invoked = age("ageDefaultExternalFunctionInvokedExpr")
            .assert(get_age_assertion())
            .default(get_age_default_value())
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageDefaultExternalFunctionInvokedExpr"),
            [invoked.to_debug_sql()]
        );

        let with_argument = age("ageTeenExternalFunctionInvokedExpr")
            .assert(get_age_assertion())
            .default(get_age_by_group_default_value(AgeGroup::Teen))
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageTeenExternalFunctionInvokedExpr"),
            [with_argument.to_debug_sql()]
        );

        let without_permissions = age("ageSeniorExternalFunctionInvokedExpr")
            .assert(get_age_assertion())
            .default(get_age_by_group_default_value(AgeGroup::Senior));
        assert_eq!(
            definitions_of(&fields, "ageSeniorExternalFunctionInvokedExpr"),
            [without_permissions.to_debug_sql()]
        );

        let without_assertion = age("ageChildExternalFunctionInvokedExpr")
            .default(get_age_by_group_default_value(AgeGroup::Child))
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageChildExternalFunctionInvokedExpr"),
            [without_assertion.to_debug_sql()]
        );

        let default_only = age("ageAdultExternalFunctionInvokedExpr")
            .default(get_age_by_group_default_value(AgeGroup::Adult));
        assert_eq!(
            definitions_of(&fields, "ageAdultExternalFunctionInvokedExpr"),
            [default_only.to_debug_sql()]
        );

        let paths = age("ageExternalFnAttrs")
            .assert(get_age_assertion())
            .default(get_age_default_value())
            .permissions(age_permissions());
        assert_eq!(
            definitions_of(&fields, "ageExternalFnAttrs"),
            [paths.to_debug_sql()]
        );

        let week = define_field(Field::new("timeToKelowna"))
            .on_table(table.clone())
            .type_(Kind::Duration)
            .assert(value().is_not(NONE))
            .default(default_duration_value());
        assert_eq!(
            definitions_of(&fields, "timeToKelowna"),
            [week.to_debug_sql()]
        );
        assert_eq!(
            definitions_of(&fields, "timeToKelowna")[0]
                .replace("timeToKelowna", "timeToKelownaInline"),
            definitions_of(&fields, "timeToKelownaInline")[0]
        );
    }

    #[test]
    fn item_assertions_define_the_array_elements() {
        let table = StudentWithGranularAttributes::table();
        let items = define_field(Field::new("semesterCourses").all())
            .on_table(table.clone())
            .assert(value().is_not(NONE));
        assert_eq!(
            definitions_of(&StudentWithGranularAttributes::define_fields(), "semesterCourses"),
            [
                "DEFINE FIELD semesterCourses ON TABLE student_with_granular_attributes TYPE array<record<book>>;"
                    .to_owned(),
                items.to_debug_sql(),
            ]
        );
    }

    #[test]
    fn renamed_fields_are_defined_under_their_stored_name() {
        let fields = Student::define_fields();
        assert_eq!(
            definitions_of(&fields, "unoBook"),
            ["DEFINE FIELD unoBook ON TABLE student TYPE none | record<book>;"]
        );
        assert!(definitions_of(&fields, "favBook").is_empty());
    }

    #[test]
    fn relations_are_never_defined() {
        let fields = joined(&Student::define_fields());
        assert!(!fields.contains("writtenBooks"));
        assert!(!fields.contains("blogs"));
    }

    #[test]
    fn define_replaces_the_table_definition() {
        let expected = define_student_with_define_attr().to_debug_sql();
        assert_eq!(StudentWithDefineAttr::define_table().as_str(), expected);
        // Given as a path instead of a call.
        assert_eq!(StudentWithDefineFnAttr::define_table().as_str(), expected);
    }

    #[test]
    fn define_adds_the_given_field_definition() {
        let fields = StudentWithDefineAttr::define_fields();
        let schema = StudentWithDefineAttr::schema();

        let first_name = definitions_of(&fields, "firstName");
        assert_eq!(
            first_name,
            [
                "DEFINE FIELD firstName ON TABLE student_with_define_attr TYPE string;".to_owned(),
                define_first_name(&schema.firstName, StudentWithDefineAttr::table()).to_debug_sql(),
            ]
        );
        assert_eq!(
            definitions_of(&fields, "lastName")[1],
            define_last_name().to_debug_sql()
        );
        assert_eq!(
            definitions_of(&fields, "ageDefineExternalInvoke")[1],
            define_age(&schema.ageDefineExternalInvoke).to_debug_sql()
        );
    }

    #[tokio::test]
    async fn definitions_are_accepted_and_enforced() {
        let db = db().await;
        apply_definitions::<Weapon>(&db).await;

        weapon(1, 10.0)
            .save(&db)
            .await
            .expect("matches the definitions");
        let wrong_type = Raw::new("CREATE weapon:2 SET name = 1, strength = 1.0;")
            .run(db.clone())
            .await;
        assert!(wrong_type.is_err(), "a field of the wrong type is rejected");
    }

    #[tokio::test]
    async fn definitions_accept_unset_links() {
        let db = db().await;
        apply_definitions::<Alien>(&db).await;

        alien("alice", "Alice")
            .save(&db)
            .await
            .expect("a model whose links are unset matches its definitions");
    }

    #[test]
    fn id_clauses_are_defined_without_a_key_type() {
        assert_eq!(
            joined(&Ticket::define_fields()),
            "DEFINE FIELD id ON TABLE ticket DEFAULT 'first' ASSERT $value != ticket:blocked;
DEFINE FIELD title ON TABLE ticket TYPE string;"
        );
    }

    #[tokio::test]
    async fn id_assertions_and_defaults_apply_to_the_record_id() {
        let db = db().await;
        apply_definitions::<Ticket>(&db).await;

        let blocked = Raw::new("CREATE ticket:blocked SET title = 'Blocked';")
            .run(db.clone())
            .await;
        assert!(blocked.is_err(), "the assertion rejects the id");
        Raw::new("CREATE ticket:open SET title = 'Open';")
            .run(db.clone())
            .await
            .expect("an id the assertion accepts");
        Raw::new("CREATE ticket SET title = 'Defaulted';")
            .run(db.clone())
            .await
            .expect("an id from the default");

        let ids: Vec<RecordId<Ticket>> =
            read(&db, "SELECT VALUE id FROM ticket ORDER BY id;").await;
        assert_eq!(ids, [Ticket::id("first"), Ticket::id("open")]);
    }

    #[test]
    fn an_array_key_type_defines_the_id_as_a_record_id() {
        assert_eq!(
            joined(&Reading::define_fields()),
            "DEFINE FIELD id ON TABLE reading TYPE array<any>;
DEFINE FIELD celsius ON TABLE reading TYPE float;"
        );
        assert!(Reading::get_link_many_fields().is_empty());
    }

    #[tokio::test]
    async fn array_keys_are_enforced_and_round_trip() {
        let db = db().await;
        apply_definitions::<Reading>(&db).await;

        let reading = Reading {
            id: Reading::id(vec![1i64, 2]),
            celsius: 21.5,
        };
        reading.clone().save(&db).await.expect("an array key");
        assert_eq!(
            Reading::get(&db, &reading.id).await.expect("read"),
            Some(reading)
        );

        let string_key = Raw::new("CREATE reading:abc SET celsius = 1.0;")
            .run(db.clone())
            .await;
        assert!(string_key.is_err(), "the key type is enforced");
    }

    #[tokio::test]
    async fn optional_and_nullable_definitions_accept_none_and_null() {
        let db = db().await;
        apply_definitions::<Profile>(&db).await;

        profile("alice", None, None).save(&db).await.expect("NONE");
        profile("bob", Some("Bobby"), Some(Nullable::Null))
            .save(&db)
            .await
            .expect("NULL");
        profile("carol", None, Some(Nullable::Value("Hello".to_owned())))
            .save(&db)
            .await
            .expect("a value");
    }
}

mod schema {
    use super::*;

    #[test]
    fn arithmetic_on_a_model_with_a_parameter_named_like_the_operand() {
        let counter = Counter::<u8>::schema();
        // `total`'s arithmetic impls take the model's `__UmbrielOperand` and a generated operand.
        assert_eq!((counter.total.clone() + 1).to_debug_sql(), "total + 1");
        assert_eq!((&counter.total * 2).to_debug_sql(), "total * 2");
    }

    #[test]
    fn fields_are_named_after_their_stored_names() {
        assert_eq!(
            Alien::schema().lifeExpectancy.to_debug_sql(),
            "lifeExpectancy"
        );
        assert_eq!(Student::schema().unoBook.to_debug_sql(), "unoBook");
        assert_eq!(StudentLikesBook::schema().r#in.to_debug_sql(), "in");
        assert_eq!(AlienVisitsPlanet::schema().r#in.to_debug_sql(), "in");
    }

    #[test]
    fn links_traverse_into_the_linked_model() {
        let schema = Alien::schema();
        assert_eq!(schema.weapon().name.to_debug_sql(), "weapon.name");
        assert_eq!(schema.ally().name.to_debug_sql(), "ally.name");
        assert_eq!(schema.spaceShips(E).name.to_debug_sql(), "spaceShips.name");
        assert_eq!(
            schema.spaceShips([1usize]).name.to_debug_sql(),
            "spaceShips[1].name"
        );

        let apollo = SpaceShip::schema().name.equal("Apollo");
        assert_eq!(
            schema.spaceShips(apollo.clone()).name.to_debug_sql(),
            format!("spaceShips[WHERE {}].name", cond(apollo).to_debug_sql())
        );
        assert_eq!(
            Alien::schema().weapon().rocket().strength.to_debug_sql(),
            "weapon.rocket.strength"
        );
    }

    #[test]
    fn nested_objects_traverse_into_the_object() {
        assert_eq!(
            Weapon::schema().rocket().strength.to_debug_sql(),
            "rocket.strength"
        );
        assert_eq!(
            Organization::schema().time().connected.to_debug_sql(),
            "time.connected"
        );
    }

    #[test]
    fn graph_traversals_follow_edges() {
        let path = Company::with(Company::id("acme")).like__(E).user(E).name;
        assert!(
            path.to_debug_sql().contains("->like->user"),
            "{}",
            path.to_debug_sql()
        );
    }

    #[test]
    fn relations_are_aliased_by_their_stored_name() {
        let aliased = Alien::get_fields_relations_aliased();
        assert_eq!(aliased.len(), 1);
        assert_eq!(
            aliased[0].to_debug_sql(),
            "->visits->planet AS planetsToVisit"
        );
    }

    #[test]
    fn tables_and_field_lists() {
        assert_eq!(Alien::table().as_str(), "alien");
        assert_eq!(<Alien as Node>::get_table().as_str(), "alien");
        assert_eq!(AlienVisitsPlanet::table().as_str(), "visits");
        assert_eq!(<AlienVisitsPlanet as Edge>::get_table().as_str(), "visits");
        // `WeaponOld` shares `Weapon`'s table under `relax_table`.
        assert_eq!(WeaponOld::table(), Weapon::table());

        assert_eq!(
            sorted(Alien::get_linked_fields()),
            ["ally", "spaceShips", "weapon"]
        );
        assert_eq!(sorted(Alien::get_link_one_fields()), ["weapon"]);
        assert_eq!(sorted(Alien::get_link_self_fields()), ["ally"]);
        assert_eq!(
            sorted(Alien::get_link_one_and_self_fields()),
            ["ally", "weapon"]
        );
        assert_eq!(sorted(Alien::get_link_many_fields()), ["spaceShips"]);

        let serializable = sorted(Alien::get_serializable_fields());
        assert!(serializable.contains(&"lifeExpectancy".to_owned()));
        assert!(!serializable.contains(&"planetsToVisit".to_owned()));
    }

    #[test]
    fn edges_name_their_nodes() {
        fn nodes<T: Edge<In = Alien, Out = Planet<u64>>>() {}
        nodes::<AlienVisitsPlanet>();
    }
}

mod setters {
    use super::*;

    #[test]
    fn assignments_bind_values_and_write_expressions() {
        let org = Organization::schema();

        let query = org.age.equal_to(34u8).to_query().expect("valid");
        assert_eq!(query.sql(), "age = $_p0");
        assert_eq!(query.vars().len(), 1);
        assert_eq!(org.age.equal_to(34u8).to_debug_sql(), "age = 34");
        assert_eq!(
            org.age.equal_to(Field::new("age")).to_debug_sql(),
            "age = age"
        );

        let connected = org
            .time()
            .connected
            .equal_to(DateTime::<Utc>::UNIX_EPOCH)
            .to_query()
            .expect("valid");
        assert_eq!(connected.sql(), "time.connected = $_p0");
    }

    #[test]
    fn numeric_and_array_assignments() {
        let org = Organization::schema();
        assert_eq!(org.age.increment_by(1u8).to_debug_sql(), "age += 1");
        assert_eq!(org.age.decrement_by(1u8).to_debug_sql(), "age -= 1");

        let planet = Planet::<u64>::schema();
        assert_eq!(planet.tags.append(45u64).to_debug_sql(), "tags += 45");
        assert_eq!(planet.tags.remove(45u64).to_debug_sql(), "tags -= 45");
        assert_eq!(
            org.users.append(User::id("alice")).to_debug_sql(),
            "users += user:alice"
        );
    }

    #[test]
    fn patch_operations_use_json_pointers() {
        let org = Organization::schema();
        assert_eq!(
            org.name.patch_add("Acme Corp").to_debug_sql(),
            "{ op: 'add', path: '/name', value: 'Acme Corp' }"
        );
        assert_eq!(
            org.name.patch_replace("Globex").to_debug_sql(),
            "{ op: 'replace', path: '/name', value: 'Globex' }"
        );
        assert_eq!(
            org.time().connected.patch_remove().to_debug_sql(),
            "{ op: 'remove', path: '/time/connected' }"
        );
        assert!(
            org.name
                .patch_change("@@ -1,4 +1,4 @@\n te\n-s\n+x\n t\n")
                .to_debug_sql()
                .starts_with("{ op: 'change', path: '/name', value: '")
        );
    }

    #[test]
    fn patch_operations_reject_paths_with_clauses() {
        let filtered = Organization::schema()
            .users(User::schema().name.equal("Alice"))
            .name
            .patch_remove();
        assert!(filtered.to_query().is_err());

        let indexed = Organization::schema().users([0usize]).name.patch_remove();
        assert!(indexed.to_query().is_err());
    }

    /// A field of the same type is accepted as a value; one of another type does not compile
    /// (`tests/ui/fail/assign_mismatched_field.rs`), and an explicit `Field` stays untyped.
    #[test]
    fn fields_stand_for_values_of_their_own_type() {
        let org = Organization::schema();
        let input = umbriel::statements::input_of::<Organization>();
        assert_eq!(
            org.age.increment_by(input.age).to_debug_sql(),
            "age += $input.age"
        );
        assert_eq!(
            org.name.equal_to(&User::schema().name).to_debug_sql(),
            "name = name"
        );
        assert_eq!(
            org.name.equal_to(Field::new("age")).to_debug_sql(),
            "name = age"
        );

        // A generic model's specialization is part of its field types: `Planet<u64>`'s `data`
        // fits `Planet<u64>`; `Planet<u32>`'s does not compile
        // (`tests/ui/fail/field_of_another_specialization.rs`).
        let planet = Planet::<u64>::schema();
        assert_eq!(
            planet
                .data
                .equal_to(Planet::<u64>::schema().data)
                .to_debug_sql(),
            "data = data"
        );
        assert_eq!(
            umbriel::statements::update::<Planet<u64>>(Planet::<u64>::id("earth"))
                .return_value(Planet::<u64>::schema().data)
                .to_debug_sql(),
            "UPDATE planet:earth RETURN VALUE data;"
        );
    }

    /// The SDK's `PatchOps` holds a list of its `PatchOp`s: Umbriel's operations convert into it
    /// and back through the per-operation conversions, with no direct conversion of their own.
    #[test]
    fn patch_operation_lists_convert_to_and_from_the_sdks_patch_ops() {
        use surrealdb::opt::{PatchOp as SdkPatchOp, PatchOps};
        use umbriel::PatchOp;

        let org = Organization::schema();
        let ops = vec![
            org.name.patch_add("Acme Corp"),
            org.name.patch_replace("Globex"),
            org.name.patch_remove(),
        ];
        let sdk: PatchOps = ops
            .into_iter()
            .map(SdkPatchOp::try_from)
            .collect::<Result<Vec<_>, _>>()
            .expect("supported")
            .into();
        let back = sdk
            .into_iter()
            .map(PatchOp::from)
            .map(|op| op.to_debug_sql())
            .collect::<Vec<_>>();
        assert_eq!(
            back,
            [
                "{ op: 'add', path: '/name', value: 'Acme Corp' }",
                "{ op: 'replace', path: '/name', value: 'Globex' }",
                "{ op: 'remove', path: '/name' }",
            ]
        );
    }

    #[test]
    fn patch_operations_convert_to_and_from_the_sdk() {
        use surrealdb::opt::PatchOp as SdkPatchOp;
        use umbriel::{PatchOp, PatchOpConversionError};

        let org = Organization::schema();
        let add = SdkPatchOp::try_from(org.name.patch_add("Acme Corp")).expect("add");
        assert!(matches!(&add, SdkPatchOp::Add { path, .. } if path == "/name"));
        assert!(matches!(
            SdkPatchOp::try_from(org.name.patch_remove()),
            Ok(SdkPatchOp::Remove { path }) if path == "/name"
        ));
        assert!(matches!(
            SdkPatchOp::try_from(org.name.patch_replace("Globex")),
            Ok(SdkPatchOp::Replace { .. })
        ));
        assert!(matches!(
            SdkPatchOp::try_from(org.name.patch_change("@@ -1 +1 @@\n-a\n+b\n")),
            Ok(SdkPatchOp::Change { .. })
        ));
        for unsupported in [
            org.name.patch_copy(Field::new("label")),
            org.name.patch_move(Field::new("label")),
            org.name.patch_test("Acme Corp"),
        ] {
            assert!(matches!(
                SdkPatchOp::try_from(unsupported),
                Err(PatchOpConversionError::Unsupported(_))
            ));
        }
        assert_eq!(
            SdkPatchOp::try_from(Organization::schema().users([0usize]).name.patch_remove()).err(),
            Some(PatchOpConversionError::InvalidPath)
        );

        assert_eq!(
            PatchOp::from(add).to_debug_sql(),
            "{ op: 'add', path: '/name', value: 'Acme Corp' }"
        );
        assert_eq!(
            PatchOp::from(SdkPatchOp::Remove {
                path: "/name".to_owned()
            })
            .to_debug_sql(),
            "{ op: 'remove', path: '/name' }"
        );
    }
}

mod record_ids {
    use super::*;

    #[test]
    fn ids_are_written_in_their_models_table() {
        assert_eq!(Alien::id("alice").to_string(), "alien:alice");
        assert_eq!(Alien::id(7).to_string(), "alien:7");
        assert_eq!(Alien::id(vec![1i64, 2]).to_string(), "alien:[1, 2]");
        assert_eq!(Alien::id("alice").table.as_str(), "alien");
        assert_eq!(Alien::id(7).key(), &RecordIdKey::Number(7));
        assert!(matches!(
            Alien::id(uuid::Uuid::nil()).key(),
            RecordIdKey::Uuid(_)
        ));
    }

    #[test]
    fn generated_keys_have_surrealdbs_formats() {
        match RecordId::<Alien>::rand().key() {
            RecordIdKey::String(key) => {
                assert_eq!(key.len(), 20);
                assert!(
                    key.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                );
            }
            other => panic!("expected a string key, got {other:?}"),
        }
        match RecordId::<Alien>::ulid().key() {
            RecordIdKey::String(key) => assert_eq!(key.len(), 26),
            other => panic!("expected a string key, got {other:?}"),
        }
        assert!(matches!(
            RecordId::<Alien>::uuid().key(),
            RecordIdKey::Uuid(_)
        ));
        assert_ne!(RecordId::<Alien>::default(), RecordId::<Alien>::default());
    }

    #[test]
    fn decoding_checks_the_table() {
        let own = types::RecordId::new("alien", "alice");
        let other = types::RecordId::new("weapon", "alice");
        assert!(RecordId::<Alien>::from_value(Value::RecordId(own.clone())).is_ok());
        assert!(RecordId::<Alien>::from_value(Value::RecordId(other.clone())).is_err());
        assert!(RecordId::<Alien>::try_from(other).is_err());
        assert!(RecordId::<Alien>::is_value(&Value::RecordId(own)));
        assert!(RecordId::<Alien>::from_value(Value::String("alien:alice".to_owned())).is_err());
    }

    #[tokio::test]
    async fn decoding_a_model_checks_the_table_of_its_id() {
        let db = db().await;
        // A record shaped like a space ship, stored in another table.
        Raw::new("CREATE alien:apollo SET name = 'Apollo', created = d'1970-01-01T00:00:00Z';")
            .run(db.clone())
            .await
            .expect("created");
        let record: Value = read(&db, "SELECT * FROM ONLY alien:apollo;").await;
        let error = SpaceShip::from_value(record).expect_err("an alien id is not a space ship's");
        assert!(error.to_string().contains("alien"), "{error}");

        Raw::new(
            "CREATE space_ship:apollo SET name = 'Apollo', created = d'1970-01-01T00:00:00Z';",
        )
        .run(db.clone())
        .await
        .expect("created");
        let record: Value = read(&db, "SELECT * FROM ONLY space_ship:apollo;").await;
        assert_eq!(
            SpaceShip::from_value(record).expect("its own table").id,
            SpaceShip::id("apollo")
        );
    }

    #[tokio::test]
    async fn every_key_kind_round_trips_through_the_database() {
        let db = db().await;
        let mut object = types::Object::new();
        object.insert("region", "eu");
        object.insert("seq", 7);
        let ids = [
            SpaceShip::id(42),
            SpaceShip::id("apollo"),
            SpaceShip::id(uuid::Uuid::from_u128(
                0x0190_7a2e_4c1d_7b3a_8f00_0000_0000_0001,
            )),
            SpaceShip::id(vec![1i64, 2]),
            SpaceShip::id(object),
            RecordId::<SpaceShip>::rand(),
            RecordId::<SpaceShip>::ulid(),
            RecordId::<SpaceShip>::uuid(),
        ];
        for id in ids {
            let ship = SpaceShip {
                id: id.clone(),
                name: "Apollo".to_owned(),
                created: DateTime::<Utc>::UNIX_EPOCH,
            };
            let created = ship.create(&db).await.expect("created");
            assert_eq!(created.id, id, "create returns the exact id");
            let stored = SpaceShip::get(&db, &id)
                .await
                .expect("read")
                .expect("stored under that id");
            assert_eq!(stored.id, id);
        }
    }

    #[test]
    fn decoding_rejects_ranges() {
        let range = types::RecordId::new(
            "alien",
            types::RecordIdKeyRange {
                start: std::ops::Bound::Included(RecordIdKey::Number(1)),
                end: std::ops::Bound::Excluded(RecordIdKey::Number(5)),
            },
        );
        assert!(RecordId::<Alien>::from_value(Value::RecordId(range)).is_err());
    }

    #[test]
    fn ranges_render_in_the_models_table() {
        let range = RecordRange::<Alien>::from(Alien::id(1)..Alien::id(5));
        assert_eq!(range.to_debug_sql(), "alien:1..5");
        let inclusive = RecordRange::<Alien>::from(Alien::id(1)..=Alien::id(5));
        assert_eq!(inclusive.to_debug_sql(), "alien:1..=5");
    }

    #[tokio::test]
    async fn ids_round_trip_through_the_database() {
        let db = db().await;
        let ship = space_ship("apollo", "Apollo");
        ship.clone().create(&db).await.expect("created");

        let id: RecordId<SpaceShip> = read(&db, "RETURN space_ship:apollo.id;").await;
        assert_eq!(id, ship.id);
    }
}

mod links {
    use super::*;

    #[test]
    fn single_links_hold_an_id_a_fetched_record_or_nothing() {
        let sword = weapon(1, 10.0);

        let by_id = LinkOne::from(sword.id.clone());
        assert_eq!(by_id.key(), Some(&sword.id));
        assert!(by_id.value().is_none());

        let by_model = LinkOne::from(&sword);
        assert_eq!(by_model.key(), Some(&sword.id));

        let null = LinkOne::<Weapon>::default();
        assert!(null.key().is_none());
        assert!(matches!(*null, Reference::Null));
    }

    #[test]
    fn single_links_store_the_id_and_decode_any_form() {
        let sword = weapon(1, 10.0);
        let fetched = LinkOne::<Weapon>::from_value(sword.clone().into_value()).expect("record");
        assert_eq!(fetched.value().map(|weapon| weapon.strength), Some(10.0));
        assert_eq!(fetched.key(), Some(&sword.id));
        // A fetched record is stored as its id.
        assert_eq!(
            fetched.into_value(),
            Value::RecordId(sword.id.clone().into_inner())
        );

        assert!(
            LinkOne::<Weapon>::from_value(Value::None)
                .expect("none")
                .key()
                .is_none()
        );
        assert!(
            LinkOne::<Weapon>::from_value(Value::Null)
                .expect("null")
                .key()
                .is_none()
        );
        assert_eq!(LinkOne::<Weapon>::null().into_value(), Value::None);
        assert!(
            LinkOne::<Weapon>::from_value(Value::RecordId(types::RecordId::new("alien", "x")))
                .is_err()
        );
    }

    #[test]
    fn many_links_report_ids_and_fetched_records() {
        let ships = LinkMany::from(vec![SpaceShip::id("apollo"), SpaceShip::id("gemini")]);
        assert_eq!(ships.keys_truthy().len(), 2);
        assert_eq!(ships.keys().len(), 2);
        assert_eq!(ships.values_truthy_count(), 0);
        assert!(ships.keys_checked().is_empty());
        assert!(ships.values().iter().all(Option::is_none));

        let fetched = LinkMany::<SpaceShip>::from_value(Value::Array(
            vec![space_ship("apollo", "Apollo").into_value(), Value::None].into(),
        ))
        .expect("records");
        assert_eq!(fetched.values_truthy_count(), 1);
        assert_eq!(fetched.keys_checked(), [&SpaceShip::id("apollo")]);
        assert_eq!(fetched.keys(), [Some(&SpaceShip::id("apollo")), None]);

        assert!(
            LinkMany::<SpaceShip>::from_value(Value::None)
                .expect("none")
                .is_empty()
        );
    }

    #[test]
    fn relations_are_never_written() {
        assert_eq!(Relate::<User>::null().into_value(), Value::None);
        assert!(
            Relate::<User>::from_value(Value::None)
                .expect("none")
                .is_empty()
        );
    }

    async fn armed_alien(db: &Surreal<Db>) {
        let sword = weapon(1, 10.0);
        sword.clone().create(db).await.expect("weapon");
        space_ship("apollo", "Apollo")
            .create(db)
            .await
            .expect("apollo");
        space_ship("gemini", "Gemini")
            .create(db)
            .await
            .expect("gemini");
        alien("bob", "Bob").create(db).await.expect("bob");

        Alien {
            ally: LinkSelf::from(Alien::id("bob")),
            weapon: LinkOne::from(sword.id),
            space_ships: LinkMany::from(vec![SpaceShip::id("apollo"), SpaceShip::id("gemini")]),
            ..alien("alice", "Alice")
        }
        .create(db)
        .await
        .expect("alice");
    }

    fn alice() -> umbriel::statements::SelectStatement<Alien> {
        Alien::find_where(Alien::schema().name.equal("Alice"))
    }

    /// The one record `statement` reads.
    async fn one(
        statement: umbriel::statements::SelectStatement<Alien>,
        db: &Surreal<Db>,
    ) -> Alien {
        let mut found = statement.execute(db).await.expect("read");
        assert_eq!(found.len(), 1, "one record");
        found.remove(0)
    }

    #[tokio::test]
    async fn links_are_ids_until_loaded() {
        let db = db().await;
        armed_alien(&db).await;

        let alice = one(alice(), &db).await;
        assert_eq!(alice.weapon.key(), Some(&Weapon::id(1)));
        assert!(alice.weapon.value().is_none());
        assert_eq!(alice.ally.key(), Some(&Alien::id("bob")));
        assert_eq!(alice.space_ships.keys_truthy().len(), 2);
        assert_eq!(alice.space_ships.values_truthy_count(), 0);
    }

    #[tokio::test]
    async fn load_all_links_fetches_every_kind_of_link() {
        let db = db().await;
        armed_alien(&db).await;

        let alice = one(alice().load_all_links(), &db).await;
        assert_eq!(alice.weapon.value().map(|w| w.strength), Some(10.0));
        assert_eq!(
            alice.ally.value().map(|ally| ally.name.as_str()),
            Some("Bob")
        );
        let names = alice
            .space_ships
            .values_truthy()
            .into_iter()
            .map(|ship| ship.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn load_by_link_kind() {
        let db = db().await;
        armed_alien(&db).await;

        let ones = one(
            alice()
                .load_links(Alien::get_link_one_fields())
                .expect("links"),
            &db,
        )
        .await;
        assert!(ones.weapon.value().is_some());
        assert!(ones.ally.value().is_none());
        assert_eq!(ones.space_ships.values_truthy_count(), 0);

        let selfs = one(
            alice()
                .load_links(Alien::get_link_self_fields())
                .expect("links"),
            &db,
        )
        .await;
        assert!(selfs.ally.value().is_some());
        assert!(selfs.weapon.value().is_none());

        let manys = one(
            alice()
                .load_links(Alien::get_link_many_fields())
                .expect("links"),
            &db,
        )
        .await;
        assert_eq!(manys.space_ships.values_truthy_count(), 2);
        assert!(manys.weapon.value().is_none());
    }

    #[tokio::test]
    async fn loading_a_link_to_a_missing_record_gives_nothing() {
        let db = db().await;
        Alien {
            weapon: LinkOne::from(Weapon::id(404)),
            ..alien("alice", "Alice")
        }
        .create(&db)
        .await
        .expect("alice");

        let alice = one(
            alice()
                .load_links(Alien::get_link_one_fields())
                .expect("links"),
            &db,
        )
        .await;
        assert!(alice.weapon.value().is_none());
    }

    #[test]
    fn load_links_rejects_fields_that_are_not_links() {
        let result = alice().load_links(vec![Field::new("name")]);
        assert!(matches!(
            result,
            Err(umbriel::Error::FieldsUnfetchableNotARecordLink(_))
        ));
        assert!(alice().load_links(vec![Alien::schema().weapon]).is_ok());
    }
}

mod patches {
    use super::*;

    #[test]
    fn kept_fields_are_omitted_and_none_null_and_values_are_distinct() {
        let patch = ProfilePatch {
            name: Patch::Keep,
            nickname: Patch::None,
            bio: Patch::Null,
            ..Default::default()
        };
        assert_eq!(
            patch.into_setters().to_debug_sql(),
            "nickname = NONE, bio = NULL"
        );

        let patch = ProfilePatch {
            bio: Patch::Set(Some(Nullable::Value("Hello".to_owned()))),
            ..Default::default()
        };
        assert_eq!(patch.into_setters().to_debug_sql(), "bio = 'Hello'");

        assert!(ProfilePatch::default().into_setters().is_empty());
        assert!(Patch::<i64>::Keep.is_keep());
        assert!(!Patch::Set(1).is_keep());
    }

    #[tokio::test]
    async fn patch_updates_only_the_given_fields() {
        let db = db().await;
        profile("alice", Some("Al"), None)
            .create(&db)
            .await
            .expect("created");

        let patched = Profile::patch(
            &db,
            &Profile::id("alice"),
            ProfilePatch {
                name: Patch::Set("Alicia".to_owned()),
                nickname: Patch::None,
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("the record exists");
        assert_eq!(patched.name, "Alicia");
        assert_eq!(patched.nickname, None);
        assert_eq!(patched.scores, HashSet::from([1, 2]));
    }

    #[tokio::test]
    async fn patch_keeps_none_and_null_distinct() {
        let db = db().await;
        profile("alice", None, Some(Nullable::Value("Hello".to_owned())))
            .create(&db)
            .await
            .expect("created");

        let nulled = Profile::patch(
            &db,
            &Profile::id("alice"),
            ProfilePatch {
                bio: Patch::Null,
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("exists");
        assert_eq!(nulled.bio, Some(Nullable::Null));
        let is_null: bool = read(&db, "RETURN type::is_null(profile:alice.bio);").await;
        assert!(is_null);

        let removed = Profile::patch(
            &db,
            &Profile::id("alice"),
            ProfilePatch {
                bio: Patch::None,
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("exists");
        assert_eq!(removed.bio, None);
        let is_none: bool = read(&db, "RETURN type::is_none(profile:alice.bio);").await;
        assert!(is_none);
    }

    #[tokio::test]
    async fn setting_a_nested_object_replaces_it_whole() {
        let db = db().await;
        let id = Marker::id("origin");
        Marker {
            id: id.clone(),
            offsets: Offsets {
                x: Some(1),
                y: Some(2),
            },
        }
        .create(&db)
        .await
        .expect("created");
        let stored = || read::<BTreeMap<String, i64>>(&db, "RETURN marker:origin.offsets;");

        let set = |offsets: Offsets| MarkerPatch {
            offsets: Patch::Set(offsets),
            ..Default::default()
        };
        Marker::patch(
            &db,
            &id,
            set(Offsets {
                x: None,
                y: Some(3),
            }),
        )
        .await
        .expect("patched")
        .expect("the record exists");
        assert_eq!(stored().await, BTreeMap::from([("y".to_owned(), 3)]));

        Marker::patch(&db, &id, set(Offsets::default()))
            .await
            .expect("patched")
            .expect("the record exists");
        assert_eq!(stored().await, BTreeMap::new());
    }

    #[tokio::test]
    async fn removing_a_field_brings_back_only_a_default_always() {
        // Typed `DEFAULT ALWAYS` support belongs to #13; the definitions are written by hand.
        for (default, expected) in [
            ("DEFAULT 'Anon'", None),
            ("DEFAULT ALWAYS 'Anon'", Some("Anon")),
        ] {
            let db = db().await;
            Raw::new(format!(
                "DEFINE TABLE profile SCHEMALESS; \
                 DEFINE FIELD nickname ON TABLE profile TYPE option<string> {default};"
            ))
            .run(db.clone())
            .await
            .expect("defined");
            profile("alice", Some("Al"), None)
                .create(&db)
                .await
                .expect("created");

            let patched = Profile::patch(
                &db,
                &Profile::id("alice"),
                ProfilePatch {
                    nickname: Patch::None,
                    ..Default::default()
                },
            )
            .await
            .expect("patched")
            .expect("the record exists");
            assert_eq!(patched.nickname.as_deref(), expected, "{default}");
        }
    }

    #[tokio::test]
    async fn a_patch_that_keeps_everything_reads_the_record() {
        let db = db().await;
        profile("alice", Some("Al"), None)
            .create(&db)
            .await
            .expect("created");

        let read = Profile::patch(&db, &Profile::id("alice"), ProfilePatch::default())
            .await
            .expect("read")
            .expect("exists");
        assert_eq!(read.nickname.as_deref(), Some("Al"));
    }

    #[tokio::test]
    async fn patching_a_missing_record_creates_nothing() {
        let db = db().await;
        // SurrealDB 3 reports reading a table that does not exist as an error.
        Profile::define_table()
            .run(db.clone())
            .await
            .expect("table defined");
        let missing = Profile::patch(
            &db,
            &Profile::id("nobody"),
            ProfilePatch {
                name: Patch::Set("Nobody".to_owned()),
                ..Default::default()
            },
        )
        .await
        .expect("ran");
        assert!(missing.is_none());
        assert!(
            Profile::get(&db, &Profile::id("nobody"))
                .await
                .expect("read")
                .is_none()
        );
    }
}

mod flatten {
    use super::*;

    fn customer(street: &str) -> Customer {
        Customer {
            id: Customer::id("alice"),
            name: "Alice".to_owned(),
            address: Address {
                street: street.to_owned(),
                city: "Springfield".to_owned(),
            },
        }
    }

    #[test]
    fn flattened_fields_are_the_models_own() {
        assert_eq!(Customer::schema().address.street.to_debug_sql(), "street");
        assert_eq!(
            joined(&Customer::define_fields()),
            "DEFINE FIELD name ON TABLE customer TYPE string;
DEFINE FIELD street ON TABLE customer TYPE string;
DEFINE FIELD city ON TABLE customer TYPE string;"
        );
    }

    #[test]
    fn a_flat_patch_assigns_every_field_of_the_object() {
        let patch = CustomerPatch {
            address: FlatPatch::Set(Address {
                street: "2 Side St".to_owned(),
                city: "Shelbyville".to_owned(),
            }),
            ..Default::default()
        };
        assert_eq!(
            patch.into_setters().to_debug_sql(),
            "street = '2 Side St', city = 'Shelbyville'"
        );
        assert!(CustomerPatch::default().into_setters().is_empty());
    }

    #[tokio::test]
    async fn flattened_objects_round_trip() {
        let db = db().await;
        customer("1 Main St").save(&db).await.expect("saved");

        let street: String = read(&db, "RETURN customer:alice.street;").await;
        assert_eq!(street, "1 Main St");
        let found = Customer::get(&db, &Customer::id("alice"))
            .await
            .expect("read")
            .expect("exists");
        assert_eq!(found.address, customer("1 Main St").address);

        let patched = Customer::patch(
            &db,
            &Customer::id("alice"),
            CustomerPatch {
                address: FlatPatch::Set(Address {
                    street: "2 Side St".to_owned(),
                    city: "Springfield".to_owned(),
                }),
                ..Default::default()
            },
        )
        .await
        .expect("patched")
        .expect("exists");
        assert_eq!(patched.address.street, "2 Side St");
    }
}

mod record_operations {
    use super::*;

    #[tokio::test]
    async fn create_stores_a_new_record_and_fails_on_an_existing_one() {
        let db = db().await;
        let apollo = space_ship("apollo", "Apollo");

        let created = apollo.clone().create(&db).await.expect("created");
        assert_eq!(created.record_id(), apollo.record_id());
        assert_eq!(created.name, "Apollo");

        assert!(matches!(
            apollo.create(&db).await,
            Err(umbriel::Error::RecordExists {
                statement: None,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn insert_many_stores_every_record_and_fails_on_a_duplicate() {
        let db = db().await;
        let inserted = SpaceShip::insert_many(
            &db,
            vec![
                space_ship("apollo", "Apollo"),
                space_ship("gemini", "Gemini"),
            ],
        )
        .await
        .expect("inserted");
        assert_eq!(inserted.len(), 2);

        // A batch holding an existing id fails as a whole.
        let duplicate = SpaceShip::insert_many(
            &db,
            vec![
                space_ship("mercury", "Mercury"),
                space_ship("apollo", "Apollo"),
            ],
        )
        .await;
        assert!(
            matches!(
                &duplicate,
                Err(umbriel::Error::RecordExists {
                    statement: None,
                    ..
                })
            ),
            "{duplicate:?}"
        );
        let names: Vec<String> =
            read(&db, "SELECT VALUE name FROM space_ship ORDER BY name;").await;
        assert_eq!(names, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn get_range_and_get_many_read_several_records() {
        let db = db().await;
        for key in 1..=5 {
            SpaceShip {
                id: SpaceShip::id(key),
                name: format!("Ship {key}"),
                created: DateTime::<Utc>::UNIX_EPOCH,
            }
            .create(&db)
            .await
            .expect("created");
        }
        let names =
            |ships: Vec<SpaceShip>| ships.into_iter().map(|ship| ship.name).collect::<Vec<_>>();
        let range = SpaceShip::get_range(&db, SpaceShip::id(2)..SpaceShip::id(4))
            .await
            .expect("read");
        assert_eq!(names(range), ["Ship 2", "Ship 3"]);
        let inclusive = SpaceShip::get_range(&db, SpaceShip::id(4)..=SpaceShip::id(5))
            .await
            .expect("read");
        assert_eq!(names(inclusive), ["Ship 4", "Ship 5"]);

        let many = SpaceShip::get_many(
            &db,
            &[SpaceShip::id(5), SpaceShip::id(99), SpaceShip::id(1)],
        )
        .await
        .expect("read");
        let mut many = names(many);
        many.sort();
        assert_eq!(many, ["Ship 1", "Ship 5"]);
        assert!(
            SpaceShip::get_many(&db, &[])
                .await
                .expect("read")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn get_reads_a_record_by_id() {
        let db = db().await;
        space_ship("apollo", "Apollo")
            .create(&db)
            .await
            .expect("created");

        let found = SpaceShip::get(&db, &SpaceShip::id("apollo"))
            .await
            .expect("read");
        assert_eq!(found.map(|ship| ship.name), Some("Apollo".to_owned()));
        assert!(
            SpaceShip::get(&db, &SpaceShip::id("nobody"))
                .await
                .expect("read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn save_inserts_or_replaces_the_whole_record() {
        let db = db().await;
        profile("alice", Some("Al"), None)
            .save(&db)
            .await
            .expect("inserted");

        // An omitted `Option` field is stored as absent: the whole record is replaced.
        let replaced = profile("alice", None, None)
            .save(&db)
            .await
            .expect("replaced");
        assert_eq!(replaced.nickname, None);
        let absent: bool = read(&db, "RETURN type::is_none(profile:alice.nickname);").await;
        assert!(absent);
    }

    #[tokio::test]
    async fn delete_removes_a_record_and_returns_it() {
        let db = db().await;
        let apollo = space_ship("apollo", "Apollo");
        apollo.clone().create(&db).await.expect("created");
        space_ship("gemini", "Gemini")
            .create(&db)
            .await
            .expect("created");

        let deleted = apollo.delete(&db).await.expect("deleted");
        assert_eq!(deleted.map(|ship| ship.name), Some("Apollo".to_owned()));
        let deleted = SpaceShip::delete_by_id(&db, &SpaceShip::id("gemini"))
            .await
            .expect("deleted");
        assert!(deleted.is_some());
        assert!(
            SpaceShip::get(&db, &SpaceShip::id("gemini"))
                .await
                .expect("read")
                .is_none()
        );

        let missing = SpaceShip::delete_by_id(&db, &SpaceShip::id("nobody"))
            .await
            .expect("ran");
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn find_where_returns_matching_records() {
        let db = db().await;
        space_ship("apollo", "Apollo")
            .save(&db)
            .await
            .expect("saved");
        space_ship("gemini", "Gemini")
            .save(&db)
            .await
            .expect("saved");
        let SpaceShipSchema { id, name, .. } = SpaceShip::schema();

        let all: Vec<SpaceShip> = SpaceShip::find_where(id.is_not(NONE))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(all.len(), 2);

        let found = SpaceShip::find_where(name.equal("Apollo"))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, SpaceShip::id("apollo"));

        let none = SpaceShip::find_where(name.equal("Mercury"))
            .execute(&db)
            .await
            .expect("read");
        assert!(none.is_empty());
    }

    async fn weapons(db: &Surreal<Db>) {
        for strength in 0..100 {
            weapon(strength, strength as f64)
                .save(db)
                .await
                .expect("saved");
        }
    }

    #[tokio::test]
    async fn count_where_counts_matching_records() {
        let db = db().await;
        weapons(&db).await;
        let WeaponSchema { strength, .. } = Weapon::schema();

        let query = Weapon::count_where(strength.gte(50));
        assert_eq!(
            query.to_debug_sql(),
            "SELECT VALUE count FROM (SELECT count(strength >= 50) FROM weapon GROUP ALL);"
        );
        assert_eq!(query.execute(&db).await.expect("count"), 50);

        let query = Weapon::count_where(cond(strength.gte(50)).and(strength.lt(73)));
        assert_eq!(
            query.to_debug_sql(),
            "SELECT VALUE count FROM (SELECT count((strength >= 50) AND (strength < 73)) FROM weapon GROUP ALL);"
        );
        assert_eq!(query.execute(&db).await.expect("count"), 23);

        let none = Weapon::count_where(strength.gt(1000));
        assert_eq!(none.execute(&db).await.expect("count"), 0);
    }

    #[tokio::test]
    async fn count_all_counts_every_record() {
        let db = db().await;
        // SurrealDB 3 reports reading a table that does not exist as an error.
        Weapon::define_table()
            .run(db.clone())
            .await
            .expect("table defined");
        let query = Weapon::count_all();
        assert_eq!(
            query.to_debug_sql(),
            "SELECT VALUE count FROM (SELECT count() FROM weapon GROUP ALL);"
        );
        assert_eq!(query.execute(&db).await.expect("count"), 0);

        weapons(&db).await;
        assert_eq!(query.execute(&db).await.expect("count"), 100);
    }

    #[tokio::test]
    async fn delete_where_removes_matching_records() {
        let db = db().await;
        space_ship("apollo", "Apollo 11")
            .save(&db)
            .await
            .expect("saved");
        space_ship("gemini", "Gemini 4")
            .save(&db)
            .await
            .expect("saved");
        let SpaceShipSchema { name, .. } = SpaceShip::schema();

        SpaceShip::delete_where(name.contains("Apollo"))
            .run(db.clone())
            .await
            .expect("deleted");

        let left = SpaceShip::find_where(name.is_not(NONE))
            .execute(&db)
            .await
            .expect("read");
        assert_eq!(
            left.into_iter().map(|ship| ship.name).collect::<Vec<_>>(),
            ["Gemini 4"]
        );
    }

    #[tokio::test]
    async fn generic_models_round_trip() {
        let db = db().await;
        let earth = Planet::<u64> {
            id: Planet::id("earth"),
            name: "Earth".to_owned(),
            population: 8,
            tags: vec![1, 2],
            data: 42,
            ..Default::default()
        };
        earth.save(&db).await.expect("saved");

        let found = Planet::<u64>::get(&db, &Planet::id("earth"))
            .await
            .expect("read")
            .expect("exists");
        assert_eq!((found.data, found.tags), (42, vec![1, 2]));
    }

    #[tokio::test]
    async fn models_with_every_stored_type_round_trip() {
        let db = db().await;
        alien("alice", "Alice").save(&db).await.expect("saved");

        let found = Alien::get(&db, &Alien::id("alice"))
            .await
            .expect("read")
            .expect("exists");
        assert_eq!(found.life_expectancy, Duration::from_secs(60));
        assert_eq!(found.home, geo::Point::new(1.0, 2.0));
        assert_eq!(found.territory_area, polygon());
        assert_eq!(found.line_string, line_string());
        assert_eq!(found.created, DateTime::<Utc>::UNIX_EPOCH);
    }
}

mod relations {
    use super::*;

    #[tokio::test]
    async fn relate_connects_two_nodes_and_relations_read_them_back() {
        let db = db().await;
        let acme = Company {
            id: Company::id("acme"),
            name: "Acme Corp".to_owned(),
            ..Default::default()
        };
        let alice = User {
            id: User::id("alice"),
            name: "Alice".to_owned(),
            ..Default::default()
        };
        acme.clone().create(&db).await.expect("acme");
        alice.clone().create(&db).await.expect("alice");

        let path = Company::with(acme.id.clone())
            .like__(E)
            .user(alice.id.clone());
        let edge = relate::<CompanyLikeUser>(path)
            .content(Like {
                id: CompanyLikeUser::id("acme_alice"),
                r#in: LinkOne::from(acme.id.clone()),
                out: LinkOne::from(alice.id.clone()),
                time: Time {
                    connected: DateTime::<Utc>::UNIX_EPOCH,
                },
            })
            .execute(&db)
            .await
            .expect("related")
            .pop()
            .expect("the edge is returned");
        assert_eq!(edge.out.key(), Some(&alice.id));
        assert_eq!(edge.r#in.key(), Some(&acme.id));

        let company: Company = read(
            &db,
            "SELECT *, ->like->user.* AS devs FROM ONLY company:acme;",
        )
        .await;
        let devs = company
            .devs
            .values_truthy()
            .into_iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(devs, ["Alice"]);

        // A relation is never stored on the node.
        let stored: bool = read(&db, "RETURN type::is_none(company:acme.devs);").await;
        assert!(stored);
    }
}

mod declared_in_functions {
    use super::*;

    #[tokio::test]
    async fn a_model_declared_in_a_function_works() {
        #[model(node, table = "pet")]
        #[derive(Debug, Clone)]
        struct Pet {
            id: RecordId<Self>,
            name: String,
        }

        let db = db().await;
        let rex = Pet {
            id: Pet::id("rex"),
            name: "Rex".to_owned(),
        };
        rex.clone().save(&db).await.expect("saved");

        let found = Pet::get(&db, &rex.id).await.expect("read");
        assert_eq!(found.map(|found| found.name), Some("Rex".to_owned()));
        assert_eq!(Pet::schema().name.to_debug_sql(), "name");
    }
}

mod conversions {
    use super::*;

    #[tokio::test]
    async fn enums_are_stored_and_decoded() {
        let db = db().await;
        apply_definitions::<Plan>(&db).await;
        let acme = Plan {
            id: Plan::id("acme"),
            tier: Tier::Pro,
        };
        acme.clone().create(&db).await.expect("created");

        let stored: String = read(&db, "RETURN plan:acme.tier;").await;
        assert_eq!(stored, "pro");
        assert_eq!(Plan::get(&db, &acme.id).await.expect("read"), Some(acme));
        assert_eq!(
            Tier::from_value(Value::String("free".to_owned())).expect("a tier"),
            Tier::Free
        );

        let unknown = Raw::new("CREATE plan:globex SET tier = 'gold';")
            .run(db.clone())
            .await;
        assert!(unknown.is_err(), "the literal type rejects other names");
    }

    #[tokio::test]
    async fn a_hand_written_conversion_is_used() {
        let db = db().await;
        assert_eq!(
            joined(&Thermostat::define_fields()),
            "DEFINE FIELD tenths ON TABLE thermostat TYPE int;"
        );
        apply_definitions::<Thermostat>(&db).await;
        let hall = Thermostat {
            id: Thermostat::id("hall"),
            tenths: 215,
        };

        assert_eq!(hall.clone().create(&db).await.expect("created"), hall);
        let stored: i64 = read(&db, "RETURN thermostat:hall.tenths;").await;
        assert_eq!(stored, 215);
        assert_eq!(
            Thermostat::get(&db, &hall.id).await.expect("read"),
            Some(hall)
        );
    }
}
