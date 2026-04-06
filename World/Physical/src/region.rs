//! Geographic regions with nesting.
//!
//! A Region is a named geographic area — a neighborhood, city, bioregion —
//! that can contain Places and nest inside other Regions. Boundaries are
//! either circles, polygons, or purely named (no geometry).
//!
//! RegionDeclaration is the pull-based mechanism: a person declares which
//! regions they belong to, rather than the system tracking them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::{point_in_polygon, GeoCoordinate};

// ---------------------------------------------------------------------------
// MARK: - RegionType
// ---------------------------------------------------------------------------

/// The scale or kind of a geographic region.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegionType {
    Neighborhood,
    City,
    County,
    State,
    Bioregion,
    Country,
    Custom(String),
}

// ---------------------------------------------------------------------------
// MARK: - RegionBoundary
// ---------------------------------------------------------------------------

/// How a region's geographic extent is defined.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegionBoundary {
    /// A circle defined by center and radius in meters.
    Circle {
        center: GeoCoordinate,
        radius_meters: f64,
    },
    /// A polygon defined by an ordered list of vertices (implicitly closed).
    Polygon(Vec<GeoCoordinate>),
    /// A named region with no geometric boundary (e.g., cultural or
    /// administrative regions whose borders are political, not spatial).
    Named,
}

impl RegionBoundary {
    /// Check whether a coordinate falls inside this boundary.
    ///
    /// - Circle: delegates to `GeoCoordinate::is_within`.
    /// - Polygon: delegates to `x::point_in_polygon`.
    /// - Named: always returns false (no geometry to test against).
    pub fn contains(&self, coord: &GeoCoordinate) -> bool {
        match self {
            RegionBoundary::Circle {
                center,
                radius_meters,
            } => coord.is_within(center, *radius_meters),
            RegionBoundary::Polygon(vertices) => point_in_polygon(coord, vertices),
            RegionBoundary::Named => false,
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - Region
// ---------------------------------------------------------------------------

/// A geographic region that can contain Places and nest inside other Regions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Region {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub region_type: RegionType,
    pub boundary: RegionBoundary,
    pub parent_id: Option<Uuid>,
    pub creator: String,
    pub created_at: DateTime<Utc>,
}

impl Region {
    /// Create a new region.
    ///
    /// Generates a random UUID and sets the timestamp to now.
    pub fn new(
        name: impl Into<String>,
        region_type: RegionType,
        boundary: RegionBoundary,
        creator: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            region_type,
            boundary,
            parent_id: None,
            creator: creator.into(),
            created_at: Utc::now(),
        }
    }

    // -- Builder methods ----------------------------------------------------

    /// Set a parent region (for nesting).
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Set an optional description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    // -- Geometry -----------------------------------------------------------

    /// Check whether a coordinate falls inside this region's boundary.
    pub fn contains_point(&self, coord: &GeoCoordinate) -> bool {
        self.boundary.contains(coord)
    }
}

// ---------------------------------------------------------------------------
// MARK: - RegionDeclaration
// ---------------------------------------------------------------------------

/// A person's self-declared region memberships.
///
/// Pull-based: the person says where they are. The network does not track
/// them. This honors the Covenant's consent principle — location is always
/// voluntary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegionDeclaration {
    pub person: String,
    pub region_ids: Vec<Uuid>,
    pub declared_at: DateTime<Utc>,
}

impl RegionDeclaration {
    /// Create a new declaration for a person.
    pub fn new(person: impl Into<String>, regions: Vec<Uuid>) -> Self {
        Self {
            person: person.into(),
            region_ids: regions,
            declared_at: Utc::now(),
        }
    }

    /// Add a region to the declaration.
    pub fn add_region(&mut self, id: Uuid) {
        if !self.region_ids.contains(&id) {
            self.region_ids.push(id);
        }
    }

