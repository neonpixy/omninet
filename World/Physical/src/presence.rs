//! Ephemeral nearby presence signals.
//!
//! Presence is the most privacy-sensitive feature in World/Physical.
//! `PresenceSignal` is intentionally **not** Serialize or Deserialize --
//! it must never be persisted to disk, logged, or transmitted over the
//! network in a durable format. It exists only in memory, only briefly,
//! and only with the participant's active, revocable consent.

use chrono::{DateTime, Duration, Utc};

use crate::error::PhysicalError;

// ---------------------------------------------------------------------------
// MARK: - Types
// ---------------------------------------------------------------------------

/// An ephemeral signal indicating a person's nearby presence.
///
/// # Privacy invariant
///
/// `PresenceSignal` does **not** implement `Serialize` or `Deserialize`.
/// This is a deliberate architectural constraint -- presence data must never
/// be persisted, logged, or stored in any durable format. If you need to
/// transmit presence over the network, use a purpose-built ephemeral
/// transport that does not retain the data.
///
/// Adding `Serialize` / `Deserialize` to this type requires a Covenant
/// review (Sovereignty + Consent).
#[derive(Debug, Clone, PartialEq)]
pub struct PresenceSignal {
    /// The person emitting this signal (crown_id).
    pub person: String,
    pub status: PresenceStatus,
    pub proximity: ProximityLevel,
    pub message: Option<String>,
    pub visible_to: PresenceAudience,
    pub emitted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Current availability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresenceStatus {
    Available,
    Busy,
    Away,
}

/// How close the person is, in rough bands.
///
/// These are intentionally imprecise -- sovereignty means never giving
/// anyone a precise fix on your location without explicit consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProximityLevel {
    /// Same general area (~10 km).
    SameArea,
    /// Nearby (~1 km).
    Nearby,
    /// Right here (~100 m).
    Here,
}

/// Who can see this presence signal.
#[derive(Debug, Clone, PartialEq)]
pub enum PresenceAudience {
    /// Ghost mode. Nobody sees you.
    Nobody,
    /// Only these specific crown IDs.
    Selected(Vec<String>),
    /// Everyone in a given community (community ID).
    Community(String),
    /// All trusted contacts (as determined by the local trust graph).
    Trusted,
}

/// Configuration for presence signal creation and extension.
#[derive(Debug, Clone, PartialEq)]
pub struct PresenceConfig {
    /// Default time-to-live in seconds for new signals.
    pub default_ttl_seconds: u64,
    /// Hard ceiling for any signal's TTL.
    pub max_ttl_seconds: u64,
    /// Default audience for new signals.
    pub default_audience: PresenceAudience,
    /// Default proximity band for new signals.
    pub default_proximity: ProximityLevel,
}

// ---------------------------------------------------------------------------
// MARK: - PresenceConfig
// ---------------------------------------------------------------------------

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: 300,   // 5 minutes
            max_ttl_seconds: 3600,      // 1 hour
            default_audience: PresenceAudience::Nobody,
            default_proximity: ProximityLevel::SameArea,
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - PresenceSignal
// ---------------------------------------------------------------------------

impl PresenceSignal {
    /// Create a new presence signal in ghost mode (audience = Nobody).
    ///
    /// Uses the config's default TTL, proximity, and audience.
    pub fn new(person: &str, config: &PresenceConfig) -> Self {
        let now = Utc::now();
        Self {
            person: person.to_string(),
            status: PresenceStatus::Available,
            proximity: config.default_proximity,
            message: None,
            visible_to: config.default_audience.clone(),
            emitted_at: now,
            expires_at: now + Duration::seconds(config.default_ttl_seconds as i64),
        }
    }

    // -- Builder methods --

    /// Set the audience (make visible or restrict).
    pub fn make_visible(mut self, audience: PresenceAudience) -> Self {
        self.visible_to = audience;
        self
    }

