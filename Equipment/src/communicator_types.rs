use serde::{Deserialize, Serialize};

/// Privacy level for a communication session.
///
/// Controls whether the session uses intermediary routing and blinded keys
/// to protect participant metadata from network observers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidentiality {
    /// Normal behavior — direct routing, standard keys.
    #[default]
    Standard,
    /// Forces intermediary route and blinded key usage.
    High,
}

/// A privacy-aware relay route for a communication session.
///
/// When set on a `CommunicatorSession`, data frames are routed through
/// the specified relay chain instead of directly between participants.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyRoute {
    /// Ordered list of relay URLs the session data traverses.
    pub relay_path: Vec<String>,
    /// If set, use a blinded pubkey derived from this context string
    /// instead of the participant's real Crown pubkey.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blinding_context: Option<String>,
}

/// An offer to start a real-time communication session.
///
/// Flows over Globe as a kind 5100 event. The content is encrypted
/// to each participant's Crown public key via Sentinal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicatorOffer {
    /// Unique session identifier.
    pub session_id: String,
    /// Which channel type this offer is for (e.g., `"voice.call"`).
    pub channel_id: String,
    /// Crown ID of the person starting the session.
    pub initiator: String,
    /// Crown IDs of all intended participants.
    pub participants: Vec<String>,
    /// Optional ECDH key exchange data for end-to-end encryption.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<OfferEncryption>,
    /// Serialized relay path hint for the recipient.
    ///
    /// When the initiator wants a privacy-routed session, this field
    /// tells the recipient which relay path to use for the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_route_hint: Option<String>,
}

/// Key exchange data for encrypted sessions.
///
/// The initiator generates an ephemeral keypair, derives a shared secret
/// with each participant's Crown public key, and wraps a session key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfferEncryption {
    /// The initiator's ephemeral public key for this session.
    pub ephemeral_public_key: String,
    /// Per-participant wrapped session keys.
    pub wrapped_session_keys: Vec<WrappedKey>,
}

/// A session key wrapped for a specific participant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WrappedKey {
    /// Crown ID of the participant this key is wrapped for.
    pub recipient: String,
    /// The session key, encrypted to the recipient's Crown public key.
    pub encrypted_key: String,
}

/// A response to a communication offer.
///
/// Flows over Globe as a kind 5101 event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicatorAnswer {
    /// The session being responded to.
    pub session_id: String,
    /// Crown ID of the person responding.
    pub responder: String,
    /// Whether the offer was accepted or declined.
    pub accepted: bool,
}

/// Signal that a session has ended.
///
/// Flows over Globe as a kind 5102 event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicatorEnd {
    /// The session that ended.
    pub session_id: String,
    /// Why the session ended.
    pub reason: EndReason,
}

