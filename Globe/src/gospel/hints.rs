use crown::CrownKeypair;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// A parsed relay hint record from a hint event.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelayHintRecord {
    /// The author who published these hints.
    pub author: String,
    /// Relay URLs where this author can be found.
    pub relays: Vec<Url>,
    /// When this hint was published (unix timestamp).
    pub published_at: i64,
}

/// Builds relay hint events for the gospel system.
pub struct HintBuilder;

impl HintBuilder {
    /// Create a relay hint event (kind 7010).
    ///
    /// Advertises which relays a user can be found on. Uses d-tag
    /// "relay-hints" so it's parameterized replaceable — the latest
    /// from the same author replaces the previous.
    ///
    /// Each relay URL is also added as an "r" tag for relay-side filtering.
    pub fn relay_hints(
        relay_urls: &[Url],
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        if relay_urls.is_empty() {
            return Err(GlobeError::InvalidConfig(
                "relay hints must contain at least one URL".into(),
            ));
        }

        let content = serde_json::json!({
            "relays": relay_urls.iter().map(|u| u.as_str()).collect::<Vec<_>>()
        });
        let content_str = serde_json::to_string(&content)
            .map_err(|e| GlobeError::SigningFailed(e.to_string()))?;

        let mut unsigned = UnsignedEvent::new(kind::RELAY_HINT, content_str)
            .with_d_tag("relay-hints");

        for url in relay_urls {
            unsigned = unsigned.with_tag("r", &[url.as_str()]);
        }

        EventBuilder::sign(&unsigned, keypair)
    }
}

/// Parse a relay hint event into a `RelayHintRecord`.
pub fn parse_hint(event: &OmniEvent) -> Result<RelayHintRecord, GlobeError> {
    if event.kind != kind::RELAY_HINT {
        return Err(GlobeError::InvalidMessage(format!(
            "expected kind {}, got {}",
            kind::RELAY_HINT,
            event.kind,
        )));
    }

    let value: serde_json::Value = serde_json::from_str(&event.content)
        .map_err(|e| GlobeError::InvalidMessage(format!("hint content: {e}")))?;

    let relay_strs = value
        .get("relays")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GlobeError::InvalidMessage("hint missing relays array".into()))?;

    let mut relays = Vec::new();
    for val in relay_strs {
        let url_str = val
            .as_str()
            .ok_or_else(|| GlobeError::InvalidMessage("relay hint must be string".into()))?;
        let url = Url::parse(url_str)
            .map_err(|e| GlobeError::InvalidMessage(format!("invalid relay URL: {e}")))?;
        relays.push(url);
    }

    Ok(RelayHintRecord {
        author: event.author.clone(),
        relays,
        published_at: event.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> CrownKeypair {
        CrownKeypair::generate()
    }

    #[test]
    fn build_and_parse_round_trip() {
        let kp = test_keypair();
        let urls = vec![
            Url::parse("wss://relay1.omnidea.com").unwrap(),
            Url::parse("wss://relay2.omnidea.com").unwrap(),
        ];

        let event = HintBuilder::relay_hints(&urls, &kp).unwrap();
        assert_eq!(event.kind, kind::RELAY_HINT);
        assert_eq!(event.d_tag(), Some("relay-hints"));
        assert_eq!(event.author, kp.public_key_hex());

        let record = parse_hint(&event).unwrap();
        assert_eq!(record.author, kp.public_key_hex());
        assert_eq!(record.relays.len(), 2);
        assert_eq!(record.relays[0].as_str(), "wss://relay1.omnidea.com/");
        assert_eq!(record.relays[1].as_str(), "wss://relay2.omnidea.com/");
    }

    #[test]
    fn relay_urls_stored_as_r_tags() {
        let kp = test_keypair();
        let urls = vec![Url::parse("wss://relay.test.com").unwrap()];
        let event = HintBuilder::relay_hints(&urls, &kp).unwrap();
        let r_values = event.tag_values("r");
        assert_eq!(r_values.len(), 1);
        assert!(r_values[0].contains("relay.test.com"));
    }

    #[test]
    fn empty_urls_rejected() {
        let kp = test_keypair();
        let result = HintBuilder::relay_hints(&[], &kp);
        assert!(result.is_err());
    }

    #[test]
    fn parse_wrong_kind_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: 1, // TEXT_NOTE, not RELAY_HINT
            tags: vec![],
            content: r#"{"relays":["wss://x.com"]}"#.into(),
            sig: "c".repeat(128),
        };
        assert!(parse_hint(&event).is_err());
    }

    #[test]
    fn parse_invalid_json_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: kind::RELAY_HINT,
            tags: vec![],
            content: "not json".into(),
            sig: "c".repeat(128),
        };
        assert!(parse_hint(&event).is_err());
    }

    #[test]
    fn parse_missing_relays_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: kind::RELAY_HINT,
            tags: vec![],
            content: r#"{"other":"data"}"#.into(),
            sig: "c".repeat(128),
        };
        assert!(parse_hint(&event).is_err());
    }

    #[test]
    fn hint_record_serde_round_trip() {
        let record = RelayHintRecord {
            author: "a".repeat(64),
            relays: vec![Url::parse("wss://relay.test.com").unwrap()],
            published_at: 1000,
        };
        let json = serde_json::to_string(&record).unwrap();
        let loaded: RelayHintRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, loaded);
    }
}
