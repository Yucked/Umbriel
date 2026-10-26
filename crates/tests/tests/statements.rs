//! Statements: every builder's rendering, checked against SurrealDB's parser, and what the
//! statements do against the in-memory engine.

use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};
use surrealdb::{
    Surreal,
    engine::local::Db,
    types::{Kind, SurrealValue, Value},
};
use umbriel::{
    AliasName, Aliasable, All,
    CrudType::*,
    E, Field, JwtAlgorithm, Model, NONE, Namespace, Node, Operatable, Param, Patchable, Raw,
    Render, ReturnType, ReturnableDefault, ReturnableSelect, ReturnableStandard, Runnable,
    SchemaGetter, SetterArray, SetterAssignable, SetterNumeric, SurrealCrud, SurrealCrudNode,
    Table, ValueLike, arr, block, chain, cond, count,
    functions::{math, search, string},
    statements::{
        AnalyzerFilter, SnowballLanguage, Tokenizer, UserRole, begin_transaction, break_,
        continue_, create, create_only, create_v2, define_access, define_analyzer, define_database,
        define_event, define_field, define_function, define_index, define_namespace, define_param,
        define_table, define_user, delete, delete_only, for_, for_permission, fulltext_analyzer,
        if_, info_for, insert, jwt_access, let_, live, order, record_access, relate, relate_only,
        remove_access, remove_analyzer, remove_database, remove_event, remove_field,
        remove_function, remove_index, remove_namespace, remove_param, remove_table, remove_user,
        return_, select, select_value, show_changes_for_table, sleep, throw, update, update_only,
        use_,
    },
    updater, value,
};
use umbriel_tests::*;

fn space_ship(key: &str, name: &str) -> SpaceShip {
    SpaceShip {
        id: SpaceShip::id(key),
        name: name.to_owned(),
        created: DateTime::<Utc>::UNIX_EPOCH,
    }
}

fn organization(key: &str, name: &str, age: u8) -> Organization {
    Organization {
        id: Organization::id(key),
        name: name.to_owned(),
        age,
        ..Default::default()
    }
}

async fn run(db: &Surreal<Db>, statement: impl Runnable + Sync) {
    statement.run(db.clone()).await.expect("statement succeeds");
}

async fn ships(db: &Surreal<Db>) -> Vec<String> {
    read(db, "SELECT VALUE name FROM space_ship ORDER BY name;").await
}

mod create {
    use super::*;

    #[test]
    fn content_binds_the_whole_record() {
        let ship = space_ship("apollo", "Apollo");
        let query = create::<SpaceShip>()
            .content(ship.clone())
            .to_query()
            .expect("valid");
        assert_eq!(query.sql(), "CREATE space_ship CONTENT $_p0;");
        assert_eq!(query.vars().get("_p0"), Some(&ship.into_value()));
        assert_parses(query.sql());
    }

    #[test]
    fn set_assigns_fields() {
        let schema = SpaceShip::schema();
        assert_sql(
            &create::<SpaceShip>().set([
                schema.id.equal_to(SpaceShip::id("apollo")),
                schema.name.equal_to("Apollo"),
            ]),
            "CREATE space_ship SET id = space_ship:apollo, name = 'Apollo';",
        );
        assert_sql(
            &create::<SpaceShip>().set(schema.name.equal_to("Apollo")),
            "CREATE space_ship SET name = 'Apollo';",
        );
    }

    #[test]
    fn only_return_types_and_timeout() {
        let name = SpaceShip::schema().name;
        let base = || create_only::<SpaceShip>().set(name.equal_to("Apollo"));
        assert_sql(
            &base()
                .return_type(ReturnType::None)
                .timeout(Duration::from_secs(5)),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN NONE TIMEOUT 5s;",
        );
        assert_sql(
            &base().return_type(ReturnType::Before),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN BEFORE;",
        );
        assert_sql(
            &base().return_type(ReturnType::After),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN AFTER;",
        );
        assert_sql(
            &base().return_type(ReturnType::Diff),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN DIFF;",
        );
        assert_sql(
            &base().return_type(vec![ValueLike::from(Field::new("name"))]),
            "CREATE ONLY space_ship SET name = 'Apollo' RETURN name;",
        );
    }

    #[test]
    fn create_v2_takes_content_or_assignments() {
        let name = SpaceShip::schema().name;
        assert_sql(
            &create_v2::<SpaceShip>(name.equal_to("Apollo")).timeout(Duration::from_secs(1)),
            "CREATE space_ship SET name = 'Apollo' TIMEOUT 1s;",
        );
        assert_sql(
            &create_v2::<SpaceShip>([name.equal_to("Apollo")]).return_type(ReturnType::None),
            "CREATE space_ship SET name = 'Apollo' RETURN NONE;",
        );
        let content = create_v2(space_ship("apollo", "Apollo"))
            .to_query()
            .expect("valid");
        assert_eq!(content.sql(), "CREATE space_ship CONTENT $_p0;");
    }

