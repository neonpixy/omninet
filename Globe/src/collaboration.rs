//! Globe Collaboration Bridge — real-time multiplayer over the relay network.
//!
//! This module bridges Equipment's synchronous Communicator sessions to
//! Globe's async relay network. It defines the wire types for collaboration
//! messages (CRDT operations, cursor updates, presence announcements, and
//! session control) plus encode/decode helpers.
//!
//! # Design
//!
//! - Globe is transport-only. The collaboration types here are serialization
//!   containers — Globe doesn't interpret CRDT operations or cursor positions.
//! - Cross-crate types (Equipment's `CursorPosition`, `PresenceInfo`) are
//!   carried as `serde_json::Value` to avoid an Equipment dependency.
//! - Messages are JSON-encoded for relay transmission via text frames.
//!
//! # Event Kind
//!
//! `KIND_COLLABORATION` (5120) — collaboration messages in the Equipment
//! range, adjacent to the Communicator signaling kinds (5100-5113).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Globe event kind for collaboration messages.
///
/// Uses Equipment's kind range (5000-5999), placed at 5120 to sit
/// alongside the Communicator signaling kinds (5100-5113) without
/// conflicting with the stream kinds at 5110-5113.
pub const KIND_COLLABORATION: u32 = 5120;

/// Configuration for a collaboration session over Globe.
///
/// Determines which relay to use and whether to enable privacy features
/// (traffic padding via Globe's camouflage system).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationConfig {
    /// The `.idea` document this session is for.
    pub idea_id: Uuid,
    /// The relay URL to use for this collaboration session.
    pub relay_url: String,
    /// Enable Globe privacy features (traffic padding).
    #[serde(default)]
    pub privacy_enabled: bool,
}

impl CollaborationConfig {
    /// Create a new collaboration config for an `.idea` document.
    pub fn new(idea_id: Uuid, relay_url: impl Into<String>) -> Self {
        Self {
            idea_id,
            relay_url: relay_url.into(),
            privacy_enabled: false,
        }
    }

    /// Builder: enable privacy features.
    #[must_use]
    pub fn with_privacy(mut self) -> Self {
        self.privacy_enabled = true;
        self
    }
}

/// A message sent over the collaboration channel.
///
/// Tagged enum serialized with `"type"` discriminant in camelCase.
/// Globe treats the content as opaque — the meaning is defined by
/// Equipment and the CRDT layer in X.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CollaborationMessage {
    /// CRDT operations (digit insert/update/delete/move + text edits).
    Operations {
        /// Serialized `DigitOperation` values from the Ideas CRDT layer.
        ops: Vec<serde_json::Value>,
    },
    /// Cursor position update.
    Cursor {
        /// The collaborator's Crown public key.
        crown_id: String,
        /// Serialized `CursorPosition` from Equipment.
        cursor: serde_json::Value,
    },
    /// Presence update (join/leave/active state).
    Presence {
        /// Serialized `PresenceInfo` from Equipment.
        info: serde_json::Value,
    },
    /// Session control (invite/leave/end).
    Control {
        /// The control action to perform.
        action: ControlAction,
    },
}

/// Session control actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlAction {
    /// Invite additional collaborators by their Crown IDs.
    Invite {
        /// Crown IDs of the people being invited.
        crown_ids: Vec<String>,
    },
    /// A collaborator is leaving the session.
    Leave {
        /// Crown ID of the departing collaborator.
        crown_id: String,
    },
    /// End the entire collaboration session.
    End {
        /// Human-readable reason for ending.
        reason: String,
    },
}

/// Encode a [`CollaborationMessage`] for transmission as Globe frame data.
///
/// Returns JSON bytes suitable for text or binary relay frames.
///
/// # Errors
///
/// Returns `serde_json::Error` if serialization fails (should not happen
/// for well-formed messages).
pub fn encode_message(msg: &CollaborationMessage) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(msg)
}

