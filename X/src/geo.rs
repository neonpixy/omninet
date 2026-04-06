//! Geographic coordinate primitives.
//!
//! Foundational types for latitude/longitude with validation, Haversine
//! distance calculation, and radius containment checks. Used by
//! World/Physical and any crate needing geographic math.

use serde::{Deserialize, Serialize};

/// Earth's mean radius in meters (WGS-84).
const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// A geographic coordinate (latitude/longitude/optional altitude).
///
/// Latitude ranges from -90.0 (south pole) to 90.0 (north pole).
/// Longitude ranges from -180.0 (west) to 180.0 (east).
/// Altitude is in meters above sea level (optional).
/// Accuracy is the horizontal accuracy in meters (optional, from GPS).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoCoordinate {
    /// Degrees north of the equator (-90.0 to 90.0).
    pub latitude: f64,
    /// Degrees east of the prime meridian (-180.0 to 180.0).
    pub longitude: f64,
    /// Meters above sea level, if known.
    pub altitude_meters: Option<f64>,
    /// Horizontal accuracy in meters (from GPS), if known.
    pub accuracy_meters: Option<f64>,
}

impl GeoCoordinate {
    /// Create a new coordinate with validation.
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, GeoError> {
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(GeoError::InvalidLatitude(latitude));
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(GeoError::InvalidLongitude(longitude));
        }
        Ok(Self {
            latitude,
            longitude,
            altitude_meters: None,
            accuracy_meters: None,
        })
    }

    /// Create with altitude.
    pub fn with_altitude(mut self, meters: f64) -> Self {
        self.altitude_meters = Some(meters);
        self
    }

    /// Create with accuracy.
    pub fn with_accuracy(mut self, meters: f64) -> Self {
        self.accuracy_meters = Some(meters);
        self
    }

    /// Haversine distance to another coordinate in meters.
    ///
    /// Uses the mean Earth radius (WGS-84). Accurate to ~0.3% for
    /// most distances. Does not account for altitude difference.
    pub fn distance_to(&self, other: &GeoCoordinate) -> f64 {
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let dlat = (other.latitude - self.latitude).to_radians();
        let dlon = (other.longitude - self.longitude).to_radians();

        let a = (dlat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_M * c
    }

    /// Whether this coordinate is within `radius_meters` of `center`.
    pub fn is_within(&self, center: &GeoCoordinate, radius_meters: f64) -> bool {
        self.distance_to(center) <= radius_meters
    }

    /// Bearing from this coordinate to another, in degrees (0 = north, 90 = east).
    pub fn bearing_to(&self, other: &GeoCoordinate) -> f64 {
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let dlon = (other.longitude - self.longitude).to_radians();

        let y = dlon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();

        (y.atan2(x).to_degrees() + 360.0) % 360.0
    }

    /// Midpoint between this coordinate and another.
    pub fn midpoint(&self, other: &GeoCoordinate) -> GeoCoordinate {
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let lon1 = self.longitude.to_radians();
        let dlon = (other.longitude - self.longitude).to_radians();

        let bx = lat2.cos() * dlon.cos();
        let by = lat2.cos() * dlon.sin();

        let lat = (lat1.sin() + lat2.sin()).atan2(((lat1.cos() + bx).powi(2) + by.powi(2)).sqrt());
        let lon = lon1 + by.atan2(lat1.cos() + bx);

        GeoCoordinate {
            latitude: lat.to_degrees(),
            longitude: ((lon.to_degrees() + 540.0) % 360.0) - 180.0,
            altitude_meters: None,
            accuracy_meters: None,
        }
    }
}

impl std::fmt::Display for GeoCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.6}, {:.6})", self.latitude, self.longitude)
    }
}

