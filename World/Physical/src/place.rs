//! Physical locations in the world.
//!
//! A Place is a named geographic point — a cafe, park, community hub, or
//! any spot that matters to the Omninet. Places are always owned by a
//! person (crown_id) and carry granular visibility controls that honor the
//! Covenant's consent principle: nothing is shared without explicit
//! permission.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::GeoCoordinate;

use crate::error::PhysicalError;

// ---------------------------------------------------------------------------
// MARK: - PlaceType
// ---------------------------------------------------------------------------

/// The kind of physical location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaceType {
    Cafe,
    Park,
    CoOp,
    CommunityHub,
    Library,
    Market,
    Residence,
    Workshop,
    Garden,
    Custom(String),
}

// ---------------------------------------------------------------------------
// MARK: - PlaceVisibility
// ---------------------------------------------------------------------------

/// Who can see this place.
///
/// Defaults to Private (only the owner). Shared exposes to a specific set
/// of crown IDs. Community exposes to everyone in a Kingdom community. Public
/// is visible to all.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaceVisibility {
    /// Only the owner.
    Private,
    /// Visible to specific crown IDs.
    Shared(Vec<String>),
    /// Visible to members of a community (community_id).
    Community(String),
    /// Visible to everyone.
    Public,
}

// ---------------------------------------------------------------------------
// MARK: - Place
// ---------------------------------------------------------------------------

