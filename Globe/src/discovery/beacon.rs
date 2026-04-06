use serde::{Deserialize, Serialize};

use crown::CrownKeypair;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// A community beacon — a self-describing, gospel-propagated entry point.
///
/// Communities publish beacons so new users can discover and browse them.
/// Beacons propagate through gospel peering, the same way names and relay
/// hints do. The app binary ships with a baked-in snapshot of beacons
/// for zero-configuration bootstrap.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BeaconRecord {
    /// Community identifier (e.g. UUID or domain name).
    pub community_id: String,
    /// Human-readable community name.
    pub name: String,
    /// Short description of what this community is about.
    pub description: String,
    /// Discoverable tags (e.g. "music", "art", "tech", "local").
    pub tags: Vec<String>,
    /// Approximate member count (refreshed periodically).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    /// Relay URLs where this community's content can be found.
    pub relay_urls: Vec<String>,
    /// Preview: recent post summaries for browsing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Vec<String>>,
    /// Community avatar/icon asset hash (from Globe asset store).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_hash: Option<String>,
}

/// Builds ORP events for community beacons.
pub struct BeaconBuilder;

impl BeaconBuilder {
    /// Build a beacon event (kind 7030).
    ///
    /// D-tag is the community_id, making it parameterized replaceable —
    /// the latest beacon from the same author for the same community wins.
    pub fn announce(
        beacon: &BeaconRecord,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let content = serde_json::to_string(beacon)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))?;

        let mut unsigned = UnsignedEvent::new(kind::BEACON, &content)
            .with_d_tag(&beacon.community_id);

        // Add each tag as a `t` tag for filtering.
        for tag in &beacon.tags {
            unsigned = unsigned.with_tag("t", &[tag]);
        }

        // Add relay URLs as `r` tags.
        for url in &beacon.relay_urls {
            unsigned = unsigned.with_tag("r", &[url]);
        }

        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a beacon update event (kind 7031).
    ///
    /// Same structure as announce but signals an update (member count,
    /// preview refresh, etc).
    pub fn update(
        beacon: &BeaconRecord,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let content = serde_json::to_string(beacon)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))?;

        let mut unsigned = UnsignedEvent::new(kind::BEACON_UPDATE, &content)
            .with_d_tag(&beacon.community_id);

        for tag in &beacon.tags {
            unsigned = unsigned.with_tag("t", &[tag]);
        }
        for url in &beacon.relay_urls {
            unsigned = unsigned.with_tag("r", &[url]);
        }

        EventBuilder::sign(&unsigned, keypair)
    }

    /// Parse a beacon record from a beacon event.
    pub fn parse(event: &OmniEvent) -> Result<BeaconRecord, GlobeError> {
        if event.kind != kind::BEACON && event.kind != kind::BEACON_UPDATE {
            return Err(GlobeError::ProtocolError(format!(
                "expected kind {} or {}, got {}",
                kind::BEACON,
                kind::BEACON_UPDATE,
                event.kind
            )));
        }
        serde_json::from_str(&event.content)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> CrownKeypair {
        CrownKeypair::generate()
    }

    fn sample_beacon() -> BeaconRecord {
        BeaconRecord {
            community_id: "music-makers-001".into(),
            name: "Music Makers".into(),
            description: "A community for musicians and music lovers".into(),
            tags: vec!["music".into(), "creative".into(), "collaboration".into()],
            member_count: Some(42),
            relay_urls: vec!["ws://music-relay.example.com:8080".into()],
            preview: Some(vec!["Check out this new track!".into()]),
            icon_hash: Some("abc123def456".into()),
        }
    }

    #[test]
    fn beacon_record_serde() {
        let beacon = sample_beacon();
        let json = serde_json::to_string(&beacon).unwrap();
        let loaded: BeaconRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.community_id, "music-makers-001");
        assert_eq!(loaded.tags.len(), 3);
        assert_eq!(loaded.member_count, Some(42));
    }

    #[test]
    fn beacon_optional_fields_skipped() {
        let beacon = BeaconRecord {
            community_id: "minimal".into(),
            name: "Minimal".into(),
            description: "A minimal beacon".into(),
            tags: vec![],
            member_count: None,
            relay_urls: vec!["ws://localhost:8080".into()],
            preview: None,
            icon_hash: None,
        };
        let json = serde_json::to_string(&beacon).unwrap();
        assert!(!json.contains("member_count"));
        assert!(!json.contains("preview"));
        assert!(!json.contains("icon_hash"));
    }

    #[test]
    fn beacon_announce_event_structure() {
        let kp = test_keypair();
        let beacon = sample_beacon();
        let event = BeaconBuilder::announce(&beacon, &kp).unwrap();

        assert_eq!(event.kind, kind::BEACON);

        // D-tag for replaceability.
        let d_tags = event.tag_values("d");
        assert_eq!(d_tags, vec!["music-makers-001"]);

        // Topic tags for filtering.
        let t_tags = event.tag_values("t");
        assert!(t_tags.contains(&"music"));
        assert!(t_tags.contains(&"creative"));

        // Relay URL tags.
        let r_tags = event.tag_values("r");
        assert_eq!(r_tags, vec!["ws://music-relay.example.com:8080"]);

        assert!(EventBuilder::verify(&event).unwrap());
    }

    #[test]
    fn beacon_update_event_structure() {
        let kp = test_keypair();
        let beacon = sample_beacon();
        let event = BeaconBuilder::update(&beacon, &kp).unwrap();
        assert_eq!(event.kind, kind::BEACON_UPDATE);
        assert!(EventBuilder::verify(&event).unwrap());
    }

    #[test]
    fn beacon_round_trip_via_event() {
        let kp = test_keypair();
        let beacon = sample_beacon();
        let event = BeaconBuilder::announce(&beacon, &kp).unwrap();
        let parsed = BeaconBuilder::parse(&event).unwrap();
        assert_eq!(parsed.community_id, beacon.community_id);
        assert_eq!(parsed.name, beacon.name);
        assert_eq!(parsed.tags, beacon.tags);
        assert_eq!(parsed.member_count, beacon.member_count);
    }

    #[test]
    fn beacon_parse_wrong_kind_fails() {
        let kp = test_keypair();
        let event = EventBuilder::sign(
            &UnsignedEvent::new(kind::TEXT_NOTE, "not a beacon"),
            &kp,
        )
        .unwrap();
        assert!(BeaconBuilder::parse(&event).is_err());
    }

    #[test]
    fn beacon_update_is_parseable() {
        let kp = test_keypair();
        let beacon = sample_beacon();
        let event = BeaconBuilder::update(&beacon, &kp).unwrap();
        let parsed = BeaconBuilder::parse(&event).unwrap();
        assert_eq!(parsed.name, "Music Makers");
    }
}
