//! Voluntary precise location sharing — "I'm exactly HERE, find me."
//!
//! A Lantern is a temporary, opt-in beacon. You light it when you want
//! someone to find you — at a meetup, for a delivery, or in an emergency.
//! The person sharing controls the audience, the purpose, and the duration.
//! When they're done, they extinguish it. Nothing persists unless explicitly
//! opted into Yoke recording.
//!
//! LanternSos is a special emergency variant that bypasses audience controls
//! and goes directly to designated emergency contacts.
//!
//! ## Covenant Alignment
//!
//! **Sovereignty** — you light your own lantern; nobody lights it for you.
//! **Consent** — audience controls are explicit and granular.
//! **Dignity** — SOS mode ensures safety access regardless of social graph.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::GeoCoordinate;

use crate::error::PhysicalError;

// ---------------------------------------------------------------------------
// MARK: - LanternAudience
// ---------------------------------------------------------------------------

/// Who can see this lantern share.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LanternAudience {
    /// Visible only to specific crown IDs.
    Selected(Vec<String>),
    /// Visible to members of a community (community_id).
    Community(String),
    /// Visible to all trusted connections.
    AllTrusted,
}

// ---------------------------------------------------------------------------
// MARK: - LanternPurpose
// ---------------------------------------------------------------------------

/// Why the lantern is lit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LanternPurpose {
    /// Turn-by-turn or "come to me" navigation.
    Navigation,
    /// Meeting up with friends.
    Meetup,
    /// Waiting for or making a delivery.
    Delivery,
    /// General safety — "know where I am."
    Safety,
    /// Free-form purpose.
    Custom(String),
}

// ---------------------------------------------------------------------------
// MARK: - LanternConfig
// ---------------------------------------------------------------------------

/// Configuration for lantern TTL limits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanternConfig {
    /// Default time-to-live in seconds (30 minutes).
    pub default_ttl_seconds: u64,
    /// Maximum allowed TTL in seconds (24 hours).
    pub max_ttl_seconds: u64,
}

impl Default for LanternConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: 1800,  // 30 minutes
            max_ttl_seconds: 86400,     // 24 hours
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - LanternShare
// ---------------------------------------------------------------------------

/// A voluntary, time-limited location beacon.
///
/// The person sharing controls everything: who sees it, why it exists,
/// and when it ends. Location updates are live — the struct is mutable
/// so the holder can push position updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanternShare {
    pub id: Uuid,
    pub person: String,
    pub location: GeoCoordinate,
    pub message: Option<String>,
    pub audience: LanternAudience,
    pub purpose: LanternPurpose,
    pub started_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub record_in_yoke: bool,
}

