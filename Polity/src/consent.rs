use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A record of consent — voluntary, informed, continuous, and revocable.
///
/// From Covenant Core Art. 2 Section 5: "Consent obtained through dependency,
/// necessity, or structural coercion shall be void."
///
/// Consent in Omnidea is not a checkbox. It is a living, continuous state
/// that can be withdrawn at any moment. Without consent, there is no lawful
/// relation — only domination.
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
    pub revocation_reason: Option<String>,
}

impl ConsentRecord {
    /// Create a new active consent record. Use builders to add expiry and conditions.
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
            revocation_reason: None,
        }
    }

    /// Set an expiry time for this consent. After this time, the consent is no longer active.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Add a condition that the consent is contingent upon (e.g., "Data must be anonymized").
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.conditions.push(condition.into());
        self
    }

    /// Whether this consent is currently active.
    pub fn is_active(&self) -> bool {
        if self.revoked_at.is_some() {
            return false;
        }
        if let Some(expires) = self.expires_at {
            return Utc::now() < expires;
        }
        true
    }

    /// Whether this consent has been revoked.
    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    /// Whether this consent has expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Utc::now() >= exp)
            .unwrap_or(false)
    }

    /// Revoke consent. Always succeeds — consent is always revocable.
    pub fn revoke(&mut self, reason: impl Into<String>) -> Result<(), crate::PolityError> {
        if self.revoked_at.is_some() {
            return Err(crate::PolityError::ConsentAlreadyRevoked(
                self.id.to_string(),
            ));
        }
        self.revoked_at = Some(Utc::now());
        self.revocation_reason = Some(reason.into());
        Ok(())
    }
}

/// What the consent covers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConsentScope {
    /// Consent to join a community
    CommunityMembership { community_id: String },
    /// Consent to a specific governance decision
    GovernanceDecision { proposal_id: String },
    /// Consent to an economic transaction
    EconomicTransaction { transaction_id: String },
    /// Consent to data sharing
    DataSharing { data_type: String, purpose: String },
    /// Consent to delegate authority
    Delegation { mandate_scope: String },
    /// General consent for a described purpose
    General { description: String },
    /// Consent to expose the device to the network (e.g., UPnP port mapping, Tower mode)
    ///
    /// From Covenant Core Art. 2: Sovereignty includes control over one's own
    /// device and network presence. Opening a port is a network configuration
    /// change that must be explicitly consented to.
    NetworkExposure {
        /// What is being exposed (e.g., "upnp_port_map", "tower_mode")
        exposure_type: String,
        /// The port or service being exposed
        description: String,
    },
    /// Consent to propose federation with another community.
    ///
    /// Federation proposals go out in your community's name.
    /// Members should consent to entering diplomatic proceedings.
    /// From Constellation Art. 3 §3.
    FederationProposal {
        /// The community proposing federation
        community_id: String,
        /// The target community being proposed to
        target_community_id: String,
    },
    /// Consent to accept a federation proposal from another community.
    ///
    /// Accepting federation creates a binding bilateral agreement.
    /// From Constellation Art. 3 §3.
    FederationAcceptance {
        /// The community accepting
        community_id: String,
        /// The proposing community
        proposing_community_id: String,
        /// The federation agreement ID
        agreement_id: String,
    },
    /// Consent to withdraw from an active federation.
    ///
    /// Withdrawal is always a right (Core Art. 8 §1), but should be
    /// consented to by the community's governance process, not unilateral.
    FederationWithdrawal {
        /// The community withdrawing
        community_id: String,
        /// The federation partner being withdrawn from
        partner_community_id: String,
        /// The federation agreement ID being withdrawn
        agreement_id: String,
    },
}

/// Validates consent conditions.
pub struct ConsentValidator;