/// Errors for geographic operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GeoError {
    /// Latitude must be between -90.0 and 90.0 degrees.
    #[error("invalid latitude: {0} (must be -90.0 to 90.0)")]
    InvalidLatitude(f64),

    /// Longitude must be between -180.0 and 180.0 degrees.
    #[error("invalid longitude: {0} (must be -180.0 to 180.0)")]
    InvalidLongitude(f64),

    /// Radius must be a positive number.
    #[error("invalid radius: must be positive")]
    InvalidRadius,

    /// A polygon requires at least 3 vertices.
    #[error("invalid polygon: need at least 3 vertices")]
    InvalidPolygon,
}

/// Check if a point is inside a polygon using ray casting.
///
/// The polygon is defined by an ordered list of vertices (implicitly closed).
/// Uses the standard even-odd rule.
pub fn point_in_polygon(point: &GeoCoordinate, polygon: &[GeoCoordinate]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let n = polygon.len();

    let mut j = n - 1;
    for i in 0..n {
        let pi = &polygon[i];
        let pj = &polygon[j];

        if ((pi.latitude > point.latitude) != (pj.latitude > point.latitude))
            && (point.longitude
                < (pj.longitude - pi.longitude) * (point.latitude - pi.latitude)
                    / (pj.latitude - pi.latitude)
                    + pi.longitude)
        {
            inside = !inside;
        }
        j = i;
    }

    inside
}

