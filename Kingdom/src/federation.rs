use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::charter::{AssetDistribution, GovernanceStructure};
use crate::mandate::Delegate;
use crate::proposal::ProposalType;

/// A federation of communities — voluntary, revocable, rooted in consent.
///
/// From Constellation Art. 3 §3: "Communities may enter into federated agreements
/// to share governance, coordinate resource stewardship, or pursue common purpose.
/// Such federations shall remain voluntary, revocable, and rooted in consent."
///
/// From Consortium Art. 1 §1: "A Consortium shall be defined as any lawful collective
/// of persons, communities, or entities who enter into durable association for the
/// purpose of shared work, mutual care, regenerative enterprise, or cultural stewardship."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Consortium {
    pub id: Uuid,
    pub name: String,
    pub purpose: String,
    pub members: Vec<ConsortiumMember>,
    pub charter: ConsortiumCharter,
    pub governance: ConsortiumGovernance,
    pub status: ConsortiumStatus,
    pub founded_at: DateTime<Utc>,
}

impl Consortium {
    pub fn new(
        name: impl Into<String>,
        purpose: impl Into<String>,
        charter: ConsortiumCharter,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            purpose: purpose.into(),
            members: Vec::new(),
            charter,
            governance: ConsortiumGovernance::default(),
            status: ConsortiumStatus::Active,
            founded_at: Utc::now(),
        }
    }

    /// Add a community to the consortium. Rejects duplicates.
    pub fn add_member(&mut self, member: ConsortiumMember) -> Result<(), crate::KingdomError> {
        if self.is_member(&member.community_id) {
            return Err(crate::KingdomError::AlreadyMember(
                member.community_id.to_string(),
            ));
        }
        self.members.push(member);
        Ok(())
    }

    /// Remove a community from the consortium.
    pub fn remove_member(&mut self, community_id: &Uuid) -> Result<(), crate::KingdomError> {
        let pos = self
            .members
            .iter()
            .position(|m| m.community_id == *community_id)
            .ok_or_else(|| crate::KingdomError::NotConsortiumMember(community_id.to_string()))?;
        self.members.remove(pos);
        Ok(())
    }

    /// Whether a community is a member of this consortium.
    pub fn is_member(&self, community_id: &Uuid) -> bool {
        self.members.iter().any(|m| m.community_id == *community_id)
    }

    /// Look up a member community by its ID.
    pub fn member(&self, community_id: &Uuid) -> Option<&ConsortiumMember> {
        self.members.iter().find(|m| m.community_id == *community_id)
    }

    /// Total number of member communities.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Collect all delegates across all member communities.
    pub fn all_delegates(&self) -> Vec<&Delegate> {
        self.members.iter().flat_map(|m| &m.delegates).collect()
    }

    /// Whether a person is currently a delegate in this consortium.
    pub fn is_delegate(&self, pubkey: &str) -> bool {
        self.all_delegates().iter().any(|d| d.pubkey == pubkey)
    }

    /// Whether the consortium is currently active.
    pub fn is_active(&self) -> bool {
        self.status == ConsortiumStatus::Active
    }
}

/// A community's membership in a consortium.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsortiumMember {
    pub community_id: Uuid,
    pub joined_at: DateTime<Utc>,
    pub delegates: Vec<Delegate>,
}

/// The consortium's founding document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsortiumCharter {
    pub id: Uuid,
    pub purpose: String,
    pub common_values: Vec<String>,
    pub delegate_selection: DelegateSelectionProcess,
    pub membership_process: ConsortiumMembershipProcess,
    pub exit_process: ExitProcess,
    pub dissolution_terms: ConsortiumDissolutionTerms,
    pub created_at: DateTime<Utc>,
    pub version: u32,
}

impl ConsortiumCharter {
    pub fn new(purpose: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            purpose: purpose.into(),
            common_values: Vec::new(),
            delegate_selection: DelegateSelectionProcess::default(),
            membership_process: ConsortiumMembershipProcess::default(),
            exit_process: ExitProcess::default(),
            dissolution_terms: ConsortiumDissolutionTerms::default(),
            created_at: Utc::now(),
            version: 1,
        }
    }
}

/// How delegates are selected for the consortium.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegateSelectionProcess {
    pub method: DelegateSelectionMethod,
    pub term_length_days: Option<u32>,
    pub allows_recall: bool,
    pub max_delegates_per_community: u32,
}

impl Default for DelegateSelectionProcess {
    fn default() -> Self {
        Self {
            method: DelegateSelectionMethod::CommunityElection,
            term_length_days: Some(365),
            allows_recall: true,
            max_delegates_per_community: 2,
        }
    }
}

/// Methods for selecting delegates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DelegateSelectionMethod {
    /// Delegates elected by their community's members.
    CommunityElection,
    /// Delegates rotate among eligible members.
    Rotation,
    /// Delegates chosen by community consensus.
    Consensus,
    /// Delegates randomly selected from an eligible pool.
    Lottery,
}

/// How communities join the consortium.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsortiumMembershipProcess {
    pub application_process: ConsortiumJoinProcess,
    pub requires_member_approval: bool,
    pub approval_threshold: f64,
}

impl Default for ConsortiumMembershipProcess {
    fn default() -> Self {
        Self {
            application_process: ConsortiumJoinProcess::Application,
            requires_member_approval: true,
            approval_threshold: 0.67,
        }
    }
}

/// How communities join a consortium.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConsortiumJoinProcess {
    /// Any community can join freely.
    Open,
    /// Communities must apply and be reviewed.
    Application,
    /// Communities must be invited by an existing member.
    Invitation,
    /// Communities must be sponsored by an existing member.
    Sponsorship,
}

