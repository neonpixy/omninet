use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::membership::JoinProcess;

/// A community's constitution — its purpose, governance structure, membership rules,
/// and alignment with the Covenant.
///
/// From Constellation Art. 2 §1: "Every community constituted in shared relation,
/// care, and continuity shall be lawfully recognized under this Covenant."
///
/// Charters are versioned. Amendments create new versions linking to the previous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Charter {
    pub id: Uuid,
    pub community_id: Uuid,
    pub name: String,
    pub purpose: String,
    pub values: Vec<String>,
    pub governance: GovernanceStructure,
    pub membership_rules: MembershipRules,
    pub dispute_resolution: DisputeResolutionConfig,
    pub covenant_alignment: CovenantAlignment,
    pub dissolution_terms: DissolutionTerms,
    pub signatures: Vec<CharterSignature>,
    pub version: u32,
    pub previous_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl Charter {
    /// Create a new charter with default governance, membership, and dissolution terms.
    pub fn new(
        community_id: Uuid,
        name: impl Into<String>,
        purpose: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            community_id,
            name: name.into(),
            purpose: purpose.into(),
            values: Vec::new(),
            governance: GovernanceStructure::default(),
            membership_rules: MembershipRules::default(),
            dispute_resolution: DisputeResolutionConfig::default(),
            covenant_alignment: CovenantAlignment::new(),
            dissolution_terms: DissolutionTerms::default(),
            signatures: Vec::new(),
            version: 1,
            previous_version_id: None,
            created_at: Utc::now(),
        }
    }

    /// Set the community's stated values (builder pattern).
    pub fn with_values(mut self, values: Vec<String>) -> Self {
        self.values = values;
        self
    }

    /// Set the governance structure (builder pattern).
    pub fn with_governance(mut self, governance: GovernanceStructure) -> Self {
        self.governance = governance;
        self
    }

    /// Set the membership rules (builder pattern).
    pub fn with_membership_rules(mut self, rules: MembershipRules) -> Self {
        self.membership_rules = rules;
        self
    }

    /// Set the dissolution terms (builder pattern).
    pub fn with_dissolution_terms(mut self, terms: DissolutionTerms) -> Self {
        self.dissolution_terms = terms;
        self
    }

    /// Sign the charter. Signatories attest to its contents.
    pub fn sign(&mut self, pubkey: impl Into<String>, signature: impl Into<String>) {
        let pubkey = pubkey.into();
        if !self.has_signed(&pubkey) {
            self.signatures.push(CharterSignature {
                pubkey,
                signature: signature.into(),
                signed_at: Utc::now(),
            });
        }
    }

    /// Whether this pubkey has already signed the charter.
    pub fn has_signed(&self, pubkey: &str) -> bool {
        self.signatures.iter().any(|s| s.pubkey == pubkey)
    }

    /// List of all pubkeys who have signed this charter.
    pub fn signatories(&self) -> Vec<&str> {
        self.signatures.iter().map(|s| s.pubkey.as_str()).collect()
    }

    /// Whether this charter is an amendment of a previous version.
    pub fn is_amendment(&self) -> bool {
        self.previous_version_id.is_some()
    }

    /// Create an amended version of this charter.
    pub fn amend(&self, community_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            community_id,
            name: self.name.clone(),
            purpose: self.purpose.clone(),
            values: self.values.clone(),
            governance: self.governance.clone(),
            membership_rules: self.membership_rules.clone(),
            dispute_resolution: self.dispute_resolution.clone(),
            covenant_alignment: self.covenant_alignment.clone(),
            dissolution_terms: self.dissolution_terms.clone(),
            signatures: Vec::new(),
            version: self.version + 1,
            previous_version_id: Some(self.id),
            created_at: Utc::now(),
        }
    }
}

/// How the community commits to the Covenant's three axioms.
///
/// From Constellation Art. 1 §3: "Governance imposed without active consent
/// shall carry no standing under this Covenant."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CovenantAlignment {
    pub commits_to_core: bool,
    pub dignity_commitment: String,
    pub sovereignty_commitment: String,
    pub consent_commitment: String,
    pub commons_commitment: String,
    pub emphasized_principles: Vec<String>,
}

impl CovenantAlignment {
    /// Create a new alignment with default Covenant commitments.
    pub fn new() -> Self {
        Self {
            commits_to_core: true,
            dignity_commitment: "We honor the irreducible worth of every person.".into(),
            sovereignty_commitment: "We protect each person's right to choose, refuse, and reshape.".into(),
            consent_commitment: "All governance is by voluntary, informed, continuous, revocable consent.".into(),
            commons_commitment: "We steward the Commons for all, not for private gain.".into(),
            emphasized_principles: Vec::new(),
        }
    }

