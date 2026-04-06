//! Globe event kind constants and tag/content builders for World/Physical.
//!
//! World/Physical doesn't depend on Globe directly — these helpers produce the
//! tag arrays and JSON content that Globe's EventBuilder and OmniFilter expect.
//! Apps use these to construct UnsignedEvent tags before signing.

use serde::Serialize;

use crate::caravan::Delivery;
use crate::handoff::Handoff;
use crate::lantern::{LanternShare, LanternSos};
use crate::omnitag::{OmniTagIdentity, TagSighting};
use crate::place::Place;
use crate::presence::PresenceSignal;
use crate::region::{Region, RegionDeclaration};
use crate::rendezvous::Rendezvous;

// ---------------------------------------------------------------------------
// MARK: - Kind Constants
// ---------------------------------------------------------------------------

/// Event kind constants for World/Physical (WORLD_RANGE 23000-24000).
pub mod kind {
    pub const PLACE: u32 = 23000;
    pub const PLACE_UPDATE: u32 = 23001;
    pub const REGION: u32 = 23002;
    pub const REGION_DECLARATION: u32 = 23003;
    pub const RENDEZVOUS: u32 = 23010;
    pub const RENDEZVOUS_RSVP: u32 = 23011;
    pub const RENDEZVOUS_UPDATE: u32 = 23012;
    pub const HANDOFF: u32 = 23020;
    pub const HANDOFF_SIGNATURE: u32 = 23021;
    pub const LANTERN_SHARE: u32 = 23030;
    pub const LANTERN_SOS: u32 = 23031;
    pub const PRESENCE_LOCAL: u32 = 23040;
    pub const PRESENCE_RELAY: u32 = 23041;
    pub const CARAVAN: u32 = 23050;
    pub const CARAVAN_UPDATE: u32 = 23051;
    pub const OMNITAG_SIGHTING: u32 = 23060;
    pub const OMNITAG_REGISTRATION: u32 = 23061;
}

// ---------------------------------------------------------------------------
// MARK: - PresenceBroadcast (relay-safe)
// ---------------------------------------------------------------------------

/// Minimal relay-safe presence broadcast.
///
/// Contains NO coordinates. Only proximity level and an optional message.
/// Coordinates never leave the local device — only this stripped-down
/// version is published to relays.
#[derive(Debug, Clone, Serialize)]
pub struct PresenceBroadcast {
    /// The person broadcasting.
    pub person: String,
    /// Status string (e.g. "available", "busy", "away").
    pub status: String,
    /// Proximity level string (e.g. "immediate", "near", "area", "city").
    pub proximity: String,
    /// Optional human-readable message.
    pub message: Option<String>,
}

impl PresenceBroadcast {
    /// Create a relay-safe broadcast from a PresenceSignal.
    ///
    /// Strips all geographic data. Only status, proximity, and message survive.
    pub fn from_signal(signal: &PresenceSignal) -> Self {
        Self {
            person: signal.person.clone(),
            status: format!("{:?}", signal.status),
            proximity: format!("{:?}", signal.proximity),
            message: signal.message.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - Place builders
// ---------------------------------------------------------------------------

/// Build tags for a PLACE event (kind 23000).
///
/// Tags: d-tag (place_id), type, region (if present).
pub fn place_tags(place: &Place) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["d".into(), place.id.to_string()],
        vec!["type".into(), format!("{:?}", place.place_type)],
    ];
    if let Some(region_id) = &place.region_id {
        tags.push(vec!["region".into(), region_id.to_string()]);
    }
    tags
}

/// Serialize a Place to JSON content.
pub fn place_content(place: &Place) -> Result<String, serde_json::Error> {
    serde_json::to_string(place)
}

// ---------------------------------------------------------------------------
// MARK: - Region builders
// ---------------------------------------------------------------------------

/// Build tags for a REGION event (kind 23002).
///
/// Tags: d-tag (region_id), type, parent (if present).
pub fn region_tags(region: &Region) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["d".into(), region.id.to_string()],
        vec!["type".into(), format!("{:?}", region.region_type)],
    ];
    if let Some(parent) = &region.parent_id {
        tags.push(vec!["parent".into(), parent.to_string()]);
    }
    tags
}

/// Serialize a Region to JSON content.
pub fn region_content(region: &Region) -> Result<String, serde_json::Error> {
    serde_json::to_string(region)
}