/// Why a session ended.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndReason {
    /// Session ended gracefully by a participant.
    Normal,
    /// The offer was declined by the recipient.
    Declined,
    /// The offer expired without a response.
    Timeout,
    /// An error forced the session to end.
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_serde_round_trip() {
        let offer = CommunicatorOffer {
            session_id: "sess-001".into(),
            channel_id: "voice.call".into(),
            initiator: "cpub_alice".into(),
            participants: vec!["cpub_alice".into(), "cpub_bob".into()],
            encryption: Some(OfferEncryption {
                ephemeral_public_key: "deadbeef".into(),
                wrapped_session_keys: vec![WrappedKey {
                    recipient: "cpub_bob".into(),
                    encrypted_key: "cafebabe".into(),
                }],
            }),
            privacy_route_hint: None,
        };

        let json = serde_json::to_string(&offer).unwrap();
        let loaded: CommunicatorOffer = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, "sess-001");
        assert_eq!(loaded.participants.len(), 2);
        assert!(loaded.encryption.is_some());
    }

    #[test]
    fn offer_without_encryption() {
        let offer = CommunicatorOffer {
            session_id: "sess-002".into(),
            channel_id: "music.stream".into(),
            initiator: "cpub_alice".into(),
            participants: vec!["cpub_alice".into()],
            encryption: None,
            privacy_route_hint: None,
        };

        let json = serde_json::to_string(&offer).unwrap();
        assert!(!json.contains("encryption"));

        let loaded: CommunicatorOffer = serde_json::from_str(&json).unwrap();
        assert!(loaded.encryption.is_none());
    }

    #[test]
    fn answer_serde_round_trip() {
        let answer = CommunicatorAnswer {
            session_id: "sess-001".into(),
            responder: "cpub_bob".into(),
            accepted: true,
        };

        let json = serde_json::to_string(&answer).unwrap();
        let loaded: CommunicatorAnswer = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, "sess-001");
        assert!(loaded.accepted);
    }

    #[test]
    fn end_serde_round_trip() {
        let end = CommunicatorEnd {
            session_id: "sess-001".into(),
            reason: EndReason::Normal,
        };

        let json = serde_json::to_string(&end).unwrap();
        let loaded: CommunicatorEnd = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, "sess-001");
        assert!(matches!(loaded.reason, EndReason::Normal));
    }

    #[test]
    fn confidentiality_default_is_standard() {
        let c = Confidentiality::default();
        assert_eq!(c, Confidentiality::Standard);
    }

    #[test]
    fn confidentiality_serde_round_trip() {
        for variant in [Confidentiality::Standard, Confidentiality::High] {
            let json = serde_json::to_string(&variant).unwrap();
            let loaded: Confidentiality = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded, variant);
        }
        // Verify camelCase serialization.
        let json = serde_json::to_string(&Confidentiality::Standard).unwrap();
        assert_eq!(json, "\"standard\"");
        let json = serde_json::to_string(&Confidentiality::High).unwrap();
        assert_eq!(json, "\"high\"");
    }

    #[test]
    fn privacy_route_creation_and_serde() {
        let route = PrivacyRoute {
            relay_path: vec![
                "wss://relay1.example.com".into(),
                "wss://relay2.example.com".into(),
            ],
            blinding_context: Some("session-blind-001".into()),
        };

        let json = serde_json::to_string(&route).unwrap();
        let loaded: PrivacyRoute = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.relay_path.len(), 2);
        assert_eq!(loaded.relay_path[0], "wss://relay1.example.com");
        assert_eq!(
            loaded.blinding_context.as_deref(),
            Some("session-blind-001")
        );
    }

    #[test]
    fn privacy_route_without_blinding_context() {
        let route = PrivacyRoute {
            relay_path: vec!["wss://relay1.example.com".into()],
            blinding_context: None,
        };

        let json = serde_json::to_string(&route).unwrap();
        assert!(!json.contains("blinding_context"));

        let loaded: PrivacyRoute = serde_json::from_str(&json).unwrap();
        assert!(loaded.blinding_context.is_none());
    }

    #[test]
    fn offer_with_privacy_route_hint() {
        let offer = CommunicatorOffer {
            session_id: "sess-priv-001".into(),
            channel_id: "voice.call".into(),
            initiator: "cpub_alice".into(),
            participants: vec!["cpub_alice".into(), "cpub_bob".into()],
            encryption: None,
            privacy_route_hint: Some("wss://relay1.example.com,wss://relay2.example.com".into()),
        };

        let json = serde_json::to_string(&offer).unwrap();
        let loaded: CommunicatorOffer = serde_json::from_str(&json).unwrap();
        assert_eq!(
            loaded.privacy_route_hint.as_deref(),
            Some("wss://relay1.example.com,wss://relay2.example.com")
        );
    }

    #[test]
    fn offer_backward_compat_without_privacy_route_hint() {
        // JSON produced by older code without privacy_route_hint field.
        let json = r#"{
            "session_id": "sess-old",
            "channel_id": "voice.call",
            "initiator": "cpub_alice",
            "participants": ["cpub_alice"]
        }"#;

        let loaded: CommunicatorOffer = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.session_id, "sess-old");
        assert!(loaded.privacy_route_hint.is_none());
        assert!(loaded.encryption.is_none());
    }

    #[test]
    fn end_reason_variants() {
        for reason in [
            EndReason::Normal,
            EndReason::Declined,
            EndReason::Timeout,
            EndReason::Error("network failure".into()),
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let loaded: EndReason = serde_json::from_str(&json).unwrap();
            // Just verify round-trip doesn't panic.
            let _ = format!("{loaded:?}");
        }
    }
}
