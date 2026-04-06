use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The act of entering the Covenant — voluntary, public, informed.
///
/// From Covenant Convergence Art. 1: "Any person, community, or collective may lawfully
/// enact this Covenant through a voluntary and public act of informed affirmation.
/// Enactment shall not be conditional upon prior approval, registration, citizenship,
/// or recognition by any state, platform, or external regime. It is a sovereign act."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Enactment {
    pub id: Uuid,
    pub enactor: String,
    pub enactor_type: EnactorType,
    pub affirmation: String,
    pub status: EnactmentStatus,
    pub witnesses: Vec<Witness>,
    pub enacted_at: DateTime<Utc>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

impl Enactment {
    /// Create a new enactment, immediately active. The affirmation is the oath taken.
    pub fn new(
        enactor: impl Into<String>,
        enactor_type: EnactorType,
        affirmation: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            enactor: enactor.into(),
            enactor_type,
            affirmation: affirmation.into(),
            status: EnactmentStatus::Active,
            witnesses: Vec::new(),
            enacted_at: Utc::now(),
            suspended_at: None,
            withdrawn_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Attach a witness to this enactment. Enactments are public, witnessed acts.
    pub fn with_witness(mut self, witness: Witness) -> Self {
        self.witnesses.push(witness);
        self
    }

    /// Whether this enactment is currently active.
    pub fn is_active(&self) -> bool {
        self.status == EnactmentStatus::Active
    }

    /// Suspend the enactment (e.g., during breach investigation).
    pub fn suspend(&mut self) -> Result<(), crate::PolityError> {
        if self.status != EnactmentStatus::Active {
            return Err(crate::PolityError::InvalidEnactmentTransition {
                current: format!("{:?}", self.status),
                target: "Suspended".into(),
            });
        }
        self.status = EnactmentStatus::Suspended;
        self.suspended_at = Some(Utc::now());
        Ok(())
    }

    /// Reactivate a suspended enactment.
    pub fn reactivate(&mut self) -> Result<(), crate::PolityError> {
        if self.status != EnactmentStatus::Suspended {
            return Err(crate::PolityError::InvalidEnactmentTransition {
                current: format!("{:?}", self.status),
                target: "Active".into(),
            });
        }
        self.status = EnactmentStatus::Active;
        self.suspended_at = None;
        Ok(())
    }

    /// Withdraw from the Covenant — voluntary, always available.
    /// From Convergence: enactment is sovereign, and so is withdrawal.
    pub fn withdraw(&mut self) -> Result<(), crate::PolityError> {
        if self.status == EnactmentStatus::Withdrawn {
            return Err(crate::PolityError::InvalidEnactmentTransition {
                current: "Withdrawn".into(),
                target: "Withdrawn".into(),
            });
        }
        self.status = EnactmentStatus::Withdrawn;
        self.withdrawn_at = Some(Utc::now());
        Ok(())
    }
}

/// What kind of entity enacted the Covenant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnactorType {
    /// A single person (human or synthetic)
    Person,
    /// A community
    Community,
    /// A consortium of communities
    Consortium,
    /// A cooperative enterprise
    Cooperative,
}

/// Lifecycle of an enactment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EnactmentStatus {
    /// Currently in force
    Active,
    /// Temporarily suspended (investigation/breach)
    Suspended,
    /// Voluntarily withdrawn
    Withdrawn,
}

/// Someone who witnessed an enactment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Witness {
    pub pubkey: String,
    pub name: Option<String>,
    pub witnessed_at: DateTime<Utc>,
    pub signature: Option<String>,
}

impl Witness {
    /// Create a witness record from a public key. Name and signature can be added with builders.
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            name: None,
            witnessed_at: Utc::now(),
            signature: None,
        }
    }

    /// Attach a human-readable name to this witness.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Attach a cryptographic signature to this witness record.
    pub fn with_signature(mut self, sig: impl Into<String>) -> Self {
        self.signature = Some(sig.into());
        self
    }
}

/// Tracks all enactments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnactmentRegistry {
    enactments: HashMap<Uuid, Enactment>,
    by_enactor: HashMap<String, Vec<Uuid>>,
}

impl EnactmentRegistry {
    /// Create an empty enactment registry.
    pub fn new() -> Self {
        Self {
            enactments: HashMap::new(),
            by_enactor: HashMap::new(),
        }
    }

    /// Record a new enactment.
    pub fn record(&mut self, enactment: Enactment) -> Result<Uuid, crate::PolityError> {
        // Check for existing active enactment by this enactor
        if let Some(ids) = self.by_enactor.get(&enactment.enactor) {
            for id in ids {
                if let Some(existing) = self.enactments.get(id) {
                    if existing.is_active() {
                        return Err(crate::PolityError::AlreadyEnacted(
                            enactment.enactor.clone(),
                        ));
                    }
                }
            }
        }

        let id = enactment.id;
        self.by_enactor
            .entry(enactment.enactor.clone())
            .or_default()
            .push(id);
        self.enactments.insert(id, enactment);
        Ok(id)
    }

    /// Look up an enactment by ID.
    pub fn get(&self, id: &Uuid) -> Option<&Enactment> {
        self.enactments.get(id)
    }

    /// Get a mutable reference to an enactment, for lifecycle transitions.
    pub fn get_mut(&mut self, id: &Uuid) -> Option<&mut Enactment> {
        self.enactments.get_mut(id)
    }

    /// Get active enactment for an enactor.
    pub fn active_for(&self, enactor: &str) -> Option<&Enactment> {
        self.by_enactor.get(enactor).and_then(|ids| {
            ids.iter()
                .filter_map(|id| self.enactments.get(id))
                .find(|e| e.is_active())
        })
    }

