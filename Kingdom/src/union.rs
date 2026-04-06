use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::charter::AssetDistribution;

/// A voluntary bond between persons — the foundational unit of relation.
///
/// From Conjunction Art. 4 §2: "All Persons shall have the right to form Unions —
/// of care, of kinship, of desire, of co-creation, of chosen family — without limit
/// on form, number, nature, or origin."
///
/// From Conjunction Art. 4 §1: Consent must be voluntary, informed, continuous, revocable.
/// "Without consent, there is no relation — only domination."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Union {
    pub id: Uuid,
    pub name: String,
    pub union_type: UnionType,
    pub members: Vec<UnionMember>,
    pub charter: Option<UnionCharter>,
    pub formation: Option<UnionFormation>,
    pub status: UnionStatus,
    pub dissolution: Option<DissolutionRecord>,
    pub formed_at: DateTime<Utc>,
}

impl Union {
    /// Create a new active union with no members yet.
    pub fn new(name: impl Into<String>, union_type: UnionType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            union_type,
            members: Vec::new(),
            charter: None,
            formation: None,
            status: UnionStatus::Active,
            dissolution: None,
            formed_at: Utc::now(),
        }
    }

    /// Attach a charter to this union (builder pattern).
    pub fn with_charter(mut self, charter: UnionCharter) -> Self {
        self.charter = Some(charter);
        self
    }

    /// Attach formation records (consent, witnesses, ceremony) to this union (builder pattern).
    pub fn with_formation(mut self, formation: UnionFormation) -> Self {
        self.formation = Some(formation);
        self
    }

    /// Add a member to the union. Rejects if dissolved or already a member.
    pub fn add_member(&mut self, member: UnionMember) -> Result<(), crate::KingdomError> {
        if !self.is_active() {
            return Err(crate::KingdomError::UnionDissolved(self.id.to_string()));
        }
        if self.is_member(&member.pubkey) {
            return Err(crate::KingdomError::AlreadyMember(member.pubkey.clone()));
        }
        self.members.push(member);
        Ok(())
    }

    /// Remove a member from the union by their pubkey.
    pub fn remove_member(&mut self, pubkey: &str) -> Result<(), crate::KingdomError> {
        let pos = self
            .members
            .iter()
            .position(|m| m.pubkey == pubkey)
            .ok_or_else(|| crate::KingdomError::MemberNotFound(pubkey.into()))?;
        self.members.remove(pos);
        Ok(())
    }

    /// Whether a pubkey belongs to a current union member.
    pub fn is_member(&self, pubkey: &str) -> bool {
        self.members.iter().any(|m| m.pubkey == pubkey)
    }

    /// Total number of current union members.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Whether the union is currently active (not dissolved or disputed).
    pub fn is_active(&self) -> bool {
        self.status == UnionStatus::Active
    }

    /// Dissolve the union.
    ///
    /// From Conjunction Art. 4 §4: "Where consent ends, the Union ends."
    pub fn dissolve(&mut self, record: DissolutionRecord) -> Result<(), crate::KingdomError> {
        if !self.is_active() {
            return Err(crate::KingdomError::UnionDissolved(self.id.to_string()));
        }
        self.status = UnionStatus::Dissolved;
        self.dissolution = Some(record);
        Ok(())
    }
}

/// The kind of union.
///
/// From Conjunction Art. 4 §2: "Unions may be lifelong or temporary. Romantic or
/// platonic. Familial or collective."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UnionType {
    /// Romantic partnership.
    Marriage,
    /// Domestic partnership.
    DomesticPartnership,
    /// Chosen family bond.
    ChosenFamily,
    /// Business partnership.
    BusinessPartnership,
    /// Worker cooperative.
    WorkerCooperative,
    /// Creative collaboration.
    CreativeCollaboration,
    /// Trade or labor union.
    TradeUnion,
}

/// A member of a union.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnionMember {
    pub pubkey: String,
    pub role: Option<String>,
    pub joined_at: DateTime<Utc>,
}

