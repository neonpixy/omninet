//! Anonymous subscriptions — subscribe without revealing Crown identity.
//!
//! Clients can subscribe to relay events using ephemeral sessions instead of
//! their Crown keypair. This prevents the relay from correlating subscriptions
//! to a specific identity.
//!
//! # How it works
//!
//! 1. Client creates an [`EphemeralSession`] with a random session ID.
//! 2. Instead of signing an auth challenge with their Crown key, the client
//!    responds with an [`AnonymousAuthResponse`] containing a hash of the
//!    session ID + challenge + relay URL.
//! 3. The relay can verify the response is consistent without learning who
//!    the subscriber is.
//! 4. When serving anonymous subscriptions, the relay uses
//!    [`strip_author_from_event`] to remove author information from response
//!    events, preventing identity leakage.
//!
//! # Example
//!
//! ```
//! use globe::privacy::anonymous::{
//!     AnonymousConfig, EphemeralSession, create_ephemeral_session,
//!     create_anonymous_auth, AnonymousFilter,
//! };
//! use globe::OmniFilter;
//!
//! // Create an ephemeral session.
//! let session = create_ephemeral_session();
//! assert_eq!(session.session_id.len(), 64); // 32 bytes hex-encoded
//!
//! // Respond to an auth challenge anonymously.
//! let response = create_anonymous_auth("challenge123", "wss://relay.omnidea.co", &session).unwrap();
//! assert_eq!(response.session_id, session.session_id);
//!
//! // Wrap a filter for anonymous subscription.
//! let filter = OmniFilter {
//!     kinds: Some(vec![1]),
//!     ..Default::default()
//! };
//! let anon_filter = AnonymousFilter::new(filter, true);
//! assert!(anon_filter.anonymous);
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::filter::OmniFilter;

// ---------------------------------------------------------------------------
// AnonymousConfig
// ---------------------------------------------------------------------------

/// Configuration for anonymous subscriptions.
///
/// When enabled, clients can subscribe to relay events using ephemeral
/// session keys instead of their Crown identity. The `rotate_session_key`
/// flag controls whether a new ephemeral key is generated per connection
/// (recommended for maximum unlinkability).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnonymousConfig {
    /// Whether anonymous subscriptions are enabled.
    pub enabled: bool,
    /// Whether to rotate the ephemeral session key on each new connection.
    /// Defaults to `true` for maximum unlinkability.
    pub rotate_session_key: bool,
}

impl Default for AnonymousConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rotate_session_key: true,
        }
    }
}

// ---------------------------------------------------------------------------
// EphemeralSession
// ---------------------------------------------------------------------------

/// An ephemeral session for anonymous subscriptions.
///
/// Contains a random session ID and creation timestamp. The session ID is
/// a 32-byte random value, hex-encoded to 64 characters. It is never
/// linked to a Crown identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EphemeralSession {
    /// Random 32-byte session identifier, hex-encoded (64 chars).
    pub session_id: String,
    /// Unix timestamp (seconds) when this session was created.
    pub created_at: i64,
}

/// Create a new ephemeral session with a random session ID.
///
/// Generates 32 random bytes, hex-encodes them, and sets `created_at`
/// to the current unix timestamp.
#[must_use]
pub fn create_ephemeral_session() -> EphemeralSession {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);

    EphemeralSession {
        session_id: hex::encode(bytes),
        created_at: chrono::Utc::now().timestamp(),
    }
}

// ---------------------------------------------------------------------------
// AnonymousAuthResponse
// ---------------------------------------------------------------------------

/// An anonymous authentication response.
///
/// Instead of signing a challenge with a Crown key, the client produces
/// a SHA-256 hash of (session_id || challenge || relay_url). This proves
/// the client holds the session without revealing any identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnonymousAuthResponse {
    /// The ephemeral session ID.
    pub session_id: String,
    /// The challenge string from the relay.
    pub challenge: String,
    /// The relay URL.
    pub relay_url: String,
    /// SHA-256 hash of (session_id + challenge + relay_url), hex-encoded.
    pub response_hash: String,
}

