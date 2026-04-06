use serde::{Deserialize, Serialize};

use crown::CrownKeypair;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

use super::network_key::NetworkKeyEnvelope;

/// An invitation into the Omnidea network.
///
/// Carries everything a new participant needs: the Network Key (encrypted
/// to their Crown public key), relay addresses to connect to, and a
/// one-time pairing token for identity verification.
///
/// The invitation event is published to the inviter's relay. The invitee
/// connects (using the relay URL from the link/QR code), subscribes to
/// events tagged with their crown ID, and picks up the invitation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invitation {
    /// One-time pairing token (random hex string).
    pub token: String,
    /// Relay URLs where the inviter can be found.
    pub relay_urls: Vec<String>,
    /// Inviter's crown ID (hex).
    pub inviter: String,
    /// The Network Key, encrypted to the invitee's Crown public key.
    pub key_envelope: NetworkKeyEnvelope,
    /// Unix timestamp when this invitation was created.
    pub created_at: i64,
    /// Optional expiration timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

/// A parsed invitation link.
///
/// Format: `omnidea://invite?relay=ws://host:port&token=abc123&inviter=cpub1...`
///
/// The link tells the invitee's app where to connect and what to look for.
/// The actual invitation data (including the encrypted Network Key) is
/// fetched from the relay as an ORP event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvitationLink {
    /// Relay URL to connect to.
    pub relay_url: String,
    /// One-time token to identify this invitation.
    pub token: String,
    /// Inviter's crown ID (hex).
    pub inviter: String,
}

impl InvitationLink {
    /// Format as a URI string.
    pub fn to_uri(&self) -> String {
        format!(
            "omnidea://invite?relay={}&token={}&inviter={}",
            self.relay_url, self.token, self.inviter
        )
    }

    /// Parse from a URI string.
    pub fn from_uri(uri: &str) -> Option<Self> {
        let stripped = uri.strip_prefix("omnidea://invite?")?;
        let mut relay_url = None;
        let mut token = None;
        let mut inviter = None;

        for pair in stripped.split('&') {
            let (key, value) = pair.split_once('=')?;
            match key {
                "relay" => relay_url = Some(value.to_string()),
                "token" => token = Some(value.to_string()),
                "inviter" => inviter = Some(value.to_string()),
                _ => {}
            }
        }

        Some(Self {
            relay_url: relay_url?,
            token: token?,
            inviter: inviter?,
        })
    }
}

/// Builds ORP events for invitations.
pub struct InvitationBuilder;

impl InvitationBuilder {
    /// Build an invitation event (kind 7042).
    ///
    /// The invitation is tagged with the invitee's crown ID so they can
    /// find it by subscribing to events addressed to them.
    pub fn invite(
        invitation: &Invitation,
        invitee_crown_id: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let content = serde_json::to_string(invitation)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))?;
        let unsigned = UnsignedEvent::new(kind::INVITATION, &content)
            .with_tag("p", &[invitee_crown_id])
            .with_tag("token", &[&invitation.token]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Parse an invitation from an invitation event.
    pub fn parse(event: &OmniEvent) -> Result<Invitation, GlobeError> {
        if event.kind != kind::INVITATION {
            return Err(GlobeError::ProtocolError(format!(
                "expected kind {}, got {}",
                kind::INVITATION,
                event.kind
            )));
        }
        serde_json::from_str(&event.content)
            .map_err(|e| GlobeError::ProtocolError(e.to_string()))
    }

    /// Generate a random invitation token (32 hex characters).
    pub fn generate_token() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // Simple token — real implementation would use Crown's RNG.
        format!("{seed:032x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::network_key::NetworkKeyEnvelope;

    fn test_keypair() -> CrownKeypair {
        CrownKeypair::generate()
    }

    fn sample_invitation() -> Invitation {
        Invitation {
            token: "abc123def456".into(),
            relay_urls: vec!["ws://192.168.1.42:8080".into()],
            inviter: "cpub_sam".into(),
            key_envelope: NetworkKeyEnvelope {
                version: 1,
                recipient: "cpub_alice".into(),
                payload: "encrypted_network_key".into(),
            },
            created_at: 1709654400,
            expires_at: Some(1709740800),
        }
    }

    #[test]
    fn invitation_serde_round_trip() {
        let inv = sample_invitation();
        let json = serde_json::to_string(&inv).unwrap();
        let loaded: Invitation = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.token, "abc123def456");
        assert_eq!(loaded.relay_urls.len(), 1);
        assert_eq!(loaded.key_envelope.version, 1);
    }

    #[test]
    fn invitation_without_expiry() {
        let mut inv = sample_invitation();
        inv.expires_at = None;
        let json = serde_json::to_string(&inv).unwrap();
        assert!(!json.contains("expires_at"));
    }

    #[test]
    fn invitation_event_structure() {
        let kp = test_keypair();
        let inv = sample_invitation();
        let event = InvitationBuilder::invite(&inv, "cpub_alice", &kp).unwrap();

        assert_eq!(event.kind, kind::INVITATION);
        assert_eq!(event.tag_values("p"), vec!["cpub_alice"]);
        assert_eq!(event.tag_values("token"), vec!["abc123def456"]);
        assert!(EventBuilder::verify(&event).unwrap());
    }

    #[test]
    fn invitation_round_trip_via_event() {
        let kp = test_keypair();
        let inv = sample_invitation();
        let event = InvitationBuilder::invite(&inv, "cpub_alice", &kp).unwrap();
        let parsed = InvitationBuilder::parse(&event).unwrap();
        assert_eq!(parsed.token, inv.token);
        assert_eq!(parsed.inviter, inv.inviter);
        assert_eq!(parsed.key_envelope.recipient, "cpub_alice");
    }

    #[test]
    fn invitation_link_to_uri() {
        let link = InvitationLink {
            relay_url: "ws://192.168.1.42:8080".into(),
            token: "abc123".into(),
            inviter: "cpub_sam".into(),
        };
        let uri = link.to_uri();
        assert!(uri.starts_with("omnidea://invite?"));
        assert!(uri.contains("relay=ws://192.168.1.42:8080"));
        assert!(uri.contains("token=abc123"));
        assert!(uri.contains("inviter=cpub_sam"));
    }

    #[test]
    fn invitation_link_from_uri() {
        let uri = "omnidea://invite?relay=ws://example.com:9090&token=xyz789&inviter=cpub_alice";
        let link = InvitationLink::from_uri(uri).unwrap();
        assert_eq!(link.relay_url, "ws://example.com:9090");
        assert_eq!(link.token, "xyz789");
        assert_eq!(link.inviter, "cpub_alice");
    }

    #[test]
    fn invitation_link_round_trip() {
        let link = InvitationLink {
            relay_url: "ws://localhost:8080".into(),
            token: "test_token".into(),
            inviter: "cpub_test".into(),
        };
        let uri = link.to_uri();
        let parsed = InvitationLink::from_uri(&uri).unwrap();
        assert_eq!(parsed.relay_url, link.relay_url);
        assert_eq!(parsed.token, link.token);
        assert_eq!(parsed.inviter, link.inviter);
    }

    #[test]
    fn invitation_link_invalid_uri() {
        assert!(InvitationLink::from_uri("https://example.com").is_none());
        assert!(InvitationLink::from_uri("omnidea://invite?relay=x").is_none());
    }

    #[test]
    fn generate_token_is_nonempty() {
        let token = InvitationBuilder::generate_token();
        assert!(!token.is_empty());
    }
}