impl ConsentValidator {
    /// Check whether consent exists and is valid for a given scope.
    pub fn validate(
        registry: &ConsentRegistry,
        grantor: &str,
        recipient: &str,
        scope: &ConsentScope,
    ) -> ConsentValidation {
        let matching: Vec<_> = registry
            .by_grantor(grantor)
            .into_iter()
            .filter(|c| c.recipient == recipient && &c.scope == scope)
            .collect();

        if matching.is_empty() {
            return ConsentValidation::Missing {
                reason: "No consent record found for this scope".into(),
            };
        }

        let active: Vec<_> = matching.iter().filter(|c| c.is_active()).collect();
        if active.is_empty() {
            if matching.iter().any(|c| c.is_revoked()) {
                return ConsentValidation::Revoked {
                    revoked_at: matching
                        .iter()
                        .filter_map(|c| c.revoked_at)
                        .max()
                        .unwrap_or_else(Utc::now),
                };
            }
            return ConsentValidation::Expired;
        }

        ConsentValidation::Valid {
            consent_id: active[0].id,
            granted_at: active[0].granted_at,
        }
    }
}

/// Result of a consent validation check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsentValidation {
    /// Active consent exists
    Valid {
        consent_id: Uuid,
        granted_at: DateTime<Utc>,
    },
    /// No consent record found
    Missing { reason: String },
    /// Consent was explicitly revoked
    Revoked { revoked_at: DateTime<Utc> },
    /// Consent has expired
    Expired,
}

impl ConsentValidation {
    /// Whether active, non-revoked, non-expired consent exists.
    pub fn is_valid(&self) -> bool {
        matches!(self, ConsentValidation::Valid { .. })
    }
}

/// Tracks all consent records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRegistry {
    records: HashMap<Uuid, ConsentRecord>,
    by_grantor: HashMap<String, Vec<Uuid>>,
}

