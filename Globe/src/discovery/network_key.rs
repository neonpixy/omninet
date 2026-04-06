use serde::{Deserialize, Serialize};

use crown::CrownKeypair;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// The Network Key — Omnidea's shared heartbeat.
///
/// A 256-bit key used to encrypt relay addresses so they're readable by
/// any Omnidea participant but opaque to outside observers. Not in the
/// source code. Propagates person-to-person through the invitation graph,
/// always encrypted to the recipient's Crown public key.
///
/// Globe defines the wire types. The actual crypto (ECDH, AES-256-GCM)
/// is performed by the caller using Crown + Sentinal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkKeyMaterial {
    /// Key version (monotonically increasing, starts at 1).
    pub version: u32,
    /// The raw 256-bit key, base64-encoded.
    /// In transit, this is always encrypted — never plaintext on a relay.
    pub key_data: String,
    /// Unix timestamp when this key version was created.
    pub created_at: i64,
    /// crown_id of the person who generated this key version.
    pub created_by: String,
}

/// An encrypted delivery of the Network Key to a specific recipient.
///
/// The `payload` contains a `NetworkKeyMaterial` encrypted to the
/// recipient's Crown public key via ECDH + AES-256-GCM. Only the
/// recipient can decrypt it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkKeyEnvelope {
    /// Which key version this delivers.
    pub version: u32,
    /// Recipient's crown ID (hex).
    pub recipient: String,
    /// Encrypted payload (base64). Contains NetworkKeyMaterial.
    /// Encrypted by the sender using Crown ECDH shared secret with
    /// the recipient's public key + Sentinal AES-256-GCM.
    pub payload: String,
}

/// A key rotation announcement.
///
/// Published when the Network Key needs to change (compromise, scheduled
/// rotation). Contains envelopes for the sender's direct connections.
/// Each recipient forwards the rotation to their connections, creating
/// a ripple through the social graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyRotation {
    /// The version being replaced.
    pub old_version: u32,
    /// The new version being distributed.
    pub new_version: u32,
    /// Seconds before the old key stops working.
    pub grace_period_secs: u64,
    /// Reason for rotation.
    pub reason: RotationReason,
}

/// Why the Network Key is being rotated.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RotationReason {
    /// Scheduled rotation (routine security hygiene).
    Scheduled,
    /// Key suspected or confirmed compromised.
    Compromise,
    /// Network upgrade requiring new key format.
    Upgrade,
}

/// Builds ORP events for Network Key delivery and rotation.
pub struct NetworkKeyBuilder;

