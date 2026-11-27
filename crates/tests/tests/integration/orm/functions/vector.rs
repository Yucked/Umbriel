//! `vector::*` (`orm/src/functions/vector.rs`, owned by #19): results and their runtime kinds.

use surrealdb::{Surreal, engine::any::Any};
use umbriel::{Function, Render, functions::vector, kinds::KindOf};
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    // The calls have different result kinds, so they are compared as rendered SurrealQL.
    let cases = [
        (vector::add(vec![1, 2], vec![3, 4]).to_debug_sql(), "[4, 6]"),
        (
            vector::cross(vec![1, 0, 0], vec![0, 1, 0]).to_debug_sql(),
            "[0, 0, 1]",
        ),
        (
            vector::divide(vec![4, 6], vec![2, 3]).to_debug_sql(),
            "[2, 2]",
        ),
        (vector::dot(vec![1, 2], vec![3, 4]).to_debug_sql(), "11"),
        (vector::magnitude(vec![3, 4]).to_debug_sql(), "5"),
        (
            vector::multiply(vec![1, 2], vec![3, 4]).to_debug_sql(),
            "[3, 8]",
        ),
        (vector::normalize(vec![3, 4]).to_debug_sql(), "[0.6, 0.8]"),
        (
            vector::project(vec![2, 0], vec![1, 0]).to_debug_sql(),
            "[2, 0]",
        ),
        (
            vector::subtract(vec![4, 6], vec![1, 2]).to_debug_sql(),
            "[3, 4]",
        ),
        (
            vector::distance::chebyshev(vec![1, 2], vec![4, 6]).to_debug_sql(),
            "4",
        ),
        (
            vector::distance::euclidean(vec![1, 2], vec![4, 6]).to_debug_sql(),
            "5",
        ),
        (
            vector::distance::hamming(vec![1, 2], vec![1, 3]).to_debug_sql(),
            "1",
        ),
        (
            vector::distance::manhattan(vec![1, 2], vec![4, 6]).to_debug_sql(),
            "7",
        ),
        (
            vector::distance::minkowski(vec![1, 2], vec![4, 6], 2).to_debug_sql(),
            "5",
        ),
        (
            vector::similarity::cosine(vec![1, 0], vec![1, 0]).to_debug_sql(),
            "1",
        ),
    ];
    for (call, expected) in &cases {
        let equal: bool = read(&db, &format!("RETURN {call} = {expected};")).await;
        assert!(equal, "`{call}` is not {expected}");
    }
    is_a(&db, &vector::angle(vec![1, 0], vec![0, 1]), "number").await;
    is_a(
        &db,
        &vector::similarity::jaccard(vec![1, 2], vec![2, 3]),
        "number",
    )
    .await;
}

/// Whether every element of the array `function` returns is a number.
async fn number_elements<K: KindOf>(db: &Surreal<Any>, function: &Function<K>) -> bool {
    let call = function.to_debug_sql();
    read(
        db,
        &format!("RETURN array::all({call}, |$element| type::is_number($element));"),
    )
    .await
}

#[tokio::test]
async fn normalize_project_and_divide_give_number_elements() {
    let (db, _guard) = db().await;
    for function in [
        vector::normalize(vec![3, 4]),
        vector::project(vec![2, 1], vec![1, 1]),
        vector::divide(vec![1, 2], vec![2, 4]),
    ] {
        is_a(&db, &function, "array").await;
        assert!(
            number_elements(&db, &function).await,
            "`{}` has an element that is not a number",
            function.to_debug_sql()
        );
    }
}

#[tokio::test]
async fn add_subtract_multiply_cross_and_integer_scale_keep_integer_elements() {
    let (db, _guard) = db().await;
    for function in [
        vector::add(vec![1, 2], vec![3, 4]),
        vector::subtract(vec![4, 6], vec![1, 2]),
        vector::multiply(vec![1, 2], vec![3, 4]),
        vector::cross(vec![1, 0, 0], vec![0, 1, 0]),
        vector::scale(vec![1, 2], 3),
    ] {
        let call = function.to_debug_sql();
        let integers: bool = read(
            &db,
            &format!("RETURN array::all({call}, |$element| type::is_int($element));"),
        )
        .await;
        assert!(integers, "`{call}` has an element that is not an integer");
    }
    holds(&db, &vector::scale(vec![1, 2], 3), "[3, 6]").await;
}