/// A physical location in the world.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Place {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub place_type: PlaceType,
    pub location: GeoCoordinate,
    pub address: Option<String>,
    pub owner: String,
    pub visibility: PlaceVisibility,
    pub region_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Place {
    /// Create a new place with Private visibility.
    ///
    /// Generates a random UUID and sets both timestamps to now.
    pub fn new(
        name: impl Into<String>,
        place_type: PlaceType,
        location: GeoCoordinate,
        owner: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            place_type,
            location,
            address: None,
            owner: owner.into(),
            visibility: PlaceVisibility::Private,
            region_id: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    // -- Builder methods ----------------------------------------------------

    /// Set the visibility level.
    pub fn with_visibility(mut self, visibility: PlaceVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Set an optional description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Associate the place with a region.
    pub fn with_region(mut self, region_id: Uuid) -> Self {
        self.region_id = Some(region_id);
        self
    }

    /// Set tags for the place.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set an optional street address.
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    // -- Visibility ---------------------------------------------------------

    /// Check whether `viewer_crown_id` is allowed to see this place.
    ///
    /// Rules:
    /// - The owner can always see their own place.
    /// - Public places are visible to everyone.
    /// - Shared places are visible to the listed crown IDs.
    /// - Community places are visible to anyone whose `community_memberships`
    ///   includes the community id.
    /// - Private places are visible only to the owner.
    pub fn is_visible_to(
        &self,
        viewer_crown_id: &str,
        community_memberships: &[String],
    ) -> bool {
        // Owner always sees their own place.
        if self.owner == viewer_crown_id {
            return true;
        }

        match &self.visibility {
            PlaceVisibility::Public => true,
            PlaceVisibility::Shared(crown_ids) => crown_ids.iter().any(|n| n == viewer_crown_id),
            PlaceVisibility::Community(community_id) => {
                community_memberships.iter().any(|m| m == community_id)
            }
            PlaceVisibility::Private => false,
        }
    }

    // -- Mutation (owner-only) ----------------------------------------------

    /// Update the place's location. Only the owner may do this.
    pub fn update_location(
        &mut self,
        new_coords: GeoCoordinate,
        updater_crown_id: &str,
    ) -> Result<(), PhysicalError> {
        if self.owner != updater_crown_id {
            return Err(PhysicalError::NotPlaceOwner(updater_crown_id.to_string()));
        }
        self.location = new_coords;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Update the place's name. Only the owner may do this.
    pub fn update_name(
        &mut self,
        name: &str,
        updater: &str,
    ) -> Result<(), PhysicalError> {
        if self.owner != updater {
            return Err(PhysicalError::NotPlaceOwner(updater.to_string()));
        }
        if name.is_empty() {
            return Err(PhysicalError::PlaceNameRequired);
        }
        self.name = name.to_string();
        self.updated_at = Utc::now();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: a coordinate in Denver.
    fn denver() -> GeoCoordinate {
        GeoCoordinate::new(39.7392, -104.9903).unwrap()
    }

    /// Helper: a coordinate in Boulder.
    fn boulder() -> GeoCoordinate {
        GeoCoordinate::new(40.0150, -105.2705).unwrap()
    }

    // -- Construction -------------------------------------------------------

    #[test]
    fn new_place_has_private_visibility() {
        let p = Place::new("Test Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        assert_eq!(p.visibility, PlaceVisibility::Private);
    }

    #[test]
    fn new_place_has_generated_uuid() {
        let p = Place::new("Park", PlaceType::Park, denver(), "cpub1owner");
        // UUID v4 has version nibble == 4
        assert_eq!(p.id.get_version_num(), 4);
    }

    #[test]
    fn new_place_sets_timestamps() {
        let before = Utc::now();
        let p = Place::new("Hub", PlaceType::CommunityHub, denver(), "cpub1owner");
        let after = Utc::now();
        assert!(p.created_at >= before && p.created_at <= after);
        assert_eq!(p.created_at, p.updated_at);
    }

    #[test]
    fn new_place_defaults_are_none_and_empty() {
        let p = Place::new("Lib", PlaceType::Library, denver(), "cpub1owner");
        assert!(p.description.is_none());
        assert!(p.address.is_none());
        assert!(p.region_id.is_none());
        assert!(p.tags.is_empty());
    }

    // -- Builder methods ----------------------------------------------------

    #[test]
    fn with_visibility_sets_public() {
        let p = Place::new("Market", PlaceType::Market, denver(), "cpub1owner")
            .with_visibility(PlaceVisibility::Public);
        assert_eq!(p.visibility, PlaceVisibility::Public);
    }

    #[test]
    fn with_description_sets_text() {
        let p = Place::new("Garden", PlaceType::Garden, denver(), "cpub1owner")
            .with_description("A community garden on Colfax");
        assert_eq!(p.description.as_deref(), Some("A community garden on Colfax"));
    }

    #[test]
    fn with_region_sets_id() {
        let region_id = Uuid::new_v4();
        let p = Place::new("Shop", PlaceType::Workshop, denver(), "cpub1owner")
            .with_region(region_id);
        assert_eq!(p.region_id, Some(region_id));
    }

    #[test]
    fn with_tags_sets_vec() {
        let tags = vec!["coffee".into(), "wifi".into()];
        let p = Place::new("Cafe", PlaceType::Cafe, denver(), "cpub1owner")
            .with_tags(tags.clone());
        assert_eq!(p.tags, tags);
    }

    #[test]
    fn with_address_sets_string() {
        let p = Place::new("CoOp", PlaceType::CoOp, denver(), "cpub1owner")
            .with_address("1600 Pennsylvania Ave");
        assert_eq!(p.address.as_deref(), Some("1600 Pennsylvania Ave"));
    }

    // -- Visibility checks --------------------------------------------------

    #[test]
    fn owner_always_sees_private_place() {
        let p = Place::new("Home", PlaceType::Residence, denver(), "cpub1owner");
        assert!(p.is_visible_to("cpub1owner", &[]));
    }

    #[test]
    fn stranger_cannot_see_private_place() {
        let p = Place::new("Home", PlaceType::Residence, denver(), "cpub1owner");
        assert!(!p.is_visible_to("cpub1stranger", &[]));
    }

    #[test]
    fn public_place_visible_to_all() {
        let p = Place::new("Park", PlaceType::Park, denver(), "cpub1owner")
            .with_visibility(PlaceVisibility::Public);
        assert!(p.is_visible_to("cpub1anyone", &[]));
    }

    #[test]
    fn shared_place_visible_to_listed_crown_ids() {
        let p = Place::new("Studio", PlaceType::Workshop, denver(), "cpub1owner")
            .with_visibility(PlaceVisibility::Shared(vec![
                "cpub1alice".into(),
                "cpub1bob".into(),
            ]));
        assert!(p.is_visible_to("cpub1alice", &[]));
        assert!(p.is_visible_to("cpub1bob", &[]));
        assert!(!p.is_visible_to("cpub1eve", &[]));
    }

    #[test]
    fn community_place_visible_to_members() {
        let community_id = "community-denver-123";
        let p = Place::new("Hub", PlaceType::CommunityHub, denver(), "cpub1owner")
            .with_visibility(PlaceVisibility::Community(community_id.into()));

        let memberships = vec![community_id.to_string()];
        assert!(p.is_visible_to("cpub1member", &memberships));
        assert!(!p.is_visible_to("cpub1outsider", &[]));
    }

    #[test]
    fn owner_sees_community_place_without_membership() {
        let p = Place::new("Hub", PlaceType::CommunityHub, denver(), "cpub1owner")
            .with_visibility(PlaceVisibility::Community("community-x".into()));
        // Owner sees it even without community membership in the list.
        assert!(p.is_visible_to("cpub1owner", &[]));
    }

    // -- Mutation ------------------------------------------------------------

    #[test]
    fn owner_can_update_location() {
        let mut p = Place::new("Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        let old_updated = p.updated_at;
        let result = p.update_location(boulder(), "cpub1owner");
        assert!(result.is_ok());
        assert_eq!(p.location, boulder());
        assert!(p.updated_at >= old_updated);
    }

    #[test]
    fn non_owner_cannot_update_location() {
        let mut p = Place::new("Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        let result = p.update_location(boulder(), "cpub1intruder");
        assert_eq!(
            result,
            Err(PhysicalError::NotPlaceOwner("cpub1intruder".into()))
        );
        // Location unchanged.
        assert_eq!(p.location, denver());
    }

    #[test]
    fn owner_can_update_name() {
        let mut p = Place::new("Old Name", PlaceType::Cafe, denver(), "cpub1owner");
        let result = p.update_name("New Name", "cpub1owner");
        assert!(result.is_ok());
        assert_eq!(p.name, "New Name");
    }

    #[test]
    fn non_owner_cannot_update_name() {
        let mut p = Place::new("Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        let result = p.update_name("Hijacked", "cpub1intruder");
        assert_eq!(
            result,
            Err(PhysicalError::NotPlaceOwner("cpub1intruder".into()))
        );
        assert_eq!(p.name, "Cafe");
    }

    #[test]
    fn empty_name_rejected() {
        let mut p = Place::new("Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        let result = p.update_name("", "cpub1owner");
        assert_eq!(result, Err(PhysicalError::PlaceNameRequired));
    }

    // -- Serde round-trip ---------------------------------------------------

    #[test]
    fn serde_round_trip() {
        let p = Place::new("Serde Cafe", PlaceType::Cafe, denver(), "cpub1owner")
            .with_visibility(PlaceVisibility::Public)
            .with_description("A cozy spot")
            .with_tags(vec!["serde".into(), "test".into()])
            .with_address("123 Main St");

        let json = serde_json::to_string(&p).unwrap();
        let parsed: Place = serde_json::from_str(&json).unwrap();
        assert_eq!(p, parsed);
    }

    #[test]
    fn custom_place_type_serde() {
        let p = Place::new(
            "Treehouse",
            PlaceType::Custom("Treehouse".into()),
            denver(),
            "cpub1owner",
        );
        let json = serde_json::to_string(&p).unwrap();
        let parsed: Place = serde_json::from_str(&json).unwrap();
        assert_eq!(p.place_type, parsed.place_type);
    }
}
