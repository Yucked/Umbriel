//! DELETE (`orm/src/statements/delete.rs`): targets, clauses, return modes and edge paths,
//! rendered and run.

use umbriel_tests::{support::statements::*, *};

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
            .timeout(Duration::from_secs(2))
            .return_before(),
        "DELETE organization WHERE age < 18 RETURN BEFORE TIMEOUT 2s;",
    );
    assert_sql(
        &delete::<Organization>(Organization::id("acme")).return_diff(),
        "DELETE organization:acme RETURN DIFF;",
    );
    assert_sql(
        &delete_only::<Organization>(Organization::id("acme")),
        "DELETE ONLY organization:acme;",
    );
    assert_sql(
        &delete::<Organization>(Organization::table()).explain(),
        "DELETE organization EXPLAIN;",
    );
    assert_sql(
        &delete_edges::<CompanyLikeUser, _>(&Company::with(Company::id("acme")).like__(E)),
        "DELETE company:acme->like;",
    );
    assert_render_error(
        &delete::<Organization>(User::id("alice")),
        "does not belong",
    );
}

#[tokio::test]
async fn delete_returns_nothing_by_default_and_only_before_or_diff_on_request() {
    let (db, _guard) = db().await;
    organization("acme", "Acme Corp", 10)
        .create(&db)
        .await
        .expect("acme");
    let () = delete::<Organization>(Organization::id("missing"))
        .execute(&db)
        .await
        .expect("deleted nothing");
    let missing = delete_only::<Organization>(Organization::id("missing"))
        .return_before()
        .execute(&db)
        .await
        .expect("deleted nothing");
    assert!(missing.is_none());
    let diff = delete_only::<Organization>(Organization::id("acme"))
        .return_diff()
        .execute(&db)
        .await
        .expect("deleted")
        .expect("returned");
    assert!(!diff.is_empty());
}

#[tokio::test]
async fn explain_deletes_nothing() {
    let (db, _guard) = db().await;
    organization("acme", "Acme Corp", 10)
        .create(&db)
        .await
        .expect("acme");
    let plan = delete::<Organization>(Organization::table())
        .explain()
        .execute(&db)
        .await
        .expect("explained");
    assert!(!plan.is_empty());
    let left: Vec<String> = read(&db, "SELECT VALUE name FROM organization;").await;
    assert_eq!(left, ["Acme Corp"]);
}

#[tokio::test]
async fn an_edge_path_deletes_only_those_edges() {
    let (db, _guard) = db().await;
    run(
        &db,
        Raw::new(
            "CREATE company:acme, company:globex, user:alice; \
                 RELATE company:acme->like->user:alice; RELATE company:globex->like->user:alice;",
        ),
    )
    .await;
    run(
        &db,
        delete_edges::<CompanyLikeUser, _>(&Company::with(Company::id("acme")).like__(E)),
    )
    .await;
    let left: Vec<String> = read(&db, "SELECT VALUE record::id(in) FROM like;").await;
    assert_eq!(left, ["globex"]);
}

#[tokio::test]
async fn deletes_by_id_or_condition() {
    let (db, _guard) = db().await;
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
        .return_before()
        .execute(&db)
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