/// Build tags for a REGION_DECLARATION event (kind 23003).
pub fn region_declaration_tags(decl: &RegionDeclaration) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["p".into(), decl.person.clone()],
    ];
    for region_id in &decl.region_ids {
        tags.push(vec!["region".into(), region_id.to_string()]);
    }
    tags
}

/// Serialize a RegionDeclaration to JSON content.
pub fn region_declaration_content(
    decl: &RegionDeclaration,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(decl)
}

// ---------------------------------------------------------------------------
// MARK: - Rendezvous builders
// ---------------------------------------------------------------------------

/// Build tags for a RENDEZVOUS event (kind 23010).
///
/// Tags: d-tag (rv_id), p-tags for each invitee, purpose.
pub fn rendezvous_tags(rv: &Rendezvous) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["d".into(), rv.id.to_string()],
        vec!["purpose".into(), format!("{:?}", rv.purpose)],
    ];
    for invitee in &rv.invitees {
        tags.push(vec!["p".into(), invitee.clone()]);
    }
    tags
}

/// Serialize a Rendezvous to JSON content.
pub fn rendezvous_content(rv: &Rendezvous) -> Result<String, serde_json::Error> {
    serde_json::to_string(rv)
}

// ---------------------------------------------------------------------------
// MARK: - Handoff builders
// ---------------------------------------------------------------------------

/// Build tags for a HANDOFF event (kind 23020).
///
/// Tags: d-tag (handoff_id), p-tags for both parties, purpose.
pub fn handoff_tags(handoff: &Handoff) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["d".into(), handoff.id.to_string()],
        vec!["p".into(), handoff.initiator.clone()],
        vec!["p".into(), handoff.counterparty.clone()],
        vec!["purpose".into(), format!("{:?}", handoff.purpose)],
    ];
    if let Some(rv_id) = &handoff.rendezvous_id {
        tags.push(vec!["rendezvous".into(), rv_id.to_string()]);
    }
    tags
}

/// Serialize a Handoff to JSON content.
pub fn handoff_content(handoff: &Handoff) -> Result<String, serde_json::Error> {
    serde_json::to_string(handoff)
}

// ---------------------------------------------------------------------------
// MARK: - Lantern builders
// ---------------------------------------------------------------------------

/// Build tags for a LANTERN_SHARE event (kind 23030).
///
/// Tags: d-tag (share_id), p-tags for audience.
pub fn lantern_share_tags(share: &LanternShare) -> Vec<Vec<String>> {
    let mut tags = vec![vec!["d".into(), share.id.to_string()]];
    match &share.audience {
        crate::lantern::LanternAudience::Selected(crown_ids) => {
            for id in crown_ids {
                tags.push(vec!["p".into(), id.clone()]);
            }
        }
        crate::lantern::LanternAudience::Community(id) => {
            tags.push(vec!["community".into(), id.clone()]);
        }
        crate::lantern::LanternAudience::AllTrusted => {
            tags.push(vec!["audience".into(), "trusted".into()]);
        }
    }
    tags
}

/// Serialize a LanternShare to JSON content.
pub fn lantern_share_content(share: &LanternShare) -> Result<String, serde_json::Error> {
    serde_json::to_string(share)
}

/// Build tags for a LANTERN_SOS event (kind 23031).
///
/// Tags: d-tag (sos_id), p-tags for audience.
pub fn lantern_sos_tags(sos: &LanternSos) -> Vec<Vec<String>> {
    let mut tags = vec![vec!["d".into(), sos.id.to_string()]];
    for contact in &sos.emergency_contacts {
        tags.push(vec!["p".into(), contact.clone()]);
    }
    tags
}

/// Serialize a LanternSos to JSON content.
pub fn lantern_sos_content(sos: &LanternSos) -> Result<String, serde_json::Error> {
    serde_json::to_string(sos)
}

// ---------------------------------------------------------------------------
// MARK: - Caravan builders
// ---------------------------------------------------------------------------

/// Build tags for a CARAVAN event (kind 23050).
///
/// Tags: d-tag (delivery_id), p-tags for sender/courier/recipient, status.
pub fn caravan_tags(delivery: &Delivery) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["d".into(), delivery.id.to_string()],
        vec!["p".into(), delivery.sender.clone()],
    ];
    if let Some(ref courier) = delivery.courier {
        tags.push(vec!["p".into(), courier.clone()]);
    }
    tags.push(vec!["p".into(), delivery.recipient.clone()]);
    tags.push(vec!["status".into(), delivery.status.to_string()]);
    tags
}

