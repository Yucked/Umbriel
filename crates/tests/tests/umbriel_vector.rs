//! `vector::*` (`orm/src/functions/vector.rs`, owned by #19): rendering and results, moved
//! unchanged from the former `functions.rs`.

use umbriel::{Field, functions::vector};
use umbriel_tests::*;

#[test]
fn rendering() {
    let a = Field::new("a");
    let b = Field::new("b");
    call(&vector::add!(a.clone(), b.clone()), "vector::add(a, b)");
    call(&vector::angle!(a.clone(), b.clone()), "vector::angle(a, b)");
    call(&vector::cross!(a.clone(), b.clone()), "vector::cross(a, b)");
    call(
        &vector::divide!(a.clone(), b.clone()),
        "vector::divide(a, b)",
    );
    call(&vector::dot!(a.clone(), b.clone()), "vector::dot(a, b)");
    call(&vector::magnitude!(a.clone()), "vector::magnitude(a)");
    call(
        &vector::multiply!(a.clone(), b.clone()),
        "vector::multiply(a, b)",
    );
    call(&vector::normalize!(a.clone()), "vector::normalize(a)");
    call(
        &vector::project!(a.clone(), b.clone()),
        "vector::project(a, b)",
    );
    call(
        &vector::subtract!(a.clone(), b.clone()),
        "vector::subtract(a, b)",
    );
    call(
        &vector::distance::chebyshev!(a.clone(), b.clone()),
        "vector::distance::chebyshev(a, b)",
    );
    call(
        &vector::distance::euclidean!(a.clone(), b.clone()),
        "vector::distance::euclidean(a, b)",
    );
    call(
        &vector::distance::hamming!(a.clone(), b.clone()),
        "vector::distance::hamming(a, b)",
    );
    call(
        &vector::distance::manhattan!(a.clone(), b.clone()),
        "vector::distance::manhattan(a, b)",
    );
    call(
        &vector::distance::minkowski!(a.clone(), b.clone(), 3),
        "vector::distance::minkowski(a, b, 3)",
    );
    call(
        &vector::similarity::cosine!(a.clone(), b.clone()),
        "vector::similarity::cosine(a, b)",
    );
    call(
        &vector::similarity::jaccard!(a, b),
        "vector::similarity::jaccard(a, b)",
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
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
