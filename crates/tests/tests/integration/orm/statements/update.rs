//! UPDATE (`orm/src/statements/update.rs`): assignments, data clauses, return modes and targets,
//! rendered and run.

use umbriel_tests::{support::statements::*, *};


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
    fn targets_clauses_and_options() {
        let org = Organization::schema();
        assert_sql(
            &update_only::<Organization>(Organization::id("acme"))
                .set(org.age.equal_to(30u8))
                .where_(org.age.lt(30))
                .timeout(Duration::from_secs(1))
                .return_diff(),
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
            "is not a table of the model",
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
            .merge(OrganizationMerge {
                name: Merge::Set("Globex".to_owned()),
                ..Default::default()
            })
            .to_query()
            .expect("valid");
        assert_eq!(merge.sql(), "UPDATE $_p0 MERGE { 'name': $_p1 };");

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
        assert_sql(
            &update::<Organization>(&acme).patch(vec![
                org.name.patch_test("Acme Corp"),
                org.name.patch_copy(Field::new("label")),
                org.name.patch_move(Field::new("title")),
            ]),
            "UPDATE organization:acme PATCH [{ op: 'test', path: '/name', value: 'Acme Corp' }, \
             { op: 'copy', path: '/name', from: '/label' }, \
             { op: 'move', path: '/name', from: '/title' }];",
        );
        assert_render_error(
            &update::<Organization>(&acme).patch(org.name.patch_copy(Field::raw("tags[0]"))),
            "The path you have provided is invalid",
        );
    }

    #[test]
    fn unset_merge_explain_and_element_assignments() {
        let org = Organization::schema();
        let acme = Organization::id("acme");
        assert_sql(
            &update::<Organization>(&acme).unset([Field::from(&org.name), Field::from(&org.age)]),
            "UPDATE organization:acme UNSET name, age;",
        );
        assert_sql(
            &update::<Organization>(&acme)
                .unset(&org.name)
                .unset(Field::new("label")),
            "UPDATE organization:acme UNSET name, label;",
        );
        assert_sql(
            &update::<Organization>(&acme).merge(OrganizationMerge {
                age: Merge::Null,
                time: Merge::Fields(TimeMerge {
                    connected: Merge::None,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            "UPDATE organization:acme MERGE { 'time': { 'connected': NONE }, 'age': NULL };",
        );
        // An empty merge is written as built: the builder never becomes a `SELECT`.
        assert_sql(
            &update_only::<Organization>(&acme)
                .merge(OrganizationMerge::default())
                .where_(org.age.gt(1))
                .return_none(),
            "UPDATE ONLY organization:acme MERGE {} WHERE age > 1 RETURN NONE;",
        );
        assert_sql(
            &update::<Organization>(Organization::table())
                .set(org.age.increment_by(1u8))
                .explain(),
            "UPDATE organization SET age += 1 EXPLAIN;",
        );
        assert_sql(
            &update::<Organization>(Organization::table())
                .set(org.age.increment_by(1u8))
                .explain_full(),
            "UPDATE organization SET age += 1 EXPLAIN FULL;",
        );

        let user = User::schema();
        assert_sql(
            &update::<User>(User::id("alice")).set(user.tags.at(0).equal_to("rust")),
            "UPDATE user:alice SET tags[0] = 'rust';",
        );
        assert_sql(
            &update::<User>(User::id("alice")).set(
                user.tags
                    .filter(Param::new("this").equal("go"))
                    .equal_to("rust"),
            ),
            "UPDATE user:alice SET tags[WHERE $this = 'go'] = 'rust';",
        );
    }

    #[test]
    fn return_modes() {
        let org = Organization::schema();
        let base = || update::<Organization>(Organization::id("acme")).set(org.age.equal_to(1u8));
        for (statement, clause) in [
            (base().return_none().to_debug_sql(), " RETURN NONE;"),
            (base().return_before().to_debug_sql(), " RETURN BEFORE;"),
            (base().return_after().to_debug_sql(), " RETURN AFTER;"),
            (base().return_diff().to_debug_sql(), " RETURN DIFF;"),
            (
                base()
                    .return_value(Organization::schema().age)
                    .to_debug_sql(),
                " RETURN VALUE age;",
            ),
            (
                base()
                    .return_fields::<Value>(Field::new("age"))
                    .to_debug_sql(),
                " RETURN age;",
            ),
        ] {
            assert!(statement.ends_with(clause), "{statement}");
            assert_parses(&statement);
        }
    }

    async fn acme(db: &Surreal<Any>) {
        organization("acme", "Acme Corp", 10)
            .create(db)
            .await
            .expect("acme");
    }

    #[tokio::test]
    async fn numeric_and_array_assignments_change_the_record() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let id = Organization::id("acme");

        let updated = update_only::<Organization>(&id)
            .set([
                org.age.increment_by(5u8),
                org.users.append(User::id("alice")),
            ])
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_eq!(updated.age, 15);
        assert_eq!(updated.users.keys_truthy(), [&User::id("alice")]);

        let updated = update_only::<Organization>(&id)
            .set([
                org.age.decrement_by(3u8),
                org.users.remove(User::id("alice")),
            ])
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_eq!(updated.age, 12);
        assert!(updated.users.is_empty());
    }

    #[tokio::test]
    async fn element_assignments_change_one_element() {
        let (db, _guard) = db().await;
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
        let updated = update::<User>(User::id("alice"))
            .set(tags.at(1).equal_to("rust"))
            .return_value(User::schema().tags)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [vec!["go".to_owned(), "rust".to_owned()]]);
        let updated = update::<User>(User::id("alice"))
            .set(tags.filter(Param::new("this").equal("go")).equal_to("ts"))
            .return_value(User::schema().tags)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [vec!["ts".to_owned(), "rust".to_owned()]]);
    }

    #[tokio::test]
    async fn unset_removes_fields_and_none_fails_on_a_required_schemafull_field() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        run(
            &db,
            update::<Organization>(Organization::id("acme")).unset(&org.name),
        )
        .await;
        let absent: bool = read(&db, "RETURN type::is_none(organization:acme.name);").await;
        assert!(absent);

        run(
            &db,
            Raw::new(
                "DEFINE TABLE strict_org SCHEMAFULL; DEFINE FIELD name ON strict_org TYPE string; \
                 CREATE strict_org:acme SET name = 'Acme Corp';",
            ),
        )
        .await;
        let failed = Raw::new("UPDATE strict_org:acme SET name = NONE;")
            .run(&db)
            .await;
        assert!(matches!(failed, Err(Error::Statement { .. })), "{failed:?}");
    }

    #[tokio::test]
    async fn a_missing_target_is_not_created() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let updated = update::<Organization>(Organization::id("missing"))
            .set(org.age.equal_to(1u8))
            .execute(&db)
            .await
            .expect("updated");
        assert!(updated.is_empty());
        let names: Vec<String> = read(&db, "SELECT VALUE name FROM organization;").await;
        assert_eq!(names, ["Acme Corp"]);
    }

    /// Since SurrealDB 3.3.0 `WHERE` is checked before the data clause, against the record as it
    /// was before the statement.
    #[tokio::test]
    async fn where_is_evaluated_before_the_data_clause() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let updated = update::<Organization>(Organization::table())
            .set(org.age.equal_to(30u8))
            .where_(org.age.equal(10))
            .return_value(Organization::schema().age)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated, [30]);
        let none = update::<Organization>(Organization::table())
            .set(org.age.equal_to(10u8))
            .where_(org.age.equal(10))
            .execute(&db)
            .await
            .expect("updated");
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn explain_returns_a_plan_and_writes_nothing() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let plan = update::<Organization>(Organization::table())
            .set(org.age.equal_to(99u8))
            .explain()
            .execute(&db)
            .await
            .expect("explained");
        assert!(!plan.is_empty());
        let ages: Vec<u8> = read(&db, "SELECT VALUE age FROM organization;").await;
        assert_eq!(ages, [10]);
    }

    #[tokio::test]
    async fn return_modes_decode_into_their_types() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let id = Organization::id("acme");
        let () = update::<Organization>(&id)
            .set(org.age.equal_to(11u8))
            .return_none()
            .execute(&db)
            .await
            .expect("updated");
        let after = update::<Organization>(&id)
            .set(org.age.equal_to(12u8))
            .return_after()
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(after[0].age, 12);
        let ages = update::<Organization>(&id)
            .set(org.age.equal_to(13u8))
            .return_value(Organization::schema().age)
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(ages, [13]);
        let diff = update_only::<Organization>(&id)
            .set(org.age.equal_to(14u8))
            .return_diff()
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_eq!(
            diff,
            [DiffOp::Replace {
                path: "/age".to_owned(),
                value: 14_i64.into_value(),
            }]
        );
        let fields = update::<Organization>(&id)
            .set(org.name.equal_to("Globex"))
            .return_fields::<BTreeMap<String, String>>(Field::new("name"))
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(
            fields,
            [BTreeMap::from([("name".to_owned(), "Globex".to_owned())])]
        );
    }

    #[tokio::test]
    async fn conditions_choose_the_updated_records() {
        let (db, _guard) = db().await;
        acme(&db).await;
        organization("globex", "Globex", 40)
            .create(&db)
            .await
            .expect("globex");
        let org = Organization::schema();

        let updated = update::<Organization>(Organization::table())
            .set(org.age.increment_by(1u8))
            .where_(org.age.gt(20))
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].age, 41);
    }

    #[tokio::test]
    async fn content_replaces_and_merge_combines() {
        let (db, _guard) = db().await;
        acme(&db).await;
        let id = Organization::id("acme");

        let replaced = update_only::<Organization>(&id)
            .content(organization("acme", "Acme Corp", 99))
            .execute(&db)
            .await
            .expect("replaced")
            .expect("returned");
        assert_eq!(replaced.age, 99);

        let merged = update_only::<Organization>(&id)
            .merge(OrganizationMerge {
                name: Merge::Set("Globex".to_owned()),
                ..Default::default()
            })
            .execute(&db)
            .await
            .expect("merged")
            .expect("returned");
        assert_eq!((merged.name.as_str(), merged.age), ("Globex", 99));

        // A nested merge changes only the fields it names; the stored object keeps the rest.
        run(
            &db,
            Raw::new("UPDATE organization:acme SET time.zone = 'UTC', tags = ['a', 'b'];"),
        )
        .await;
        run(
            &db,
            update::<Organization>(&id).merge(OrganizationMerge {
                time: Merge::Fields(TimeMerge {
                    connected: Merge::Set(DateTime::<Utc>::from_timestamp(60, 0).expect("time")),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        )
        .await;
        let zone: String = read(&db, "RETURN organization:acme.time.zone;").await;
        assert_eq!(zone, "UTC");
        let connected: DateTime<Utc> = read(&db, "RETURN organization:acme.time.connected;").await;
        assert_eq!(connected.timestamp(), 60);

        // An array in a merge replaces the stored array whole.
        run(
            &db,
            Raw::new("UPDATE organization:acme MERGE { tags: ['c'] };"),
        )
        .await;
        let tags: Vec<String> = read(&db, "RETURN organization:acme.tags;").await;
        assert_eq!(tags, ["c"]);

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
        let (db, _guard) = db().await;
        acme(&db).await;
        let id = Organization::id("acme");
        let org = Organization::schema();

        let patched = update_only::<Organization>(&id)
            .patch(org.name.patch_replace("Globex"))
            .execute(&db)
            .await
            .expect("patched")
            .expect("returned");
        assert_eq!(patched.name, "Globex");

        let names = update::<Organization>(&id)
            .patch(org.name.patch_add("Initech"))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("patched");
        assert_eq!(names, ["Initech"]);

        // "test" becomes "text".
        run(
            &db,
            update::<Organization>(&id).set(org.name.equal_to("test")),
        )
        .await;
        let names = update::<Organization>(&id)
            .patch(org.name.patch_change("@@ -1,4 +1,4 @@\n te\n-s\n+x\n t\n"))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("patched");
        assert_eq!(names, ["text"]);

        run(
            &db,
            Raw::new("UPDATE organization:acme SET label = 'Copied', title = 'Moved';"),
        )
        .await;
        let names = update::<Organization>(&id)
            .patch(org.name.patch_copy(Field::new("label")))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("copied");
        assert_eq!(names, ["Copied"]);
        let names = update::<Organization>(&id)
            .patch(org.name.patch_move(Field::new("title")))
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("moved");
        assert_eq!(names, ["Moved"]);
        let title_gone: bool = read(&db, "RETURN type::is_none(organization:acme.title);").await;
        assert!(title_gone);

        let names = update::<Organization>(&id)
            .patch(vec![
                org.name.patch_test("Moved"),
                org.name.patch_replace("Tested"),
            ])
            .return_value(Organization::schema().name)
            .execute(&db)
            .await
            .expect("test passed");
        assert_eq!(names, ["Tested"]);
        let failed = update::<Organization>(&id)
            .patch(vec![
                org.name.patch_test("Wrong"),
                org.name.patch_replace("Never"),
            ])
            .execute(&db)
            .await;
        assert!(matches!(failed, Err(Error::Statement { .. })), "{failed:?}");
        let name: String = read(&db, "RETURN organization:acme.name;").await;
        assert_eq!(name, "Tested");

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
        let (db, _guard) = db().await;
        acme(&db).await;
        let org = Organization::schema();
        let before = update::<Organization>(Organization::id("acme"))
            .set(org.age.equal_to(50u8))
            .return_before()
            .execute(&db)
            .await
            .expect("updated");
        assert_eq!(before[0].age, 10);
    }
