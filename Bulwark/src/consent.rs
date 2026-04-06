use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A consent record — voluntary, informed, continuous, revocable.
///
/// From Conjunction Art. 4 §1: "Consent is the lawful condition of any
/// relation between Persons. It must be voluntary, informed, continuous,
/// and revocable."
///
/// Bulwark's consent is scoped to safety operations (data sharing, health
/// monitoring, etc). Polity handles broader constitutional consent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsentRecord {
    pub id: Uuid,
    pub grantor: String,
    pub recipient: String,
    pub scope: ConsentScope,
    pub conditions: Vec<String>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl ConsentRecord {
    /// Create a new consent record from grantor to recipient for a specific scope.
    pub fn new(
        grantor: impl Into<String>,
        recipient: impl Into<String>,
        scope: ConsentScope,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            grantor: grantor.into(),
            recipient: recipient.into(),
            scope,
            conditions: Vec::new(),
            granted_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
        }
    }

    /// Attach conditions to this consent record.
    pub fn with_conditions(mut self, conditions: Vec<String>) -> Self {
        self.conditions = conditions;
        self
    }

    /// Set an expiry time for this consent.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Whether this consent is currently active (not revoked and not expired).
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
            && self
                .expires_at
                .is_none_or(|exp| Utc::now() < exp)
    }

    /// Revoke consent. Always available — "Consent must be revocable."
    pub fn revoke(&mut self) {
        self.revoked_at = Some(Utc::now());
    }
}

/// What the consent covers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConsentScope {
    /// Sharing health pulse data with a community.
    HealthDataSharing,
    /// Allowing parent oversight of a minor's activity.
    ParentOversight,
    /// Allowing a verification method to access device sensors.
    VerificationAccess,
    /// Sharing reputation data with a community.
    ReputationSharing,
    /// Allowing another person to see your trust layer.
    TrustLayerVisibility,
    /// General data sharing between persons.
    DataSharing,
}

/// Validates that consent exists for a given operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentValidator {
    pub records: Vec<ConsentRecord>,
}

impl ConsentValidator {
    /// Create a new consent validator with no records.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Record a consent grant.
    pub fn grant(&mut self, record: ConsentRecord) {
        self.records.push(record);
    }

    /// Check if active consent exists from grantor to recipient for a scope.
    pub fn has_consent(&self, grantor: &str, recipient: &str, scope: ConsentScope) -> bool {
        self.records.iter().any(|r| {
            r.grantor == grantor
                && r.recipient == recipient
                && r.scope == scope
                && r.is_active()
        })
    }

    /// Revoke all consent from grantor to recipient.
    pub fn revoke_all(&mut self, grantor: &str, recipient: &str) {
        for record in &mut self.records {
            if record.grantor == grantor && record.recipient == recipient {
                record.revoke();
            }
        }
    }

    /// Get all active consent records for a grantor.
    pub fn active_for(&self, grantor: &str) -> Vec<&ConsentRecord> {
        self.records
            .iter()
            .filter(|r| r.grantor == grantor && r.is_active())
            .collect()
    }
}

impl Default for ConsentValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_lifecycle() {
        let mut record = ConsentRecord::new("alice", "community_x", ConsentScope::HealthDataSharing);
        assert!(record.is_active());

        record.revoke();
        assert!(!record.is_active());
    }

    #[test]
    fn consent_with_expiry() {
        let record = ConsentRecord::new("alice", "bob", ConsentScope::DataSharing)
            .with_expiry(Utc::now() + chrono::Duration::days(30));
        assert!(record.is_active());

        let expired = ConsentRecord::new("alice", "bob", ConsentScope::DataSharing)
            .with_expiry(Utc::now() - chrono::Duration::days(1));
        assert!(!expired.is_active());
    }

    #[test]
    fn validator_checks() {
        let mut validator = ConsentValidator::new();
        validator.grant(ConsentRecord::new(
            "alice",
            "community_x",
            ConsentScope::HealthDataSharing,
        ));

        assert!(validator.has_consent("alice", "community_x", ConsentScope::HealthDataSharing));
        assert!(!validator.has_consent("alice", "community_x", ConsentScope::DataSharing));
        assert!(!validator.has_consent("bob", "community_x", ConsentScope::HealthDataSharing));
    }

    #[test]
    fn revoke_all() {
        let mut validator = ConsentValidator::new();
        validator.grant(ConsentRecord::new("alice", "bob", ConsentScope::DataSharing));
        validator.grant(ConsentRecord::new(
            "alice",
            "bob",
            ConsentScope::HealthDataSharing,
        ));

        assert_eq!(validator.active_for("alice").len(), 2);
        validator.revoke_all("alice", "bob");
        assert_eq!(validator.active_for("alice").len(), 0);
    }
}