/// How the union was formed.
///
/// From Conjunction Art. 4 §5: "In its place stands the lawful sanctity of
/// Personal Unions, defined by the Persons who live them."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnionFormation {
    pub require_unanimous_consent: bool,
    pub witnesses: Vec<String>,
    pub ceremony_record: Option<CeremonyRecord>,
    pub consent_signatures: Vec<ConsentSignature>,
}

impl UnionFormation {
    /// Create a new formation record requiring unanimous consent.
    pub fn new() -> Self {
        Self {
            require_unanimous_consent: true,
            witnesses: Vec::new(),
            ceremony_record: None,
            consent_signatures: Vec::new(),
        }
    }

    /// Record a person's consent to the union formation.
    pub fn add_consent(&mut self, pubkey: impl Into<String>, signature: impl Into<String>) {
        self.consent_signatures.push(ConsentSignature {
            pubkey: pubkey.into(),
            signature: signature.into(),
            signed_at: Utc::now(),
        });
    }

    /// Whether a specific person has given their consent.
    pub fn has_consent_from(&self, pubkey: &str) -> bool {
        self.consent_signatures.iter().any(|c| c.pubkey == pubkey)
    }

    /// Check if all members have given consent.
    pub fn all_consented(&self, member_pubkeys: &[String]) -> bool {
        member_pubkeys
            .iter()
            .all(|pk| self.has_consent_from(pk))
    }
}

impl Default for UnionFormation {
    fn default() -> Self {
        Self::new()
    }
}

/// A consent signature for union formation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsentSignature {
    pub pubkey: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
}

/// A record of a union ceremony.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CeremonyRecord {
    pub occurred_at: DateTime<Utc>,
    pub location: Option<String>,
    pub officiant: Option<String>,
    pub witnesses: Vec<String>,
    pub vows: Option<String>,
}

/// The union's internal charter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnionCharter {
    pub id: Uuid,
    pub purpose: String,
    pub decision_making: String,
    pub resource_sharing: Option<String>,
    pub dissolution_terms: UnionDissolutionTerms,
    pub additional_terms: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub signatures: Vec<ConsentSignature>,
    pub version: u32,
}

impl UnionCharter {
    pub fn new(purpose: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            purpose: purpose.into(),
            decision_making: "consensus".into(),
            resource_sharing: None,
            dissolution_terms: UnionDissolutionTerms::default(),
            additional_terms: Vec::new(),
            created_at: Utc::now(),
            signatures: Vec::new(),
            version: 1,
        }
    }
}

/// How the union's dissolution is handled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnionDissolutionTerms {
    pub decision_process: String,
    pub waiting_period_days: u32,
    pub asset_distribution: AssetDistribution,
    pub notice_required_days: u32,
}

impl Default for UnionDissolutionTerms {
    fn default() -> Self {
        Self {
            decision_process: "consensus".into(),
            waiting_period_days: 30,
            asset_distribution: AssetDistribution::EqualSplit,
            notice_required_days: 14,
        }
    }
}

/// Lifecycle of a union.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UnionStatus {
    /// The union is active and in good standing.
    Active,
    /// The union has been formally dissolved.
    Dissolved,
    /// The union is under dispute resolution.
    Disputed,
}

