//! `geo::*` (`orm/src/functions/geo.rs`): every function and method form run, and the
//! results SurrealDB computes.

use umbriel::{GeometryLike, Param, Render, functions::geo as geo_fn};
use umbriel_tests::*;

fn square() -> geo::Polygon {
    geo::Polygon::new(
        geo::LineString::from(vec![
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (0.0, 0.0),
        ]),
        vec![],
    )
}

#[tokio::test]
async fn results() {
    let (db, _guard) = db().await;
    let london = geo::Point::new(-0.118092, 51.509865);
    let paris = geo::Point::new(2.352222, 48.856614);
    for function in [
        geo_fn::area(square()),
        geo_fn::bearing(london, paris),
        geo_fn::distance(london, paris),
    ] {
        is_a(&db, &function, "number").await;
    }
    is_a(&db, &geo_fn::centroid(square()), "point").await;
    is_a(&db, &geo_fn::hash::encode!(london), "string").await;
    holds(&db, &geo_fn::hash::encode!(london, 5), "'gcpvj'").await;
    holds(
        &db,
        &GeometryLike::from(london).hash_encode(Some(5)),
        "'gcpvj'",
    )
    .await;
    is_a(&db, &geo_fn::hash::decode("gcpvj"), "point").await;
    holds(&db, &geo_fn::is_valid(square()), "true").await;
}

#[test]
fn generated_bindings() {
    let nested = geo_fn::area(geo_fn::centroid(Param::new("shape")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "geo::area(geo::centroid($shape))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let (db, _guard) = db().await;
    let error = failure(&db, "RETURN geo::area('not a geometry');").await;
    assert!(error.contains("geo::area"), "{error}");
    // `geo::hash::decode` takes any value and gives `NONE` for one that is not a geohash string.
    holds(
        &db,
        &GeometryLike::from(geo::Point::new(1.0, 2.0)).hash_decode(),
        "NONE",
    )
    .await;
}

/// Every mapped method runs on a geometry and evaluates like its function form.
#[tokio::test]
async fn methods_run() {
    let (db, _guard) = db().await;
    let london = geo::Point::new(-0.118092, 51.509865);
    let paris = geo::Point::new(2.352222, 48.856614);
    let g = || GeometryLike::from(square());
    let p = || GeometryLike::from(london);
    same_all!(
        &db,
        [
            (g().area(), geo_fn::area(square())),
            (p().bearing(paris), geo_fn::bearing(london, paris)),
            (g().centroid(), geo_fn::centroid(square())),
            (p().distance(paris), geo_fn::distance(london, paris)),
            (g().is_valid(), geo_fn::is_valid(square())),
            (p().hash_encode(Some(5)), geo_fn::hash::encode!(london, 5)),
        ]
    );
    // `geo::hash::decode` takes a geohash, so its function form has no geometry argument to
    // compare with; on a geometry the method is `NONE` (`runtime_type_failures`).
    holds(&db, &p().hash_decode(), "NONE").await;
}
