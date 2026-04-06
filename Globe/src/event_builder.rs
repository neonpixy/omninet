use chrono::Utc;
use crown::{CrownKeypair, Signature};

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_id;
use crate::kind;

/// An unsigned event template, ready to be signed.
///
/// Use the builder methods to construct, then pass to
/// [`EventBuilder::sign`] to produce a signed [`OmniEvent`].
#[derive(Clone, Debug)]
pub struct UnsignedEvent {
    /// Event type (determines which ABC subsystem handles it).
    pub kind: u32,
    /// Extensible metadata tags. Each tag is a string array: `["name", "value", ...]`.
    pub tags: Vec<Vec<String>>,
    /// The event payload (text, JSON, or empty string).
    pub content: String,
}

impl UnsignedEvent {
    /// Create a new unsigned event.
    pub fn new(kind: u32, content: impl Into<String>) -> Self {
        Self {
            kind,
            tags: Vec::new(),
            content: content.into(),
        }
    }

    /// Add a tag.
    pub fn with_tag(mut self, name: &str, values: &[&str]) -> Self {
        let mut tag = vec![name.to_string()];
        tag.extend(values.iter().map(|v| v.to_string()));
        self.tags.push(tag);
        self
    }

    /// Add a `d` tag (for parameterized replaceable events).
    pub fn with_d_tag(self, value: &str) -> Self {
        self.with_tag("d", &[value])
    }

    /// Add an `application` tag.
    pub fn with_application_tag(self, app: &str) -> Self {
        self.with_tag("application", &[app])
    }

    /// Add a pubkey reference (`p` tag).
    pub fn with_pubkey_ref(self, pubkey: &str) -> Self {
        self.with_tag("p", &[pubkey])
    }

    /// Add an event reference (`e` tag).
    pub fn with_event_ref(self, event_id: &str) -> Self {
        self.with_tag("e", &[event_id])
    }
}

/// Builds and verifies signed OmniEvents.
pub struct EventBuilder;

impl EventBuilder {
    /// Sign an unsigned event, producing a complete OmniEvent.
    ///
    /// 1. Gets the author's public key hex from the keypair
    /// 2. Computes the deterministic event ID (SHA-256)
    /// 3. Signs the canonical serialization via Crown's Schnorr signatures
    /// 4. Returns the complete signed event
    pub fn sign(unsigned: &UnsignedEvent, keypair: &CrownKeypair) -> Result<OmniEvent, GlobeError> {
        let author = keypair.public_key_hex();
        let created_at = Utc::now().timestamp();

        // Compute the content-addressed ID.
        let id = event_id::compute_id(&author, created_at, unsigned.kind, &unsigned.tags, &unsigned.content);

        // Get canonical bytes and sign them.
        // Crown's Signature::sign() SHA-256 hashes internally, producing the
        // Schnorr signature of the hash — which equals the event ID hash.
        let canonical = event_id::canonical_serialize(
            &author,
            created_at,
            unsigned.kind,
            &unsigned.tags,
            &unsigned.content,
        );
        let signature = Signature::sign(&canonical, keypair)
            .map_err(|e| GlobeError::SigningFailed(e.to_string()))?;

        Ok(OmniEvent {
            id,
            author,
            created_at,
            kind: unsigned.kind,
            tags: unsigned.tags.clone(),
            content: unsigned.content.clone(),
            sig: signature.hex(),
        })
    }

    /// Verify an event's ID and signature.
    ///
    /// 1. Recomputes the expected ID from the event's fields
    /// 2. Compares with the claimed ID
    /// 3. Verifies the Schnorr signature against the author's public key
    pub fn verify(event: &OmniEvent) -> Result<bool, GlobeError> {
        // Recompute expected ID.
        let expected_id = event_id::compute_id(
            &event.author,
            event.created_at,
            event.kind,
            &event.tags,
            &event.content,
        );
        if expected_id != event.id {
            return Ok(false);
        }

        // Decode author pubkey and signature from hex.
        let pubkey_bytes: [u8; 32] = hex::decode(&event.author)
            .map_err(|_| GlobeError::VerificationFailed)?
            .try_into()
            .map_err(|_| GlobeError::VerificationFailed)?;

        let sig_bytes: [u8; 64] = hex::decode(&event.sig)
            .map_err(|_| GlobeError::VerificationFailed)?
            .try_into()
            .map_err(|_| GlobeError::VerificationFailed)?;

        // Reconstruct signature and verify against canonical bytes.
        let canonical = event_id::canonical_serialize(
            &event.author,
            event.created_at,
            event.kind,
            &event.tags,
            &event.content,
        );
        let signature = Signature::new(sig_bytes, String::new(), Utc::now());
        Ok(signature.verify(&canonical, &pubkey_bytes))
    }

