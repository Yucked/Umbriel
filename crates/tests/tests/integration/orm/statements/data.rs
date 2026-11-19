//! Data clauses (`orm/src/statements/data.rs`): repeated calls of one clause extend it, a
//! different clause replaces it, and a statement without one writes none.

use umbriel::{
    Field, Model, Node, Patchable, Render, SchemaGetter, SetterAssignable, SetterNumeric,
    statements::{CreateTarget, create, relate, update, upsert},
};
use umbriel_tests::{support::statements::*, *};

#[test]
fn repeated_clauses_extend_and_different_ones_replace() {
    let org = Organization::schema();
    let acme = Organization::id("acme");
    assert_sql(
        &update::<Organization>(&acme)
            .set(org.age.increment_by(1u8))
            .set(org.name.equal_to("Acme Corp")),
        "UPDATE organization:acme SET age += 1, name = 'Acme Corp';",
    );
    assert_sql(
        &update::<Organization>(&acme)
            .patch(org.name.patch_remove())
            .patch(org.age.patch_replace(2u8)),
        "UPDATE organization:acme PATCH [{ op: 'remove', path: '/name' }, \
         { op: 'replace', path: '/age', value: 2 }];",
    );
    // `SET` and `UNSET` are alternatives: the later clause is the statement's.
    assert_sql(
        &update::<Organization>(&acme)
            .set(org.age.increment_by(1u8))
            .unset(Field::new("label")),
        "UPDATE organization:acme UNSET label;",
    );
    assert_sql(
        &upsert::<Organization>(&acme)
            .unset(Field::new("label"))
            .content(Organization {
                id: acme.clone(),
                ..Default::default()
            })
            .return_none(),
        &format!(
            "UPSERT organization:acme CONTENT {} RETURN NONE;",
            umbriel::ValueLike::value(Organization {
                id: acme.clone(),
                ..Default::default()
            })
            .to_debug_sql()
        ),
    );
}

#[test]
fn statements_without_a_data_clause_write_none() {
    assert_sql(
        &update::<Organization>(Organization::table()),
        "UPDATE organization;",
    );
    assert_sql(
        &create::<SpaceShip>(CreateTarget::count(2)),
        "CREATE |space_ship:2|;",
    );
    assert_sql(
        &relate::<CompanyLikeUser>(
            Company::with(Company::id("acme"))
                .like__(umbriel::E)
                .user(User::id("alice")),
        ),
        "RELATE company:acme->like->user:alice;",
    );
}

/// `MERGE`, `PATCH`, `REPLACE` and `UNSET` on `CREATE` and `RELATE`, and `TIMEOUT` on `INSERT`:
/// forms the 3.3.4 parser accepts and the engine applies through the shared data-clause path,
/// each tested separately.
mod data_clauses {
    use umbriel::{Merge, RecordId};

    use super::*;

    fn acme() -> RecordId<Organization> {
        Organization::id("acme")
    }

    #[test]
    fn every_clause_renders_and_parses() {
        let org = Organization::schema();
        let statements = [
            create::<Organization>(acme())
                .merge(OrganizationMerge {
                    name: Merge::Set("Acme Corp".to_owned()),
                    ..Default::default()
                })
                .to_debug_sql(),
            create::<Organization>(acme())
                .patch(org.name.patch_add("Acme Corp"))
                .to_debug_sql(),
            create::<Organization>(acme())
                .replace(organization("acme", "Acme Corp", 10))
                .to_debug_sql(),
            create::<Organization>(acme())
                .unset(org.name)
                .to_debug_sql(),
            insert(space_ship("apollo", "Apollo"))
                .timeout(Duration::from_secs(1))
                .to_debug_sql(),
        ];
        for (statement, clause) in statements.iter().zip([
            " MERGE {",
            " PATCH [",
            " REPLACE {",
            " UNSET name",
            " TIMEOUT 1s",
        ]) {
            assert!(statement.contains(clause), "{statement}");
            assert_parses(statement);
        }
    }

    #[tokio::test]
    async fn create_merge_patch_and_replace_fill_the_new_record() {
        let (db, _guard) = db().await;
        let org = Organization::schema();
        create::<Organization>(acme())
            .merge(OrganizationMerge {
                name: Merge::Set("Acme Corp".to_owned()),
                ..Default::default()
            })
            .return_none()
            .execute(&db)
            .await
            .expect("merged");
        let name: String = read(&db, "RETURN organization:acme.name;").await;
        assert_eq!(name, "Acme Corp");

        create::<Organization>(Organization::id("globex"))
            .patch(org.name.patch_add("Globex"))
            .return_none()
            .execute(&db)
            .await
            .expect("patched");
        let name: String = read(&db, "RETURN organization:globex.name;").await;
        assert_eq!(name, "Globex");

        // A path that does not exist yet cannot be replaced.
        let missing = create::<Organization>(Organization::id("initech"))
            .patch(org.name.patch_replace("Initech"))
            .execute(&db)
            .await;
        assert!(missing.is_err(), "{missing:?}");

        let replaced = create_only::<Organization>(Organization::id("umbrella"))
            .replace(organization("umbrella", "Umbrella", 7))
            .execute(&db)
            .await
            .expect("replaced")
            .expect("returned");
        let content = create_only::<Organization>(Organization::id("hooli"))
            .content(organization("hooli", "Umbrella", 7))
            .execute(&db)
            .await
            .expect("created")
            .expect("returned");
        assert_eq!((replaced.name, replaced.age), (content.name, content.age));
    }

