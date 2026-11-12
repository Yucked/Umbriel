//! `geo::*` (`orm/src/functions/geo.rs`): every function and method form rendered, and the
//! results SurrealDB computes.

use umbriel::{Field, GeometryLike, Param, Render, functions::geo as geo_fn};
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

#[test]
fn rendering() {
    let area = Field::new("area");
    let from = Field::new("from");
    let to = Field::new("to");
    calls!(
        (geo_fn::area!(area.clone()), "geo::area(area)"),
        (
            geo_fn::bearing!(from.clone(), to.clone()),
            "geo::bearing(from, to)"
        ),
        (geo_fn::centroid!(area.clone()), "geo::centroid(area)"),
        (
            geo_fn::distance!(from.clone(), to),
            "geo::distance(from, to)"
        ),
        (geo_fn::is_valid!(area), "geo::is_valid(area)"),
        (
            geo_fn::hash::decode!("mpuxk4s24f51"),
            "geo::hash::decode('mpuxk4s24f51')"
        ),
        (
            geo_fn::hash::encode!(from.clone()),
            "geo::hash::encode(from)"
        ),
        (geo_fn::hash::encode!(from, 5), "geo::hash::encode(from, 5)"),
    );
    assert_parses(&format!(
        "RETURN {};",
        geo_fn::area!(square()).to_debug_sql()
    ));
}

#[test]
fn method_rendering() {
    let area = GeometryLike::from(Field::new("area"));
    calls!(
        (area.area(), "area.area()"),
        (area.bearing(Field::new("to")), "area.bearing(to)"),
        (area.centroid(), "area.centroid()"),
        (area.distance(Field::new("to")), "area.distance(to)"),
        (area.is_valid(), "area.is_valid()"),
        (area.hash_decode(), "area.hash_decode()"),
        (area.hash_encode(Some(5)), "area.hash_encode(5)"),
    );
}

#[tokio::test]
async fn results() {
    let db = db().await;
    let london = geo::Point::new(-0.118092, 51.509865);
    let paris = geo::Point::new(2.352222, 48.856614);
    for function in [
        geo_fn::area!(square()),
        geo_fn::bearing!(london, paris),
        geo_fn::distance!(london, paris),
    ] {
        is_a(&db, &function, "number").await;
    }
    is_a(&db, &geo_fn::centroid!(square()), "point").await;
    is_a(&db, &geo_fn::hash::encode!(london), "string").await;
    holds(&db, &geo_fn::hash::encode!(london, 5), "'gcpvj'").await;
    holds(
        &db,
        &GeometryLike::from(london).hash_encode(Some(5)),
        "'gcpvj'",
    )
    .await;
    is_a(&db, &geo_fn::hash::decode!("gcpvj"), "point").await;
    holds(&db, &geo_fn::is_valid!(square()), "true").await;
}

#[test]
fn generated_bindings() {
    let nested = geo_fn::area!(geo_fn::centroid!(Param::new("shape")));
    let (sql, vars) = nested.to_query().expect("renders").into_parts();
    assert_eq!(sql, "geo::area(geo::centroid($shape))");
    assert_eq!(vars.len(), 0);
}

#[tokio::test]
async fn runtime_type_failures() {
    let db = db().await;
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
    let db = db().await;
    let london = geo::Point::new(-0.118092, 51.509865);
    let paris = geo::Point::new(2.352222, 48.856614);
    let g = || GeometryLike::from(square());
    let p = || GeometryLike::from(london);
    same_all!(&db, [
        (g().area(), geo_fn::area!(square())),
        (p().bearing(paris), geo_fn::bearing!(london, paris)),
        (g().centroid(), geo_fn::centroid!(square())),
        (p().distance(paris), geo_fn::distance!(london, paris)),
        (g().is_valid(), geo_fn::is_valid!(square())),
        (p().hash_encode(Some(5)), geo_fn::hash::encode!(london, 5)),
    ]);
    // `geo::hash::decode` takes a geohash, so its function form has no geometry argument to
    // compare with; on a geometry the method is `NONE` (`runtime_type_failures`).
    holds(&db, &p().hash_decode(), "NONE").await;
}