    /// Remove a region from the declaration.
    pub fn remove_region(&mut self, id: Uuid) {
        self.region_ids.retain(|r| *r != id);
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: Denver coordinate.
    fn denver() -> GeoCoordinate {
        GeoCoordinate::new(39.7392, -104.9903).unwrap()
    }

    /// Helper: Boulder coordinate.
    fn boulder() -> GeoCoordinate {
        GeoCoordinate::new(40.0150, -105.2705).unwrap()
    }

    /// Helper: a triangle polygon around Denver.
    fn denver_triangle() -> Vec<GeoCoordinate> {
        vec![
            GeoCoordinate::new(39.5, -105.5).unwrap(),
            GeoCoordinate::new(39.5, -104.5).unwrap(),
            GeoCoordinate::new(40.0, -105.0).unwrap(),
        ]
    }

    // -- Region construction ------------------------------------------------

    #[test]
    fn new_region_has_uuid_v4() {
        let r = Region::new(
            "Denver Metro",
            RegionType::City,
            RegionBoundary::Named,
            "cpub1creator",
        );
        assert_eq!(r.id.get_version_num(), 4);
    }

    #[test]
    fn new_region_defaults() {
        let r = Region::new(
            "Test",
            RegionType::Neighborhood,
            RegionBoundary::Named,
            "cpub1creator",
        );
        assert!(r.description.is_none());
        assert!(r.parent_id.is_none());
    }

    #[test]
    fn new_region_sets_timestamp() {
        let before = Utc::now();
        let r = Region::new(
            "Test",
            RegionType::County,
            RegionBoundary::Named,
            "cpub1creator",
        );
        let after = Utc::now();
        assert!(r.created_at >= before && r.created_at <= after);
    }

    // -- Builder methods ----------------------------------------------------

    #[test]
    fn with_parent_sets_id() {
        let parent_id = Uuid::new_v4();
        let r = Region::new(
            "Child",
            RegionType::Neighborhood,
            RegionBoundary::Named,
            "cpub1creator",
        )
        .with_parent(parent_id);
        assert_eq!(r.parent_id, Some(parent_id));
    }

    #[test]
    fn with_description_sets_text() {
        let r = Region::new(
            "Bio",
            RegionType::Bioregion,
            RegionBoundary::Named,
            "cpub1creator",
        )
        .with_description("Front Range bioregion");
        assert_eq!(r.description.as_deref(), Some("Front Range bioregion"));
    }

    // -- Circle boundary containment ----------------------------------------

    #[test]
    fn circle_contains_point_inside() {
        let r = Region::new(
            "Denver 50km",
            RegionType::City,
            RegionBoundary::Circle {
                center: denver(),
                radius_meters: 50_000.0,
            },
            "cpub1creator",
        );
        // Denver itself is inside a 50km circle centered on Denver.
        assert!(r.contains_point(&denver()));
    }

    #[test]
    fn circle_excludes_point_outside() {
        let r = Region::new(
            "Denver 1km",
            RegionType::Neighborhood,
            RegionBoundary::Circle {
                center: denver(),
                radius_meters: 1_000.0,
            },
            "cpub1creator",
        );
        // Boulder is ~40km from Denver, well outside a 1km circle.
        assert!(!r.contains_point(&boulder()));
    }

    // -- Polygon boundary containment ---------------------------------------

    #[test]
    fn polygon_contains_point_inside() {
        let r = Region::new(
            "Denver Triangle",
            RegionType::Custom("Triangle".into()),
            RegionBoundary::Polygon(denver_triangle()),
            "cpub1creator",
        );
        assert!(r.contains_point(&denver()));
    }

    #[test]
    fn polygon_excludes_point_outside() {
        let r = Region::new(
            "Denver Triangle",
            RegionType::Custom("Triangle".into()),
            RegionBoundary::Polygon(denver_triangle()),
            "cpub1creator",
        );
        // A point far north of the triangle.
        let far_north = GeoCoordinate::new(41.0, -105.0).unwrap();
        assert!(!r.contains_point(&far_north));
    }

    // -- Named boundary -----------------------------------------------------

    #[test]
    fn named_boundary_never_contains() {
        let r = Region::new(
            "Cultural Region",
            RegionType::Custom("Cultural".into()),
            RegionBoundary::Named,
            "cpub1creator",
        );
        assert!(!r.contains_point(&denver()));
    }

    // -- RegionBoundary::contains directly ----------------------------------

    #[test]
    fn boundary_circle_contains() {
        let b = RegionBoundary::Circle {
            center: denver(),
            radius_meters: 100_000.0,
        };
        assert!(b.contains(&denver()));
        // Boulder is ~40km away, within 100km.
        assert!(b.contains(&boulder()));
    }

    #[test]
    fn boundary_polygon_contains() {
        let b = RegionBoundary::Polygon(denver_triangle());
        assert!(b.contains(&denver()));
    }

    #[test]
    fn boundary_named_contains() {
        let b = RegionBoundary::Named;
        assert!(!b.contains(&denver()));
    }

    // -- RegionDeclaration --------------------------------------------------

    #[test]
    fn declaration_new() {
        let r1 = Uuid::new_v4();
        let r2 = Uuid::new_v4();
        let d = RegionDeclaration::new("cpub1alice", vec![r1, r2]);
        assert_eq!(d.person, "cpub1alice");
        assert_eq!(d.region_ids.len(), 2);
        assert!(d.region_ids.contains(&r1));
        assert!(d.region_ids.contains(&r2));
    }

    #[test]
    fn declaration_add_region() {
        let mut d = RegionDeclaration::new("cpub1bob", vec![]);
        let id = Uuid::new_v4();
        d.add_region(id);
        assert_eq!(d.region_ids, vec![id]);
    }

    #[test]
    fn declaration_add_region_is_idempotent() {
        let id = Uuid::new_v4();
        let mut d = RegionDeclaration::new("cpub1bob", vec![id]);
        d.add_region(id);
        // Should not duplicate.
        assert_eq!(d.region_ids.len(), 1);
    }

    #[test]
    fn declaration_remove_region() {
        let id = Uuid::new_v4();
        let mut d = RegionDeclaration::new("cpub1bob", vec![id]);
        d.remove_region(id);
        assert!(d.region_ids.is_empty());
    }

    #[test]
    fn declaration_remove_nonexistent_is_no_op() {
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut d = RegionDeclaration::new("cpub1bob", vec![id]);
        d.remove_region(other);
        assert_eq!(d.region_ids.len(), 1);
    }

    // -- Serde round-trip ---------------------------------------------------

    #[test]
    fn region_serde_round_trip() {
        let r = Region::new(
            "Denver",
            RegionType::City,
            RegionBoundary::Circle {
                center: denver(),
                radius_meters: 50_000.0,
            },
            "cpub1creator",
        )
        .with_description("Mile High City");

        let json = serde_json::to_string(&r).unwrap();
        let parsed: Region = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
    }

    #[test]
    fn declaration_serde_round_trip() {
        let d = RegionDeclaration::new("cpub1alice", vec![Uuid::new_v4(), Uuid::new_v4()]);
        let json = serde_json::to_string(&d).unwrap();
        let parsed: RegionDeclaration = serde_json::from_str(&json).unwrap();
        assert_eq!(d, parsed);
    }

    #[test]
    fn custom_region_type_serde() {
        let r = Region::new(
            "Watershed",
            RegionType::Custom("Watershed".into()),
            RegionBoundary::Named,
            "cpub1creator",
        );
        let json = serde_json::to_string(&r).unwrap();
        let parsed: Region = serde_json::from_str(&json).unwrap();
        assert_eq!(r.region_type, parsed.region_type);
    }

    #[test]
    fn polygon_boundary_serde_round_trip() {
        let b = RegionBoundary::Polygon(denver_triangle());
        let json = serde_json::to_string(&b).unwrap();
        let parsed: RegionBoundary = serde_json::from_str(&json).unwrap();
        assert_eq!(b, parsed);
    }
}