/// Record of a dissolution.
///
/// From Conjunction Art. 4 §4: "The end of a Union shall not be a rupture in
/// the eyes of law, but a transition, carried out in honor."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DissolutionRecord {
    pub requested_by: String,
    pub reason: Option<String>,
    pub member_consent: Vec<ConsentSignature>,
    pub asset_distribution: AssetDistribution,
    pub finalized_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_union() {
        let u = Union::new("Alice & Bob", UnionType::Marriage);
        assert!(u.is_active());
        assert_eq!(u.member_count(), 0);
    }

    #[test]
    fn union_formation_with_consent() {
        let mut u = Union::new("The Cooperative", UnionType::WorkerCooperative);
        let mut formation = UnionFormation::new();
        formation.add_consent("alice", "sig_alice");
        formation.add_consent("bob", "sig_bob");
        formation.add_consent("charlie", "sig_charlie");

        let members = vec!["alice".into(), "bob".into(), "charlie".into()];
        assert!(formation.all_consented(&members));

        u.formation = Some(formation);

        u.add_member(UnionMember {
            pubkey: "alice".into(),
            role: Some("coordinator".into()),
            joined_at: Utc::now(),
        })
        .unwrap();
        u.add_member(UnionMember {
            pubkey: "bob".into(),
            role: None,
            joined_at: Utc::now(),
        })
        .unwrap();
        u.add_member(UnionMember {
            pubkey: "charlie".into(),
            role: None,
            joined_at: Utc::now(),
        })
        .unwrap();

        assert_eq!(u.member_count(), 3);
    }

    #[test]
    fn consent_not_complete() {
        let mut formation = UnionFormation::new();
        formation.add_consent("alice", "sig");

        let members = vec!["alice".into(), "bob".into()];
        assert!(!formation.all_consented(&members));
    }

    #[test]
    fn dissolve_union() {
        let mut u = Union::new("Test", UnionType::ChosenFamily);
        u.add_member(UnionMember {
            pubkey: "alice".into(),
            role: None,
            joined_at: Utc::now(),
        })
        .unwrap();

        let record = DissolutionRecord {
            requested_by: "alice".into(),
            reason: Some("Moving to different cities".into()),
            member_consent: vec![],
            asset_distribution: AssetDistribution::EqualSplit,
            finalized_at: Utc::now(),
        };

        u.dissolve(record).unwrap();
        assert_eq!(u.status, UnionStatus::Dissolved);
        assert!(u.dissolution.is_some());
    }

    #[test]
    fn cannot_dissolve_twice() {
        let mut u = Union::new("Test", UnionType::DomesticPartnership);
        let record = DissolutionRecord {
            requested_by: "alice".into(),
            reason: None,
            member_consent: vec![],
            asset_distribution: AssetDistribution::EqualSplit,
            finalized_at: Utc::now(),
        };
        u.dissolve(record.clone()).unwrap();
        assert!(u.dissolve(record).is_err());
    }

    #[test]
    fn cannot_add_member_to_dissolved_union() {
        let mut u = Union::new("Test", UnionType::TradeUnion);
        let record = DissolutionRecord {
            requested_by: "x".into(),
            reason: None,
            member_consent: vec![],
            asset_distribution: AssetDistribution::DonatedToCommons,
            finalized_at: Utc::now(),
        };
        u.dissolve(record).unwrap();

        assert!(u
            .add_member(UnionMember {
                pubkey: "alice".into(),
                role: None,
                joined_at: Utc::now(),
            })
            .is_err());
    }

    #[test]
    fn union_types() {
        // All 7 types from Conjunction
        let types = [
            UnionType::Marriage,
            UnionType::DomesticPartnership,
            UnionType::ChosenFamily,
            UnionType::BusinessPartnership,
            UnionType::WorkerCooperative,
            UnionType::CreativeCollaboration,
            UnionType::TradeUnion,
        ];
        assert_eq!(types.len(), 7);
    }

    #[test]
    fn union_charter() {
        let charter = UnionCharter::new("Shared creative endeavor");
        assert_eq!(charter.version, 1);
        assert_eq!(charter.decision_making, "consensus");
    }

    #[test]
    fn union_serialization_roundtrip() {
        let u = Union::new("Test", UnionType::CreativeCollaboration)
            .with_charter(UnionCharter::new("Making art together"));

        let json = serde_json::to_string(&u).unwrap();
        let restored: Union = serde_json::from_str(&json).unwrap();
        assert_eq!(u.name, restored.name);
        assert!(restored.charter.is_some());
    }
}