impl NetworkKeyBuilder {
    /// Build a key delivery event (kind 7040).
    ///
    /// Delivers the Network Key to a specific recipient as part of an
    /// invitation or key rotation. The envelope's payload is pre-encrypted
    /// by the caller.
    pub fn deliver(
        envelope: &NetworkKeyEnvelope,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let content = serde_json::to_string(envelope)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))?;
        let unsigned = UnsignedEvent::new(kind::KEY_DELIVERY, &content)
            .with_tag("p", &[&envelope.recipient])
            .with_tag("key_version", &[&envelope.version.to_string()]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a key rotation event (kind 7041).
    ///
    /// Announces that a new Network Key version is available. Recipients
    /// should look for their personalized delivery event (kind 7040) with
    /// the new version number.
    pub fn rotate(
        rotation: &KeyRotation,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let content = serde_json::to_string(rotation)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))?;
        let unsigned = UnsignedEvent::new(kind::KEY_ROTATION, &content)
            .with_tag("key_version", &[&rotation.new_version.to_string()])
            .with_tag("old_version", &[&rotation.old_version.to_string()]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Parse a key envelope from a delivery event.
    pub fn parse_delivery(event: &OmniEvent) -> Result<NetworkKeyEnvelope, GlobeError> {
        if event.kind != kind::KEY_DELIVERY {
            return Err(GlobeError::ProtocolError(format!(
                "expected kind {}, got {}",
                kind::KEY_DELIVERY,
                event.kind
            )));
        }
        serde_json::from_str(&event.content)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))
    }

    /// Parse a rotation announcement from a rotation event.
    pub fn parse_rotation(event: &OmniEvent) -> Result<KeyRotation, GlobeError> {
        if event.kind != kind::KEY_ROTATION {
            return Err(GlobeError::ProtocolError(format!(
                "expected kind {}, got {}",
                kind::KEY_ROTATION,
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

    #[test]
    fn network_key_material_serde() {
        let material = NetworkKeyMaterial {
            version: 1,
            key_data: "dGVzdGtleWRhdGExMjM0NTY3ODkwYWJjZGVm".into(),
            created_at: 1709654400,
            created_by: "cpub_sam".into(),
        };
        let json = serde_json::to_string(&material).unwrap();
        let loaded: NetworkKeyMaterial = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.created_by, "cpub_sam");
    }

    #[test]
    fn envelope_serde() {
        let envelope = NetworkKeyEnvelope {
            version: 1,
            recipient: "cpub_alice".into(),
            payload: "encrypted_base64_data".into(),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let loaded: NetworkKeyEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.recipient, "cpub_alice");
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn key_delivery_event_structure() {
        let kp = test_keypair();
        let envelope = NetworkKeyEnvelope {
            version: 1,
            recipient: "cpub_alice".into(),
            payload: "encrypted_data".into(),
        };

        let event = NetworkKeyBuilder::deliver(&envelope, &kp).unwrap();
        assert_eq!(event.kind, kind::KEY_DELIVERY);

        let p_tags = event.tag_values("p");
        assert_eq!(p_tags, vec!["cpub_alice"]);

        let version_tags = event.tag_values("key_version");
        assert_eq!(version_tags, vec!["1"]);

        assert!(EventBuilder::verify(&event).unwrap());
    }

    #[test]
    fn key_delivery_round_trip() {
        let kp = test_keypair();
        let envelope = NetworkKeyEnvelope {
            version: 2,
            recipient: "cpub_bob".into(),
            payload: "super_secret_encrypted_key".into(),
        };

        let event = NetworkKeyBuilder::deliver(&envelope, &kp).unwrap();
        let parsed = NetworkKeyBuilder::parse_delivery(&event).unwrap();
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.recipient, "cpub_bob");
        assert_eq!(parsed.payload, "super_secret_encrypted_key");
    }

    #[test]
    fn key_rotation_event_structure() {
        let kp = test_keypair();
        let rotation = KeyRotation {
            old_version: 1,
            new_version: 2,
            grace_period_secs: 604800, // 1 week
            reason: RotationReason::Scheduled,
        };

        let event = NetworkKeyBuilder::rotate(&rotation, &kp).unwrap();
        assert_eq!(event.kind, kind::KEY_ROTATION);

        let version_tags = event.tag_values("key_version");
        assert_eq!(version_tags, vec!["2"]);

        let old_tags = event.tag_values("old_version");
        assert_eq!(old_tags, vec!["1"]);

        assert!(EventBuilder::verify(&event).unwrap());
    }

    #[test]
    fn key_rotation_round_trip() {
        let kp = test_keypair();
        let rotation = KeyRotation {
            old_version: 1,
            new_version: 2,
            grace_period_secs: 86400,
            reason: RotationReason::Compromise,
        };

        let event = NetworkKeyBuilder::rotate(&rotation, &kp).unwrap();
        let parsed = NetworkKeyBuilder::parse_rotation(&event).unwrap();
        assert_eq!(parsed.old_version, 1);
        assert_eq!(parsed.new_version, 2);
        assert!(matches!(parsed.reason, RotationReason::Compromise));
    }

    #[test]
    fn rotation_reason_variants() {
        for reason in [
            RotationReason::Scheduled,
            RotationReason::Compromise,
            RotationReason::Upgrade,
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let loaded: RotationReason = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{loaded:?}"), format!("{reason:?}"));
        }
    }

    #[test]
    fn parse_delivery_wrong_kind_fails() {
        let kp = test_keypair();
        let event = EventBuilder::sign(
            &UnsignedEvent::new(kind::TEXT_NOTE, "not a delivery"),
            &kp,
        )
        .unwrap();
        assert!(NetworkKeyBuilder::parse_delivery(&event).is_err());
    }

    #[test]
    fn parse_rotation_wrong_kind_fails() {
        let kp = test_keypair();
        let event = EventBuilder::sign(
            &UnsignedEvent::new(kind::TEXT_NOTE, "not a rotation"),
            &kp,
        )
        .unwrap();
        assert!(NetworkKeyBuilder::parse_rotation(&event).is_err());
    }
}