/// Serialize a Delivery to JSON content.
pub fn caravan_content(delivery: &Delivery) -> Result<String, serde_json::Error> {
    serde_json::to_string(delivery)
}

// ---------------------------------------------------------------------------
// MARK: - OmniTag builders
// ---------------------------------------------------------------------------

/// Build tags for an OMNITAG_SIGHTING event (kind 23060).
///
/// Tags: p-tag for the tag's pubkey. NO location information in tags —
/// location is encrypted in the content.
pub fn omnitag_sighting_tags(sighting: &TagSighting) -> Vec<Vec<String>> {
    vec![vec!["p".into(), sighting.tag_pubkey.clone()]]
}

/// Serialize a TagSighting to JSON content.
pub fn omnitag_sighting_content(
    sighting: &TagSighting,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(sighting)
}

/// Build tags for an OMNITAG_REGISTRATION event (kind 23061).
///
/// Tags: d-tag (tag_id), p-tag for owner.
pub fn omnitag_registration_tags(tag: &OmniTagIdentity) -> Vec<Vec<String>> {
    vec![
        vec!["d".into(), tag.id.to_string()],
        vec!["p".into(), tag.owner.clone()],
    ]
}

/// Serialize an OmniTagIdentity to JSON content.
pub fn omnitag_registration_content(
    tag: &OmniTagIdentity,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(tag)
}

// ---------------------------------------------------------------------------
// MARK: - Presence builders
// ---------------------------------------------------------------------------

/// Build tags for a PRESENCE_RELAY event (kind 23041).
///
/// Minimal tags. NO coordinate information whatsoever — only the person.
pub fn presence_tags(signal: &PresenceSignal) -> Vec<Vec<String>> {
    vec![vec!["p".into(), signal.person.clone()]]
}

