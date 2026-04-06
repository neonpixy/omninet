//! Relay forwarding protocol for multi-hop message delivery.
//!
//! Enables messages to traverse a chain of relays before reaching their
//! destination. Each relay receives a [`ForwardEnvelope`] containing the
//! next hop URL and a TTL counter. The relay forwards the envelope to
//! the next hop (decrementing TTL) or delivers the payload if it is the
//! final destination.
//!
//! Combined with Sentinal's onion encryption, this provides unlinkability:
//! each relay only knows the previous and next hop, never the full path.
//!
//! # Example
//!
//! ```
//! use globe::privacy::relay_forward::{
//!     build_forward_envelope, process_forward, ForwardAction, ForwardingConfig,
//! };
//!
//! let payload = b"encrypted message";
//! let path = vec![
//!     "wss://relay1.example.com".to_string(),
//!     "wss://relay2.example.com".to_string(),
//!     "wss://relay3.example.com".to_string(),
//! ];
//!
//! let envelope = build_forward_envelope(payload, &path).unwrap();
//! assert_eq!(envelope.ttl, 3);
//! assert_eq!(envelope.next_hop.as_deref(), Some("wss://relay2.example.com"));
//! ```

use serde::{Deserialize, Serialize};

use crate::error::GlobeError;

// ---------------------------------------------------------------------------
// ForwardingConfig
// ---------------------------------------------------------------------------

/// Configuration for relay forwarding.
///
/// Controls whether the local relay participates in forwarding and the
/// default path length for outgoing messages. Disabled by default for
/// backward compatibility.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardingConfig {
    /// Whether forwarding is enabled on this relay.
    pub enabled: bool,
    /// Default number of forwarding hops for outgoing messages.
    /// Only used as a hint when the caller does not specify a path.
    pub default_hops: usize,
    /// Maximum number of hops allowed. Envelopes with more hops are
    /// rejected to prevent abuse.
    pub max_hops: u8,
}

impl Default for ForwardingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_hops: 1,
            max_hops: 3,
        }
    }
}

impl ForwardingConfig {
    /// Validate the configuration, returning a human-readable error if
    /// any field is invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_hops == 0 {
            return Err("max_hops must be > 0".into());
        }
        if self.default_hops > self.max_hops as usize {
            return Err(format!(
                "default_hops ({}) must be <= max_hops ({})",
                self.default_hops, self.max_hops,
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RelayPath
// ---------------------------------------------------------------------------

/// An ordered sequence of relay URLs forming a forwarding path.
///
/// The first hop is the entry point, the last hop is the destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayPath {
    /// Relay URLs in traversal order.
    pub hops: Vec<String>,
    /// Index of the current hop being processed (0-based).
    pub current_hop: usize,
}

impl RelayPath {
    /// Create a new relay path from a list of hop URLs.
    ///
    /// Returns an error if the path is empty.
    pub fn new(hops: Vec<String>) -> Result<Self, GlobeError> {
        if hops.is_empty() {
            return Err(GlobeError::InvalidConfig(
                "relay path must have at least one hop".into(),
            ));
        }
        Ok(Self {
            hops,
            current_hop: 0,
        })
    }

    /// The total number of hops in the path.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hops.len()
    }

    /// Whether the path is empty (should not happen after construction).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }

    /// Whether the current hop is the final destination.
    #[must_use]
    pub fn is_final_hop(&self) -> bool {
        self.current_hop >= self.hops.len().saturating_sub(1)
    }
}

// ---------------------------------------------------------------------------
// ForwardEnvelope
// ---------------------------------------------------------------------------

/// A forwarding envelope that wraps a payload for relay-to-relay delivery.
///
/// Each relay in the chain reads `next_hop` to decide where to send
/// the envelope next, and decrements `ttl` before forwarding. When
/// `next_hop` is `None`, the payload has reached its destination.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForwardEnvelope {
    /// The URL of the next relay to forward to, or `None` if this is
    /// the final destination.
    pub next_hop: Option<String>,
    /// The (possibly onion-encrypted) payload.
    pub payload: Vec<u8>,
    /// Time-to-live: decremented at each hop. When it reaches 0, the
    /// envelope is expired and must not be forwarded further.
    pub ttl: u8,
}