impl LanternShare {
    /// Light a new lantern. TTL comes from the config's default.
    pub fn new(
        person: impl Into<String>,
        location: GeoCoordinate,
        audience: LanternAudience,
        purpose: LanternPurpose,
        config: &LanternConfig,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            person: person.into(),
            location,
            message: None,
            audience,
            purpose,
            started_at: now,
            expires_at: now + Duration::seconds(config.default_ttl_seconds as i64),
            record_in_yoke: false,
        }
    }

    // -- Builder methods ----------------------------------------------------

    /// Attach a short message to the lantern (e.g. "I'm by the fountain").
    pub fn with_message(mut self, text: impl Into<String>) -> Self {
        self.message = Some(text.into());
        self
    }

    /// Opt in or out of recording this share in Yoke history.
    pub fn with_yoke_recording(mut self, record: bool) -> Self {
        self.record_in_yoke = record;
        self
    }

    // -- Live updates -------------------------------------------------------

    /// Update the lantern's position (e.g. from a GPS stream).
    pub fn update_location(&mut self, new_location: GeoCoordinate) {
        self.location = new_location;
    }

    // -- Queries ------------------------------------------------------------

    /// Whether this lantern has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Whether `viewer` is allowed to see this lantern.
    ///
    /// The person who lit it can always see it. Otherwise, audience rules
    /// apply: Selected checks the crown_id list, Community checks memberships,
    /// AllTrusted allows everyone (trust verification is the caller's job).
    pub fn is_visible_to(&self, viewer: &str, community_memberships: &[String]) -> bool {
        // The person always sees their own lantern.
        if self.person == viewer {
            return true;
        }

        match &self.audience {
            LanternAudience::Selected(crown_ids) => crown_ids.iter().any(|n| n == viewer),
            LanternAudience::Community(community_id) => {
                community_memberships.iter().any(|m| m == community_id)
            }
            LanternAudience::AllTrusted => true,
        }
    }

    // -- Lifecycle ----------------------------------------------------------

    /// Extinguish the lantern immediately (set expiry to now).
    pub fn extinguish(&mut self) {
        self.expires_at = Utc::now();
    }

    /// Extend the lantern's lifetime by `seconds`.
    ///
    /// The new expiry cannot exceed `max_ttl_seconds` from the original
    /// start time.
    pub fn extend(&mut self, seconds: u64, config: &LanternConfig) -> Result<(), PhysicalError> {
        let new_expires = self.expires_at + Duration::seconds(seconds as i64);
        let max_allowed = self.started_at + Duration::seconds(config.max_ttl_seconds as i64);

        if new_expires > max_allowed {
            return Err(PhysicalError::TtlExceedsMaximum {
                requested: (new_expires - self.started_at).num_seconds() as u64,
                maximum: config.max_ttl_seconds,
            });
        }

        self.expires_at = new_expires;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MARK: - LanternSos
// ---------------------------------------------------------------------------

/// Emergency location beacon.
///
/// Bypasses normal audience controls and goes directly to designated
/// emergency contacts. Active until explicitly resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanternSos {
    pub id: Uuid,
    pub person: String,
    pub location: GeoCoordinate,
    pub message: Option<String>,
    pub emergency_contacts: Vec<String>,
    pub activated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl LanternSos {
    /// Activate an SOS beacon.
    pub fn activate(
        person: impl Into<String>,
        location: GeoCoordinate,
        emergency_contacts: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            person: person.into(),
            location,
            message: None,
            emergency_contacts,
            activated_at: Utc::now(),
            resolved_at: None,
        }
    }

    /// Attach a message to the SOS (e.g. "car broke down on I-70").
    pub fn with_message(mut self, text: impl Into<String>) -> Self {
        self.message = Some(text.into());
        self
    }

    /// Update the SOS position (person is moving, or GPS refining).
    pub fn update_location(&mut self, location: GeoCoordinate) {
        self.location = location;
    }

    /// Mark the SOS as resolved.
    pub fn resolve(&mut self) {
        self.resolved_at = Some(Utc::now());
    }

    /// Whether the SOS is still active (not yet resolved).
    pub fn is_active(&self) -> bool {
        self.resolved_at.is_none()
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: a coordinate in Denver.
    fn denver() -> GeoCoordinate {
        GeoCoordinate::new(39.7392, -104.9903).unwrap()
    }

    /// Helper: a coordinate in Boulder.
    fn boulder() -> GeoCoordinate {
        GeoCoordinate::new(40.0150, -105.2705).unwrap()
    }

    fn default_config() -> LanternConfig {
        LanternConfig::default()
    }

    // -- LanternConfig ------------------------------------------------------

    #[test]
    fn config_defaults() {
        let config = LanternConfig::default();
        assert_eq!(config.default_ttl_seconds, 1800);
        assert_eq!(config.max_ttl_seconds, 86400);
    }

    // -- LanternShare construction ------------------------------------------

    #[test]
    fn new_lantern_has_uuid_and_timestamps() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Meetup,
            &config,
        );
        assert_eq!(lantern.id.get_version_num(), 4);
        assert!(lantern.expires_at > lantern.started_at);
        assert_eq!(lantern.person, "cpub1alice");
        assert!(lantern.message.is_none());
        assert!(!lantern.record_in_yoke);
    }

    #[test]
    fn new_lantern_ttl_from_config() {
        let config = LanternConfig {
            default_ttl_seconds: 600,
            max_ttl_seconds: 3600,
        };
        let lantern = LanternShare::new(
            "cpub1bob",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Navigation,
            &config,
        );
        let ttl = (lantern.expires_at - lantern.started_at).num_seconds();
        assert_eq!(ttl, 600);
    }

    // -- Builder methods ----------------------------------------------------

    #[test]
    fn with_message_sets_text() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Meetup,
            &config,
        )
        .with_message("By the fountain");
        assert_eq!(lantern.message.as_deref(), Some("By the fountain"));
    }

    #[test]
    fn with_yoke_recording_enables_history() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Safety,
            &config,
        )
        .with_yoke_recording(true);
        assert!(lantern.record_in_yoke);
    }

    // -- Visibility ---------------------------------------------------------

    #[test]
    fn person_always_sees_own_lantern() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::Selected(vec!["cpub1bob".into()]),
            LanternPurpose::Meetup,
            &config,
        );
        assert!(lantern.is_visible_to("cpub1alice", &[]));
    }

    #[test]
    fn selected_audience_filters_correctly() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::Selected(vec!["cpub1bob".into(), "cpub1carol".into()]),
            LanternPurpose::Meetup,
            &config,
        );
        assert!(lantern.is_visible_to("cpub1bob", &[]));
        assert!(lantern.is_visible_to("cpub1carol", &[]));
        assert!(!lantern.is_visible_to("cpub1eve", &[]));
    }

    #[test]
    fn community_audience_checks_memberships() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::Community("denver-devs".into()),
            LanternPurpose::Meetup,
            &config,
        );
        assert!(lantern.is_visible_to("cpub1bob", &["denver-devs".into()]));
        assert!(!lantern.is_visible_to("cpub1bob", &["boulder-hikers".into()]));
    }

    #[test]
    fn all_trusted_visible_to_everyone() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Navigation,
            &config,
        );
        assert!(lantern.is_visible_to("cpub1anyone", &[]));
    }

    // -- Lifecycle ----------------------------------------------------------

    #[test]
    fn update_location_changes_position() {
        let config = default_config();
        let mut lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Delivery,
            &config,
        );
        lantern.update_location(boulder());
        assert_eq!(lantern.location, boulder());
    }

    #[test]
    fn extinguish_sets_expiry_to_now() {
        let config = default_config();
        let mut lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Meetup,
            &config,
        );
        assert!(!lantern.is_expired());
        lantern.extinguish();
        assert!(lantern.is_expired());
    }

    #[test]
    fn extend_within_max_succeeds() {
        let config = LanternConfig {
            default_ttl_seconds: 600,
            max_ttl_seconds: 3600,
        };
        let mut lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Meetup,
            &config,
        );
        let result = lantern.extend(600, &config);
        assert!(result.is_ok());
        let total_ttl = (lantern.expires_at - lantern.started_at).num_seconds();
        assert_eq!(total_ttl, 1200);
    }

    #[test]
    fn extend_beyond_max_fails() {
        let config = LanternConfig {
            default_ttl_seconds: 600,
            max_ttl_seconds: 900,
        };
        let mut lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::AllTrusted,
            LanternPurpose::Meetup,
            &config,
        );
        let result = lantern.extend(600, &config);
        assert!(matches!(
            result,
            Err(PhysicalError::TtlExceedsMaximum { .. })
        ));
    }

    // -- LanternSos ---------------------------------------------------------

    #[test]
    fn sos_activate_creates_active_beacon() {
        let sos = LanternSos::activate(
            "cpub1alice",
            denver(),
            vec!["cpub1bob".into(), "cpub1carol".into()],
        );
        assert!(sos.is_active());
        assert_eq!(sos.person, "cpub1alice");
        assert_eq!(sos.emergency_contacts.len(), 2);
        assert!(sos.resolved_at.is_none());
    }

    #[test]
    fn sos_resolve_marks_inactive() {
        let mut sos = LanternSos::activate(
            "cpub1alice",
            denver(),
            vec!["cpub1bob".into()],
        );
        assert!(sos.is_active());
        sos.resolve();
        assert!(!sos.is_active());
        assert!(sos.resolved_at.is_some());
    }

    #[test]
    fn sos_update_location() {
        let mut sos = LanternSos::activate(
            "cpub1alice",
            denver(),
            vec!["cpub1bob".into()],
        );
        sos.update_location(boulder());
        assert_eq!(sos.location, boulder());
    }

    #[test]
    fn sos_with_message() {
        let sos = LanternSos::activate(
            "cpub1alice",
            denver(),
            vec!["cpub1bob".into()],
        )
        .with_message("Car broke down on I-70");
        assert_eq!(sos.message.as_deref(), Some("Car broke down on I-70"));
    }

    // -- Serde round-trip ---------------------------------------------------

    #[test]
    fn lantern_share_serde_round_trip() {
        let config = default_config();
        let lantern = LanternShare::new(
            "cpub1alice",
            denver(),
            LanternAudience::Selected(vec!["cpub1bob".into()]),
            LanternPurpose::Custom("block party".into()),
            &config,
        )
        .with_message("Find me!")
        .with_yoke_recording(true);

        let json = serde_json::to_string(&lantern).unwrap();
        let parsed: LanternShare = serde_json::from_str(&json).unwrap();
        assert_eq!(lantern, parsed);
    }

    #[test]
    fn lantern_sos_serde_round_trip() {
        let sos = LanternSos::activate(
            "cpub1alice",
            denver(),
            vec!["cpub1bob".into()],
        )
        .with_message("Help!");

        let json = serde_json::to_string(&sos).unwrap();
        let parsed: LanternSos = serde_json::from_str(&json).unwrap();
        assert_eq!(sos, parsed);
    }
}
