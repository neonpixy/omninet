use crown::CrownKeypair;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// A parsed asset announcement record.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetRecord {
    /// SHA-256 hex hash of the asset.
    pub hash: String,
    /// MIME type (e.g., "image/png", "audio/opus").
    pub mime: String,
    /// Size in bytes.
    pub size: u64,
    /// Relay URLs where this asset is available.
    pub relay_urls: Vec<Url>,
    /// When this announcement was published (unix timestamp).
    pub announced_at: i64,
}

/// Builds asset announcement events for the gospel system.
pub struct AssetBuilder;

impl AssetBuilder {
    /// Create an asset announcement event (kind 7020).
    ///
    /// Advertises that a relay stores a binary asset identified by its
    /// SHA-256 hash. Uses d-tag = hash so it's parameterized replaceable —
    /// the latest announcement for the same asset from the same author
    /// replaces the previous.
    pub fn announce(
        hash: &str,
        mime: &str,
        size: u64,
        relay_url: &Url,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        if hash.is_empty() {
            return Err(GlobeError::InvalidConfig(
                "asset hash must not be empty".into(),
            ));
        }
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(GlobeError::InvalidConfig(
                "asset hash must be 64 hex characters".into(),
            ));
        }
        if mime.is_empty() {
            return Err(GlobeError::InvalidConfig(
                "asset MIME type must not be empty".into(),
            ));
        }

        let unsigned = UnsignedEvent::new(kind::ASSET_ANNOUNCE, "")
            .with_d_tag(hash)
            .with_tag("asset", &[hash, mime, &size.to_string()])
            .with_tag("r", &[relay_url.as_str()]);

        EventBuilder::sign(&unsigned, keypair)
    }
}

/// Parse an asset announcement event into an `AssetRecord`.
pub fn parse_announcement(event: &OmniEvent) -> Result<AssetRecord, GlobeError> {
    if event.kind != kind::ASSET_ANNOUNCE {
        return Err(GlobeError::InvalidMessage(format!(
            "expected kind {}, got {}",
            kind::ASSET_ANNOUNCE,
            event.kind,
        )));
    }

    // Parse the "asset" tag: ["asset", hash, mime, size]
    let asset_tag = event
        .tags
        .iter()
        .find(|t| t.first().is_some_and(|n| n == "asset"))
        .ok_or_else(|| GlobeError::InvalidMessage("missing asset tag".into()))?;

    let hash = asset_tag
        .get(1)
        .ok_or_else(|| GlobeError::InvalidMessage("asset tag missing hash".into()))?
        .clone();

    let mime = asset_tag
        .get(2)
        .ok_or_else(|| GlobeError::InvalidMessage("asset tag missing mime".into()))?
        .clone();

    let size: u64 = asset_tag
        .get(3)
        .ok_or_else(|| GlobeError::InvalidMessage("asset tag missing size".into()))?
        .parse()
        .map_err(|e| GlobeError::InvalidMessage(format!("invalid asset size: {e}")))?;

    // Collect relay URLs from "r" tags.
    let relay_urls: Vec<Url> = event
        .tags
        .iter()
        .filter(|t| t.first().is_some_and(|n| n == "r"))
        .filter_map(|t| t.get(1))
        .filter_map(|s| Url::parse(s).ok())
        .collect();

    Ok(AssetRecord {
        hash,
        mime,
        size,
        relay_urls,
        announced_at: event.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> CrownKeypair {
        CrownKeypair::generate()
    }

    fn test_hash() -> String {
        "a1b2c3d4e5f6".to_string() + &"0".repeat(52)
    }

    #[test]
    fn build_and_parse_round_trip() {
        let kp = test_keypair();
        let hash = test_hash();
        let url = Url::parse("wss://relay.omnidea.com").unwrap();

        let event = AssetBuilder::announce(&hash, "image/png", 2_400_000, &url, &kp).unwrap();
        assert_eq!(event.kind, kind::ASSET_ANNOUNCE);
        assert_eq!(event.d_tag(), Some(hash.as_str()));
        assert_eq!(event.author, kp.public_key_hex());

        let record = parse_announcement(&event).unwrap();
        assert_eq!(record.hash, hash);
        assert_eq!(record.mime, "image/png");
        assert_eq!(record.size, 2_400_000);
        assert_eq!(record.relay_urls.len(), 1);
        assert!(record.relay_urls[0].as_str().contains("relay.omnidea.com"));
    }

    #[test]
    fn asset_tag_present() {
        let kp = test_keypair();
        let hash = test_hash();
        let url = Url::parse("wss://relay.test.com").unwrap();

        let event = AssetBuilder::announce(&hash, "audio/opus", 64_000, &url, &kp).unwrap();
        let asset_values = event.tag_values("asset");
        assert!(asset_values.contains(&hash.as_str()));
        assert!(asset_values.contains(&"audio/opus"));
        assert!(asset_values.contains(&"64000"));
    }

    #[test]
    fn relay_url_in_r_tag() {
        let kp = test_keypair();
        let hash = test_hash();
        let url = Url::parse("wss://relay.test.com").unwrap();

        let event = AssetBuilder::announce(&hash, "image/jpeg", 1000, &url, &kp).unwrap();
        let r_values = event.tag_values("r");
        assert_eq!(r_values.len(), 1);
        assert!(r_values[0].contains("relay.test.com"));
    }

    #[test]
    fn empty_hash_rejected() {
        let kp = test_keypair();
        let url = Url::parse("wss://relay.test.com").unwrap();
        assert!(AssetBuilder::announce("", "image/png", 1000, &url, &kp).is_err());
    }

    #[test]
    fn invalid_hash_rejected() {
        let kp = test_keypair();
        let url = Url::parse("wss://relay.test.com").unwrap();
        assert!(AssetBuilder::announce("tooshort", "image/png", 1000, &url, &kp).is_err());
    }

    #[test]
    fn empty_mime_rejected() {
        let kp = test_keypair();
        let hash = test_hash();
        let url = Url::parse("wss://relay.test.com").unwrap();
        assert!(AssetBuilder::announce(&hash, "", 1000, &url, &kp).is_err());
    }

    #[test]
    fn parse_wrong_kind_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![vec![
                "asset".into(),
                "a".repeat(64),
                "image/png".into(),
                "1000".into(),
            ]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(parse_announcement(&event).is_err());
    }

    #[test]
    fn parse_missing_asset_tag_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: kind::ASSET_ANNOUNCE,
            tags: vec![],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(parse_announcement(&event).is_err());
    }

    #[test]
    fn parse_invalid_size_rejected() {
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: kind::ASSET_ANNOUNCE,
            tags: vec![vec![
                "asset".into(),
                "a".repeat(64),
                "image/png".into(),
                "not_a_number".into(),
            ]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(parse_announcement(&event).is_err());
    }

    #[test]
    fn is_gospel_registry() {
        assert!(kind::is_gospel_registry(kind::ASSET_ANNOUNCE));
    }

    #[test]
    fn asset_record_serde_round_trip() {
        let record = AssetRecord {
            hash: test_hash(),
            mime: "image/png".into(),
            size: 2_400_000,
            relay_urls: vec![Url::parse("wss://relay.test.com").unwrap()],
            announced_at: 1000,
        };
        let json = serde_json::to_string(&record).unwrap();
        let loaded: AssetRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, loaded);
    }
}