/// Serialize a PresenceSignal as a relay-safe PresenceBroadcast (no coordinates).
pub fn presence_content(signal: &PresenceSignal) -> Result<String, serde_json::Error> {
    let broadcast = PresenceBroadcast::from_signal(signal);
    serde_json::to_string(&broadcast)
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::PlaceType;
    use crate::rendezvous::{RendezvousPurpose, Rendezvous};
    use chrono::{Duration, Utc};
    use uuid::Uuid;
    use x::GeoCoordinate;

    fn denver() -> GeoCoordinate {
        GeoCoordinate::new(39.7392, -104.9903).unwrap()
    }

    // -- Kind constants -----------------------------------------------------

    #[test]
    fn kind_constants_in_world_range() {
        let kinds = [
            kind::PLACE,
            kind::PLACE_UPDATE,
            kind::REGION,
            kind::REGION_DECLARATION,
            kind::RENDEZVOUS,
            kind::RENDEZVOUS_RSVP,
            kind::RENDEZVOUS_UPDATE,
            kind::HANDOFF,
            kind::HANDOFF_SIGNATURE,
            kind::LANTERN_SHARE,
            kind::LANTERN_SOS,
            kind::PRESENCE_LOCAL,
            kind::PRESENCE_RELAY,
            kind::CARAVAN,
            kind::CARAVAN_UPDATE,
            kind::OMNITAG_SIGHTING,
            kind::OMNITAG_REGISTRATION,
        ];
        for k in kinds {
            assert!(
                (23000..24000).contains(&k),
                "kind {} is outside WORLD_RANGE",
                k,
            );
        }
    }

    #[test]
    fn kind_constants_are_unique() {
        let kinds = vec![
            kind::PLACE,
            kind::PLACE_UPDATE,
            kind::REGION,
            kind::REGION_DECLARATION,
            kind::RENDEZVOUS,
            kind::RENDEZVOUS_RSVP,
            kind::RENDEZVOUS_UPDATE,
            kind::HANDOFF,
            kind::HANDOFF_SIGNATURE,
            kind::LANTERN_SHARE,
            kind::LANTERN_SOS,
            kind::PRESENCE_LOCAL,
            kind::PRESENCE_RELAY,
            kind::CARAVAN,
            kind::CARAVAN_UPDATE,
            kind::OMNITAG_SIGHTING,
            kind::OMNITAG_REGISTRATION,
        ];
        let mut deduped = kinds.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(kinds.len(), deduped.len(), "duplicate kind constants");
    }

    // -- Place tags ---------------------------------------------------------

    #[test]
    fn place_tags_structure() {
        let place = Place::new("Test Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        let tags = place_tags(&place);

        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], place.id.to_string());
        assert_eq!(tags[1][0], "type");
        assert_eq!(tags[1][1], "Cafe");
        // No region tag
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn place_tags_with_region() {
        let region_id = Uuid::new_v4();
        let place = Place::new("Hub", PlaceType::CommunityHub, denver(), "cpub1owner")
            .with_region(region_id);
        let tags = place_tags(&place);

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[2][0], "region");
        assert_eq!(tags[2][1], region_id.to_string());
    }

    #[test]
    fn place_content_round_trip() {
        let place = Place::new("Serde Cafe", PlaceType::Cafe, denver(), "cpub1owner");
        let json = place_content(&place).unwrap();
        let parsed: Place = serde_json::from_str(&json).unwrap();
        assert_eq!(place, parsed);
    }

    // -- Rendezvous tags ----------------------------------------------------

    #[test]
    fn rendezvous_tags_structure() {
        let rv = Rendezvous::new(
            "Coffee meetup",
            "cpub1organizer",
            Utc::now() + Duration::hours(24),
            RendezvousPurpose::Social,
        )
        .with_invitees(vec!["cpub1alice".into(), "cpub1bob".into()]);

        let tags = rendezvous_tags(&rv);
        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], rv.id.to_string());
        assert_eq!(tags[1][0], "purpose");
        assert_eq!(tags[1][1], "Social");
        // p-tags for invitees
        assert_eq!(tags[2], vec!["p", "cpub1alice"]);
        assert_eq!(tags[3], vec!["p", "cpub1bob"]);
    }

    #[test]
    fn rendezvous_content_round_trip() {
        let rv = Rendezvous::new(
            "Meetup",
            "cpub1org",
            Utc::now() + Duration::hours(1),
            RendezvousPurpose::CashExchange,
        );
        let json = rendezvous_content(&rv).unwrap();
        let parsed: Rendezvous = serde_json::from_str(&json).unwrap();
        assert_eq!(rv, parsed);
    }

    // -- OmniTag tags -------------------------------------------------------

    #[test]
    fn omnitag_sighting_tags_no_location() {
        let sighting = TagSighting::new("cpub1tag", vec![0xDE, 0xAD], "cpub1node");
        let tags = omnitag_sighting_tags(&sighting);

        // Only p-tag for the tag pubkey — no location leaked
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["p", "cpub1tag"]);
    }

    #[test]
    fn omnitag_registration_tags_structure() {
        let tag = OmniTagIdentity::new("cpub1owner", "cpub1tag").with_name("Backpack");
        let tags = omnitag_registration_tags(&tag);

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], tag.id.to_string());
        assert_eq!(tags[1], vec!["p", "cpub1owner"]);
    }

    // -- Caravan tags -------------------------------------------------------

    #[test]
    fn caravan_tags_structure() {
        let mut d = Delivery::new("cpub1sender", "cpub1recipient", "Books");
        d.assign_courier("cpub1courier").unwrap();

        let tags = caravan_tags(&d);

        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], d.id.to_string());
        assert_eq!(tags[1], vec!["p", "cpub1sender"]);
        assert_eq!(tags[2], vec!["p", "cpub1courier"]);
        assert_eq!(tags[3], vec!["p", "cpub1recipient"]);
        assert_eq!(tags[4], vec!["status", "CourierAssigned"]);
    }

    #[test]
    fn caravan_tags_no_courier() {
        let d = Delivery::new("cpub1sender", "cpub1recipient", "Books");
        let tags = caravan_tags(&d);

        // d-tag, sender p-tag, recipient p-tag, status — no courier
        assert_eq!(tags.len(), 4);
        assert_eq!(tags[1], vec!["p", "cpub1sender"]);
        assert_eq!(tags[2], vec!["p", "cpub1recipient"]);
        assert_eq!(tags[3], vec!["status", "Created"]);
    }

    #[test]
    fn caravan_content_round_trip() {
        let d = Delivery::new("cpub1sender", "cpub1recipient", "Package");
        let json = caravan_content(&d).unwrap();
        let parsed: Delivery = serde_json::from_str(&json).unwrap();
        assert_eq!(d, parsed);
    }
}