    #[tokio::test]
    async fn content_creates_the_record_once() {
        let db = db().await;
        let ship = space_ship("apollo", "Apollo");

        let created = create::<SpaceShip>()
            .content(ship.clone())
            .get_one(db.clone())
            .await
            .expect("created");
        assert_eq!(created.id, ship.id);
        assert!(
            create::<SpaceShip>()
                .content(ship)
                .run(db.clone())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_database_generates_a_missing_id() {
        let db = db().await;
        let schema = SpaceShip::schema();
        let created = create::<SpaceShip>()
            .set([
                schema.name.equal_to("Apollo"),
                schema.created.equal_to(DateTime::<Utc>::UNIX_EPOCH),
            ])
            .get_one(db.clone())
            .await
            .expect("created");
        assert_eq!(created.id.table.as_str(), "space_ship");
        assert_eq!(created.name, "Apollo");
    }

    #[tokio::test]
    async fn return_projections_select_the_returned_fields() {
        let db = db().await;
        let returned = create::<SpaceShip>()
            .content(space_ship("apollo", "Apollo"))
            .return_one_projections::<BTreeMap<String, String>>(db.clone(), Field::new("name"))
            .await
            .expect("created")
            .expect("returned");
        assert_eq!(
            returned,
            BTreeMap::from([("name".to_owned(), "Apollo".to_owned())])
        );

        let none = create::<SpaceShip>()
            .content(space_ship("gemini", "Gemini"))
            .return_type(ReturnType::None)
            .return_many_explicit::<Value>(db.clone())
            .await
            .expect("created");
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn created_records_return_their_links_loaded() {
        let db = db().await;
        User {
            id: User::id("alice"),
            name: "Alice".to_owned(),
            ..Default::default()
        }
        .create(&db)
        .await
        .expect("alice");

        let acme = Organization {
            users: vec![User::id("alice")].into(),
            ..organization("acme", "Acme Corp", 10)
        };
        let created = create::<Organization>()
            .content(acme)
            .load_link_manys()
            .expect("links")
            .get_one(db.clone())
            .await
            .expect("created");
        let names = created
            .users
            .values_truthy()
            .into_iter()
            .map(|user| user.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Alice"]);

        let explicit = create::<SpaceShip>()
            .content(space_ship("apollo", "Apollo"))
            .return_one_explicit::<BTreeMap<String, Value>>(db.clone())
            .await
            .expect("created")
            .expect("returned");
        assert!(explicit.contains_key("id"));
    }
}

mod insert {
    use super::*;

    #[test]
    fn records_become_rows_over_their_sorted_fields() {
        let query = insert(space_ship("apollo", "Apollo"))
            .to_query()
            .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO space_ship (created, id, name) VALUES ($_p0, $_p1, $_p2);"
        );
        assert_parses(query.sql());

        let query = insert(vec![
            space_ship("apollo", "Apollo"),
            space_ship("gemini", "Gemini"),
        ])
        .to_query()
        .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO space_ship (created, id, name) VALUES ($_p0, $_p1, $_p2), ($_p3, $_p4, $_p5);"
        );
        assert_eq!(
            query.vars().get("_p4"),
            Some(&SpaceShip::id("gemini").into_value())
        );
    }

    #[test]
    fn duplicate_keys_update_and_return_types() {
        let query = insert(space_ship("apollo", "Apollo"))
            .on_duplicate_key_update(updater(Field::new("name")).equal("Apollo 2"))
            .return_type(ReturnType::None)
            .to_query()
            .expect("valid");
        assert_eq!(
            query.sql(),
            "INSERT INTO space_ship (created, id, name) VALUES ($_p0, $_p1, $_p2) \
             ON DUPLICATE KEY UPDATE name = $_p3 RETURN NONE;"
        );
        assert_parses(query.sql());

        let name = SpaceShip::schema().name;
        let query = insert(space_ship("apollo", "Apollo"))
            .on_duplicate_key_update(vec![
                updater(Field::new("visits")).increment_by(1),
                updater(Field::new("tags")).append("seen"),
            ])
            .on_duplicate_key_update(name.equal_to("Apollo"))
            .to_query()
            .expect("valid");
        assert!(
            query
                .sql()
                .ends_with("ON DUPLICATE KEY UPDATE visits += $_p3, tags += $_p4, name = $_p5;")
        );
    }

    #[test]
    fn a_select_copies_records() {
        assert_sql(
            &insert::<SpaceShip>(select(All).from(Table::from("old_ship"))),
            "INSERT INTO space_ship (SELECT * FROM old_ship);",
        );
    }

    #[tokio::test]
    async fn inserts_one_or_many_records() {
        let db = db().await;
        let inserted = insert(vec![
            space_ship("apollo", "Apollo"),
            space_ship("gemini", "Gemini"),
        ])
        .return_many(db.clone())
        .await
        .expect("inserted");
        assert_eq!(inserted.len(), 2);

        let one = insert(space_ship("mercury", "Mercury"))
            .get_one(db.clone())
            .await
            .expect("inserted");
        assert_eq!(one.name, "Mercury");
        assert_eq!(ships(&db).await, ["Apollo", "Gemini", "Mercury"]);
    }

    #[tokio::test]
    async fn duplicate_keys_update_the_existing_record() {
        let db = db().await;
        run(&db, insert(space_ship("apollo", "Apollo"))).await;
        run(
            &db,
            insert(space_ship("apollo", "Apollo"))
                .on_duplicate_key_update(updater(Field::new("name")).equal("Apollo 2")),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo 2"]);
    }

    #[tokio::test]
    async fn inserting_from_a_select_copies_records() {
        let db = db().await;
        run(
            &db,
            Raw::new(
                "CREATE old_ship:apollo SET name = 'Apollo', created = d'1970-01-01T00:00:00Z';",
            ),
        )
        .await;
        let copied = insert::<SpaceShip>(
            select(vec![
                ValueLike::from(Field::new("name")),
                ValueLike::from(Field::new("created")),
            ])
            .from(Table::from("old_ship")),
        )
        .return_many(db.clone())
        .await
        .expect("copied");
        assert_eq!(copied.len(), 1);
        assert_eq!(copied[0].name, "Apollo");
    }
}

mod update {
    use super::*;

    #[test]
    fn set_assignments() {
        let org = Organization::schema();
        let acme = Organization::id("acme");
        assert_sql(
            &update::<Organization>(&acme).set(org.age.increment_by(1u8)),
            "UPDATE organization:acme SET age += 1;",
        );
        assert_sql(
            &update::<Organization>(&acme)
                .set([org.name.equal_to("Acme Corp"), org.age.decrement_by(2u8)]),
            "UPDATE organization:acme SET name = 'Acme Corp', age -= 2;",
        );
        assert_sql(
            &update::<Organization>(&acme).set(org.users.append(User::id("alice"))),
            "UPDATE organization:acme SET users += user:alice;",
        );
        assert_sql(
            &update::<Organization>(&acme).set(org.users.remove(User::id("alice"))),
            "UPDATE organization:acme SET users -= user:alice;",
        );
    }

    #[test]
    fn element_assignments_render() {
        let user = User::schema();
        assert_sql(
            &update::<User>(User::id("alice")).set(user.tags.at(0).equal_to("rust")),
            "UPDATE user:alice SET tags[0] = 'rust';",
        );
        assert_sql(
            &update::<User>(User::id("alice"))
                .set(user.tags.filter(Param::new("this").equal("go")).equal_to("rust")),
            "UPDATE user:alice SET tags[WHERE $this = 'go'] = 'rust';",
        );
    }

    #[tokio::test]
    async fn element_assignments_change_one_element() {
        let db = db().await;
        User {
            id: User::id("alice"),
            name: "Alice".to_owned(),
            tags: vec!["go".to_owned(), "sql".to_owned()],
            ..Default::default()
        }
        .create(&db)
        .await
        .expect("alice");
        let tags = User::schema().tags;
        run(&db, update::<User>(User::id("alice")).set(tags.at(1).equal_to("rust"))).await;
        let stored: Vec<String> = read(&db, "RETURN user:alice.tags;").await;
        assert_eq!(stored, ["go", "rust"]);
        run(
            &db,
            update::<User>(User::id("alice"))
                .set(tags.filter(Param::new("this").equal("go")).equal_to("ts")),
        )
        .await;
        let stored: Vec<String> = read(&db, "RETURN user:alice.tags;").await;
        assert_eq!(stored, ["ts", "rust"]);
    }

    #[test]
    fn targets_clauses_and_options() {
        let org = Organization::schema();
        assert_sql(
            &update_only::<Organization>(Organization::id("acme"))
                .set(org.age.equal_to(30u8))
                .where_(org.age.lt(30))
                .return_type(ReturnType::Diff)
                .timeout(Duration::from_secs(1)),
            "UPDATE ONLY organization:acme SET age = 30 WHERE age < 30 RETURN DIFF TIMEOUT 1s;",
        );
        assert_sql(
            &update::<Organization>(Organization::table())
                .set(org.age.increment_by(1u8))
                .where_(cond(org.age.gte(18)).and(org.name.is_not(NONE))),
            "UPDATE organization SET age += 1 WHERE (age >= 18) AND (name IS NOT NONE);",
        );
        // A model instance targets its own record.
        assert_sql(
            &update::<Organization>(organization("acme", "Acme Corp", 1))
                .set(org.age.equal_to(2u8)),
            "UPDATE organization:acme SET age = 2;",
        );
    }

    #[test]
    fn targets_outside_the_models_table_are_rejected() {
        let org = Organization::schema();
        assert_render_error(
            &update::<Organization>(User::id("alice")).set(org.age.equal_to(1u8)),
            "does not belong",
        );
        assert_render_error(
            &update::<Organization>(Table::from("user")).set(org.age.equal_to(1u8)),
            "is not the table",
        );
    }

    #[test]
    fn content_merge_replace_and_patch() {
        let acme = Organization::id("acme");
        let content = update::<Organization>(&acme)
            .content(organization("acme", "Acme Corp", 1))
            .to_query()
            .expect("valid");
        assert_eq!(content.sql(), "UPDATE $_p0 CONTENT $_p1;");

        let merge = update::<Organization>(&acme)
            .merge(BTreeMap::from([("name".to_owned(), "Globex".to_owned())]))
            .to_query()
            .expect("valid");
        assert_eq!(merge.sql(), "UPDATE $_p0 MERGE $_p1;");

        let replace = update::<Organization>(&acme)
            .replace(BTreeMap::from([("name".to_owned(), "Globex".to_owned())]))
            .to_query()
            .expect("valid");
        assert_eq!(replace.sql(), "UPDATE $_p0 REPLACE $_p1;");
        for query in [content, merge, replace] {
            assert_parses(query.sql());
        }

        let org = Organization::schema();
        assert_sql(
            &update::<Organization>(&acme).patch(org.name.patch_replace("Globex")),
            "UPDATE organization:acme PATCH [{ op: 'replace', path: '/name', value: 'Globex' }];",
        );
        assert_sql(
            &update::<Organization>(&acme).patch(vec![
                org.name.patch_add("Globex"),
                org.time().connected.patch_remove(),
            ]),
            "UPDATE organization:acme PATCH [{ op: 'add', path: '/name', value: 'Globex' }, \
             { op: 'remove', path: '/time/connected' }];",
        );
    }

    async fn acme(db: &Surreal<Db>) {
        organization("acme", "Acme Corp", 10)
            .create(db)
            .await
            .expect("acme");
    }

    #[tokio::test]
    async fn numeric_and_array_assignments_change_the_record() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let id = Organization::id("acme");

        let updated = update::<Organization>(&id)
            .set([
                org.age.increment_by(5u8),
                org.users.append(User::id("alice")),
            ])
            .get_one(db.clone())
            .await
            .expect("updated");
        assert_eq!(updated.age, 15);
        assert_eq!(updated.users.keys_truthy(), [&User::id("alice")]);

        let updated = update::<Organization>(&id)
            .set([
                org.age.decrement_by(3u8),
                org.users.remove(User::id("alice")),
            ])
            .get_one(db.clone())
            .await
            .expect("updated");
        assert_eq!(updated.age, 12);
        assert!(updated.users.is_empty());
    }

    #[tokio::test]
    async fn conditions_choose_the_updated_records() {
        let db = db().await;
        acme(&db).await;
        organization("globex", "Globex", 40)
            .create(&db)
            .await
            .expect("globex");
        let org = Organization::schema();

        let updated = update::<Organization>(Organization::table())
            .set(org.age.increment_by(1u8))
            .where_(org.age.gt(20))
            .return_many(db.clone())
            .await
            .expect("updated");
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].age, 41);
    }

