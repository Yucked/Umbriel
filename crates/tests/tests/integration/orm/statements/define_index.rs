//! `DEFINE INDEX` (`orm/src/statements/define_index.rs`): unique, composite and full-text indexes, and what SurrealDB does with them.

use umbriel_tests::{support::statements::*, *};

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

#[tokio::test]
async fn unique_indexes_reject_duplicates() {
    let (db, _guard) = db().await;
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
            .run(&db)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn fulltext_indexes_answer_matches_with_scores() {
    let (db, _guard) = db().await;
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
        .decode()
        .execute(&db)
        .await
        .expect("searched");
    assert_eq!(found, ["Hello world"]);

    let scored: Vec<BTreeMap<String, Value>> = select(arr![
        search::score!(1).__as__("score"),
        search::highlight!("<b>", "</b>", 1).__as__("marked")
    ])
    .from(Table::from("post"))
    .where_(content.matches_with_ref(1, "hello"))
    .decode()
    .execute(&db)
    .await
    .expect("scored");
    assert_eq!(scored.len(), 1);
    assert!(matches!(scored[0].get("score"), Some(Value::Number(_))));
    assert_eq!(
        scored[0].get("marked"),
        Some(&Value::String("<b>Hello</b> world".to_owned()))
    );
}
