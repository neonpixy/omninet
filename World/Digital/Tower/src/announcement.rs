use crown::CrownKeypair;
use globe::event_builder::{EventBuilder, UnsignedEvent};
use globe::event::OmniEvent;
use globe::kind;
use serde::{Deserialize, Serialize};

use crate::config::TowerMode;
use crate::error::TowerError;

/// A Tower lighthouse announcement — broadcast via gospel to let
/// Omnibus nodes and other Towers discover this node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerAnnouncement {
    /// Tower operating mode.
    pub mode: TowerMode,
    /// Public relay URL (WebSocket).
    pub relay_url: String,
    /// Human-readable name.
    pub name: String,
    /// Number of gospel records cached.
    pub gospel_count: u64,
    /// Number of stored events (Harbor mode).
    pub event_count: u64,
    /// Uptime in seconds since start.
    pub uptime_secs: u64,
    /// Tower software version.
    pub version: String,
    /// Community pubkeys served (Harbor mode only).
    pub communities: Vec<String>,
}

impl TowerAnnouncement {
    /// Build a signed lighthouse announcement event (kind 7032).
    ///
    /// Tags:
    /// - d-tag = Tower pubkey (replaceable per author)
    /// - ["mode", "pharos"|"harbor"]
    /// - ["r", relay_url]
    pub fn to_event(&self, keypair: &CrownKeypair) -> Result<OmniEvent, TowerError> {
        let content = serde_json::to_string(self)
            .map_err(|e| TowerError::AnnounceFailed(format!("serialize: {e}")))?;

        let unsigned = UnsignedEvent::new(kind::LIGHTHOUSE_ANNOUNCE, content)
            .with_d_tag(&keypair.public_key_hex())
            .with_tag("mode", &[self.mode.as_str()])
            .with_tag("r", &[&self.relay_url]);

        EventBuilder::sign(&unsigned, keypair).map_err(TowerError::Globe)
    }

    /// Parse a TowerAnnouncement from an OmniEvent's content.
    pub fn from_event(event: &OmniEvent) -> Result<Self, TowerError> {
        if event.kind != kind::LIGHTHOUSE_ANNOUNCE {
            return Err(TowerError::AnnounceFailed(format!(
                "expected kind {}, got {}",
                kind::LIGHTHOUSE_ANNOUNCE,
                event.kind
            )));
        }
        serde_json::from_str(&event.content)
            .map_err(|e| TowerError::AnnounceFailed(format!("parse: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_announcement() -> TowerAnnouncement {
        TowerAnnouncement {
            mode: TowerMode::Pharos,
            relay_url: "wss://tower.example.com".into(),
            name: "Test Tower".into(),
            gospel_count: 42,
            event_count: 0,
            uptime_secs: 3600,
            version: "0.1.0".into(),
            communities: vec![],
        }
    }

    #[test]
    fn announcement_serde_round_trip() {
        let ann = test_announcement();
        let json = serde_json::to_string(&ann).unwrap();
        let loaded: TowerAnnouncement = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.mode, TowerMode::Pharos);
        assert_eq!(loaded.relay_url, "wss://tower.example.com");
        assert_eq!(loaded.gospel_count, 42);
        assert_eq!(loaded.uptime_secs, 3600);
    }

    #[test]
    fn announcement_to_event() {
        let ann = test_announcement();
        let kp = CrownKeypair::generate();
        let event = ann.to_event(&kp).unwrap();

        assert_eq!(event.kind, kind::LIGHTHOUSE_ANNOUNCE);
        assert!(event.has_tag("mode", "pharos"));
        assert!(event.has_tag("r", "wss://tower.example.com"));
        assert!(event.has_tag("d", &kp.public_key_hex()));
    }

    #[test]
    fn announcement_from_event() {
        let ann = test_announcement();
        let kp = CrownKeypair::generate();
        let event = ann.to_event(&kp).unwrap();

        let parsed = TowerAnnouncement::from_event(&event).unwrap();
        assert_eq!(parsed.mode, TowerMode::Pharos);
        assert_eq!(parsed.relay_url, "wss://tower.example.com");
        assert_eq!(parsed.name, "Test Tower");
        assert_eq!(parsed.gospel_count, 42);
    }

    #[test]
    fn harbor_announcement() {
        let ann = TowerAnnouncement {
            mode: TowerMode::Harbor,
            relay_url: "wss://harbor.community.idea".into(),
            name: "Community Harbor".into(),
            gospel_count: 100,
            event_count: 5000,
            uptime_secs: 86400,
            version: "0.1.0".into(),
            communities: vec!["community_pubkey_1".into(), "community_pubkey_2".into()],
        };
        let kp = CrownKeypair::generate();
        let event = ann.to_event(&kp).unwrap();

        assert!(event.has_tag("mode", "harbor"));
        let parsed = TowerAnnouncement::from_event(&event).unwrap();
        assert_eq!(parsed.communities.len(), 2);
        assert_eq!(parsed.event_count, 5000);
    }

    #[test]
    fn wrong_kind_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 0,
            kind: 1, // TEXT_NOTE, not LIGHTHOUSE_ANNOUNCE
            tags: vec![],
            content: "{}".into(),
            sig: "c".repeat(128),
        };
        assert!(TowerAnnouncement::from_event(&event).is_err());
    }
}