    #[tokio::test]
    async fn content_replaces_and_merge_combines() {
        let db = db().await;
        acme(&db).await;
        let id = Organization::id("acme");

        let replaced = update::<Organization>(&id)
            .content(organization("acme", "Acme Corp", 99))
            .get_one(db.clone())
            .await
            .expect("replaced");
        assert_eq!(replaced.age, 99);

        let merged = update::<Organization>(&id)
            .merge(BTreeMap::from([("name".to_owned(), "Globex".to_owned())]))
            .get_one(db.clone())
            .await
            .expect("merged");
        assert_eq!((merged.name.as_str(), merged.age), ("Globex", 99));

        // REPLACE drops fields the replacement lacks.
        run(
            &db,
            update::<Organization>(&id)
                .replace(BTreeMap::from([("name".to_owned(), "Initech".to_owned())])),
        )
        .await;
        let absent: bool = read(&db, "RETURN type::is_none(organization:acme.age);").await;
        assert!(absent);
    }

    #[tokio::test]
    async fn patch_operations_change_the_record() {
        let db = db().await;
        acme(&db).await;
        let id = Organization::id("acme");
        let org = Organization::schema();

        let patched = update::<Organization>(&id)
            .patch(org.name.patch_replace("Globex"))
            .get_one(db.clone())
            .await
            .expect("patched");
        assert_eq!(patched.name, "Globex");

        run(
            &db,
            update::<Organization>(&id).patch(org.time().connected.patch_remove()),
        )
        .await;
        let removed: bool = read(
            &db,
            "RETURN type::is_none(organization:acme.time.connected);",
        )
        .await;
        assert!(removed);
    }

    #[tokio::test]
    async fn return_before_gives_the_previous_state() {
        let db = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let before = update::<Organization>(Organization::id("acme"))
            .set(org.age.equal_to(50u8))
            .return_many_before(db.clone())
            .await
            .expect("updated");
        assert_eq!(before[0].age, 10);
    }
}

mod delete {
    use super::*;

    #[test]
    fn targets_clauses_and_options() {
        let org = Organization::schema();
        assert_sql(
            &delete::<Organization>(Organization::id("acme")),
            "DELETE organization:acme;",
        );
        assert_sql(
            &delete::<Organization>(Organization::table())
                .where_(org.age.lt(18))
                .return_type(ReturnType::Before)
                .timeout(Duration::from_secs(2)),
            "DELETE organization WHERE age < 18 RETURN BEFORE TIMEOUT 2s;",
        );
        assert_sql(
            &delete_only::<Organization>(Organization::id("acme")),
            "DELETE ONLY organization:acme;",
        );
        assert_render_error(
            &delete::<Organization>(User::id("alice")),
            "does not belong",
        );
    }

    #[tokio::test]
    async fn deletes_by_id_or_condition() {
        let db = db().await;
        organization("acme", "Acme Corp", 10)
            .create(&db)
            .await
            .expect("acme");
        organization("globex", "Globex", 40)
            .create(&db)
            .await
            .expect("globex");
        organization("initech", "Initech", 50)
            .create(&db)
            .await
            .expect("initech");
        let org = Organization::schema();

        let deleted = delete::<Organization>(Organization::id("acme"))
            .return_type(ReturnType::Before)
            .return_many(db.clone())
            .await
            .expect("deleted");
        assert_eq!(deleted[0].name, "Acme Corp");

        run(
            &db,
            delete::<Organization>(Organization::table()).where_(org.age.gt(45)),
        )
        .await;
        let left: Vec<String> = read(&db, "SELECT VALUE name FROM organization;").await;
        assert_eq!(left, ["Globex"]);
    }
}

mod relate {
    use super::*;

    fn acme_likes_alice() -> Field {
        Company::with(Company::id("acme"))
            .like__(E)
            .user(User::id("alice"))
            .into()
    }

    #[test]
    fn relates_records_subqueries_and_options() {
        assert_sql(
            &relate::<CompanyLikeUser>(acme_likes_alice()).return_type(ReturnType::None),
            "RELATE company:acme->like->user:alice RETURN NONE;",
        );
        assert_sql(
            &relate_only::<CompanyLikeUser>(acme_likes_alice()).set(
                CompanyLikeUser::schema()
                    .time()
                    .connected
                    .equal_to(Field::new("time::now()")),
            ),
            "RELATE ONLY company:acme->like->user:alice SET time.connected = time::now();",
        );
        assert_sql(
            &relate::<CompanyLikeUser>(
                Company::with(select(All).from(Company::table()))
                    .like__(E)
                    .user(select(All).from(User::table())),
            )
            .timeout(Duration::from_secs(1)),
            "RELATE (SELECT * FROM company)->like->(SELECT * FROM user) TIMEOUT 1s;",
        );
        let content = relate::<CompanyLikeUser>(acme_likes_alice())
            .content(Like::default())
            .to_query()
            .expect("valid");
        assert_eq!(content.sql(), "RELATE $_p0->like->$_p1 CONTENT $_p2;");
        assert_parses(content.sql());
    }

    #[test]
    fn ids_of_another_table_are_rejected() {
        let path: Field = Company::with(User::id("alice"))
            .like__(E)
            .user(User::id("bob"))
            .into();
        assert_render_error(&relate::<CompanyLikeUser>(path), "does not belong to table");
    }

    #[tokio::test]
    async fn relating_creates_edges_between_every_pair() {
        let db = db().await;
        for (key, name) in [("acme", "Acme Corp"), ("globex", "Globex")] {
            Company {
                id: Company::id(key),
                name: name.to_owned(),
                ..Default::default()
            }
            .create(&db)
            .await
            .expect("company");
        }
        for (key, name) in [("alice", "Alice"), ("bob", "Bob")] {
            User {
                id: User::id(key),
                name: name.to_owned(),
                ..Default::default()
            }
            .create(&db)
            .await
            .expect("user");
        }

        let edges = relate::<CompanyLikeUser>(
            Company::with(select(All).from(Company::table()))
                .like__(E)
                .user(select(All).from(User::table())),
        )
        .set(
            CompanyLikeUser::schema()
                .time()
                .connected
                .equal_to(Field::new("time::now()")),
        )
        .return_many(db.clone())
        .await
        .expect("related");
        assert_eq!(edges.len(), 4);

        let liked: Vec<String> = read(
            &db,
            "SELECT VALUE ->like->user.name FROM ONLY company:acme;",
        )
        .await;
        assert_eq!(liked.len(), 2);
    }
}

mod select {
    use super::*;