/// How communities can leave.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitProcess {
    pub notice_period_days: u32,
    pub requires_asset_settlement: bool,
}

impl Default for ExitProcess {
    fn default() -> Self {
        Self {
            notice_period_days: 30,
            requires_asset_settlement: true,
        }
    }
}

/// Consortium dissolution terms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsortiumDissolutionTerms {
    pub required_approval: f64,
    pub waiting_period_days: u32,
    pub asset_distribution: AssetDistribution,
}

impl Default for ConsortiumDissolutionTerms {
    fn default() -> Self {
        Self {
            required_approval: 0.75,
            waiting_period_days: 60,
            asset_distribution: AssetDistribution::DonatedToCommons,
        }
    }
}

/// How the consortium governs itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsortiumGovernance {
    pub structure: GovernanceStructure,
    pub voting_model: VotingModel,
}

impl Default for ConsortiumGovernance {
    fn default() -> Self {
        Self {
            structure: GovernanceStructure::default(),
            voting_model: VotingModel::OneCommunityOneVote,
        }
    }
}

/// How votes are counted at the consortium level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VotingModel {
    /// Each individual delegate gets one vote.
    OnePersonOneVote,
    /// Each member community gets one vote regardless of size.
    OneCommunityOneVote,
    /// Votes weighted by community population.
    Proportional,
    /// Full consensus of all delegates required.
    Consensus,
}

/// Lifecycle of a consortium.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConsortiumStatus {
    /// Consortium is fully operational.
    Active,
    /// Consortium is inactive but not dissolved.
    Dormant,
    /// Consortium has been formally ended.
    Dissolved,
    /// Consortium is under dispute resolution.
    Disputed,
}

/// A check to ensure decisions are made at the appropriate level.
///
/// From Constellation Art. 8 §1: "All decisions shall be made at the most local
/// level capable of addressing their scope and impact."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubsidiarityCheck {
    pub decision_type: ProposalType,
    pub proposed_level: GovernanceLevel,
    pub local_capable: bool,
    pub justification: Option<String>,
}

impl SubsidiarityCheck {
    /// Determine if this decision should be elevated to a higher level.
    pub fn should_elevate(&self) -> bool {
        !self.local_capable
    }

    /// Validate that the subsidiarity principle is respected.
    pub fn validate(&self) -> Result<(), crate::KingdomError> {
        if self.local_capable && self.proposed_level != GovernanceLevel::Community {
            return Err(crate::KingdomError::SubsidiarityViolation(format!(
                "{:?} decision can be handled at community level but proposed at {:?}",
                self.decision_type, self.proposed_level
            )));
        }
        Ok(())
    }
}

/// Level in the governance hierarchy.
///
/// From Constellation Art. 8 §2: "Communities form the foundation... Bioregional
/// councils coordinate... Continental assemblies address... Planetary convergences
/// convene only for issues affecting all Earth's peoples."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GovernanceLevel {
    Community,
    Bioregional,
    Continental,
    Planetary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_consortium() {
        let charter = ConsortiumCharter::new("Bioregional water stewardship");
        let c = Consortium::new("Watershed Council", "Protect the river", charter);
        assert!(c.is_active());
        assert_eq!(c.member_count(), 0);
    }

    #[test]
    fn add_and_remove_members() {
        let charter = ConsortiumCharter::new("Test");
        let mut c = Consortium::new("Test", "Test", charter);

        let community_a = Uuid::new_v4();
        let community_b = Uuid::new_v4();

        c.add_member(ConsortiumMember {
            community_id: community_a,
            joined_at: Utc::now(),
            delegates: Vec::new(),
        })
        .unwrap();

        c.add_member(ConsortiumMember {
            community_id: community_b,
            joined_at: Utc::now(),
            delegates: Vec::new(),
        })
        .unwrap();

        assert_eq!(c.member_count(), 2);
        assert!(c.is_member(&community_a));

        // Duplicate fails
        assert!(c
            .add_member(ConsortiumMember {
                community_id: community_a,
                joined_at: Utc::now(),
                delegates: Vec::new(),
            })
            .is_err());

        c.remove_member(&community_a).unwrap();
        assert_eq!(c.member_count(), 1);
        assert!(!c.is_member(&community_a));
    }

    #[test]
    fn subsidiarity_check_passes() {
        let check = SubsidiarityCheck {
            decision_type: ProposalType::Standard,
            proposed_level: GovernanceLevel::Community,
            local_capable: true,
            justification: None,
        };
        assert!(check.validate().is_ok());
        assert!(!check.should_elevate());
    }

    #[test]
    fn subsidiarity_check_fails() {
        let check = SubsidiarityCheck {
            decision_type: ProposalType::Policy,
            proposed_level: GovernanceLevel::Continental,
            local_capable: true,
            justification: None,
        };
        assert!(check.validate().is_err());
    }

    #[test]
    fn subsidiarity_elevation() {
        let check = SubsidiarityCheck {
            decision_type: ProposalType::Federation,
            proposed_level: GovernanceLevel::Bioregional,
            local_capable: false,
            justification: Some("Affects multiple communities".into()),
        };
        assert!(check.should_elevate());
        assert!(check.validate().is_ok());
    }

    #[test]
    fn consortium_charter_defaults() {
        let charter = ConsortiumCharter::new("Test");
        assert!(charter.delegate_selection.allows_recall);
        assert_eq!(charter.delegate_selection.max_delegates_per_community, 2);
        assert!(charter.membership_process.requires_member_approval);
    }

    #[test]
    fn voting_models() {
        let gov = ConsortiumGovernance::default();
        assert_eq!(gov.voting_model, VotingModel::OneCommunityOneVote);
    }
}