// ---------------------------------------------------------------------------
// ForwardAction
// ---------------------------------------------------------------------------

/// The result of processing a [`ForwardEnvelope`] at a relay.
#[derive(Clone, Debug)]
pub enum ForwardAction {
    /// The envelope should be forwarded to the next relay.
    Forward {
        /// URL of the next relay.
        next: String,
        /// The envelope to send (with decremented TTL).
        envelope: ForwardEnvelope,
    },
    /// The payload has reached its final destination and should be
    /// delivered locally.
    Deliver {
        /// The unwrapped payload.
        payload: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build an initial forwarding envelope for a message entering the relay chain.
///
/// `payload` is the (typically onion-encrypted) message body.
/// `path` is the ordered list of relay URLs, where `path[0]` is the first
/// relay that will receive this envelope.
///
/// The envelope's `next_hop` is set to `path[1]` (if it exists), and `ttl`
/// is set to the path length. The caller is responsible for sending the
/// envelope to `path[0]`.
///
/// # Errors
///
/// Returns `GlobeError::InvalidConfig` if the path is empty.
pub fn build_forward_envelope(
    payload: &[u8],
    path: &[String],
) -> Result<ForwardEnvelope, GlobeError> {
    if path.is_empty() {
        return Err(GlobeError::InvalidConfig(
            "forwarding path must have at least one hop".into(),
        ));
    }

    let next_hop = if path.len() > 1 {
        Some(path[1].clone())
    } else {
        None
    };

    Ok(ForwardEnvelope {
        next_hop,
        payload: payload.to_vec(),
        ttl: path.len() as u8,
    })
}

/// Process a forwarding envelope at the current relay.
///
/// Determines whether the envelope should be forwarded to the next hop
/// or delivered locally based on TTL and the `next_hop` field.
///
/// # Errors
///
/// Returns `GlobeError::ProtocolError` if the TTL has expired.
pub fn process_forward(envelope: ForwardEnvelope) -> Result<ForwardAction, GlobeError> {
    if envelope.ttl == 0 {
        return Err(GlobeError::ProtocolError(
            "forwarding envelope TTL expired".into(),
        ));
    }

    match envelope.next_hop {
        Some(next) => {
            let forwarded = ForwardEnvelope {
                next_hop: None, // The next relay will set this from its own routing
                payload: envelope.payload,
                ttl: envelope.ttl.saturating_sub(1),
            };
            Ok(ForwardAction::Forward {
                next,
                envelope: forwarded,
            })
        }
        None => Ok(ForwardAction::Deliver {
            payload: envelope.payload,
        }),
    }
}

/// Validate that a forwarding path respects the given configuration.
///
/// Returns an error if the path exceeds `max_hops`.
pub fn validate_path(
    path: &[String],
    config: &ForwardingConfig,
) -> Result<(), GlobeError> {
    if path.len() > config.max_hops as usize {
        return Err(GlobeError::InvalidConfig(format!(
            "forwarding path has {} hops but max_hops is {}",
            path.len(),
            config.max_hops,
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- ForwardingConfig tests --

    #[test]
    fn default_config() {
        let config = ForwardingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.default_hops, 1);
        assert_eq!(config.max_hops, 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_zero_max_hops_fails() {
        let config = ForwardingConfig {
            max_hops: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_default_hops_exceeds_max_fails() {
        let config = ForwardingConfig {
            default_hops: 5,
            max_hops: 3,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("default_hops"));
    }

    #[test]
    fn config_serde_roundtrip() {
        let config = ForwardingConfig {
            enabled: true,
            default_hops: 2,
            max_hops: 5,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: ForwardingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, loaded);
    }

    // -- RelayPath tests --

    #[test]
    fn relay_path_basic() {
        let path = RelayPath::new(vec![
            "wss://a.com".into(),
            "wss://b.com".into(),
        ])
        .unwrap();
        assert_eq!(path.len(), 2);
        assert!(!path.is_empty());
        assert!(!path.is_final_hop());
    }

    #[test]
    fn relay_path_single_hop_is_final() {
        let path = RelayPath::new(vec!["wss://only.com".into()]).unwrap();
        assert!(path.is_final_hop());
    }

    #[test]
    fn relay_path_empty_fails() {
        let result = RelayPath::new(vec![]);
        assert!(result.is_err());
    }

    // -- build_forward_envelope tests --

    #[test]
    fn build_single_hop_envelope() {
        let path = vec!["wss://dest.com".into()];
        let envelope = build_forward_envelope(b"payload", &path).unwrap();
        assert!(envelope.next_hop.is_none());
        assert_eq!(envelope.payload, b"payload");
        assert_eq!(envelope.ttl, 1);
    }

    #[test]
    fn build_multi_hop_envelope() {
        let path = vec![
            "wss://relay1.com".into(),
            "wss://relay2.com".into(),
            "wss://relay3.com".into(),
        ];
        let envelope = build_forward_envelope(b"secret", &path).unwrap();
        assert_eq!(envelope.next_hop.as_deref(), Some("wss://relay2.com"));
        assert_eq!(envelope.ttl, 3);
    }

    #[test]
    fn build_empty_path_fails() {
        let result = build_forward_envelope(b"data", &[]);
        assert!(result.is_err());
    }

    // -- process_forward tests --

    #[test]
    fn process_deliver_no_next_hop() {
        let envelope = ForwardEnvelope {
            next_hop: None,
            payload: b"final message".to_vec(),
            ttl: 1,
        };
        match process_forward(envelope).unwrap() {
            ForwardAction::Deliver { payload } => {
                assert_eq!(payload, b"final message");
            }
            _ => panic!("expected Deliver"),
        }
    }

    #[test]
    fn process_forward_with_next_hop() {
        let envelope = ForwardEnvelope {
            next_hop: Some("wss://next.com".into()),
            payload: b"keep going".to_vec(),
            ttl: 3,
        };
        match process_forward(envelope).unwrap() {
            ForwardAction::Forward { next, envelope } => {
                assert_eq!(next, "wss://next.com");
                assert_eq!(envelope.ttl, 2);
                assert_eq!(envelope.payload, b"keep going");
                // next_hop is cleared — next relay sets its own routing
                assert!(envelope.next_hop.is_none());
            }
            _ => panic!("expected Forward"),
        }
    }

    #[test]
    fn process_ttl_expired() {
        let envelope = ForwardEnvelope {
            next_hop: Some("wss://next.com".into()),
            payload: b"expired".to_vec(),
            ttl: 0,
        };
        let result = process_forward(envelope);
        assert!(result.is_err());
    }

    #[test]
    fn process_ttl_decrements() {
        let envelope = ForwardEnvelope {
            next_hop: Some("wss://hop.com".into()),
            payload: vec![],
            ttl: 5,
        };
        if let ForwardAction::Forward { envelope, .. } = process_forward(envelope).unwrap() {
            assert_eq!(envelope.ttl, 4);
        } else {
            panic!("expected Forward");
        }
    }

    // -- Full chain simulation --

    #[test]
    fn full_forward_chain() {
        // Simulate: relay1 -> relay2 -> relay3 (deliver)
        let path = vec![
            "wss://relay1.com".into(),
            "wss://relay2.com".into(),
            "wss://relay3.com".into(),
        ];

        // Build initial envelope (caller sends to relay1)
        let envelope = build_forward_envelope(b"hello", &path).unwrap();
        assert_eq!(envelope.ttl, 3);
        assert_eq!(envelope.next_hop.as_deref(), Some("wss://relay2.com"));

        // relay1 processes: forwards to relay2
        match process_forward(envelope).unwrap() {
            ForwardAction::Forward { next, mut envelope } => {
                assert_eq!(next, "wss://relay2.com");
                assert_eq!(envelope.ttl, 2);

                // relay2 sets next_hop for relay3 (in real usage, read from onion layer)
                envelope.next_hop = Some("wss://relay3.com".into());
                match process_forward(envelope).unwrap() {
                    ForwardAction::Forward { next, mut envelope } => {
                        assert_eq!(next, "wss://relay3.com");
                        assert_eq!(envelope.ttl, 1);

                        // relay3 has no next_hop: deliver
                        envelope.next_hop = None;
                        match process_forward(envelope).unwrap() {
                            ForwardAction::Deliver { payload } => {
                                assert_eq!(payload, b"hello");
                            }
                            _ => panic!("expected Deliver at relay3"),
                        }
                    }
                    _ => panic!("expected Forward at relay2"),
                }
            }
            _ => panic!("expected Forward at relay1"),
        }
    }

    // -- validate_path tests --

    #[test]
    fn validate_path_within_limit() {
        let config = ForwardingConfig {
            max_hops: 3,
            ..Default::default()
        };
        let path: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        assert!(validate_path(&path, &config).is_ok());
    }

    #[test]
    fn validate_path_exceeds_max_hops() {
        let config = ForwardingConfig {
            max_hops: 2,
            ..Default::default()
        };
        let path: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        assert!(validate_path(&path, &config).is_err());
    }

    // -- ForwardEnvelope serde tests --

    #[test]
    fn envelope_serde_roundtrip() {
        let envelope = ForwardEnvelope {
            next_hop: Some("wss://relay.com".into()),
            payload: b"test data".to_vec(),
            ttl: 3,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let loaded: ForwardEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.next_hop, envelope.next_hop);
        assert_eq!(loaded.payload, envelope.payload);
        assert_eq!(loaded.ttl, envelope.ttl);
    }

    #[test]
    fn envelope_serde_no_next_hop() {
        let envelope = ForwardEnvelope {
            next_hop: None,
            payload: b"final".to_vec(),
            ttl: 1,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let loaded: ForwardEnvelope = serde_json::from_str(&json).unwrap();
        assert!(loaded.next_hop.is_none());
    }

    // -- GlobeConfig backward compatibility --

    #[test]
    fn globe_config_without_forwarding_field_deserializes() {
        // Simulate a GlobeConfig JSON from before the forwarding field existed.
        let json = r#"{
            "relay_urls": [],
            "max_relays": 10,
            "reconnect_min_delay": 500,
            "reconnect_max_delay": 60000,
            "reconnect_max_attempts": null,
            "heartbeat_interval": 30000,
            "connection_timeout": 10000,
            "max_seen_events": 10000,
            "max_pending_messages": 1000,
            "protocol_version": 1
        }"#;
        let config: crate::config::GlobeConfig = serde_json::from_str(json).unwrap();
        assert!(!config.forwarding.enabled);
        assert_eq!(config.forwarding.default_hops, 1);
        assert_eq!(config.forwarding.max_hops, 3);
    }

    #[test]
    fn globe_config_with_forwarding_roundtrip() {
        let config = crate::config::GlobeConfig {
            forwarding: ForwardingConfig {
                enabled: true,
                default_hops: 2,
                max_hops: 5,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: crate::config::GlobeConfig = serde_json::from_str(&json).unwrap();
        assert!(loaded.forwarding.enabled);
        assert_eq!(loaded.forwarding.default_hops, 2);
        assert_eq!(loaded.forwarding.max_hops, 5);
    }
}