/// Calculate the approximate area of a polygon in square meters.
///
/// Uses the spherical excess formula (Girard's theorem). Accurate for
/// polygons that don't span more than a hemisphere.
pub fn polygon_area(polygon: &[GeoCoordinate]) -> f64 {
    if polygon.len() < 3 {
        return 0.0;
    }

    let n = polygon.len();
    let mut sum = 0.0_f64;

    for i in 0..n {
        let p1 = &polygon[i];
        let p2 = &polygon[(i + 1) % n];

        let lat1 = p1.latitude.to_radians();
        let lat2 = p2.latitude.to_radians();
        let dlon = (p2.longitude - p1.longitude).to_radians();

        sum += dlon * (2.0 + lat1.sin() + lat2.sin());
    }

    (sum.abs() / 2.0) * EARTH_RADIUS_M * EARTH_RADIUS_M
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_coordinates() {
        let c = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        assert_eq!(c.latitude, 39.7392);
        assert_eq!(c.longitude, -104.9903);
        assert!(c.altitude_meters.is_none());
    }

    #[test]
    fn with_altitude_and_accuracy() {
        let c = GeoCoordinate::new(0.0, 0.0)
            .unwrap()
            .with_altitude(1609.0)
            .with_accuracy(5.0);
        assert_eq!(c.altitude_meters, Some(1609.0));
        assert_eq!(c.accuracy_meters, Some(5.0));
    }

    #[test]
    fn invalid_latitude() {
        assert!(matches!(
            GeoCoordinate::new(91.0, 0.0),
            Err(GeoError::InvalidLatitude(91.0))
        ));
        assert!(matches!(
            GeoCoordinate::new(-91.0, 0.0),
            Err(GeoError::InvalidLatitude(-91.0))
        ));
    }

    #[test]
    fn invalid_longitude() {
        assert!(matches!(
            GeoCoordinate::new(0.0, 181.0),
            Err(GeoError::InvalidLongitude(181.0))
        ));
    }

    #[test]
    fn boundary_values() {
        assert!(GeoCoordinate::new(90.0, 180.0).is_ok());
        assert!(GeoCoordinate::new(-90.0, -180.0).is_ok());
        assert!(GeoCoordinate::new(0.0, 0.0).is_ok());
    }

    #[test]
    fn haversine_denver_to_boulder() {
        let denver = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        let boulder = GeoCoordinate::new(40.0150, -105.2705).unwrap();
        let dist = denver.distance_to(&boulder);
        // ~40km, allow 5% tolerance
        assert!((dist - 40_000.0).abs() < 2_000.0, "got {dist}m");
    }

    #[test]
    fn haversine_same_point() {
        let p = GeoCoordinate::new(51.5074, -0.1278).unwrap();
        assert!(p.distance_to(&p) < 0.01);
    }

    #[test]
    fn haversine_antipodal() {
        let north = GeoCoordinate::new(90.0, 0.0).unwrap();
        let south = GeoCoordinate::new(-90.0, 0.0).unwrap();
        let dist = north.distance_to(&south);
        let half_circumference = std::f64::consts::PI * EARTH_RADIUS_M;
        assert!((dist - half_circumference).abs() < 1000.0);
    }

    #[test]
    fn is_within_radius() {
        let center = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        let nearby = GeoCoordinate::new(39.7400, -104.9900).unwrap();
        let far = GeoCoordinate::new(40.0150, -105.2705).unwrap();

        assert!(nearby.is_within(&center, 1000.0));
        assert!(!far.is_within(&center, 1000.0));
    }

    #[test]
    fn bearing_north() {
        let a = GeoCoordinate::new(0.0, 0.0).unwrap();
        let b = GeoCoordinate::new(1.0, 0.0).unwrap();
        let bearing = a.bearing_to(&b);
        assert!((bearing - 0.0).abs() < 1.0, "got {bearing}°");
    }

    #[test]
    fn bearing_east() {
        let a = GeoCoordinate::new(0.0, 0.0).unwrap();
        let b = GeoCoordinate::new(0.0, 1.0).unwrap();
        let bearing = a.bearing_to(&b);
        assert!((bearing - 90.0).abs() < 1.0, "got {bearing}°");
    }

    #[test]
    fn midpoint_equator() {
        let a = GeoCoordinate::new(0.0, 0.0).unwrap();
        let b = GeoCoordinate::new(0.0, 10.0).unwrap();
        let mid = a.midpoint(&b);
        assert!((mid.latitude - 0.0).abs() < 0.01);
        assert!((mid.longitude - 5.0).abs() < 0.01);
    }

    #[test]
    fn point_in_triangle() {
        let triangle = vec![
            GeoCoordinate::new(0.0, 0.0).unwrap(),
            GeoCoordinate::new(0.0, 10.0).unwrap(),
            GeoCoordinate::new(10.0, 5.0).unwrap(),
        ];
        let inside = GeoCoordinate::new(3.0, 5.0).unwrap();
        let outside = GeoCoordinate::new(11.0, 5.0).unwrap();

        assert!(point_in_polygon(&inside, &triangle));
        assert!(!point_in_polygon(&outside, &triangle));
    }

    #[test]
    fn point_in_polygon_too_few_vertices() {
        let line = vec![
            GeoCoordinate::new(0.0, 0.0).unwrap(),
            GeoCoordinate::new(1.0, 1.0).unwrap(),
        ];
        let p = GeoCoordinate::new(0.5, 0.5).unwrap();
        assert!(!point_in_polygon(&p, &line));
    }

    #[test]
    fn serde_round_trip() {
        let c = GeoCoordinate::new(39.7392, -104.9903)
            .unwrap()
            .with_altitude(1609.0);
        let json = serde_json::to_string(&c).unwrap();
        let parsed: GeoCoordinate = serde_json::from_str(&json).unwrap();
        assert_eq!(c, parsed);
    }

    #[test]
    fn display_format() {
        let c = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        assert_eq!(c.to_string(), "(39.739200, -104.990300)");
    }

    #[test]
    fn polygon_area_nonzero() {
        let square = vec![
            GeoCoordinate::new(0.0, 0.0).unwrap(),
            GeoCoordinate::new(0.0, 1.0).unwrap(),
            GeoCoordinate::new(1.0, 1.0).unwrap(),
            GeoCoordinate::new(1.0, 0.0).unwrap(),
        ];
        let area = polygon_area(&square);
        assert!(area > 0.0);
        // ~1 degree square at equator ≈ 12,300 km²
        let expected_km2 = 12_300.0;
        let actual_km2 = area / 1_000_000.0;
        assert!(
            (actual_km2 - expected_km2).abs() < 500.0,
            "got {actual_km2} km²"
        );
    }
}