    /// `UNSET` on a new record, with `DEFAULT`, `VALUE` and `ASSERT` clauses, on a SCHEMAFULL
    /// and a SCHEMALESS table. Recorded as found.
    #[tokio::test]
    async fn create_unset_meets_default_value_and_assert() {
        for mode in ["SCHEMAFULL", "SCHEMALESS"] {
            let (db, _guard) = db().await;
            run(
                &db,
                Raw::new(format!(
                    "DEFINE TABLE organization {mode}; \
                     DEFINE FIELD name ON organization TYPE string DEFAULT 'Unnamed'; \
                     DEFINE FIELD age ON organization TYPE int VALUE 7;"
                )),
            )
            .await;
            let org = Organization::schema();
            create::<Organization>(acme())
                .unset([Field::from(&org.name), Field::from(&org.age)])
                .return_none()
                .execute(&db)
                .await
                .expect("created");
            let stored: (String, u8) = read(
                &db,
                "RETURN [organization:acme.name, organization:acme.age];",
            )
            .await;
            assert_eq!(stored, ("Unnamed".to_owned(), 7), "{mode}");

            run(
                &db,
                Raw::new(
                    "DEFINE FIELD OVERWRITE name ON organization TYPE string ASSERT $value != '';",
                ),
            )
            .await;
            let asserted = create::<Organization>(Organization::id("globex"))
                .unset(org.name)
                .execute(&db)
                .await;
            assert!(asserted.is_err(), "{mode}: {asserted:?}");
        }
    }

    async fn endpoints(db: &Surreal<Any>) {
        run(db, Raw::new("CREATE company:acme, user:alice;")).await;
    }

    fn edge_path() -> Field {
        Company::with(Company::id("acme"))
            .like__(CompanyLikeUser::id("l1"))
            .user(User::id("alice"))
            .into()
    }

    async fn edge_endpoints(db: &Surreal<Any>) -> (String, String) {
        read(
            db,
            "RETURN [record::id(like:l1.in), record::id(like:l1.out)];",
        )
        .await
    }

    #[tokio::test]
    async fn relate_merge_patch_replace_and_unset_on_a_new_edge() {
        let (db, _guard) = db().await;
        endpoints(&db).await;
        let schema = CompanyLikeUser::schema();
        let edge = relate_only::<CompanyLikeUser>(edge_path())
            .merge(LikeMerge::<Company, User> {
                time: Merge::Fields(TimeMerge {
                    connected: Merge::Set(DateTime::<Utc>::UNIX_EPOCH),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .execute(&db)
            .await
            .expect("related")
            .expect("returned");
        assert_eq!(edge.time.connected, DateTime::<Utc>::UNIX_EPOCH);
        run(&db, Raw::new("DELETE like:l1;")).await;

        relate::<CompanyLikeUser>(edge_path())
            .patch(schema.time.patch_add(Time::default()))
            .return_none()
            .execute(&db)
            .await
            .expect("patched");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
        run(&db, Raw::new("DELETE like:l1;")).await;

        relate::<CompanyLikeUser>(edge_path())
            .replace(BTreeMap::from([("note".to_owned(), "new".to_owned())]))
            .return_none()
            .execute(&db)
            .await
            .expect("replaced");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
        run(&db, Raw::new("DELETE like:l1;")).await;

        relate::<CompanyLikeUser>(edge_path())
            .unset(schema.time)
            .return_none()
            .execute(&db)
            .await
            .expect("unset");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
    }

    /// Under `OR UPDATE` the clauses change the existing edge; `UNSET` removes a stored property,
    /// and `REPLACE` and `UNSET` keep `in` and `out`.
    #[tokio::test]
    async fn relate_or_update_changes_the_existing_edge() {
        let (db, _guard) = db().await;
        endpoints(&db).await;
        let schema = CompanyLikeUser::schema();
        run(
            &db,
            Raw::new("RELATE company:acme->like:l1->user:alice SET note = 'old', time = { connected: d'2026-01-01T00:00:00Z' };"),
        )
        .await;

        let before = relate_only::<CompanyLikeUser>(edge_path())
            .or_update()
            .unset(Field::from(&schema.time))
            .return_before()
            .execute(&db)
            .await
            .expect("updated")
            .expect("returned");
        assert_ne!(before.time.connected, DateTime::<Utc>::UNIX_EPOCH);
        let removed: bool = read(&db, "RETURN type::is_none(like:l1.time);").await;
        assert!(removed, "UNSET removes the stored property");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );

        relate::<CompanyLikeUser>(edge_path())
            .or_update()
            .merge(LikeMerge::<Company, User> {
                time: Merge::Set(Time::default()),
                ..Default::default()
            })
            .return_none()
            .execute(&db)
            .await
            .expect("merged");
        let note: String = read(&db, "RETURN like:l1.note;").await;
        assert_eq!(note, "old", "a merge keeps the fields it does not name");

        relate::<CompanyLikeUser>(edge_path())
            .or_update()
            .patch(schema.time.patch_remove())
            .return_none()
            .execute(&db)
            .await
            .expect("patched");
        let removed: bool = read(&db, "RETURN type::is_none(like:l1.time);").await;
        assert!(removed);

        relate::<CompanyLikeUser>(edge_path())
            .or_update()
            .replace(BTreeMap::from([("note".to_owned(), "new".to_owned())]))
            .return_none()
            .execute(&db)
            .await
            .expect("replaced");
        let note: String = read(&db, "RETURN like:l1.note;").await;
        assert_eq!(note, "new");
        assert_eq!(
            edge_endpoints(&db).await,
            ("acme".to_owned(), "alice".to_owned())
        );
    }
}