/// Create an anonymous auth response for a relay challenge.
///
/// Computes `SHA-256(session_id || challenge || relay_url)` as the
/// response hash, proving the client holds the ephemeral session
/// without revealing their Crown identity.
///
/// # Errors
///
/// Returns [`GlobeError::AuthFailed`] if the session ID is empty.
pub fn create_anonymous_auth(
    challenge: &str,
    relay_url: &str,
    session: &EphemeralSession,
) -> Result<AnonymousAuthResponse, GlobeError> {
    if session.session_id.is_empty() {
        return Err(GlobeError::AuthFailed(
            "ephemeral session has empty session_id".into(),
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(session.session_id.as_bytes());
    hasher.update(challenge.as_bytes());
    hasher.update(relay_url.as_bytes());
    let hash = hex::encode(hasher.finalize());

    Ok(AnonymousAuthResponse {
        session_id: session.session_id.clone(),
        challenge: challenge.to_string(),
        relay_url: relay_url.to_string(),
        response_hash: hash,
    })
}

// ---------------------------------------------------------------------------
// strip_author_from_event
// ---------------------------------------------------------------------------

/// Strip the author field from an event JSON string.
///
/// Parses the JSON, replaces the `"author"` field value with an empty string,
/// and re-serializes. This is used server-side when serving events to
/// anonymous subscribers — the relay removes identity information before
/// delivering the event.
///
/// Other fields (id, kind, tags, content, sig, created_at) are preserved.
///
/// # Errors
///
/// Returns [`GlobeError::Serialization`] if the JSON is malformed.
pub fn strip_author_from_event(event_json: &str) -> Result<String, GlobeError> {
    let mut value: serde_json::Value = serde_json::from_str(event_json)?;

    if let Some(obj) = value.as_object_mut() {
        if obj.contains_key("author") {
            obj.insert(
                "author".to_string(),
                serde_json::Value::String(String::new()),
            );
        }
    }

    Ok(serde_json::to_string(&value)?)
}

// ---------------------------------------------------------------------------
// AnonymousFilter
// ---------------------------------------------------------------------------

/// A subscription filter with anonymous mode support.
///
/// Wraps an [`OmniFilter`] and adds an `anonymous` flag. When anonymous,
/// [`matches_anonymous`](AnonymousFilter::matches_anonymous) skips the
/// author check, matching only by kind, tags, timestamps, and IDs.
#[derive(Clone, Debug)]
pub struct AnonymousFilter {
    /// The underlying subscription filter.
    pub filter: OmniFilter,
    /// Whether this subscription is anonymous.
    pub anonymous: bool,
}

impl AnonymousFilter {
    /// Create a new anonymous filter.
    #[must_use]
    pub fn new(filter: OmniFilter, anonymous: bool) -> Self {
        Self { filter, anonymous }
    }

    /// Check whether an event matches this filter with anonymous semantics.
    ///
    /// When `anonymous` is `true`, the author field on the filter is ignored
    /// (even if set), and matching is done by kind, tags, timestamps, and IDs
    /// only. When `anonymous` is `false`, this delegates to the standard
    /// [`OmniFilter::matches`].
    pub fn matches_anonymous(&self, event: &OmniEvent) -> bool {
        if !self.anonymous {
            return self.filter.matches(event);
        }

        // Anonymous mode: match everything except author.
        // We re-implement the filter logic, skipping the author check.

        // IDs.
        if let Some(ids) = &self.filter.ids {
            if !ids.iter().any(|id| id == &event.id) {
                return false;
            }
        }

        // Kinds.
        if let Some(kinds) = &self.filter.kinds {
            if !kinds.contains(&event.kind) {
                return false;
            }
        }

        // Since.
        if let Some(since) = self.filter.since {
            if event.created_at < since {
                return false;
            }
        }

        // Until.
        if let Some(until) = self.filter.until {
            if event.created_at > until {
                return false;
            }
        }

        // Tag filters.
        for (tag_name, filter_values) in &self.filter.tag_filters {
            let tag_str = tag_name.to_string();
            let event_values = event.tag_values(&tag_str);
            if !filter_values
                .iter()
                .any(|fv| event_values.contains(&fv.as_str()))
            {
                return false;
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_event(kind: u32, author: &str, tags: Vec<Vec<String>>) -> OmniEvent {
        OmniEvent {
            id: "a".repeat(64),
            author: author.to_string(),
            created_at: Utc::now().timestamp(),
            kind,
            tags,
            content: String::new(),
            sig: "c".repeat(128),
        }
    }

    // -- EphemeralSession tests --

    #[test]
    fn ephemeral_session_produces_valid_session_id() {
        let session = create_ephemeral_session();
        assert_eq!(session.session_id.len(), 64);
        assert!(session.session_id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn ephemeral_session_ids_are_unique() {
        let s1 = create_ephemeral_session();
        let s2 = create_ephemeral_session();
        assert_ne!(s1.session_id, s2.session_id);
    }

    #[test]
    fn ephemeral_session_has_reasonable_timestamp() {
        let before = Utc::now().timestamp();
        let session = create_ephemeral_session();
        let after = Utc::now().timestamp();
        assert!(session.created_at >= before);
        assert!(session.created_at <= after);
    }

    // -- AnonymousAuthResponse tests --

    #[test]
    fn anonymous_auth_hash_is_deterministic() {
        let session = EphemeralSession {
            session_id: "ab".repeat(32),
            created_at: 1000,
        };
        let r1 = create_anonymous_auth("challenge", "wss://relay.co", &session).unwrap();
        let r2 = create_anonymous_auth("challenge", "wss://relay.co", &session).unwrap();
        assert_eq!(r1.response_hash, r2.response_hash);
    }

    #[test]
    fn anonymous_auth_different_challenge_different_hash() {
        let session = EphemeralSession {
            session_id: "ab".repeat(32),
            created_at: 1000,
        };
        let r1 = create_anonymous_auth("challenge_a", "wss://relay.co", &session).unwrap();
        let r2 = create_anonymous_auth("challenge_b", "wss://relay.co", &session).unwrap();
        assert_ne!(r1.response_hash, r2.response_hash);
    }

    #[test]
    fn anonymous_auth_different_relay_different_hash() {
        let session = EphemeralSession {
            session_id: "ab".repeat(32),
            created_at: 1000,
        };
        let r1 = create_anonymous_auth("challenge", "wss://relay-a.co", &session).unwrap();
        let r2 = create_anonymous_auth("challenge", "wss://relay-b.co", &session).unwrap();
        assert_ne!(r1.response_hash, r2.response_hash);
    }

    #[test]
    fn anonymous_auth_different_session_different_hash() {
        let s1 = EphemeralSession {
            session_id: "aa".repeat(32),
            created_at: 1000,
        };
        let s2 = EphemeralSession {
            session_id: "bb".repeat(32),
            created_at: 1000,
        };
        let r1 = create_anonymous_auth("challenge", "wss://relay.co", &s1).unwrap();
        let r2 = create_anonymous_auth("challenge", "wss://relay.co", &s2).unwrap();
        assert_ne!(r1.response_hash, r2.response_hash);
    }

    #[test]
    fn anonymous_auth_preserves_fields() {
        let session = EphemeralSession {
            session_id: "ab".repeat(32),
            created_at: 1000,
        };
        let response =
            create_anonymous_auth("my_challenge", "wss://relay.omnidea.co", &session).unwrap();
        assert_eq!(response.session_id, session.session_id);
        assert_eq!(response.challenge, "my_challenge");
        assert_eq!(response.relay_url, "wss://relay.omnidea.co");
        assert_eq!(response.response_hash.len(), 64); // SHA-256 hex
    }

    #[test]
    fn anonymous_auth_empty_session_fails() {
        let session = EphemeralSession {
            session_id: String::new(),
            created_at: 1000,
        };
        let result = create_anonymous_auth("challenge", "wss://relay.co", &session);
        assert!(result.is_err());
    }

    // -- strip_author_from_event tests --

    #[test]
    fn strip_author_replaces_author_field() {
        let event = make_event(1, &"b".repeat(64), vec![]);
        let json = serde_json::to_string(&event).unwrap();
        let stripped = strip_author_from_event(&json).unwrap();
        let parsed: OmniEvent = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed.author, "");
    }

    #[test]
    fn strip_author_preserves_other_fields() {
        let mut event = make_event(42, &"b".repeat(64), vec![vec!["t".into(), "test".into()]]);
        event.content = "hello world".to_string();
        let json = serde_json::to_string(&event).unwrap();
        let stripped = strip_author_from_event(&json).unwrap();
        let parsed: OmniEvent = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed.kind, 42);
        assert_eq!(parsed.content, "hello world");
        assert_eq!(parsed.id, event.id);
        assert_eq!(parsed.sig, event.sig);
        assert_eq!(parsed.tags, event.tags);
        assert_eq!(parsed.created_at, event.created_at);
    }

    #[test]
    fn strip_author_invalid_json_fails() {
        let result = strip_author_from_event("not json");
        assert!(result.is_err());
    }

    #[test]
    fn strip_author_no_author_field_is_noop() {
        let json = r#"{"id": "test", "kind": 1}"#;
        let stripped = strip_author_from_event(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        // No author field was present, so none should be added.
        assert!(!parsed.as_object().unwrap().contains_key("author"));
    }

    // -- AnonymousFilter tests --

    #[test]
    fn anonymous_filter_matches_by_kind_when_anonymous() {
        let filter = OmniFilter {
            kinds: Some(vec![1, 42]),
            authors: Some(vec!["someone_else".to_string()]),
            ..Default::default()
        };
        let anon = AnonymousFilter::new(filter, true);
        let event = make_event(1, &"b".repeat(64), vec![]);
        // Should match: anonymous mode skips author check.
        assert!(anon.matches_anonymous(&event));
    }

    #[test]
    fn anonymous_filter_skips_author_check_when_anonymous() {
        let filter = OmniFilter {
            authors: Some(vec!["specific_author".to_string()]),
            ..Default::default()
        };
        let anon = AnonymousFilter::new(filter, true);
        let event = make_event(1, &"b".repeat(64), vec![]);
        // Author doesn't match, but anonymous mode skips author check.
        assert!(anon.matches_anonymous(&event));
    }

    #[test]
    fn anonymous_filter_checks_author_when_not_anonymous() {
        let filter = OmniFilter {
            authors: Some(vec!["specific_author".to_string()]),
            ..Default::default()
        };
        let anon = AnonymousFilter::new(filter, false);
        let event = make_event(1, &"b".repeat(64), vec![]);
        // Not anonymous: author check applies and fails.
        assert!(!anon.matches_anonymous(&event));
    }

    #[test]
    fn anonymous_filter_still_checks_kind_when_anonymous() {
        let filter = OmniFilter {
            kinds: Some(vec![42]),
            ..Default::default()
        };
        let anon = AnonymousFilter::new(filter, true);
        let event = make_event(1, &"b".repeat(64), vec![]);
        // Kind 1 doesn't match filter for kind 42.
        assert!(!anon.matches_anonymous(&event));
    }

    #[test]
    fn anonymous_filter_checks_tags_when_anonymous() {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('t', vec!["rust".to_string()]);
        let filter = OmniFilter {
            tag_filters,
            ..Default::default()
        };
        let anon = AnonymousFilter::new(filter, true);

        let matching = make_event(1, &"b".repeat(64), vec![vec!["t".into(), "rust".into()]]);
        assert!(anon.matches_anonymous(&matching));

        let not_matching = make_event(1, &"b".repeat(64), vec![vec!["t".into(), "swift".into()]]);
        assert!(!anon.matches_anonymous(&not_matching));
    }

    #[test]
    fn anonymous_filter_checks_since_until_when_anonymous() {
        let now = Utc::now().timestamp();
        let filter = OmniFilter {
            since: Some(now + 1000),
            ..Default::default()
        };
        let anon = AnonymousFilter::new(filter, true);
        let event = make_event(1, &"b".repeat(64), vec![]);
        // Event is before since — should not match.
        assert!(!anon.matches_anonymous(&event));
    }

    // -- AnonymousConfig tests --

    #[test]
    fn anonymous_config_defaults() {
        let config = AnonymousConfig::default();
        assert!(!config.enabled);
        assert!(config.rotate_session_key);
    }

    // -- Serde roundtrip tests --

    #[test]
    fn anonymous_config_serde_roundtrip() {
        let config = AnonymousConfig {
            enabled: true,
            rotate_session_key: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: AnonymousConfig = serde_json::from_str(&json).unwrap();
        assert!(loaded.enabled);
        assert!(!loaded.rotate_session_key);
    }

    #[test]
    fn ephemeral_session_serde_roundtrip() {
        let session = create_ephemeral_session();
        let json = serde_json::to_string(&session).unwrap();
        let loaded: EphemeralSession = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, session.session_id);
        assert_eq!(loaded.created_at, session.created_at);
    }

    #[test]
    fn anonymous_auth_response_serde_roundtrip() {
        let session = EphemeralSession {
            session_id: "ab".repeat(32),
            created_at: 1000,
        };
        let response = create_anonymous_auth("ch", "wss://r.co", &session).unwrap();
        let json = serde_json::to_string(&response).unwrap();
        let loaded: AnonymousAuthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, response.session_id);
        assert_eq!(loaded.challenge, response.challenge);
        assert_eq!(loaded.relay_url, response.relay_url);
        assert_eq!(loaded.response_hash, response.response_hash);
    }

    // -- GlobeConfig backward compatibility --

    #[test]
    fn globe_config_without_anonymous_field_deserializes() {
        // Simulate a GlobeConfig JSON from before the anonymous field existed.
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
        assert!(!config.anonymous.enabled);
        assert!(config.anonymous.rotate_session_key);
    }
}
