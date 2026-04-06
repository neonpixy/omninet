use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// A signed, content-addressed event in the Omnidea Relay Protocol.
///
/// Every event has 7 fields:
/// - `id`: SHA-256 of the canonical serialization (64 hex chars)
/// - `author`: signer's x-only public key (64 hex chars)
/// - `created_at`: unix timestamp in seconds
/// - `kind`: event type (determines which ABC module handles it)
/// - `tags`: extensible metadata as arrays of strings
/// - `content`: the payload (text, JSON, or empty)
/// - `sig`: BIP-340 Schnorr signature (128 hex chars)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OmniEvent {
    pub id: String,
    pub author: String,
    pub created_at: i64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

impl OmniEvent {
    // -- Tag accessors --

    /// All values for a given tag name.
    ///
    /// For a tag `["e", "abc", "wss://relay"]`, `tag_values("e")` returns `["abc", "wss://relay"]`.
    pub fn tag_values(&self, name: &str) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|t| t.first().is_some_and(|n| n == name))
            .flat_map(|t| t.iter().skip(1).map(|s| s.as_str()))
            .collect()
    }

    /// First value for a given tag name, if any.
    pub fn tag_value(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|t| t.first().is_some_and(|n| n == name))
            .and_then(|t| t.get(1))
            .map(|s| s.as_str())
    }

    /// Whether a tag with the given name and value exists.
    pub fn has_tag(&self, name: &str, value: &str) -> bool {
        self.tags
            .iter()
            .any(|t| t.first().is_some_and(|n| n == name) && t.get(1).is_some_and(|v| v == value))
    }

    /// The `d` tag value (used for parameterized replaceable events).
    pub fn d_tag(&self) -> Option<&str> {
        self.tag_value("d")
    }

    /// All pubkey references (`p` tags).
    pub fn p_tags(&self) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|t| t.first().is_some_and(|n| n == "p"))
            .filter_map(|t| t.get(1).map(|s| s.as_str()))
            .collect()
    }

    /// All event references (`e` tags).
    pub fn e_tags(&self) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|t| t.first().is_some_and(|n| n == "e"))
            .filter_map(|t| t.get(1).map(|s| s.as_str()))
            .collect()
    }

    /// The `application` tag value.
    pub fn application_tag(&self) -> Option<&str> {
        self.tag_value("application")
    }

    // -- Convenience --

    /// Created timestamp as a `DateTime<Utc>`.
    pub fn created_date(&self) -> Option<DateTime<Utc>> {
        Utc.timestamp_opt(self.created_at, 0).single()
    }

    // -- Validation --

    /// Validate the structural format of this event.
    ///
    /// Returns `Ok(())` if valid, or a list of all validation errors.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.id.len() != 64 || !self.id.chars().all(|c| c.is_ascii_hexdigit()) {
            errors.push(format!("id must be 64 hex chars, got {}", self.id.len()));
        }

        if self.author.len() != 64 || !self.author.chars().all(|c| c.is_ascii_hexdigit()) {
            errors.push(format!(
                "author must be 64 hex chars, got {}",
                self.author.len()
            ));
        }

        if self.sig.len() != 128 || !self.sig.chars().all(|c| c.is_ascii_hexdigit()) {
            errors.push(format!("sig must be 128 hex chars, got {}", self.sig.len()));
        }

        let now = Utc::now().timestamp();
        if self.created_at > now + 3600 {
            errors.push("created_at is more than 1 hour in the future".into());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_event() -> OmniEvent {
        OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: 1,
            tags: vec![
                vec!["e".into(), "event123".into()],
                vec!["p".into(), "pubkey456".into()],
                vec!["d".into(), "sam.com".into()],
                vec!["application".into(), "omnidea".into()],
            ],
            content: "hello world".into(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn tag_values_returns_all_values() {
        let event = test_event();
        assert_eq!(event.tag_values("e"), vec!["event123"]);
        assert_eq!(event.tag_values("p"), vec!["pubkey456"]);
    }

    #[test]
    fn tag_value_returns_first() {
        let event = test_event();
        assert_eq!(event.tag_value("e"), Some("event123"));
        assert_eq!(event.tag_value("nonexistent"), None);
    }

    #[test]
    fn has_tag_checks_name_and_value() {
        let event = test_event();
        assert!(event.has_tag("e", "event123"));
        assert!(!event.has_tag("e", "wrong"));
        assert!(!event.has_tag("x", "event123"));
    }

    #[test]
    fn d_tag_accessor() {
        let event = test_event();
        assert_eq!(event.d_tag(), Some("sam.com"));
    }

    #[test]
    fn p_tags_and_e_tags() {
        let event = test_event();
        assert_eq!(event.p_tags(), vec!["pubkey456"]);
        assert_eq!(event.e_tags(), vec!["event123"]);
    }

    #[test]
    fn application_tag_accessor() {
        let event = test_event();
        assert_eq!(event.application_tag(), Some("omnidea"));
    }

    #[test]
    fn created_date_conversion() {
        let event = test_event();
        let date = event.created_date();
        assert!(date.is_some());
    }

    #[test]
    fn validate_valid_event() {
        let event = test_event();
        assert!(event.validate().is_ok());
    }

    #[test]
    fn validate_bad_id_length() {
        let mut event = test_event();
        event.id = "short".into();
        let errors = event.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("id")));
    }

    #[test]
    fn validate_bad_sig_non_hex() {
        let mut event = test_event();
        event.sig = "g".repeat(128);
        let errors = event.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("sig")));
    }

    #[test]
    fn validate_future_timestamp() {
        let mut event = test_event();
        event.created_at = Utc::now().timestamp() + 7200;
        let errors = event.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("future")));
    }

    #[test]
    fn serde_round_trip() {
        let event = test_event();
        let json = serde_json::to_string(&event).unwrap();
        let loaded: OmniEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, loaded);
    }
}