/// Decode a [`CollaborationMessage`] from Globe frame data.
///
/// # Errors
///
/// Returns `serde_json::Error` if the data is not valid JSON or does not
/// match the `CollaborationMessage` schema.
pub fn decode_message(data: &[u8]) -> Result<CollaborationMessage, serde_json::Error> {
    serde_json::from_slice(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_collaboration_in_equipment_range() {
        assert!((5000..6000).contains(&KIND_COLLABORATION));
        // Must not collide with existing signaling kinds.
        assert_ne!(KIND_COLLABORATION, 5100); // COMMUNICATOR_OFFER
        assert_ne!(KIND_COLLABORATION, 5101); // COMMUNICATOR_ANSWER
        assert_ne!(KIND_COLLABORATION, 5102); // COMMUNICATOR_END
        assert_ne!(KIND_COLLABORATION, 5103); // ICE_CANDIDATE
        assert_ne!(KIND_COLLABORATION, 5110); // STREAM_ANNOUNCE
        assert_ne!(KIND_COLLABORATION, 5111); // STREAM_UPDATE
        assert_ne!(KIND_COLLABORATION, 5112); // STREAM_END
        assert_ne!(KIND_COLLABORATION, 5113); // STREAM_RECORDING
    }

    #[test]
    fn config_new_defaults() {
        let iid = Uuid::new_v4();
        let config = CollaborationConfig::new(iid, "wss://relay.example.com");
        assert_eq!(config.idea_id, iid);
        assert_eq!(config.relay_url, "wss://relay.example.com");
        assert!(!config.privacy_enabled);
    }

    #[test]
    fn config_with_privacy() {
        let config =
            CollaborationConfig::new(Uuid::new_v4(), "wss://relay.example.com").with_privacy();
        assert!(config.privacy_enabled);
    }

    #[test]
    fn config_serde_round_trip() {
        let config =
            CollaborationConfig::new(Uuid::new_v4(), "wss://relay.example.com").with_privacy();
        let json = serde_json::to_string(&config).unwrap();
        let loaded: CollaborationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.idea_id, config.idea_id);
        assert_eq!(loaded.relay_url, config.relay_url);
        assert!(loaded.privacy_enabled);
    }

    #[test]
    fn config_privacy_defaults_to_false_on_missing() {
        let iid = Uuid::new_v4();
        let json = format!(
            r#"{{"idea_id":"{}","relay_url":"wss://relay.example.com"}}"#,
            iid
        );
        let loaded: CollaborationConfig = serde_json::from_str(&json).unwrap();
        assert!(!loaded.privacy_enabled);
    }

    // -- CollaborationMessage encode/decode --

    #[test]
    fn encode_decode_operations_message() {
        let msg = CollaborationMessage::Operations {
            ops: vec![
                serde_json::json!({"type": "insert", "digit_id": "abc", "position": 0}),
                serde_json::json!({"type": "update", "digit_id": "abc", "field": "text"}),
            ],
        };

        let bytes = encode_message(&msg).unwrap();
        let decoded = decode_message(&bytes).unwrap();

        match decoded {
            CollaborationMessage::Operations { ops } => {
                assert_eq!(ops.len(), 2);
            }
            _ => panic!("expected Operations variant"),
        }
    }

    #[test]
    fn encode_decode_cursor_message() {
        let msg = CollaborationMessage::Cursor {
            crown_id: "cpub_alice".into(),
            cursor: serde_json::json!({
                "digit_id": Uuid::new_v4().to_string(),
                "field": "text",
                "offset": 42
            }),
        };

        let bytes = encode_message(&msg).unwrap();
        let decoded = decode_message(&bytes).unwrap();

        match decoded {
            CollaborationMessage::Cursor { crown_id, cursor } => {
                assert_eq!(crown_id, "cpub_alice");
                assert_eq!(cursor["offset"], 42);
            }
            _ => panic!("expected Cursor variant"),
        }
    }

    #[test]
    fn encode_decode_presence_message() {
        let msg = CollaborationMessage::Presence {
            info: serde_json::json!({
                "crown_id": "cpub_bob",
                "display_name": "Bob",
                "color": "#3498db",
                "is_active": true
            }),
        };

        let bytes = encode_message(&msg).unwrap();
        let decoded = decode_message(&bytes).unwrap();

        match decoded {
            CollaborationMessage::Presence { info } => {
                assert_eq!(info["crown_id"], "cpub_bob");
            }
            _ => panic!("expected Presence variant"),
        }
    }

    #[test]
    fn encode_decode_control_invite() {
        let msg = CollaborationMessage::Control {
            action: ControlAction::Invite {
                crown_ids: vec!["cpub_charlie".into(), "cpub_dave".into()],
            },
        };

        let bytes = encode_message(&msg).unwrap();
        let decoded = decode_message(&bytes).unwrap();

        match decoded {
            CollaborationMessage::Control {
                action: ControlAction::Invite { crown_ids },
            } => {
                assert_eq!(crown_ids.len(), 2);
                assert_eq!(crown_ids[0], "cpub_charlie");
            }
            _ => panic!("expected Control/Invite variant"),
        }
    }

    #[test]
    fn encode_decode_control_leave() {
        let msg = CollaborationMessage::Control {
            action: ControlAction::Leave {
                crown_id: "cpub_alice".into(),
            },
        };

        let bytes = encode_message(&msg).unwrap();
        let decoded = decode_message(&bytes).unwrap();

        match decoded {
            CollaborationMessage::Control {
                action: ControlAction::Leave { crown_id },
            } => {
                assert_eq!(crown_id, "cpub_alice");
            }
            _ => panic!("expected Control/Leave variant"),
        }
    }

    #[test]
    fn encode_decode_control_end() {
        let msg = CollaborationMessage::Control {
            action: ControlAction::End {
                reason: "session complete".into(),
            },
        };

        let bytes = encode_message(&msg).unwrap();
        let decoded = decode_message(&bytes).unwrap();

        match decoded {
            CollaborationMessage::Control {
                action: ControlAction::End { reason },
            } => {
                assert_eq!(reason, "session complete");
            }
            _ => panic!("expected Control/End variant"),
        }
    }

    #[test]
    fn decode_invalid_data_returns_error() {
        let result = decode_message(b"not valid json {{{");
        assert!(result.is_err());
    }

    #[test]
    fn operations_serde_camel_case_tag() {
        let msg = CollaborationMessage::Operations { ops: vec![] };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"operations""#));
    }

    #[test]
    fn cursor_serde_camel_case_tag() {
        let msg = CollaborationMessage::Cursor {
            crown_id: "cpub_test".into(),
            cursor: serde_json::json!({}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"cursor""#));
    }

    #[test]
    fn control_serde_camel_case_tag() {
        let msg = CollaborationMessage::Control {
            action: ControlAction::End {
                reason: "done".into(),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"control""#));
    }
}