    #[test]
    fn every_clause_in_order() {
        let user = Table::from("user");
        let age = Field::new("age");
        assert_sql(
            &select(All)
                .omit(Field::new("password"))
                .from(&user)
                .with_index(Field::new("user_age"))
                .where_(age.gte(18))
                .order_by(order(&age).numeric().desc())
                .order_by(order(Field::new("name")).collate().asc())
                .limit(10)
                .start(5)
                .fetch(Field::new("account"))
                .timeout(Duration::from_secs(5))
                .explain_full(),
            "SELECT * OMIT password FROM user WITH INDEX user_age WHERE age >= 18 \
             ORDER BY age NUMERIC DESC, name COLLATE ASC LIMIT 10 START AT 5 FETCH account \
             TIMEOUT 5s EXPLAIN FULL;",
        );
        assert_sql(
            &select(All).from(&user).with_no_index().explain(),
            "SELECT * FROM user WITH NOINDEX EXPLAIN;",
        );
    }

    #[test]
    fn projections_and_targets() {
        let user = Table::from("user");
        let country = Field::new("country");
        assert_sql(
            &select(arr![count!().__as__(AliasName::new("total")), &country])
                .from(&user)
                .group_by(&country),
            "SELECT count() AS total, country FROM user GROUP BY country;",
        );
        assert_sql(
            &select(math::sum!(Field::new("age")))
                .from(&user)
                .group_all(),
            "SELECT math::sum(age) FROM user GROUP ALL;",
        );
        assert_sql(
            &select(&[Field::new("name"), Field::new("age")]).from_only(User::id("alice")),
            "SELECT name, age FROM ONLY user:alice;",
        );
        assert_sql(
            &select_value(Field::new("name")).from(vec![User::id("alice"), User::id("bob")]),
            "SELECT VALUE name FROM user:alice, user:bob;",
        );
        assert_sql(
            &select(All).from(&[Table::from("user"), Table::from("company")]),
            "SELECT * FROM user, company;",
        );
        assert_sql(
            &select(All).from(select(All).from(&user).limit(1)),
            "SELECT * FROM (SELECT * FROM user LIMIT 1);",
        );
        assert_sql(
            &select(All).from(User::id(1)..User::id(5)),
            "SELECT * FROM user:1..5;",
        );
        assert_sql(
            &select(All).from(&user).split(Field::new("emails")),
            "SELECT * FROM user SPLIT emails;",
        );
    }

    #[test]
    fn rejected_combinations() {
        let user = Table::from("user");
        assert_render_error(
            &select(All)
                .from(&user)
                .split(Field::new("emails"))
                .group_all(),
            "SPLIT and GROUP cannot be combined",
        );
        assert_render_error(
            &select(All)
                .from(&user)
                .split(Field::new("emails"))
                .group_by(Field::new("country")),
            "SPLIT and GROUP cannot be combined",
        );
        assert_render_error(
            &select(All)
                .from(&user)
                .order_rand()
                .order_by(order(Field::new("age"))),
            "ORDER BY RAND()",
        );
        assert_sql(
            &select(All).from(&user).order_rand().limit(3),
            "SELECT * FROM user ORDER BY RAND() LIMIT 3;",
        );
    }

    #[test]
    fn values_are_bound_and_rendering_is_stable() {
        let statement = select(All)
            .from(Table::from("user"))
            .where_(cond(Field::new("name").equal("Alice")).or(Field::new("age").gt(30)))
            .limit(5);
        let query = statement.to_query().expect("valid");
        assert_eq!(
            query.sql(),
            "SELECT * FROM user WHERE (name = $_p0) OR (age > $_p1) LIMIT $_p2;"
        );
        assert_eq!(query.vars().len(), 3);
        assert_eq!(statement.to_query().expect("valid"), query);
    }

    async fn seeded() -> Surreal<Db> {
        let db = db().await;
        run(
            &db,
            Raw::new(
                "CREATE user:alice SET name = 'Alice', age = 30, country = 'CA', emails = ['a@example.com', 'alice@example.com'];
                 CREATE user:bob SET name = 'Bob', age = 20, country = 'CA', emails = ['bob@example.com'];
                 CREATE user:carol SET name = 'Carol', age = 40, country = 'NO', emails = ['carol@example.com'];",
            ),
        )
        .await;
        db
    }

    #[tokio::test]
    async fn filters_orders_and_pages() {
        let db = seeded().await;
        let user = Table::from("user");
        let age = Field::new("age");
        let names: Vec<String> = select_value(Field::new("name"))
            .from(&user)
            .where_(age.gte(20))
            .order_by(order(&age).desc())
            .limit(2)
            .start(1)
            .return_many(db.clone())
            .await
            .expect("read");
        assert_eq!(names, ["Alice", "Bob"]);
    }

    #[tokio::test]
    async fn groups_splits_and_omits() {
        let db = seeded().await;
        let user = Table::from("user");
        let country = Field::new("country");

        let groups: Vec<BTreeMap<String, Value>> = select(arr![count!().__as__("total"), &country])
            .from(&user)
            .group_by(&country)
            .return_many(db.clone())
            .await
            .expect("grouped");
        assert_eq!(groups.len(), 2);

        let split: Vec<Value> = select(All)
            .from(&user)
            .split(Field::new("emails"))
            .return_many(db.clone())
            .await
            .expect("split");
        assert_eq!(split.len(), 4);

        let omitted: Vec<BTreeMap<String, Value>> = select(All)
            .omit(Field::new("emails"))
            .from(&user)
            .return_many(db.clone())
            .await
            .expect("omitted");
        assert!(omitted.iter().all(|row| !row.contains_key("emails")));
    }

    #[tokio::test]
    async fn record_ranges_select_by_key() {
        let db = db().await;
        for strength in 1..=6 {
            Weapon {
                id: Weapon::id(strength),
                ..Default::default()
            }
            .save(&db)
            .await
            .expect("saved");
        }
        let in_range: Vec<Value> = select(All)
            .from(Weapon::id(2)..Weapon::id(5))
            .return_many(db.clone())
            .await
            .expect("read");
        assert_eq!(in_range.len(), 3);
        let inclusive: Vec<Value> = select(All)
            .from(Weapon::id(2)..=Weapon::id(5))
            .return_many(db.clone())
            .await
            .expect("read");
        assert_eq!(inclusive.len(), 4);
    }

    #[tokio::test]
    async fn fetch_replaces_links_with_records() {
        let db = db().await;
        run(
            &db,
            Raw::new(
                "CREATE account:one SET balance = 5; CREATE user:alice SET account = account:one;",
            ),
        )
        .await;
        let balance: Vec<i64> = select_value(Field::new("account.balance"))
            .from(User::id("alice"))
            .return_many(db.clone())
            .await
            .expect("read");
        assert_eq!(balance, [5]);
        let fetched: BTreeMap<String, Value> = select(All)
            .from(User::id("alice"))
            .fetch(Field::new("account"))
            .return_one(db.clone())
            .await
            .expect("read")
            .expect("alice");
        assert!(matches!(fetched.get("account"), Some(Value::Object(_))));
    }
}

mod definitions {
    use super::*;

    #[test]
    fn table_definitions() {
        let age = Field::new("age");
        let name = Field::new("name");
        assert_sql(&define_table(Table::from("user")), "DEFINE TABLE user;");
        assert_sql(
            &define_table(Table::from("user"))
                .schemaless()
                .permissions_none(),
            "DEFINE TABLE user SCHEMALESS PERMISSIONS NONE;",
        );
        assert_sql(
            &define_table(Table::from("user"))
                .schemafull()
                .permissions_full(),
            "DEFINE TABLE user SCHEMAFULL PERMISSIONS FULL;",
        );
        assert_sql(
            &define_table(Table::from("adult"))
                .drop()
                .as_(select(All).from(Table::from("user")).where_(age.gte(18)))
                .permissions(for_permission(Select).where_(age.gte(18)))
                .permissions([
                    for_permission([Create, Delete]).where_(name.is("Alice")),
                    for_permission(Update).where_(age.lte(130)),
                ]),
            "DEFINE TABLE adult DROP AS \n\tSELECT * FROM user WHERE age >= 18\n\
             PERMISSIONS\nFOR select\n\tWHERE age >= 18\n\
             FOR create, delete\n\tWHERE name IS 'Alice'\nFOR update\n\tWHERE age <= 130;",
        );
    }