#[tokio::test]
async fn new_functions_give_their_results() {
    let (db, _guard) = db().await;
    holds(&db, &vector::sum(vec![vec![1, 2], vec![3, 4]]), "[4, 6]").await;
    holds(
        &db,
        &vector::distance::mahalanobis(vec![1, 2], vec![1, 2], vec![vec![1, 0], vec![0, 1]]),
        "0",
    )
    .await;
    for function in [
        vector::similarity::pearson(vec![1, 2, 3], vec![2, 4, 6]),
        vector::similarity::spearman(vec![1, 2, 3], vec![1, 4, 9]),
    ] {
        let call = function.to_debug_sql();
        let one: bool = read(&db, &format!("RETURN math::abs({call} - 1) < 0.000001;")).await;
        assert!(one, "`{call}` is not 1");
    }
}

#[tokio::test]
async fn sum_of_no_vectors_is_none() {
    let (db, _guard) = db().await;
    let call = vector::sum(Vec::<Vec<i32>>::new()).to_debug_sql();
    let none: bool = read(&db, &format!("RETURN {call} IS NONE;")).await;
    assert!(none, "`{call}` is not NONE");
}

#[tokio::test]
async fn float_results_decode_as_floats() {
    let (db, _guard) = db().await;
    for function in [
        vector::distance::minkowski(vec![1, 2], vec![4, 6], 2),
        vector::distance::mahalanobis(vec![1, 2], vec![2, 2], vec![vec![1, 0], vec![0, 1]]),
        vector::similarity::jaccard(vec![1, 2], vec![2, 3]),
        vector::similarity::pearson(vec![1, 2, 3], vec![1, 2, 4]),
        vector::similarity::spearman(vec![1, 2, 3], vec![1, 3, 2]),
    ] {
        is_a(&db, &function, "float").await;
    }
}

#[tokio::test]
async fn mahalanobis_rejects_a_matrix_that_is_not_positive_definite() {
    let (db, _guard) = db().await;
    let message = call_failure(
        &db,
        &vector::distance::mahalanobis(vec![1, 2], vec![2, 2], vec![vec![0, 0], vec![0, 0]]),
    )
    .await;
    assert!(message.contains("positive-definite"), "{message}");
}

#[tokio::test]
async fn pearson_rejects_zero_variance() {
    let (db, _guard) = db().await;
    let message = call_failure(
        &db,
        &vector::similarity::pearson(vec![1, 1, 1], vec![1, 2, 3]),
    )
    .await;
    assert!(message.contains("variance"), "{message}");
}

#[tokio::test]
async fn knn_distance_fails_outside_a_knn_query() {
    let (db, _guard) = db().await;
    let call = vector::distance::knn!().to_debug_sql();
    let message = failure(&db, &format!("RETURN {call};")).await;
    assert!(message.contains("no KNN operator"), "{call}: {message}");
}

#[tokio::test]
async fn knn_distance_reads_the_distance_of_a_knn_query() {
    let (db, _guard) = db().await;
    assert!(
        succeeds(
            &db,
            "CREATE doc:near SET embedding = [3, 4]; CREATE doc:far SET embedding = [30, 40];"
        )
        .await
    );
    let distance = vector::distance::knn!().to_debug_sql();
    let distances: Vec<f64> = read(
        &db,
        &format!(
            "RETURN (SELECT VALUE {distance} FROM doc WHERE embedding <|1,EUCLIDEAN|> [0, 0]);"
        ),
    )
    .await;
    assert_eq!(distances, [5.0]);
}

#[test]
fn new_functions_render() {
    call(&vector::distance::knn!(), "vector::distance::knn()");
    call(&vector::distance::knn!(1), "vector::distance::knn(1)");
}

#[tokio::test]
async fn dot_of_integers_is_an_integer() {
    let (db, _guard) = db().await;
    is_a(&db, &vector::dot(vec![1, 2], vec![3, 4]), "int").await;
}

#[tokio::test]
async fn hamming_is_an_integer_count() {
    let (db, _guard) = db().await;
    let function = vector::distance::hamming(vec![1.5, 2.5], vec![1.5, 3.5]);
    is_a(&db, &function, "int").await;
    holds(&db, &function, "1").await;
}