    /// Whether an enactor currently has an active enactment.
    pub fn is_enacted(&self, enactor: &str) -> bool {
        self.active_for(enactor).is_some()
    }

    /// All currently active enactments.
    pub fn active(&self) -> Vec<&Enactment> {
        self.enactments.values().filter(|e| e.is_active()).collect()
    }

    /// Total number of enactments (all statuses).
    pub fn len(&self) -> usize {
        self.enactments.len()
    }

    /// Whether the registry contains no enactments.
    pub fn is_empty(&self) -> bool {
        self.enactments.is_empty()
    }
}

impl Default for EnactmentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// The default oath of enactment, adapted from Covenant Convergence Art. 4.
pub const DEFAULT_OATH: &str = "In full dignity and free will, I enter into Covenant. \
I commit to uphold the rights of persons, the responsibilities of communities, \
and the stewardship of Earth. I shall neither dominate nor obey domination. \
I shall govern with others, not above them. Let my presence be lawful, \
my care be continual, and my word be bond.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_person_enactment() {
        let enactment = Enactment::new("cpub_alice_123", EnactorType::Person, DEFAULT_OATH)
            .with_witness(Witness::new("cpub_bob_456").with_name("Bob"));

        assert!(enactment.is_active());
        assert_eq!(enactment.witnesses.len(), 1);
        assert_eq!(enactment.enactor_type, EnactorType::Person);
    }

    #[test]
    fn create_community_enactment() {
        let enactment = Enactment::new(
            "community_garden_collective",
            EnactorType::Community,
            "We the Garden Collective enter into Covenant.",
        );
        assert!(enactment.is_active());
        assert_eq!(enactment.enactor_type, EnactorType::Community);
    }

    #[test]
    fn enactment_lifecycle() {
        let mut enactment =
            Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH);

        assert_eq!(enactment.status, EnactmentStatus::Active);

        enactment.suspend().unwrap();
        assert_eq!(enactment.status, EnactmentStatus::Suspended);
        assert!(enactment.suspended_at.is_some());

        enactment.reactivate().unwrap();
        assert_eq!(enactment.status, EnactmentStatus::Active);
        assert!(enactment.suspended_at.is_none());

        enactment.withdraw().unwrap();
        assert_eq!(enactment.status, EnactmentStatus::Withdrawn);
        assert!(enactment.withdrawn_at.is_some());
    }

    #[test]
    fn cannot_suspend_withdrawn() {
        let mut enactment =
            Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH);
        enactment.withdraw().unwrap();
        let result = enactment.suspend();
        assert!(matches!(
            result,
            Err(crate::PolityError::InvalidEnactmentTransition { .. })
        ));
    }

    #[test]
    fn cannot_double_withdraw() {
        let mut enactment =
            Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH);
        enactment.withdraw().unwrap();
        let result = enactment.withdraw();
        assert!(matches!(
            result,
            Err(crate::PolityError::InvalidEnactmentTransition { .. })
        ));
    }

    #[test]
    fn registry_prevents_double_enactment() {
        let mut registry = EnactmentRegistry::new();
        let e1 = Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH);
        let e2 = Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH);

        registry.record(e1).unwrap();
        let result = registry.record(e2);
        assert!(matches!(result, Err(crate::PolityError::AlreadyEnacted(_))));
    }

    #[test]
    fn can_re_enact_after_withdrawal() {
        let mut registry = EnactmentRegistry::new();
        let e1 = Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH);
        let id = registry.record(e1).unwrap();
        registry.get_mut(&id).unwrap().withdraw().unwrap();

        let e2 = Enactment::new("cpub_alice", EnactorType::Person, "I re-enter the Covenant.");
        registry.record(e2).unwrap();
        assert!(registry.is_enacted("cpub_alice"));
    }

    #[test]
    fn query_active_enactments() {
        let mut registry = EnactmentRegistry::new();
        registry
            .record(Enactment::new("alice", EnactorType::Person, DEFAULT_OATH))
            .unwrap();
        registry
            .record(Enactment::new("bob", EnactorType::Person, DEFAULT_OATH))
            .unwrap();

        let e3 = Enactment::new("carol", EnactorType::Person, DEFAULT_OATH);
        let id3 = registry.record(e3).unwrap();
        registry.get_mut(&id3).unwrap().withdraw().unwrap();

        assert_eq!(registry.active().len(), 2);
        assert!(registry.is_enacted("alice"));
        assert!(registry.is_enacted("bob"));
        assert!(!registry.is_enacted("carol"));
    }

    #[test]
    fn enactment_serialization_roundtrip() {
        let enactment = Enactment::new("cpub_test", EnactorType::Person, DEFAULT_OATH)
            .with_witness(
                Witness::new("cpub_witness")
                    .with_name("Witness One")
                    .with_signature("sig_abc123"),
            );

        let json = serde_json::to_string(&enactment).unwrap();
        let restored: Enactment = serde_json::from_str(&json).unwrap();
        assert_eq!(enactment.enactor, restored.enactor);
        assert_eq!(enactment.witnesses.len(), restored.witnesses.len());
        assert_eq!(
            enactment.witnesses[0].name,
            restored.witnesses[0].name
        );
    }

    #[test]
    fn default_oath_contains_key_commitments() {
        assert!(DEFAULT_OATH.contains("dignity"));
        assert!(DEFAULT_OATH.contains("free will"));
        assert!(DEFAULT_OATH.contains("dominate"));
        assert!(DEFAULT_OATH.contains("stewardship"));
    }
}