    // -- Standard event helpers --

    /// Create a profile event (kind 0).
    pub fn profile(
        name: &str,
        about: Option<&str>,
        picture: Option<&str>,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let mut content = serde_json::Map::new();
        content.insert("name".into(), serde_json::Value::String(name.into()));
        if let Some(about) = about {
            content.insert("about".into(), serde_json::Value::String(about.into()));
        }
        if let Some(picture) = picture {
            content.insert("picture".into(), serde_json::Value::String(picture.into()));
        }
        let content_str = serde_json::to_string(&content)
            .map_err(|e| GlobeError::SigningFailed(e.to_string()))?;

        let unsigned = UnsignedEvent::new(kind::PROFILE, content_str);
        Self::sign(&unsigned, keypair)
    }

    /// Create a text note event (kind 1).
    pub fn text_note(content: &str, keypair: &CrownKeypair) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::TEXT_NOTE, content);
        Self::sign(&unsigned, keypair)
    }

    /// Create a contact list event (kind 3).
    pub fn contact_list(
        following: &[&str],
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let mut unsigned = UnsignedEvent::new(kind::CONTACT_LIST, "");
        for pubkey in following {
            unsigned = unsigned.with_pubkey_ref(pubkey);
        }
        Self::sign(&unsigned, keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let kp = CrownKeypair::generate();
        let unsigned = UnsignedEvent::new(1, "test content")
            .with_application_tag("omnidea");

        let event = EventBuilder::sign(&unsigned, &kp).unwrap();
        assert_eq!(event.kind, 1);
        assert_eq!(event.content, "test content");
        assert!(event.has_tag("application", "omnidea"));
        assert_eq!(event.author, kp.public_key_hex());

        let valid = EventBuilder::verify(&event).unwrap();
        assert!(valid);
    }

    #[test]
    fn tampered_content_fails_verification() {
        let kp = CrownKeypair::generate();
        let unsigned = UnsignedEvent::new(1, "original");
        let mut event = EventBuilder::sign(&unsigned, &kp).unwrap();
        event.content = "tampered".into();

        let valid = EventBuilder::verify(&event).unwrap();
        assert!(!valid);
    }

    #[test]
    fn tampered_id_fails_verification() {
        let kp = CrownKeypair::generate();
        let unsigned = UnsignedEvent::new(1, "test");
        let mut event = EventBuilder::sign(&unsigned, &kp).unwrap();
        event.id = "f".repeat(64);

        let valid = EventBuilder::verify(&event).unwrap();
        assert!(!valid);
    }

    #[test]
    fn sign_without_private_key_fails() {
        let kp = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(kp.crown_id()).unwrap();
        let unsigned = UnsignedEvent::new(1, "test");
        let result = EventBuilder::sign(&unsigned, &pubonly);
        assert!(result.is_err());
    }

    #[test]
    fn unsigned_event_builder_methods() {
        let unsigned = UnsignedEvent::new(7000, "my domain")
            .with_d_tag("sam.com")
            .with_pubkey_ref("abc123")
            .with_event_ref("def456")
            .with_tag("custom", &["val1", "val2"]);

        assert_eq!(unsigned.kind, 7000);
        assert_eq!(unsigned.tags.len(), 4);
    }

    #[test]
    fn profile_event() {
        let kp = CrownKeypair::generate();
        let event = EventBuilder::profile("Sam", Some("Builder"), None, &kp).unwrap();
        assert_eq!(event.kind, 0);
        assert!(event.content.contains("Sam"));
        assert!(event.content.contains("Builder"));
    }

    #[test]
    fn text_note_event() {
        let kp = CrownKeypair::generate();
        let event = EventBuilder::text_note("Hello world", &kp).unwrap();
        assert_eq!(event.kind, 1);
        assert_eq!(event.content, "Hello world");
    }

    #[test]
    fn contact_list_event() {
        let kp = CrownKeypair::generate();
        let event = EventBuilder::contact_list(&["pub1", "pub2", "pub3"], &kp).unwrap();
        assert_eq!(event.kind, 3);
        assert_eq!(event.p_tags().len(), 3);
    }
}
