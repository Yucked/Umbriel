//! RELATE (`orm/src/statements/relate.rs`): edges between records, their ids and contents, and
//! the endpoints the database enforces.

use umbriel_tests::{support::statements::*, *};

fn acme_likes_alice() -> Field {
    Company::with(Company::id("acme"))
        .like__(E)
        .user(User::id("alice"))
        .into()
}

#[test]
fn relates_records_subqueries_and_options() {
    assert_sql(
        &relate::<CompanyLikeUser>(acme_likes_alice()).return_none(),
        "RELATE company:acme->like->user:alice RETURN NONE;",
    );
    assert_sql(
        &relate::<CompanyLikeUser>(acme_likes_alice())
            .or_update()
            .return_before(),
        "RELATE OR UPDATE company:acme->like->user:alice RETURN BEFORE;",
    );
    assert_sql(
        &relate_only::<CompanyLikeUser>(acme_likes_alice()).or_update(),
        "RELATE ONLY OR UPDATE company:acme->like->user:alice;",
    );
    assert_sql(
        &relate::<CompanyLikeUser>(
            Company::with(Company::id("acme"))
                .like__(CompanyLikeUser::id("acme_alice"))
                .user(User::id("alice")),
        ),
        "RELATE company:acme->like:acme_alice->user:alice;",
    );
    assert_sql(
        &relate::<CompanyLikeUser>(
            Company::with(vec![Company::id("acme"), Company::id("globex")])
                .like__(E)
                .user([User::id("alice"), User::id("bob")]),
        ),
        "RELATE [company:acme, company:globex]->like->[user:alice, user:bob];",
    );
    let base = || relate::<CompanyLikeUser>(acme_likes_alice());
    for (statement, clause) in [
        (base().return_after().to_debug_sql(), " RETURN AFTER;"),
        (base().return_diff().to_debug_sql(), " RETURN DIFF;"),
        (
            base()
                .return_value(CompanyLikeUser::schema().time)
                .to_debug_sql(),
            " RETURN VALUE time;",
        ),
        (
            base()
                .return_fields::<Value>(Field::new("time"))
                .to_debug_sql(),
            " RETURN time;",
        ),
    ] {
        assert!(statement.ends_with(clause), "{statement}");
        assert_parses(&statement);
    }
    assert_sql(
        &relate_only::<CompanyLikeUser>(acme_likes_alice()).set(
            CompanyLikeUser::schema()
                .time()
                .connected
                .equal_to(Field::from_expr(umbriel::functions::time::now!())),
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

/// A typed id of another model does not compile (`tests/ui/fail/relate_wrong_endpoint.rs`);
/// an untyped SDK id is checked when the query is rendered.
#[test]
fn untyped_ids_of_another_table_are_rejected() {
    let path: Field = Company::with(surrealdb::types::RecordId::from(User::id("alice")))
        .like__(E)
        .user(User::id("bob"))
        .into();
    assert_render_error(&relate::<CompanyLikeUser>(path), "does not belong to table");
    let path: Field = Company::with(Company::id("acme"))
        .like__(E)
        .user(vec![User::id("bob")])
        .into();
    assert!(relate::<CompanyLikeUser>(path).to_query().is_ok());
}

async fn companies_and_users(db: &Surreal<Any>) {
    for (key, name) in [("acme", "Acme Corp"), ("globex", "Globex")] {
        Company {
            id: Company::id(key),
            name: name.to_owned(),
            ..Default::default()
        }
        .create(db)
        .await
        .expect("company");
    }
    for (key, name) in [("alice", "Alice"), ("bob", "Bob")] {
        User {
            id: User::id(key),
            name: name.to_owned(),
            ..Default::default()
        }
        .create(db)
        .await
        .expect("user");
    }
}

fn connected_at(seconds: i64) -> umbriel::Setter {
    CompanyLikeUser::schema()
        .time()
        .connected
        .equal_to(DateTime::<Utc>::from_timestamp(seconds, 0).expect("time"))
}

#[tokio::test]
async fn an_edge_id_in_the_path_names_the_edge() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;
    let path = || -> Field {
        Company::with(Company::id("acme"))
            .like__(CompanyLikeUser::id("acme_alice"))
            .user(User::id("alice"))
            .into()
    };
    let edges = relate::<CompanyLikeUser>(path())
        .set(connected_at(1))
        .execute(&db)
        .await
        .expect("related");
    assert_eq!(edges[0].id, CompanyLikeUser::id("acme_alice"));

    let before = relate::<CompanyLikeUser>(path())
        .or_update()
        .set(connected_at(2))
        .return_before()
        .execute(&db)
        .await
        .expect("updated");
    assert_eq!(before[0].time.connected.timestamp(), 1);
    let connected: DateTime<Utc> = read(&db, "RETURN like:acme_alice.time.connected;").await;
    assert_eq!(connected.timestamp(), 2);

    // Assigning an existing edge's id on the create path fails.
    let clash = relate::<CompanyLikeUser>(
        Company::with(Company::id("globex"))
            .like__(E)
            .user(User::id("bob")),
    )
    .set(
        CompanyLikeUser::schema()
            .id
            .equal_to(CompanyLikeUser::id("acme_alice")),
    )
    .execute(&db)
    .await;
    assert!(
        matches!(&clash, Err(Error::RecordExists { .. })),
        "{clash:?}"
    );
}

/// What SurrealDB 3.3.4 does with ids in a `RELATE`'s data, which the typed check below
/// enforces before running. Under a table step nothing is rejected: a record of another table
/// is moved into the edge table, and `in` and `out` are replaced by the path's endpoints. Under
/// an edge record step, a different id, `in` or `out` fails the statement.
#[tokio::test]
async fn content_ids_follow_the_database_rules() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;
    run(
        &db,
        Raw::new(
            "RELATE company:globex->like->user:bob \
                 CONTENT { id: other:moved, in: company:acme, out: user:alice };",
        ),
    )
    .await;
    let stored: Vec<Vec<String>> = read(
        &db,
        "SELECT VALUE [<string> id, record::id(in), record::id(out)] FROM like;",
    )
    .await;
    assert_eq!(stored, [["like:moved", "globex", "bob"]]);

    for sql in [
        "RELATE company:acme->like:first->user:alice CONTENT { id: like:other };",
        "RELATE company:acme->like:second->user:alice CONTENT { in: company:globex };",
        "RELATE company:acme->like:third->user:alice CONTENT { out: user:bob };",
    ] {
        let (_, source, _) = statement_error(Raw::new(sql).run(&db).await);
        assert!(
            source
                .to_string()
                .contains("but a specific record has been specified"),
            "{sql}: {source}"
        );
    }
}

/// A `CONTENT` id, `in` or `out` that contradicts the typed path fails when the statement is
/// rendered; matching or unset ones run.
#[tokio::test]
async fn content_contradicting_the_path_fails_before_it_runs() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;
    let path = || -> Field {
        Company::with(Company::id("acme"))
            .like__(CompanyLikeUser::id("acme_alice"))
            .user(User::id("alice"))
            .into()
    };
    let like = |key: &str, from: &str, to: &str| CompanyLikeUser {
        id: CompanyLikeUser::id(key),
        r#in: LinkOne::from(Company::id(from)),
        out: LinkOne::from(User::id(to)),
        ..Default::default()
    };
    for (content, message) in [
        (
            like("other", "acme", "alice"),
            "is not the edge record `like:acme_alice` in the path",
        ),
        (
            like("acme_alice", "globex", "alice"),
            "the content's `in` is `company:globex`, but the path relates `company:acme`",
        ),
        (
            like("acme_alice", "acme", "bob"),
            "the content's `out` is `user:bob`, but the path relates `user:alice`",
        ),
    ] {
        assert_render_error(&relate::<CompanyLikeUser>(path()).content(content), message);
    }

    // Every pair of a list must agree, so a single `in` cannot match two endpoints.
    let pairs: Field = Company::with(vec![Company::id("acme"), Company::id("globex")])
        .like__(E)
        .user(User::id("alice"))
        .into();
    assert_render_error(
        &relate::<CompanyLikeUser>(pairs).content(like("pair", "acme", "alice")),
        "but the path relates `company:globex`",
    );

    // A parameter endpoint is left to the database, but the known `out` is still checked.
    let dynamic: Field = Company::with(Param::new("company"))
        .like__(E)
        .user(User::id("alice"))
        .into();
    assert!(
        relate::<CompanyLikeUser>(dynamic.clone())
            .content(like("dynamic", "globex", "alice"))
            .to_query()
            .is_ok()
    );
    assert_render_error(
        &relate::<CompanyLikeUser>(dynamic).content(like("dynamic", "globex", "bob")),
        "the content's `out` is `user:bob`",
    );

    let edges = relate::<CompanyLikeUser>(path())
        .content(like("acme_alice", "acme", "alice"))
        .execute(&db)
        .await
        .expect("related");
    assert_eq!(edges[0].id, CompanyLikeUser::id("acme_alice"));
    let unset = relate::<CompanyLikeUser>(acme_likes_alice())
        .content(CompanyLikeUser {
            id: CompanyLikeUser::id("unset"),
            ..Default::default()
        })
        .execute(&db)
        .await
        .expect("related");
    assert_eq!(unset[0].r#in.key(), Some(&Company::id("acme")));
}

#[tokio::test]
async fn endpoint_lists_relate_every_pair() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;
    let edges = relate::<CompanyLikeUser>(
        Company::with([Company::id("acme"), Company::id("globex")])
            .like__(E)
            .user(User::id("alice")),
    )
    .set(connected_at(1))
    .execute(&db)
    .await
    .expect("related");
    assert_eq!(edges.len(), 2);
}