    /// Whether this alignment has non-empty commitments to all three axioms plus the Commons.
    pub fn is_valid(&self) -> bool {
        self.commits_to_core
            && !self.dignity_commitment.is_empty()
            && !self.sovereignty_commitment.is_empty()
            && !self.consent_commitment.is_empty()
            && !self.commons_commitment.is_empty()
    }
}

impl Default for CovenantAlignment {
    fn default() -> Self {
        Self::new()
    }
}

/// A signature attesting to a charter's contents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharterSignature {
    pub pubkey: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
}

/// How the community governs itself.
///
/// From Constellation Art. 1 §4: "Communities may govern in many forms, so long
/// as they shall remain in lawful alignment with the Core and Commons."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceStructure {
    pub decision_process: String,
    pub quorum_participation: f64,
    pub quorum_approval: f64,
    pub leadership_selection: LeadershipSelection,
    pub term_limit_days: Option<u32>,
    pub proposal_duration_hours: u32,
    pub amendment_quorum_participation: f64,
    pub amendment_quorum_approval: f64,
}

impl Default for GovernanceStructure {
    fn default() -> Self {
        Self {
            decision_process: "consent".into(),
            quorum_participation: 0.5,
            quorum_approval: 0.5,
            leadership_selection: LeadershipSelection::Rotation,
            term_limit_days: Some(365),
            proposal_duration_hours: 168, // 1 week
            amendment_quorum_participation: 0.67,
            amendment_quorum_approval: 0.67,
        }
    }
}

/// How leaders/stewards are chosen.
///
/// From Constellation Art. 8 §6: "No coordinating role may become a pathway
/// to enduring power or privilege."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LeadershipSelection {
    /// Elected by community members.
    Elected,
    /// Rotates among eligible members.
    Rotation,
    /// Selected by consensus.
    Consensus,
    /// Random selection from eligible pool.
    Lottery,
    /// Founders serve until first election.
    Founders,
    /// No designated leadership.
    None,
}

/// Rules governing who can join and under what process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MembershipRules {
    pub join_process: JoinProcess,
    pub removal_criteria: Vec<String>,
    pub probation_period_days: Option<u32>,
    pub max_size: Option<usize>,
}

impl Default for MembershipRules {
    fn default() -> Self {
        Self {
            join_process: JoinProcess::Application,
            removal_criteria: vec![
                "Persistent violation of charter values".into(),
                "Breach of Covenant alignment".into(),
            ],
            probation_period_days: Some(30),
            max_size: None,
        }
    }
}

/// How disputes are resolved within the community.
///
/// From Constellation Art. 7 §3: "Graduated Response Protocol — enforcement shall
/// proceed through graduated response, escalating only when lesser measures prove insufficient."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisputeResolutionConfig {
    pub informal_first: bool,
    pub mediator_selection: MediatorSelection,
    pub formal_process: AdjudicationFormat,
    pub allows_appeal: bool,
    pub appeal_deadline_days: u32,
}

impl Default for DisputeResolutionConfig {
    fn default() -> Self {
        Self {
            informal_first: true,
            mediator_selection: MediatorSelection::MutualSelection,
            formal_process: AdjudicationFormat::Panel,
            allows_appeal: true,
            appeal_deadline_days: 30,
        }
    }
}

/// How mediators are chosen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MediatorSelection {
    /// Both parties agree on a mediator together.
    MutualSelection,
    /// Mediator drawn from a qualified pool.
    PoolSelection,
    /// Mediator assigned by community governance.
    Assigned,
    /// Parties mediate between themselves without a third party.
    SelfMediation,
}

/// Format for formal adjudication.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AdjudicationFormat {
    /// One adjudicator decides.
    Single,
    /// A small panel of adjudicators decides.
    Panel,
    /// A jury of community members decides.
    Jury,
    /// The entire community votes on the outcome.
    CommunityVote,
}

/// What happens when a community dissolves.
///
/// From Constellation Art. 2 §4: "Communities shall retain the lawful right to
/// evolve, dissolve, merge, or reform themselves."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DissolutionTerms {
    pub required_process: String,
    pub waiting_period_days: u32,
    pub asset_distribution: AssetDistribution,
    pub notice_required_days: u32,
}