    #[test]
    fn field_definitions() {
        let name = Field::new("name");
        let age = Field::new("age");
        assert_sql(
            &define_field(Field::new("email"))
                .on_table(Table::from("user"))
                .type_(Kind::String)
                .default("alice@example.com")
                .assert(cond(value().is_not(NONE)).and(value().contains("@")))
                .permissions(for_permission(Select).where_(age.gte(18)))
                .permissions(for_permission([Create, Update]).where_(name.is("Alice"))),
            "DEFINE FIELD email ON TABLE user TYPE string DEFAULT 'alice@example.com' \
             ASSERT ($value IS NOT NONE) AND ($value CONTAINS '@')\n\
             PERMISSIONS\nFOR select\n\tWHERE age >= 18\nFOR create, update\n\tWHERE name IS 'Alice';",
        );
        assert_sql(
            &define_field(Field::new("email"))
                .on_table(Table::from("user"))
                .value(string::lowercase!(Param::new("value"))),
            "DEFINE FIELD email ON TABLE user VALUE string::lowercase($value);",
        );
        assert_sql(
            &define_field(Field::new("is_adult"))
                .on_table(Table::from("user"))
                .computed(age.gte(18))
                .type_(Kind::Bool)
                .permissions_none(),
            "DEFINE FIELD is_adult ON TABLE user COMPUTED age >= 18 TYPE bool PERMISSIONS NONE;",
        );
        // Definitions are stored, so their values are written inline even when bound.
        let query = define_field(Field::new("score"))
            .on_table(Table::from("user"))
            .default(0)
            .to_query()
            .expect("valid");
        assert_eq!(query.sql(), "DEFINE FIELD score ON TABLE user DEFAULT 0;");
        assert!(query.vars().is_empty());
    }