    /// Set the availability status.
    pub fn with_status(mut self, status: PresenceStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the proximity level.
    pub fn with_proximity(mut self, level: ProximityLevel) -> Self {
        self.proximity = level;
        self
    }

    /// Attach an optional message.
    pub fn with_message(mut self, text: impl Into<String>) -> Self {
        self.message = Some(text.into());
        self
    }

    // -----------------------------------------------------------------------
    // MARK: - Queries
    // -----------------------------------------------------------------------

    /// Whether this signal has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Whether this signal is visible to anyone (audience is not `Nobody`).
    pub fn is_visible(&self) -> bool {
        !matches!(self.visible_to, PresenceAudience::Nobody)
    }

    /// Whether a specific viewer can see this signal.
    ///
    /// - `Nobody` -- no one sees it.
    /// - `Selected(list)` -- only if `viewer` is in `list`.
    /// - `Community(id)` -- only if `viewer` belongs to that community
    ///   (determined by `community_memberships`).
    /// - `Trusted` -- always returns true (the caller is expected to
    ///   pre-filter to trusted contacts before calling this).
    pub fn is_visible_to(&self, viewer: &str, community_memberships: &[String]) -> bool {
        match &self.visible_to {
            PresenceAudience::Nobody => false,
            PresenceAudience::Selected(crown_ids) => crown_ids.iter().any(|n| n == viewer),
            PresenceAudience::Community(id) => community_memberships.contains(id),
            PresenceAudience::Trusted => true,
        }
    }

    // -----------------------------------------------------------------------
    // MARK: - Mutations
    // -----------------------------------------------------------------------

    /// Extend the signal's lifetime by `additional_seconds`.
    ///
    /// The total remaining TTL cannot exceed `config.max_ttl_seconds` from now.
    pub fn extend(
        &mut self,
        additional_seconds: u64,
        config: &PresenceConfig,
    ) -> Result<(), PhysicalError> {
        let now = Utc::now();
        let new_expires = self.expires_at + Duration::seconds(additional_seconds as i64);
        let max_allowed = now + Duration::seconds(config.max_ttl_seconds as i64);

        if new_expires > max_allowed {
            return Err(PhysicalError::TtlExceedsMaximum {
                requested: (new_expires - now).num_seconds() as u64,
                maximum: config.max_ttl_seconds,
            });
        }

        self.expires_at = new_expires;
        Ok(())
    }

    /// Immediately become invisible. Sets audience to `Nobody`.
    pub fn go_ghost(&mut self) {
        self.visible_to = PresenceAudience::Nobody;
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> PresenceConfig {
        PresenceConfig::default()
    }

    #[test]
    fn config_defaults() {
        let cfg = default_config();
        assert_eq!(cfg.default_ttl_seconds, 300);
        assert_eq!(cfg.max_ttl_seconds, 3600);
        assert_eq!(cfg.default_audience, PresenceAudience::Nobody);
        assert_eq!(cfg.default_proximity, ProximityLevel::SameArea);
    }

    #[test]
    fn new_signal_is_ghost() {
        let cfg = default_config();
        let sig = PresenceSignal::new("cpub_alice", &cfg);
        assert_eq!(sig.person, "cpub_alice");
        assert_eq!(sig.status, PresenceStatus::Available);
        assert_eq!(sig.proximity, ProximityLevel::SameArea);
        assert_eq!(sig.visible_to, PresenceAudience::Nobody);
        assert!(sig.message.is_none());
        assert!(!sig.is_visible());
    }

    #[test]
    fn builder_methods() {
        let cfg = default_config();
        let sig = PresenceSignal::new("cpub_alice", &cfg)
            .with_status(PresenceStatus::Busy)
            .with_proximity(ProximityLevel::Here)
            .with_message("At the park")
            .make_visible(PresenceAudience::Trusted);

        assert_eq!(sig.status, PresenceStatus::Busy);
        assert_eq!(sig.proximity, ProximityLevel::Here);
        assert_eq!(sig.message.as_deref(), Some("At the park"));
        assert_eq!(sig.visible_to, PresenceAudience::Trusted);
        assert!(sig.is_visible());
    }

    #[test]
    fn not_expired_when_fresh() {
        let cfg = default_config();
        let sig = PresenceSignal::new("cpub_alice", &cfg);
        assert!(!sig.is_expired());
    }

    #[test]
    fn expired_when_past_ttl() {
        let cfg = default_config();
        let mut sig = PresenceSignal::new("cpub_alice", &cfg);
        // Force expiration into the past
        sig.expires_at = Utc::now() - Duration::seconds(1);
        assert!(sig.is_expired());
    }

    #[test]
    fn visibility_nobody() {
        let cfg = default_config();
        let sig = PresenceSignal::new("cpub_alice", &cfg);
        assert!(!sig.is_visible());
        assert!(!sig.is_visible_to("cpub_bob", &[]));
    }

    #[test]
    fn visibility_selected() {
        let cfg = default_config();
        let sig = PresenceSignal::new("cpub_alice", &cfg).make_visible(
            PresenceAudience::Selected(vec!["cpub_bob".into(), "cpub_charlie".into()]),
        );
        assert!(sig.is_visible());
        assert!(sig.is_visible_to("cpub_bob", &[]));
        assert!(sig.is_visible_to("cpub_charlie", &[]));
        assert!(!sig.is_visible_to("cpub_dave", &[]));
    }

    #[test]
    fn visibility_community() {
        let cfg = default_config();
        let sig = PresenceSignal::new("cpub_alice", &cfg)
            .make_visible(PresenceAudience::Community("denver-hackers".into()));
        assert!(sig.is_visible());

        // Bob is a member of denver-hackers
        assert!(sig.is_visible_to("cpub_bob", &["denver-hackers".into()]));
        // Dave is not
        assert!(!sig.is_visible_to("cpub_dave", &["rust-devs".into()]));
    }

    #[test]
    fn visibility_trusted() {
        let cfg = default_config();
        let sig =
            PresenceSignal::new("cpub_alice", &cfg).make_visible(PresenceAudience::Trusted);
        assert!(sig.is_visible());
        // Trusted is always visible (caller pre-filters)
        assert!(sig.is_visible_to("cpub_anyone", &[]));
    }

    #[test]
    fn go_ghost() {
        let cfg = default_config();
        let mut sig =
            PresenceSignal::new("cpub_alice", &cfg).make_visible(PresenceAudience::Trusted);
        assert!(sig.is_visible());
        sig.go_ghost();
        assert!(!sig.is_visible());
        assert_eq!(sig.visible_to, PresenceAudience::Nobody);
    }

    #[test]
    fn extend_within_limits() {
        let cfg = default_config();
        let mut sig = PresenceSignal::new("cpub_alice", &cfg);
        let old_expires = sig.expires_at;

        // Extend by 60 seconds (total ~360s, well under 3600s max)
        assert!(sig.extend(60, &cfg).is_ok());
        assert!(sig.expires_at > old_expires);
    }

    #[test]
    fn extend_exceeds_max_ttl() {
        let cfg = default_config();
        let mut sig = PresenceSignal::new("cpub_alice", &cfg);

        // Try to extend way past max (asking for 4000s more when max is 3600 from now)
        let err = sig.extend(4000, &cfg).unwrap_err();
        assert!(matches!(
            err,
            PhysicalError::TtlExceedsMaximum {
                maximum: 3600,
                ..
            }
        ));
    }

    #[test]
    fn custom_config() {
        let cfg = PresenceConfig {
            default_ttl_seconds: 60,
            max_ttl_seconds: 120,
            default_audience: PresenceAudience::Trusted,
            default_proximity: ProximityLevel::Nearby,
        };
        let sig = PresenceSignal::new("cpub_alice", &cfg);
        assert_eq!(sig.proximity, ProximityLevel::Nearby);
        assert_eq!(sig.visible_to, PresenceAudience::Trusted);
        assert!(sig.is_visible());

        // TTL should be ~60s
        let ttl = (sig.expires_at - sig.emitted_at).num_seconds();
        assert_eq!(ttl, 60);
    }

    #[test]
    fn presence_signal_is_not_serializable() {
        // This test documents the intentional absence of Serialize/Deserialize
        // on PresenceSignal. Presence data is ephemeral by design -- it must
        // never be persisted, logged, or transmitted durably.
        //
        // If someone adds #[derive(Serialize)] to PresenceSignal, this test
        // will still pass, but the doc comment on PresenceSignal and this
        // comment should trigger a code review discussion requiring a
        // Covenant review (Sovereignty + Consent).
        //
        // Rust does not support negative trait bounds, so we verify the
        // constraint through documentation and review culture rather than
        // the type system.
        let cfg = PresenceConfig::default();
        let sig = PresenceSignal::new("cpub_alice", &cfg);
        // Verify it has the traits it SHOULD have
        let _ = sig.clone();
        let _ = format!("{:?}", sig);
    }

    #[test]
    fn proximity_level_equality() {
        assert_eq!(ProximityLevel::SameArea, ProximityLevel::SameArea);
        assert_ne!(ProximityLevel::SameArea, ProximityLevel::Nearby);
        assert_ne!(ProximityLevel::Nearby, ProximityLevel::Here);
    }

    #[test]
    fn presence_status_equality() {
        assert_eq!(PresenceStatus::Available, PresenceStatus::Available);
        assert_ne!(PresenceStatus::Available, PresenceStatus::Busy);
        assert_ne!(PresenceStatus::Busy, PresenceStatus::Away);
    }
}
