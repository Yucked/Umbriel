//! Geospatial functions, `geo::*`.
//!
//! ```rust
//! use umbriel::{*, functions::geo};
//! let area = Field::new("area");
//! assert_eq!(geo::area!(area.clone()).to_debug_sql(), "geo::area(area)");
//! assert_eq!(GeometryLike::from(area).centroid().to_debug_sql(), "area.centroid()");
//! ```

use crate::kinds;

functions! { $ "geo" "::", geo, geo;
    /// The area of a geometry, in square metres.
    area(geometry: GeometryLike) -> kinds::Number [GeometryLike.area];
    /// The bearing from one point to another, in degrees.
    bearing(from: GeometryLike, to: GeometryLike) -> kinds::Number [GeometryLike.bearing];
    /// The centroid of a geometry.
    centroid(geometry: GeometryLike) -> kinds::Point [GeometryLike.centroid];
    /// The great-circle distance between two points, in metres.
    distance(from: GeometryLike, to: GeometryLike) -> kinds::Number [GeometryLike.distance];
    /// Whether the geometry is valid.
    is_valid(geometry: GeometryLike) -> kinds::Bool [GeometryLike.is_valid];
}

/// Geohash functions, `geo::hash::*`.
pub mod hash {
    use crate::{StrandLike, kinds};

    /// A geohash: a string.
    pub type GeoHash = StrandLike;

    functions! { $ "geo::hash" "::", geo_hash, geo::hash;
        /// The point a geohash encodes; `NONE` for a value that is not a string. SurrealDB
        /// dispatches `.hash_decode()` on geometries, where it is always `NONE`.
        decode(geohash: StrandLike) -> kinds::Point [GeometryLike.hash_decode];
        /// The geohash of a point, `accuracy` characters long (12 by default).
        encode(geometry: GeometryLike; accuracy: NumberLike) -> kinds::String
            [GeometryLike.hash_encode];
    }
}
