//! `DEFINE INDEX` (`orm/src/statements/define_index.rs`): unique, composite, full-text, vector
//! and count indexes, and what SurrealDB does with them.

use umbriel::statements::{Distance, VectorType, diskann, hnsw};
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
    .where_(content.matches_with(umbriel::Match::reference(1), "hello"))
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

#[test]
fn index_kinds_render() {
    let doc = Table::from("doc");
    let embedding = Field::new("embedding");
    let cases = [
        (
            define_index("doc_embedding")
                .on_table(doc.clone())
                .fields(embedding.clone())
                .hnsw(
                    hnsw(4)
                        .distance(Distance::Cosine)
                        .vector_type(VectorType::F64)
                        .efc(150)
                        .m(12)
                        .m0(24)
                        .lm(0.5)
                        .extend_candidates()
                        .keep_pruned_connections()
                        .hashed_vector(),
                ),
            "DEFINE INDEX doc_embedding ON TABLE doc FIELDS embedding HNSW DIMENSION 4 DIST COSINE \
             TYPE F64 EFC 150 M 12 M0 24 LM 0.5 EXTEND_CANDIDATES KEEP_PRUNED_CONNECTIONS \
             HASHED_VECTOR;",
        ),
        (
            define_index("doc_embedding")
                .on_table(doc.clone())
                .fields(embedding.clone())
                .diskann(
                    diskann(4)
                        .distance(Distance::Cosine)
                        .vector_type(VectorType::F16)
                        .degree(32)
                        .l_build(88)
                        .alpha(1.4)
                        .hashed_vector(),
                ),
            "DEFINE INDEX doc_embedding ON TABLE doc FIELDS embedding DISKANN DIMENSION 4 \
             DIST COSINE TYPE F16 DEGREE 32 L_BUILD 88 ALPHA 1.4 HASHED_VECTOR;",
        ),
        (
            define_index("doc_embedding")
                .on_table(doc.clone())
                .fields(embedding.clone())
                .hnsw(hnsw(4).distance(Distance::Minkowski(3.0))),
            "DEFINE INDEX doc_embedding ON TABLE doc FIELDS embedding HNSW DIMENSION 4 \
             DIST MINKOWSKI 3;",
        ),
        (
            define_index("doc_count").on_table(doc.clone()).count(),
            "DEFINE INDEX doc_count ON TABLE doc COUNT;",
        ),
        (
            define_index("doc_count")
                .on_table(doc.clone())
                .count_where(Field::new("published").is(true)),
            "DEFINE INDEX doc_count ON TABLE doc COUNT WHERE published IS true;",
        ),
        (
            define_index("doc_title")
                .on_table(doc.clone())
                .fields(Field::new("title"))
                .comment("Titles")
                .concurrently(),
            "DEFINE INDEX doc_title ON TABLE doc FIELDS title COMMENT 'Titles' CONCURRENTLY;",
        ),
    ];
    for (statement, expected) in cases {
        assert_sql(&statement, expected);
    }
    assert!(
        define_index("doc_count")
            .on_table(doc)
            .fields(embedding)
            .count()
            .to_query()
            .is_err(),
        "a COUNT index has no fields"
    );
}

/// `doc:near`, `doc:middle` and `doc:far` with two-dimensional embeddings at growing distances
/// from the origin, under `index` if there is one.
async fn documents(db: &Surreal<Any>, index: Option<umbriel::statements::DefineIndexStatement>) {
    if let Some(index) = index {
        run(db, index).await;
    }
    run(
        db,
        Raw::new(
            "CREATE doc:near SET embedding = [1, 1]; \
             CREATE doc:middle SET embedding = [3, 3]; \
             CREATE doc:far SET embedding = [9, 9];",
        ),
    )
    .await;
}

/// The keys of the documents `condition` selects, in order.
async fn nearest(db: &Surreal<Any>, condition: &umbriel::Operation) -> Vec<String> {
    read(
        db,
        &format!(
            "RETURN (SELECT VALUE record::id(id) FROM doc WHERE {} ORDER BY id);",
            condition.to_debug_sql()
        ),
    )
    .await
}

fn embedding_index() -> umbriel::statements::DefineIndexStatement {
    define_index("doc_embedding")
        .on_table(Table::from("doc"))
        .fields(Field::new("embedding"))
}

#[tokio::test]
async fn an_hnsw_index_answers_knn() {
    let (db, _guard) = db().await;
    documents(
        &db,
        Some(embedding_index().hnsw(hnsw(2).distance(Distance::Euclidean))),
    )
    .await;
    let found = nearest(&db, &Field::new("embedding").knn_ef(2, 40, [0, 0])).await;
    assert_eq!(found, ["middle", "near"]);
}

#[tokio::test]
async fn a_diskann_index_answers_knn() {
    let (db, _guard) = db().await;
    documents(
        &db,
        Some(embedding_index().diskann(diskann(2).distance(Distance::Euclidean))),
    )
    .await;
    // For DiskANN the second number is the search list size.
    let found = nearest(&db, &Field::new("embedding").knn_ef(2, 40, [0, 0])).await;
    assert_eq!(found, ["middle", "near"]);
}

/// SurrealDB 3.3.4 refuses `<|k|>`, which `Operatable` therefore does not offer.
#[tokio::test]
async fn knn_with_only_k_is_refused() {
    let (db, _guard) = db().await;
    documents(&db, Some(embedding_index().hnsw(hnsw(2)))).await;
    let message = failure(&db, "SELECT id FROM doc WHERE embedding <|2|> [0, 0];").await;
    assert!(message.contains("no longer supported"), "{message}");
}

#[tokio::test]
async fn a_metric_search_answers_over_an_indexed_field() {
    let (db, _guard) = db().await;
    documents(
        &db,
        Some(embedding_index().hnsw(hnsw(2).distance(Distance::Euclidean))),
    )
    .await;
    let condition = Field::new("embedding").knn_metric(2, Distance::Euclidean, [0, 0]);
    assert_eq!(nearest(&db, &condition).await, ["middle", "near"]);
}

#[tokio::test]
async fn a_count_index_counts_and_its_condition_filters() {
    let (db, _guard) = db().await;
    let published = Field::new("published");
    run(
        &db,
        define_index("doc_published")
            .on_table(Table::from("doc"))
            .count_where(published.clone().is(true)),
    )
    .await;
    run(
        &db,
        Raw::new(
            "CREATE doc:one SET published = true; \
             CREATE doc:two SET published = true; \
             CREATE doc:three SET published = false;",
        ),
    )
    .await;
    let counted: i64 = read(
        &db,
        "RETURN (SELECT count() FROM doc WHERE published IS true GROUP ALL)[0].count;",
    )
    .await;
    assert_eq!(counted, 2);
    let all: i64 = read(&db, "RETURN (SELECT count() FROM doc GROUP ALL)[0].count;").await;
    assert_eq!(all, 3);
}

#[tokio::test]
async fn a_dimension_mismatch_is_rejected_on_write() {
    let (db, _guard) = db().await;
    run(&db, embedding_index().hnsw(hnsw(2))).await;
    let message = failure(&db, "CREATE doc:wrong SET embedding = [1, 2, 3];").await;
    assert!(message.to_lowercase().contains("dimension"), "{message}");
}