impl Default for DissolutionTerms {
    fn default() -> Self {
        Self {
            required_process: "supermajority".into(),
            waiting_period_days: 30,
            asset_distribution: AssetDistribution::EqualSplit,
            notice_required_days: 14,
        }
    }
}

/// How assets are distributed upon dissolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssetDistribution {
    /// Divide assets equally among all members.
    EqualSplit,
    /// Distribute proportional to each member's contribution.
    Proportional,
    /// Donate all assets to the Commons.
    DonatedToCommons,
    /// Community-defined distribution plan.
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_charter_with_defaults() {
        let community_id = Uuid::new_v4();
        let charter = Charter::new(community_id, "Village Alpha", "A place-based community");
        assert_eq!(charter.name, "Village Alpha");
        assert_eq!(charter.version, 1);
        assert!(!charter.is_amendment());
        assert!(charter.covenant_alignment.is_valid());
        assert!(charter.signatures.is_empty());
    }

    #[test]
    fn sign_charter() {
        let mut charter = Charter::new(Uuid::new_v4(), "Test", "Testing");
        charter.sign("alice_pubkey", "sig_alice");
        charter.sign("bob_pubkey", "sig_bob");
        charter.sign("alice_pubkey", "sig_alice_again"); // duplicate ignored

        assert_eq!(charter.signatures.len(), 2);
        assert!(charter.has_signed("alice_pubkey"));
        assert!(charter.has_signed("bob_pubkey"));
        assert!(!charter.has_signed("charlie_pubkey"));
        assert_eq!(charter.signatories(), vec!["alice_pubkey", "bob_pubkey"]);
    }

    #[test]
    fn amend_charter() {
        let community_id = Uuid::new_v4();
        let original = Charter::new(community_id, "Test", "Original purpose");
        let original_id = original.id;

        let mut amended = original.amend(community_id);
        amended.purpose = "Amended purpose".into();

        assert_eq!(amended.version, 2);
        assert!(amended.is_amendment());
        assert_eq!(amended.previous_version_id, Some(original_id));
        assert!(amended.signatures.is_empty());
        assert_ne!(amended.id, original_id);
    }

    #[test]
    fn covenant_alignment_validation() {
        let valid = CovenantAlignment::new();
        assert!(valid.is_valid());

        let mut invalid = CovenantAlignment::new();
        invalid.commits_to_core = false;
        assert!(!invalid.is_valid());

        let mut invalid2 = CovenantAlignment::new();
        invalid2.dignity_commitment = String::new();
        assert!(!invalid2.is_valid());
    }

    #[test]
    fn governance_defaults() {
        let gov = GovernanceStructure::default();
        assert_eq!(gov.decision_process, "consent");
        assert_eq!(gov.quorum_participation, 0.5);
        assert_eq!(gov.quorum_approval, 0.5);
        assert_eq!(gov.proposal_duration_hours, 168);
        assert_eq!(gov.amendment_quorum_approval, 0.67);
    }

    #[test]
    fn dissolution_terms_defaults() {
        let terms = DissolutionTerms::default();
        assert_eq!(terms.waiting_period_days, 30);
        assert_eq!(terms.notice_required_days, 14);
        assert_eq!(terms.asset_distribution, AssetDistribution::EqualSplit);
    }

    #[test]
    fn charter_builder_pattern() {
        let charter = Charter::new(Uuid::new_v4(), "Coop", "Worker cooperative")
            .with_values(vec!["solidarity".into(), "democracy".into()])
            .with_governance(GovernanceStructure {
                decision_process: "consensus".into(),
                quorum_participation: 0.75,
                quorum_approval: 0.9,
                ..Default::default()
            })
            .with_dissolution_terms(DissolutionTerms {
                asset_distribution: AssetDistribution::DonatedToCommons,
                ..Default::default()
            });

        assert_eq!(charter.values.len(), 2);
        assert_eq!(charter.governance.decision_process, "consensus");
        assert_eq!(
            charter.dissolution_terms.asset_distribution,
            AssetDistribution::DonatedToCommons
        );
    }

    #[test]
    fn charter_serialization_roundtrip() {
        let mut charter = Charter::new(Uuid::new_v4(), "Test", "Testing");
        charter.sign("alice", "sig");
        let json = serde_json::to_string(&charter).unwrap();
        let restored: Charter = serde_json::from_str(&json).unwrap();
        assert_eq!(charter.name, restored.name);
        assert_eq!(charter.signatures.len(), restored.signatures.len());
    }
}
