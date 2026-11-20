//! `vector::*` (`orm/src/functions/vector.rs`, owned by #19): results, moved
//! unchanged from the former `functions.rs`.

use umbriel::functions::vector;
use umbriel_tests::*;

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    let cases = [
        (vector::add!(vec![1, 2], vec![3, 4]), "[4, 6]"),
        (vector::cross!(vec![1, 0, 0], vec![0, 1, 0]), "[0, 0, 1]"),
        (vector::divide!(vec![4, 6], vec![2, 3]), "[2, 2]"),
        (vector::dot!(vec![1, 2], vec![3, 4]), "11"),
        (vector::magnitude!(vec![3, 4]), "5"),
        (vector::multiply!(vec![1, 2], vec![3, 4]), "[3, 8]"),
        (vector::normalize!(vec![3, 4]), "[0.6, 0.8]"),
        (vector::project!(vec![2, 0], vec![1, 0]), "[2, 0]"),
        (vector::subtract!(vec![4, 6], vec![1, 2]), "[3, 4]"),
        (vector::distance::chebyshev!(vec![1, 2], vec![4, 6]), "4"),
        (vector::distance::euclidean!(vec![1, 2], vec![4, 6]), "5"),
        (vector::distance::hamming!(vec![1, 2], vec![1, 3]), "1"),
        (vector::distance::manhattan!(vec![1, 2], vec![4, 6]), "7"),
        (vector::distance::minkowski!(vec![1, 2], vec![4, 6], 2), "5"),
        (vector::similarity::cosine!(vec![1, 0], vec![1, 0]), "1"),
    ];
    for (function, expected) in &cases {
        holds(&db, function, expected).await;
    }
    is_a(&db, &vector::angle!(vec![1, 0], vec![0, 1]), "number").await;
    is_a(
        &db,
        &vector::similarity::jaccard!(vec![1, 2], vec![2, 3]),
        "number",
    )
    .await;
}