impl ConsentRegistry {
    /// Create an empty consent registry.
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            by_grantor: HashMap::new(),
        }
    }

    /// Record a new consent and return its ID.
    pub fn record(&mut self, consent: ConsentRecord) -> Uuid {
        let id = consent.id;
        self.by_grantor
            .entry(consent.grantor.clone())
            .or_default()
            .push(id);
        self.records.insert(id, consent);
        id
    }

    /// Look up a consent record by ID.
    pub fn get(&self, id: &Uuid) -> Option<&ConsentRecord> {
        self.records.get(id)
    }

    /// Get a mutable reference to a consent record (e.g., for revocation).
    pub fn get_mut(&mut self, id: &Uuid) -> Option<&mut ConsentRecord> {
        self.records.get_mut(id)
    }

    /// Find all consent records granted by a given person or entity.
    pub fn by_grantor(&self, grantor: &str) -> Vec<&ConsentRecord> {
        self.by_grantor
            .get(grantor)
            .map(|ids| ids.iter().filter_map(|id| self.records.get(id)).collect())
            .unwrap_or_default()
    }

    /// Revoke a consent record by ID. Consent is always revocable -- this is a Covenant guarantee.
    pub fn revoke(&mut self, id: &Uuid, reason: impl Into<String>) -> Result<(), crate::PolityError> {
        let record = self
            .records
            .get_mut(id)
            .ok_or_else(|| crate::PolityError::ConsentNotFound(id.to_string()))?;
        record.revoke(reason)
    }

    /// All currently active (not revoked, not expired) consent records.
    pub fn active(&self) -> Vec<&ConsentRecord> {
        self.records.values().filter(|c| c.is_active()).collect()
    }

    /// Total number of consent records (all states).
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the registry contains no consent records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl Default for ConsentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_consent() {
        let consent = ConsentRecord::new(
            "alice",
            "garden_collective",
            ConsentScope::CommunityMembership {
                community_id: "garden_001".into(),
            },
        );
        assert!(consent.is_active());
        assert!(!consent.is_revoked());
        assert!(!consent.is_expired());
    }

    #[test]
    fn revoke_consent() {
        let mut consent = ConsentRecord::new(
            "alice",
            "platform",
            ConsentScope::DataSharing {
                data_type: "location".into(),
                purpose: "recommendations".into(),
            },
        );
        assert!(consent.is_active());

        consent.revoke("I no longer trust this platform").unwrap();
        assert!(!consent.is_active());
        assert!(consent.is_revoked());
        assert_eq!(
            consent.revocation_reason.as_deref(),
            Some("I no longer trust this platform")
        );
    }

    #[test]
    fn cannot_double_revoke() {
        let mut consent = ConsentRecord::new(
            "alice",
            "service",
            ConsentScope::General {
                description: "usage".into(),
            },
        );
        consent.revoke("done").unwrap();
        let result = consent.revoke("again");
        assert!(matches!(result, Err(crate::PolityError::ConsentAlreadyRevoked(_))));
    }

    #[test]
    fn expired_consent_is_inactive() {
        let consent = ConsentRecord::new(
            "alice",
            "temp_service",
            ConsentScope::General {
                description: "trial".into(),
            },
        )
        .with_expiry(Utc::now() - chrono::Duration::hours(1));

        assert!(!consent.is_active());
        assert!(consent.is_expired());
    }

    #[test]
    fn validate_consent_valid() {
        let mut registry = ConsentRegistry::new();
        registry.record(ConsentRecord::new(
            "alice",
            "bob",
            ConsentScope::General {
                description: "collaborate".into(),
            },
        ));

        let result = ConsentValidator::validate(
            &registry,
            "alice",
            "bob",
            &ConsentScope::General {
                description: "collaborate".into(),
            },
        );
        assert!(result.is_valid());
    }

    #[test]
    fn validate_consent_missing() {
        let registry = ConsentRegistry::new();
        let result = ConsentValidator::validate(
            &registry,
            "alice",
            "bob",
            &ConsentScope::General {
                description: "anything".into(),
            },
        );
        assert!(matches!(result, ConsentValidation::Missing { .. }));
    }

    #[test]
    fn validate_consent_revoked() {
        let mut registry = ConsentRegistry::new();
        let consent = ConsentRecord::new(
            "alice",
            "platform",
            ConsentScope::DataSharing {
                data_type: "email".into(),
                purpose: "marketing".into(),
            },
        );
        let id = registry.record(consent);
        registry.revoke(&id, "changed mind").unwrap();

        let result = ConsentValidator::validate(
            &registry,
            "alice",
            "platform",
            &ConsentScope::DataSharing {
                data_type: "email".into(),
                purpose: "marketing".into(),
            },
        );
        assert!(matches!(result, ConsentValidation::Revoked { .. }));
    }

    #[test]
    fn consent_with_conditions() {
        let consent = ConsentRecord::new(
            "alice",
            "research_org",
            ConsentScope::DataSharing {
                data_type: "health_metrics".into(),
                purpose: "community wellness study".into(),
            },
        )
        .with_condition("Data must be anonymized")
        .with_condition("Results must be shared with community")
        .with_condition("Consent expires after study completion");

        assert_eq!(consent.conditions.len(), 3);
    }

    #[test]
    fn consent_serialization_roundtrip() {
        let consent = ConsentRecord::new(
            "alice",
            "coop",
            ConsentScope::Delegation {
                mandate_scope: "budget decisions under 1000 Cool".into(),
            },
        )
        .with_condition("Monthly review required");

        let json = serde_json::to_string(&consent).unwrap();
        let restored: ConsentRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(consent.grantor, restored.grantor);
        assert_eq!(consent.scope, restored.scope);
        assert_eq!(consent.conditions, restored.conditions);
    }

    #[test]
    fn network_exposure_consent() {
        let mut registry = ConsentRegistry::new();
        let consent = ConsentRecord::new(
            "alice",
            "omnibus",
            ConsentScope::NetworkExposure {
                exposure_type: "tower_mode".into(),
                description: "Open port 7777 for Tower relay service".into(),
            },
        );
        let id = registry.record(consent);

        let result = ConsentValidator::validate(
            &registry,
            "alice",
            "omnibus",
            &ConsentScope::NetworkExposure {
                exposure_type: "tower_mode".into(),
                description: "Open port 7777 for Tower relay service".into(),
            },
        );
        assert!(result.is_valid());

        // Revoke — consent is always revocable
        registry.revoke(&id, "closing Tower").unwrap();
        let result = ConsentValidator::validate(
            &registry,
            "alice",
            "omnibus",
            &ConsentScope::NetworkExposure {
                exposure_type: "tower_mode".into(),
                description: "Open port 7777 for Tower relay service".into(),
            },
        );
        assert!(matches!(result, ConsentValidation::Revoked { .. }));
    }

    #[test]
    fn registry_active_filter() {
        let mut registry = ConsentRegistry::new();
        let c1 = ConsentRecord::new("alice", "a", ConsentScope::General { description: "1".into() });
        let c2 = ConsentRecord::new("alice", "b", ConsentScope::General { description: "2".into() });
        let id1 = registry.record(c1);
        let _id2 = registry.record(c2);

        registry.revoke(&id1, "done").unwrap();
        assert_eq!(registry.active().len(), 1);
    }

    #[test]
    fn federation_proposal_consent() {
        let mut registry = ConsentRegistry::new();
        let consent = ConsentRecord::new(
            "community_a_governance",
            "community_a",
            ConsentScope::FederationProposal {
                community_id: "community_a".into(),
                target_community_id: "community_b".into(),
            },
        );
        let id = registry.record(consent);

        let result = ConsentValidator::validate(
            &registry,
            "community_a_governance",
            "community_a",
            &ConsentScope::FederationProposal {
                community_id: "community_a".into(),
                target_community_id: "community_b".into(),
            },
        );
        assert!(result.is_valid());

        // Revoke — consent is always revocable
        registry.revoke(&id, "community voted against proposal").unwrap();
        let result = ConsentValidator::validate(
            &registry,
            "community_a_governance",
            "community_a",
            &ConsentScope::FederationProposal {
                community_id: "community_a".into(),
                target_community_id: "community_b".into(),
            },
        );
        assert!(matches!(result, ConsentValidation::Revoked { .. }));
    }

    #[test]
    fn federation_acceptance_consent() {
        let mut registry = ConsentRegistry::new();
        let consent = ConsentRecord::new(
            "community_b_governance",
            "community_b",
            ConsentScope::FederationAcceptance {
                community_id: "community_b".into(),
                proposing_community_id: "community_a".into(),
                agreement_id: "agreement-001".into(),
            },
        );
        registry.record(consent);

        let result = ConsentValidator::validate(
            &registry,
            "community_b_governance",
            "community_b",
            &ConsentScope::FederationAcceptance {
                community_id: "community_b".into(),
                proposing_community_id: "community_a".into(),
                agreement_id: "agreement-001".into(),
            },
        );
        assert!(result.is_valid());
    }

    #[test]
    fn federation_withdrawal_consent() {
        let consent = ConsentRecord::new(
            "community_a_governance",
            "community_a",
            ConsentScope::FederationWithdrawal {
                community_id: "community_a".into(),
                partner_community_id: "community_b".into(),
                agreement_id: "agreement-001".into(),
            },
        );
        assert!(consent.is_active());
    }

    #[test]
    fn federation_consent_serde_round_trip() {
        let consent = ConsentRecord::new(
            "governance",
            "community",
            ConsentScope::FederationProposal {
                community_id: "community_a".into(),
                target_community_id: "community_b".into(),
            },
        );
        let json = serde_json::to_string(&consent).unwrap();
        let restored: ConsentRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(consent.scope, restored.scope);

        let consent2 = ConsentRecord::new(
            "governance",
            "community",
            ConsentScope::FederationAcceptance {
                community_id: "b".into(),
                proposing_community_id: "a".into(),
                agreement_id: "ag1".into(),
            },
        );
        let json2 = serde_json::to_string(&consent2).unwrap();
        let restored2: ConsentRecord = serde_json::from_str(&json2).unwrap();
        assert_eq!(consent2.scope, restored2.scope);

        let consent3 = ConsentRecord::new(
            "governance",
            "community",
            ConsentScope::FederationWithdrawal {
                community_id: "a".into(),
                partner_community_id: "b".into(),
                agreement_id: "ag1".into(),
            },
        );
        let json3 = serde_json::to_string(&consent3).unwrap();
        let restored3: ConsentRecord = serde_json::from_str(&json3).unwrap();
        assert_eq!(consent3.scope, restored3.scope);
    }
}