    #[test]
    fn computed_fields_reject_other_clauses_nested_paths_and_id() {
        let age = Field::new("age");
        let computed = |field: &'static str| {
            define_field(Field::new(field))
                .on_table(Table::from("person"))
                .computed(age.gte(18))
        };
        assert!(computed("is_adult").to_query().is_ok());
        assert!(computed("`is-adult`").to_query().is_ok());
        assert_render_error(&computed("is_adult").default(false), "cannot also have");
        assert_render_error(&computed("is_adult").value(false), "cannot also have");
        assert_render_error(
            &computed("is_adult").assert(value().is_not(NONE)),
            "cannot also have",
        );
        assert_render_error(&computed("flags.is_adult"), "top-level field");
        assert_render_error(&computed("`flags`.`is_adult`"), "top-level field");
        assert_render_error(&computed("id"), "`id` field");
        assert_render_error(&computed("`id`"), "`id` field");
    }

    #[test]
    fn index_definitions() {
        let user = Table::from("user");
        assert_sql(
            &define_index("user_email")
                .on_table(user.clone())
                .fields(Field::new("email"))
                .unique(),
            "DEFINE INDEX user_email ON TABLE user FIELDS email UNIQUE;",
        );
        assert_sql(
            &define_index("user_name")
                .on_table(user.clone())
                .columns(&[Field::new("first"), Field::new("last")]),
            "DEFINE INDEX user_name ON TABLE user COLUMNS first, last;",
        );
        assert_sql(
            &define_index("post_content")
                .on_table(Table::from("post"))
                .fields(Field::new("content"))
                .fulltext(fulltext_analyzer("ascii").bm25(1.2, 0.75).highlights()),
            "DEFINE INDEX post_content ON TABLE post FIELDS content \
             FULLTEXT ANALYZER ascii BM25(1.2, 0.75) HIGHLIGHTS;",
        );
        assert_sql(
            &define_index("post_title")
                .on_table(Table::from("post"))
                .fields(Field::new("title"))
                .fulltext(fulltext_analyzer("ascii")),
            "DEFINE INDEX post_title ON TABLE post FIELDS title FULLTEXT ANALYZER ascii;",
        );
    }

    #[test]
    fn define_analyzer_event_param_namespace_database_and_function() {
        assert_sql(
            &define_analyzer("ascii")
                .tokenizers([Tokenizer::Class, Tokenizer::Blank])
                .filters([
                    AnalyzerFilter::Lowercase,
                    AnalyzerFilter::Ascii,
                    AnalyzerFilter::Edgengram(2, 15),
                    AnalyzerFilter::Snowball(SnowballLanguage::English),
                ]),
            "DEFINE ANALYZER ascii TOKENIZERS Class,Blank \
             FILTERS lowercase,ascii,edgengram(2, 15),snowball(english);",
        );
        assert_sql(
            &define_event("email_changed")
                .on_table(Table::from("user"))
                .when(Field::new("$before.email").not_equal(Field::new("$after.email")))
                .then(create::<SpaceShip>().set(SpaceShip::schema().name.equal_to("Apollo"))),
            "DEFINE EVENT email_changed ON TABLE user WHEN $before.email != $after.email \
             THEN (CREATE space_ship SET name = 'Apollo');",
        );
        umbriel::create_param_name_fn!(endpoint_base);
        assert_sql(
            &define_param(endpoint_base()).value("https://example.com"),
            "DEFINE PARAM $endpoint_base VALUE 'https://example.com';",
        );
        assert_sql(
            &define_namespace(Namespace::new("acme")),
            "DEFINE NAMESPACE acme;",
        );
        assert_sql(&define_database("acme"), "DEFINE DATABASE acme;");

        let total = Param::new("total");
        assert_sql(
            &define_function("add_one")
                .arguments(vec![umbriel::statements::FunctionArgument {
                    name: total.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&total).add(1)))),
            "DEFINE FUNCTION fn::add_one($total: int) {\nRETURN $total + 1;\n};",
        );
    }

    #[test]
    fn user_definitions() {
        assert_sql(
            &define_user("alice")
                .on_root()
                .password("example-password")
                .role(UserRole::Owner),
            "DEFINE USER alice ON ROOT PASSWORD 'example-password' ROLES OWNER;",
        );
        assert_sql(
            &define_user("bob")
                .on_namespace()
                .passhash("$argon2id$example")
                .role(UserRole::Editor),
            "DEFINE USER bob ON NAMESPACE PASSHASH '$argon2id$example' ROLES EDITOR;",
        );
        assert_sql(
            &define_user("carol")
                .on_database()
                .password("example")
                .role(UserRole::Viewer),
            "DEFINE USER carol ON DATABASE PASSWORD 'example' ROLES VIEWER;",
        );
    }

    #[test]
    fn access_definitions() {
        let signup =
            Raw::new("CREATE user SET email = $email, pass = crypto::argon2::generate($pass)");
        let signin = Raw::new(
            "SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)",
        );
        assert_sql(
            &define_access("account").on_database().record(
                record_access()
                    .signup(signup)
                    .signin(signin)
                    .with_jwt(JwtAlgorithm::HS512, "example-key")
                    .duration_for_token(Duration::from_secs(15 * 60))
                    .duration_for_session(Duration::from_secs(12 * 60 * 60)),
            ),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD \
             SIGNUP (CREATE user SET email = $email, pass = crypto::argon2::generate($pass)) \
             SIGNIN (SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass)) \
             WITH JWT ALGORITHM HS512 KEY 'example-key' \
             DURATION FOR TOKEN 15m, FOR SESSION 12h;",
        );
        assert_sql(
            &define_access("account").on_database().record(
                record_access()
                    .signin(select(All).from(Table::from("user")))
                    .duration_for_session(Duration::from_secs(60 * 60)),
            ),
            "DEFINE ACCESS account ON DATABASE TYPE RECORD SIGNIN (SELECT * FROM user) \
             DURATION FOR SESSION 1h;",
        );
        assert_sql(
            &define_access("api").on_namespace().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::RS256)
                    .key("example-public-key"),
            ),
            "DEFINE ACCESS api ON NAMESPACE TYPE JWT ALGORITHM RS256 KEY 'example-public-key';",
        );
        assert_sql(
            &define_access("issuer").on_root().jwt(
                jwt_access()
                    .url("https://example.com/jwks.json")
                    .duration_for_session(Duration::from_secs(60)),
            ),
            "DEFINE ACCESS issuer ON ROOT TYPE JWT URL 'https://example.com/jwks.json' \
             DURATION FOR SESSION 1m;",
        );
    }

    #[test]
    fn define_access_rejects_invalid_combinations() {
        assert_render_error(
            &define_access("account").on_root().record(record_access()),
            "only be defined on a database",
        );
        assert_render_error(
            &define_access("account")
                .on_namespace()
                .record(record_access()),
            "only be defined on a database",
        );
        assert_render_error(
            &define_access("api").jwt(jwt_access().url("https://example.com")),
            "needs a level",
        );
        assert_render_error(&define_access("api").on_database(), "needs a type");
        assert_render_error(
            &define_access("api").on_database().jwt(jwt_access()),
            "needs `algorithm` and `key`",
        );
        assert_render_error(
            &define_access("api").on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example")
                    .url("https://example.com"),
            ),
            "not both",
        );
    }

    #[test]
    fn for_permissions() {
        let age = Field::new("age");
        assert_eq!(
            for_permission(Select).where_(age.gte(18)).to_debug_sql(),
            "FOR select\n\tWHERE age >= 18"
        );
        assert_eq!(
            for_permission([Create, Update, Delete])
                .where_(age.lte(130))
                .to_debug_sql(),
            "FOR create, update, delete\n\tWHERE age <= 130"
        );
    }

    #[test]
    fn remove_statements() {
        assert_sql(&remove_table(Table::from("user")), "REMOVE TABLE user;");
        assert_sql(
            &remove_field(Field::new("email")).on_table(Table::from("user")),
            "REMOVE FIELD email ON TABLE user;",
        );
        assert_sql(
            &remove_index("user_email").on_table(Table::from("user")),
            "REMOVE INDEX user_email ON TABLE user;",
        );
        assert_sql(
            &remove_event("email_changed").on_table(Table::from("user")),
            "REMOVE EVENT email_changed ON TABLE user;",
        );
        assert_sql(&remove_analyzer("ascii"), "REMOVE ANALYZER ascii;");
        assert_sql(
            &remove_param("endpoint_base"),
            "REMOVE PARAM $endpoint_base;",
        );
        assert_sql(
            &remove_function("fn::add_one"),
            "REMOVE FUNCTION fn::add_one;",
        );
        assert_sql(&remove_namespace("acme"), "REMOVE NAMESPACE acme;");
        assert_sql(&remove_database("acme"), "REMOVE DATABASE acme;");
        assert_sql(
            &remove_user("alice").on_root(),
            "REMOVE USER alice ON ROOT;",
        );
        assert_sql(
            &remove_user("alice").on_namespace(),
            "REMOVE USER alice ON NAMESPACE;",
        );
        assert_sql(
            &remove_user("alice").on_database(),
            "REMOVE USER alice ON DATABASE;",
        );
        assert_sql(
            &remove_access("account").on_database(),
            "REMOVE ACCESS account ON DATABASE;",
        );
        assert_sql(
            &remove_access("api").on_namespace(),
            "REMOVE ACCESS api ON NAMESPACE;",
        );
        assert_sql(
            &remove_access("issuer").on_root(),
            "REMOVE ACCESS issuer ON ROOT;",
        );
        assert_render_error(&remove_access("account"), "needs a level");
        assert_render_error(&remove_param("_p0"), "reserved");
    }

    async fn define_score_with_default(db: &Surreal<Db>) {
        run(db, define_table(Table::from("player")).schemaless()).await;
        run(
            db,
            define_field(Field::new("score"))
                .on_table(Table::from("player"))
                .type_(Kind::Int)
                .default(10),
        )
        .await;
    }

    #[tokio::test]
    async fn default_fills_a_missing_field_on_create() {
        let db = db().await;
        define_score_with_default(&db).await;

        run(&db, Raw::new("CREATE player:alice;")).await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:alice;").await;
        assert_eq!(score, Some(10));
    }

    #[tokio::test]
    async fn default_keeps_falsy_values() {
        let db = db().await;
        define_score_with_default(&db).await;
        run(
            &db,
            define_field(Field::new("active"))
                .on_table(Table::from("player"))
                .type_(Kind::Bool)
                .default(true),
        )
        .await;
        run(
            &db,
            define_field(Field::new("nickname"))
                .on_table(Table::from("player"))
                .type_(Kind::String)
                .default("Al"),
        )
        .await;

        run(
            &db,
            Raw::new("CREATE player:bob SET score = 0, active = false, nickname = '';"),
        )
        .await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:bob;").await;
        let active: Option<bool> = read(&db, "SELECT VALUE active FROM ONLY player:bob;").await;
        let nickname: Option<String> =
            read(&db, "SELECT VALUE nickname FROM ONLY player:bob;").await;
        assert_eq!(score, Some(0));
        assert_eq!(active, Some(false));
        assert_eq!(nickname.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn default_is_not_reapplied_on_update() {
        let db = db().await;
        define_score_with_default(&db).await;

        run(&db, Raw::new("CREATE player:carol SET score = 5;")).await;
        run(&db, Raw::new("UPDATE player:carol SET level = 2;")).await;
        let score: Option<i64> = read(&db, "SELECT VALUE score FROM ONLY player:carol;").await;
        assert_eq!(score, Some(5));

        // Without `ALWAYS`, removing the field does not bring the default back, so the `int`
        // field rejects the NONE left behind.
        let error = Raw::new("UPDATE player:carol UNSET score;")
            .run(db.clone())
            .await
            .expect_err("NONE is not an int");
        let error = format!("{error:?}");
        assert!(error.contains("Expected `int` but found `NONE`"), "{error}");
    }

    #[tokio::test]
    async fn value_transforms_on_create_and_update() {
        let db = db().await;
        run(&db, define_table(Table::from("player")).schemaless()).await;
        run(
            &db,
            define_field(Field::new("name"))
                .on_table(Table::from("player"))
                .type_(Kind::String)
                .value(string::lowercase!(Param::new("value"))),
        )
        .await;

        run(&db, Raw::new("CREATE player:dave SET name = 'ALICE';")).await;
        let name: Option<String> = read(&db, "SELECT VALUE name FROM ONLY player:dave;").await;
        assert_eq!(name.as_deref(), Some("alice"));

        run(&db, Raw::new("UPDATE player:dave SET name = 'BOB';")).await;
        let name: Option<String> = read(&db, "SELECT VALUE name FROM ONLY player:dave;").await;
        assert_eq!(name.as_deref(), Some("bob"));
    }

    #[tokio::test]
    async fn computed_fields_are_evaluated_on_read() {
        let db = db().await;
        run(&db, define_table(Table::from("person")).schemaless()).await;
        run(
            &db,
            define_field(Field::new("is_adult"))
                .on_table(Table::from("person"))
                .computed(Field::new("age").gte(18))
                .type_(Kind::Bool),
        )
        .await;

        run(&db, Raw::new("CREATE person:erin SET age = 20;")).await;
        let adult: Option<bool> = read(&db, "SELECT VALUE is_adult FROM ONLY person:erin;").await;
        assert_eq!(adult, Some(true));

        run(&db, Raw::new("UPDATE person:erin SET age = 10;")).await;
        let adult: Option<bool> = read(&db, "SELECT VALUE is_adult FROM ONLY person:erin;").await;
        assert_eq!(adult, Some(false));
    }

    #[tokio::test]
    async fn schemafull_tables_reject_undefined_fields() {
        let db = db().await;
        run(&db, define_table(Table::from("person")).schemafull()).await;
        run(
            &db,
            define_field(Field::new("name"))
                .on_table(Table::from("person"))
                .type_(Kind::String),
        )
        .await;

        run(&db, Raw::new("CREATE person:alice SET name = 'Alice';")).await;
        assert!(
            Raw::new("CREATE person:carol SET name = 'Carol', extra = 1;")
                .run(db.clone())
                .await
                .is_err(),
            "an undefined field is rejected"
        );
        assert!(
            Raw::new("CREATE person:bob SET name = 1;")
                .run(db.clone())
                .await
                .is_err(),
            "a field of the wrong type is rejected"
        );
    }

    #[tokio::test]
    async fn unique_indexes_reject_duplicates() {
        let db = db().await;
        run(
            &db,
            define_index("user_email")
                .on_table(Table::from("user"))
                .fields(Field::new("email"))
                .unique(),
        )
        .await;
        run(
            &db,
            Raw::new("CREATE user:alice SET email = 'alice@example.com';"),
        )
        .await;
        assert!(
            Raw::new("CREATE user:bob SET email = 'alice@example.com';")
                .run(db.clone())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn fulltext_indexes_answer_matches_with_scores() {
        let db = db().await;
        run(
            &db,
            define_analyzer("simple")
                .tokenizers([Tokenizer::Blank])
                .filters([AnalyzerFilter::Lowercase]),
        )
        .await;
        run(
            &db,
            define_index("post_content")
                .on_table(Table::from("post"))
                .fields(Field::new("content"))
                .fulltext(fulltext_analyzer("simple").bm25(1.2, 0.75).highlights()),
        )
        .await;
        run(
            &db,
            Raw::new(
                "CREATE post:one SET content = 'Hello world'; CREATE post:two SET content = 'Goodbye moon';",
            ),
        )
        .await;

        let content = Field::new("content");
        let found: Vec<String> = select_value(content.clone())
            .from(Table::from("post"))
            .where_(content.matches("hello"))
            .return_many(db.clone())
            .await
            .expect("searched");
        assert_eq!(found, ["Hello world"]);

        let scored: Vec<BTreeMap<String, Value>> = select(arr![
            search::score!(1).__as__("score"),
            search::highlight!("<b>", "</b>", 1).__as__("marked")
        ])
        .from(Table::from("post"))
        .where_(content.matches_with_ref(1, "hello"))
        .return_many(db.clone())
        .await
        .expect("scored");
        assert_eq!(scored.len(), 1);
        assert!(matches!(scored[0].get("score"), Some(Value::Number(_))));
        assert_eq!(
            scored[0].get("marked"),
            Some(&Value::String("<b>Hello</b> world".to_owned()))
        );
    }

    #[tokio::test]
    async fn events_run_on_changes() {
        let db = db().await;
        run(
            &db,
            define_event("ship_created")
                .on_table(Table::from("space_ship"))
                .when(Field::new("$event").equal("CREATE"))
                .then(
                    create::<WeaponStats>()
                        .set(WeaponStats::schema().averageStrength.equal_to(1.5)),
                ),
        )
        .await;
        space_ship("apollo", "Apollo")
            .create(&db)
            .await
            .expect("created");
        let stats: usize = read(&db, "RETURN count(SELECT * FROM weapon_stats);").await;
        assert_eq!(stats, 1);
    }

    #[tokio::test]
    async fn params_and_functions_are_usable_after_definition() {
        let db = db().await;
        umbriel::create_param_name_fn!(greeting);
        run(&db, define_param(greeting()).value("Hello")).await;
        let value: String = read(&db, "RETURN $greeting;").await;
        assert_eq!(value, "Hello");

        let total = Param::new("total");
        run(
            &db,
            define_function("add_one")
                .arguments(vec![umbriel::statements::FunctionArgument {
                    name: total.clone(),
                    type_: Kind::Int,
                }])
                .body(chain(return_(Field::from(&total).add(1)))),
        )
        .await;
        let result: i64 = read(&db, "RETURN fn::add_one(41);").await;
        assert_eq!(result, 42);

        run(&db, remove_function("fn::add_one")).await;
        assert!(
            Raw::new("RETURN fn::add_one(1);")
                .run(db.clone())
                .await
                .is_err()
        );
        run(&db, remove_param("greeting")).await;
    }

    #[tokio::test]
    async fn every_definition_is_accepted_and_removable() {
        let db = db().await;
        let user = Table::from("user");
        run(&db, define_table(user.clone()).schemaless()).await;
        run(
            &db,
            define_field(Field::new("email"))
                .on_table(user.clone())
                .type_(Kind::String),
        )
        .await;
        run(
            &db,
            define_index("user_email")
                .on_table(user.clone())
                .fields(Field::new("email"))
                .unique(),
        )
        .await;
        run(
            &db,
            define_event("audit")
                .on_table(user.clone())
                .when(Field::new("$event").equal("CREATE"))
                .then(select(All).from(user.clone())),
        )
        .await;
        run(
            &db,
            define_analyzer("simple").tokenizers([Tokenizer::Blank]),
        )
        .await;
        run(
            &db,
            define_access("account").on_database().record(
                record_access()
                    .signup(Raw::new("CREATE user SET email = $email"))
                    .signin(Raw::new("SELECT * FROM user WHERE email = $email"))
                    .with_jwt(JwtAlgorithm::HS512, "example-key")
                    .duration_for_session(Duration::from_secs(3600)),
            ),
        )
        .await;
        run(
            &db,
            define_access("api").on_database().jwt(
                jwt_access()
                    .algorithm(JwtAlgorithm::HS256)
                    .key("example-key"),
            ),
        )
        .await;
        run(
            &db,
            define_user("alice")
                .on_database()
                .password("example-password")
                .role(UserRole::Viewer),
        )
        .await;
        run(&db, define_namespace(Namespace::new("acme"))).await;
        run(&db, define_database("acme")).await;

        let info: Option<BTreeMap<String, Value>> = info_for()
            .database()
            .run(db.clone())
            .await
            .expect("info")
            .take(0)
            .expect("object");
        let info = info.expect("database info");
        for key in ["accesses", "analyzers", "tables", "users"] {
            assert!(info.contains_key(key), "INFO FOR DB has `{key}`: {info:?}");
        }
        let table: Option<BTreeMap<String, Value>> = info_for()
            .table(user.clone())
            .run(db.clone())
            .await
            .expect("info")
            .take(0)
            .expect("object");
        assert!(table.expect("table info").contains_key("fields"));

        run(&db, remove_access("account").on_database()).await;
        run(&db, remove_access("api").on_database()).await;
        run(&db, remove_user("alice").on_database()).await;
        run(&db, remove_analyzer("simple")).await;
        run(&db, remove_event("audit").on_table(user.clone())).await;
        run(&db, remove_index("user_email").on_table(user.clone())).await;
        run(
            &db,
            remove_field(Field::new("email")).on_table(user.clone()),
        )
        .await;
        run(&db, remove_table(user)).await;
        run(&db, remove_database("acme")).await;
        run(&db, remove_namespace("acme")).await;
    }
}

mod info_show_and_utilities {
    use super::*;

    #[test]
    fn info() {
        assert_sql(&info_for().root(), "INFO FOR ROOT;");
        assert_sql(&info_for().namespace(), "INFO FOR NS;");
        assert_sql(&info_for().database(), "INFO FOR DB;");
        assert_sql(&info_for().table("user"), "INFO FOR TABLE user;");
    }

    #[test]
    fn show_changes() {
        assert_sql(
            &show_changes_for_table("reading", 5u64).limit(10),
            "SHOW CHANGES FOR TABLE reading SINCE 5 LIMIT 10;",
        );
        assert_sql(
            &show_changes_for_table("reading", DateTime::<Utc>::UNIX_EPOCH),
            "SHOW CHANGES FOR TABLE reading SINCE d'1970-01-01T00:00:00Z';",
        );
    }

    #[test]
    fn use_sleep_throw_live_and_loop_control() {
        assert_sql(&use_().namespace("test"), "USE NS test;");
        assert_sql(&use_().database("test"), "USE DB test;");
        assert_sql(
            &use_().database("test").namespace("acme"),
            "USE NS acme DB test;",
        );
        assert_render_error(&use_(), "needs a namespace");
        assert_sql(&sleep(Duration::from_secs(43)), "SLEEP 43s;");
        assert_sql(
            &throw("something went wrong"),
            "THROW 'something went wrong';",
        );
        assert_sql(
            &live(select(All).from(Table::from("person"))),
            "LIVE SELECT * FROM person;",
        );
        assert_eq!(break_().to_debug_sql(), "BREAK;");
        assert_eq!(continue_().to_debug_sql(), "CONTINUE;");
    }

    #[test]
    fn return_and_let() {
        assert_sql(
            &return_(User::id("alice")).fetch(&[Field::new("account"), Field::new("friends")]),
            "RETURN user:alice FETCH account, friends;",
        );
        assert_sql(&return_(Field::new("age").add(1)), "RETURN age + 1;");
        assert_sql(&let_("name").equal_to("Alice"), "LET $name = 'Alice';");
        assert_sql(
            &let_("users").equal_to(select(All).from(Table::from("user"))),
            "LET $users = (SELECT * FROM user);",
        );
        assert_render_error(&let_("_p1").equal_to(1), "reserved");
        assert_render_error(&let_("not valid").equal_to(1), "must start with a letter");
    }

    #[tokio::test]
    async fn throw_fails_and_sleep_and_return_succeed() {
        let db = db().await;
        match throw("something went wrong").run(db.clone()).await {
            Err(error) => assert!(error.to_string().contains("something went wrong")),
            Ok(_) => panic!("THROW succeeded"),
        }
        run(&db, sleep(Duration::from_millis(1))).await;
        let value: Option<i64> = return_(41)
            .run(db.clone())
            .await
            .expect("ran")
            .take(0)
            .expect("value");
        assert_eq!(value, Some(41));
    }
}

mod control_flow {
    use super::*;

    #[test]
    fn if_else_branches() {
        let age = Field::new("age");
        let country = Field::new("country");
        assert_sql(
            &if_(age.gte(18)).then("adult").end(),
            "IF age >= 18 \n\t{ 'adult' };",
        );
        assert_sql(
            &if_(age.gte(18))
                .then("adult")
                .else_if(cond(country.is("CA")).or(country.is("NO")))
                .then("cold")
                .else_("other")
                .end(),
            "IF age >= 18 \n\t{ 'adult' }\nELSE IF (country IS 'CA') OR (country IS 'NO') \n\t{ 'cold' }\
             \nELSE\n\t{ 'other' };",
        );
        assert_sql(
            &if_(age.gte(18))
                .then(select(All).from(Table::from("user")))
                .end(),
            "IF age >= 18 \n\t{ (SELECT * FROM user) };",
        );
    }

    #[test]
    fn for_loops_iterate_one_parameter() {
        let name = Param::new("name");
        assert_sql(
            &for_(&name).in_(vec!["Alice", "Bob"]).block(chain(
                create::<SpaceShip>().set(SpaceShip::schema().name.equal_to(&name)),
            )),
            "FOR $name IN ['Alice', 'Bob'] {\nCREATE space_ship SET name = $name;\n};",
        );
    }

    #[test]
    fn transactions_commit_or_cancel() {
        let create_ship = || create::<SpaceShip>().set(SpaceShip::schema().name.equal_to("Apollo"));
        let committed = begin_transaction()
            .query(create_ship())
            .query(select(All).from(SpaceShip::table()))
            .commit_transaction();
        assert_eq!(
            committed.to_debug_sql(),
            "BEGIN TRANSACTION;\n\nCREATE space_ship SET name = 'Apollo';\n\n\
             SELECT * FROM space_ship;\n\nCOMMIT TRANSACTION;\n\t"
        );
        assert_parses(&committed.to_debug_sql());
        let cancelled = begin_transaction()
            .query(create_ship())
            .cancel_transaction();
        assert!(
            cancelled
                .to_debug_sql()
                .ends_with("CANCEL TRANSACTION;\n\t")
        );
        assert_parses(cancelled.to_query().expect("valid").sql());
    }

    #[test]
    fn chains_and_blocks() {
        let total = let_("total").equal_to(41);
        let chained = chain(total.clone()).chain(return_(Field::from(&total.get_param()).add(1)));
        assert_eq!(
            chained.to_debug_sql(),
            "LET $total = 41;\n\nRETURN $total + 1;"
        );
        assert_parses(&chained.to_debug_sql());
        assert_eq!(
            chained.clone().parenthesized().to_debug_sql(),
            "(LET $total = 41;\n\nRETURN $total + 1;)"
        );
        let block = block(chained.clone());
        assert_eq!(
            block.to_debug_sql(),
            "{\nLET $total = 41;\n\nRETURN $total + 1;\n}"
        );
        assert_eq!(chained.as_block(), block);
    }

    #[tokio::test]
    async fn control_flow_runs_in_the_database() {
        let db = db().await;
        let name = Param::new("name");
        run(
            &db,
            for_(&name).in_(vec!["Apollo", "Gemini"]).block(chain(
                create::<SpaceShip>().set(SpaceShip::schema().name.equal_to(&name)),
            )),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);

        let age = Field::new("$age");
        let branch = chain(let_("age").equal_to(20))
            .chain(if_(age.gte(18)).then("adult").else_("minor").end());
        let result: Option<String> = branch
            .run(db.clone())
            .await
            .expect("ran")
            .take(1)
            .expect("value");
        assert_eq!(result.as_deref(), Some("adult"));

        let block_value: Option<i64> = return_(block(
            chain(let_("total").equal_to(41)).chain(return_(Field::new("$total").add(1))),
        ))
        .run(db.clone())
        .await
        .expect("ran")
        .take(0)
        .expect("value");
        assert_eq!(block_value, Some(42));
    }

    #[tokio::test]
    async fn committed_transactions_persist_and_cancelled_ones_do_not() {
        let db = db().await;
        run(
            &db,
            begin_transaction()
                .query(create::<SpaceShip>().content(space_ship("apollo", "Apollo")))
                .query(create::<SpaceShip>().content(space_ship("gemini", "Gemini")))
                .commit_transaction(),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);

        run(
            &db,
            begin_transaction()
                .query(create::<SpaceShip>().content(space_ship("mercury", "Mercury")))
                .cancel_transaction(),
        )
        .await;
        assert_eq!(ships(&db).await, ["Apollo", "Gemini"]);
    }

    #[tokio::test]
    async fn a_transfer_between_accounts_is_atomic() {
        let db = db().await;
        let acc = Account::schema();
        let one = Account::id("one");
        let two = Account::id("two");
        run(
            &db,
            begin_transaction()
                .query(create::<Account>().content(Account {
                    id: one.clone(),
                    balance: 100.0,
                }))
                .query(create::<Account>().content(Account {
                    id: two.clone(),
                    balance: 50.0,
                }))
                .query(update::<Account>(&one).set(acc.balance.decrement_by(30.0)))
                .query(update::<Account>(&two).set(acc.balance.increment_by(30.0)))
                .commit_transaction(),
        )
        .await;
        let balances: Vec<f64> = read(&db, "SELECT VALUE balance FROM account ORDER BY id;").await;
        assert_eq!(balances, [70.0, 80.0]);

        let failed = begin_transaction()
            .query(update::<Account>(&one).set(acc.balance.decrement_by(30.0)))
            .query(throw("insufficient funds"))
            .commit_transaction()
            .run(db.clone())
            .await;
        assert!(failed.is_err());
        let balances: Vec<f64> = read(&db, "SELECT VALUE balance FROM account ORDER BY id;").await;
        assert_eq!(balances, [70.0, 80.0]);
    }

    #[tokio::test]
    async fn let_parameters_carry_results_between_statements() {
        let db = db().await;
        let balance = let_("balance")
            .equal_to(create_only::<Balance>().set(Balance::schema().amount.equal_to(25.0)));
        let created_balance = balance.with_path::<Balance>(E).amount;
        let acc = Account::schema();
        run(
            &db,
            chain(balance.clone())
                .chain(create::<Account>().content(Account {
                    id: Account::id("one"),
                    balance: 0.0,
                }))
                .chain(
                    update::<Account>(Account::id("one"))
                        .set(acc.balance.increment_by(created_balance)),
                ),
        )
        .await;
        let amount: f64 = read(&db, "RETURN account:one.balance;").await;
        assert_eq!(amount, 25.0);
    }
}
