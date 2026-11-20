//! Vector functions, `vector::*`.
//!
//! Each result kind follows what SurrealDB 3.3.4 returns at run time, not the kind its function
//! table declares: element-wise results keep `Number` arithmetic (integer inputs to `add` stay
//! integers, `normalize` gives floats), `dot` and the distances built from it keep the input's
//! number kind, and `hamming` counts positions.
//!
//! ```rust
//! use umbriel::{*, functions::vector};
//! let embedding = Field::new("embedding");
//! assert_eq!(
//!     vector::similarity::cosine!(embedding, [1, 0, 0]).to_debug_sql(),
//!     "vector::similarity::cosine(embedding, [1, 0, 0])"
//! );
//! assert_eq!(vector::sum!([[1, 2], [3, 4]]).to_debug_sql(), "vector::sum([[1, 2], [3, 4]])");
//! ```

use crate::kinds;

functions! { $ "vector" "::", vector, vector;
    /// The element-wise sum of `a` and `b`.
    add(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Number>;
    /// The angle between `a` and `b`, in radians.
    angle(a: ArrayLike, b: ArrayLike) -> kinds::Number;
    /// The cross product of two three-dimensional vectors.
    cross(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Number>;
    /// The element-wise quotient of `a` by `b`.
    divide(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Number>;
    /// The dot product of `a` and `b`: an integer for integer vectors.
    dot(a: ArrayLike, b: ArrayLike) -> kinds::Number;
    /// The magnitude (length) of `vector`.
    magnitude(vector: ArrayLike) -> kinds::Number;
    /// The element-wise product of `a` and `b`.
    multiply(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Number>;
    /// `vector` scaled to a magnitude of 1.
    normalize(vector: ArrayLike) -> kinds::Array<kinds::Number>;
    /// The projection of `a` onto `b`.
    project(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Number>;
    /// Every element of `vector` multiplied by `scalar`.
    scale(vector: ArrayLike, scalar: NumberLike) -> kinds::Array<kinds::Number>;
    /// The element-wise difference of `a` and `b`.
    subtract(a: ArrayLike, b: ArrayLike) -> kinds::Array<kinds::Number>;
    /// The element-wise sum of every vector in `vectors`; `NONE` when there are none.
    sum(vectors: ArrayLike) -> kinds::Optional<kinds::Array<kinds::Number>>;
}

// The paths `vector` exported before its functions moved to the catalogue: the distance and
// similarity functions at the module root, and each two-vector macro under its full name.
pub use distance::{
    chebyshev as vector_distance_chebyshev, chebyshev_fn as distance_chebyshev_fn,
    euclidean as vector_distance_euclidean, euclidean_fn as distance_euclidean_fn,
    hamming as vector_distance_hamming, hamming_fn as distance_hamming_fn,
    manhattan as vector_distance_manhattan, manhattan_fn as distance_manhattan_fn,
    minkowski as vector_distance_minkowski, minkowski_fn as distance_minkowski_fn,
};
pub use similarity::{
    cosine as vector_similarity_cosine, cosine_fn as similarity_cosine_fn,
    jaccard as vector_similarity_jaccard, jaccard_fn as similarity_jaccard_fn,
};
pub use {
    add as vector_add, angle as vector_angle, cross as vector_cross, divide as vector_divide,
    dot as vector_dot, multiply as vector_multiply, project as vector_project,
    subtract as vector_subtract,
};

/// Distances between vectors, `vector::distance::*`.
pub mod distance {
    use crate::kinds;

    functions! { $ "vector::distance" "::", vector_distance, vector::distance;
        /// The greatest difference along any dimension.
        chebyshev(a: ArrayLike, b: ArrayLike) -> kinds::Number;
        /// The straight-line distance.
        euclidean(a: ArrayLike, b: ArrayLike) -> kinds::Number;
        /// The number of positions where `a` and `b` differ.
        hamming(a: ArrayLike, b: ArrayLike) -> kinds::Int;
        /// The distance a KNN operator computed for the current record, optionally of the
        /// `reference`th KNN operator of the query; `NONE` for a record it computed none for. A
        /// query calling it without a KNN operator in its `WHERE` fails when SurrealDB plans it.
        knn(; reference: NumberLike) -> kinds::Optional<kinds::Number>;
        /// The Mahalanobis distance under `covariance`, a symmetric positive-definite matrix of
        /// the vectors' dimension; anything else fails the call.
        mahalanobis(a: ArrayLike, b: ArrayLike, covariance: ArrayLike) -> kinds::Float;
        /// The sum of the absolute differences.
        manhattan(a: ArrayLike, b: ArrayLike) -> kinds::Number;
        /// The Minkowski distance of order `order`.
        minkowski(a: ArrayLike, b: ArrayLike, order: NumberLike) -> kinds::Float;
    }
}

/// Similarities between vectors, `vector::similarity::*`.
pub mod similarity {
    use crate::kinds;

    functions! { $ "vector::similarity" "::", vector_similarity, vector::similarity;
        /// The cosine of the angle between `a` and `b`.
        cosine(a: ArrayLike, b: ArrayLike) -> kinds::Number;
        /// The Jaccard similarity of the elements of `a` and `b`, as sets; 1 when both are empty.
        jaccard(a: ArrayLike, b: ArrayLike) -> kinds::Float;
        /// The Pearson correlation; fails for fewer than two dimensions or a zero variance.
        pearson(a: ArrayLike, b: ArrayLike) -> kinds::Float;
        /// The Spearman rank correlation; fails for fewer than two dimensions or a zero variance.
        spearman(a: ArrayLike, b: ArrayLike) -> kinds::Float;
    }
}