/// Dynamic endpoints are checked by the database only: `TYPE RELATION IN … OUT …` restricts
/// their tables, and `ENFORCED` requires the records to exist.
#[tokio::test]
async fn type_relation_rejects_a_dynamic_endpoint_of_another_table() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;
    run(
        &db,
        Raw::new(
            "DEFINE TABLE like TYPE RELATION IN company OUT user; \
                 CREATE organization:acme SET name = 'Acme Corp';",
        ),
    )
    .await;
    // The subquery's records are known only when the statement runs.
    let wrong_table = relate::<CompanyLikeUser>(
        Company::with(select(All).from(Organization::table()))
            .like__(E)
            .user(User::id("alice")),
    )
    .set(connected_at(1))
    .execute(&db)
    .await;
    assert!(wrong_table.is_err(), "{wrong_table:?}");
}

#[tokio::test]
async fn enforced_rejects_a_missing_endpoint_record() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;
    run(
        &db,
        Raw::new("DEFINE TABLE like TYPE RELATION IN company OUT user ENFORCED;"),
    )
    .await;
    let missing = relate::<CompanyLikeUser>(
        Company::with(Company::id("initech"))
            .like__(E)
            .user(User::id("alice")),
    )
    .set(connected_at(1))
    .execute(&db)
    .await;
    assert!(missing.is_err(), "{missing:?}");
    let existing = relate::<CompanyLikeUser>(acme_likes_alice())
        .set(connected_at(1))
        .execute(&db)
        .await
        .expect("related");
    assert_eq!(existing.len(), 1);
}

#[tokio::test]
async fn relating_creates_edges_between_every_pair() {
    let (db, _guard) = db().await;
    companies_and_users(&db).await;

    let edges = relate::<CompanyLikeUser>(
        Company::with(select(All).from(Company::table()))
            .like__(E)
            .user(select(All).from(User::table())),
    )
    .set(
        CompanyLikeUser::schema()
            .time()
            .connected
            .equal_to(Field::from_expr(umbriel::functions::time::now!())),
    )
    .execute(&db)
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
