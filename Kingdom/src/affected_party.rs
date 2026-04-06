//! # Affected-Party Consent (R3B)
//!
//! Proposals that specifically affect a minority group require that group's consent,
//! not just majority approval. A block vote from the affected party prevents passage
//! regardless of majority support.
//!
//! From Constellation Art. 8 SS4: "Dissenting communities retain the right to abstain
//! or propose alternatives." Extended here: affected minorities get a consent gate.
//!
//! ## How affected parties are identified
//!
//! - Proposal author tags affected parties when creating the proposal.
//! - Any community member can add an `AffectedPartyTag` during the Discussion phase.
//! - Community governance can add tags during the Discussion phase.
//! - Advisor can suggest affected parties in its `ProposalAnalysis` (R1D).
//!
//! ## Integration
//!
//! When a `Proposal` has `AffectedPartyConsent` constraints, Kingdom's voting flow
//! adds a parallel consent vote for the affected group. The proposal requires BOTH
//! the community vote to pass AND the affected party consent. If the affected party
//! blocks, the proposal enters a mediation phase (`MediationStatus::Pending`) where
//! the proposer can modify the proposal to address the block.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::vote::{Vote, VotePosition};
use crate::KingdomError;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Identifies a group affected by a proposal and describes the impact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AffectedPartyTag {
    /// Identifier for the affected group (e.g. a community name or role category).
    pub group_identifier: String,
    /// Pubkeys of the affected members.
    pub affected_members: Vec<String>,
    /// Human-readable description of how this group is affected.
    pub impact_description: String,
}

impl AffectedPartyTag {
    pub fn new(
        group_identifier: impl Into<String>,
        affected_members: Vec<String>,
        impact_description: impl Into<String>,
    ) -> Self {
        Self {
            group_identifier: group_identifier.into(),
            affected_members,
            impact_description: impact_description.into(),
        }
    }

    /// Whether a pubkey is a member of this affected group.
    pub fn is_affected(&self, pubkey: &str) -> bool {
        self.affected_members.iter().any(|m| m == pubkey)
    }
}

/// A constraint that may be attached to a proposal to require additional safeguards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProposalConstraint {
    /// The affected group gets a separate consent vote. If the group blocks
    /// (`VotePosition::Block`), the proposal cannot pass regardless of majority.
    AffectedPartyConsent(AffectedPartyTag),

    /// No Advisor delegation for this proposal — humans must vote directly.
    HumanVoteRequired,

    /// Override the community's default decision process threshold.
    SuperMajorityRequired(f64),

    /// Minimum discussion period (in seconds) before voting may open.
    DeliberationMinimum(u64),
}

impl ProposalConstraint {
    /// Convenience: create an affected-party consent constraint.
    pub fn affected_party(tag: AffectedPartyTag) -> Self {
        Self::AffectedPartyConsent(tag)
    }

    /// Convenience: create a deliberation-minimum constraint from a `Duration`.
    pub fn deliberation(duration: Duration) -> Self {
        Self::DeliberationMinimum(duration.as_secs())
    }
}

// ---------------------------------------------------------------------------
// Affected-party vote
// ---------------------------------------------------------------------------

/// The result of a separate consent vote by an affected party group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AffectedPartyVote {
    pub id: Uuid,
    /// Which affected group this vote represents.
    pub group_identifier: String,
    /// Individual votes cast by members of the affected group.
    pub votes: Vec<Vote>,
    /// True when no Block votes exist among the group's votes.
    pub consent_given: bool,
    /// Block votes with reasoning — the voices that must be heard.
    pub blockers: Vec<Vote>,
}

impl AffectedPartyVote {
    /// Create a new (empty) affected-party vote for the given group.
    pub fn new(group_identifier: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            group_identifier: group_identifier.into(),
            votes: Vec::new(),
            consent_given: true,
            blockers: Vec::new(),
        }
    }

    /// Cast a vote from a member of the affected group. Prevents duplicates.
    pub fn cast(&mut self, vote: Vote) -> Result<(), KingdomError> {
        if self.has_voted(&vote.voter) {
            return Err(KingdomError::AlreadyVoted(vote.voter.clone()));
        }
        if vote.position == VotePosition::Block {
            self.blockers.push(vote.clone());
            self.consent_given = false;
        }
        self.votes.push(vote);
        Ok(())
    }

    /// Whether a member has already cast a vote.
    pub fn has_voted(&self, pubkey: &str) -> bool {
        self.votes.iter().any(|v| v.voter == pubkey)
    }

    /// Recompute consent status from current votes.
    pub fn recompute_consent(&mut self) {
        self.blockers = self
            .votes
            .iter()
            .filter(|v| v.position == VotePosition::Block)
            .cloned()
            .collect();
        self.consent_given = self.blockers.is_empty();
    }

    /// Number of votes cast.
    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }

    /// Number of blockers.
    pub fn blocker_count(&self) -> usize {
        self.blockers.len()
    }
}

// ---------------------------------------------------------------------------
// Mediation
// ---------------------------------------------------------------------------

/// Status of the mediation phase triggered when an affected party blocks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MediationStatus {
    /// Mediation is needed but has not yet started.
    Pending,
    /// Proposer is revising the proposal to address the block.
    InProgress,
    /// Affected party accepted the revised proposal.
    Resolved,
    /// Mediation failed — the proposal cannot proceed.
    Failed,
}

/// Record of a mediation process between proposer and an affected party.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediationRecord {
    pub id: Uuid,
    pub proposal_id: Uuid,
    pub group_identifier: String,
    pub status: MediationStatus,
    /// The concerns raised by blockers.
    pub concerns: Vec<String>,
    /// Revisions proposed during mediation.
    pub revisions: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl MediationRecord {
    pub fn new(
        proposal_id: Uuid,
        group_identifier: impl Into<String>,
        concerns: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            proposal_id,
            group_identifier: group_identifier.into(),
            status: MediationStatus::Pending,
            concerns,
            revisions: Vec::new(),
            started_at: Utc::now(),
            resolved_at: None,
        }
    }

    /// Begin the mediation process.
    pub fn begin(&mut self) -> Result<(), KingdomError> {
        if self.status != MediationStatus::Pending {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "InProgress".into(),
            });
        }
        self.status = MediationStatus::InProgress;
        Ok(())
    }

    /// Add a revision proposed during mediation.
    pub fn add_revision(&mut self, revision: impl Into<String>) {
        self.revisions.push(revision.into());
    }

    /// Mark mediation as resolved — affected party accepted.
    pub fn resolve(&mut self) -> Result<(), KingdomError> {
        if self.status != MediationStatus::InProgress {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Resolved".into(),
            });
        }
        self.status = MediationStatus::Resolved;
        self.resolved_at = Some(Utc::now());
        Ok(())
    }

    /// Mark mediation as failed — impasse reached.
    pub fn fail(&mut self) -> Result<(), KingdomError> {
        if self.status != MediationStatus::InProgress {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Failed".into(),
            });
        }
        self.status = MediationStatus::Failed;
        self.resolved_at = Some(Utc::now());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Constraint evaluation
// ---------------------------------------------------------------------------

/// Evaluate whether all affected-party constraints on a proposal are satisfied.
///
/// Returns `Ok(())` if all affected parties have given consent, or an error
/// describing which groups blocked.
pub fn evaluate_affected_party_constraints(
    constraints: &[ProposalConstraint],
    group_votes: &[AffectedPartyVote],
) -> Result<(), KingdomError> {
    for constraint in constraints {
        if let ProposalConstraint::AffectedPartyConsent(tag) = constraint {
            let vote = group_votes
                .iter()
                .find(|v| v.group_identifier == tag.group_identifier);

            match vote {
                Some(v) if !v.consent_given => {
                    return Err(KingdomError::AffectedPartyBlocked {
                        group: tag.group_identifier.clone(),
                        blocker_count: v.blocker_count(),
                    });
                }
                None => {
                    return Err(KingdomError::AffectedPartyNotVoted(
                        tag.group_identifier.clone(),
                    ));
                }
                _ => {} // consent given
            }
        }
    }
    Ok(())
}

/// Check whether a deliberation minimum constraint is satisfied.
///
/// `discussion_opened_at` is when the proposal entered the Discussion phase.
pub fn check_deliberation_minimum(
    constraints: &[ProposalConstraint],
    discussion_opened_at: DateTime<Utc>,
) -> Result<(), KingdomError> {
    let now = Utc::now();
    for constraint in constraints {
        if let ProposalConstraint::DeliberationMinimum(min_secs) = constraint {
            let elapsed = now.signed_duration_since(discussion_opened_at);
            let required = chrono::Duration::seconds(*min_secs as i64);
            if elapsed < required {
                return Err(KingdomError::DeliberationMinimumNotMet {
                    required_secs: *min_secs,
                    elapsed_secs: elapsed.num_seconds().max(0) as u64,
                });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tag(group: &str, members: &[&str]) -> AffectedPartyTag {
        AffectedPartyTag::new(
            group,
            members.iter().map(|s| s.to_string()).collect(),
            format!("Impact on {group}"),
        )
    }

    fn make_vote(voter: &str, proposal_id: Uuid, position: VotePosition) -> Vote {
        Vote::new(voter, proposal_id, position)
    }

    // --- AffectedPartyTag ---

    #[test]
    fn tag_creation() {
        let tag = make_tag("elders", &["alice", "bob"]);
        assert_eq!(tag.group_identifier, "elders");
        assert_eq!(tag.affected_members.len(), 2);
        assert!(tag.is_affected("alice"));
        assert!(!tag.is_affected("charlie"));
    }

    #[test]
    fn tag_empty_members() {
        let tag = make_tag("nobody", &[]);
        assert!(!tag.is_affected("anyone"));
        assert!(tag.affected_members.is_empty());
    }

    #[test]
    fn tag_serialization_roundtrip() {
        let tag = make_tag("workers", &["a", "b", "c"]);
        let json = serde_json::to_string(&tag).unwrap();
        let restored: AffectedPartyTag = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, restored);
    }

    // --- ProposalConstraint ---

    #[test]
    fn constraint_affected_party() {
        let tag = make_tag("group", &["alice"]);
        let c = ProposalConstraint::affected_party(tag.clone());
        assert!(matches!(c, ProposalConstraint::AffectedPartyConsent(_)));
    }

    #[test]
    fn constraint_human_vote_required() {
        let c = ProposalConstraint::HumanVoteRequired;
        assert!(matches!(c, ProposalConstraint::HumanVoteRequired));
    }

    #[test]
    fn constraint_supermajority() {
        let c = ProposalConstraint::SuperMajorityRequired(0.75);
        if let ProposalConstraint::SuperMajorityRequired(t) = c {
            assert!((t - 0.75).abs() < f64::EPSILON);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn constraint_deliberation_from_duration() {
        let c = ProposalConstraint::deliberation(Duration::from_secs(86400));
        if let ProposalConstraint::DeliberationMinimum(s) = c {
            assert_eq!(s, 86400);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn constraint_serialization_roundtrip() {
        let constraints = vec![
            ProposalConstraint::HumanVoteRequired,
            ProposalConstraint::SuperMajorityRequired(0.8),
            ProposalConstraint::DeliberationMinimum(3600),
            ProposalConstraint::AffectedPartyConsent(make_tag("g", &["x"])),
        ];
        let json = serde_json::to_string(&constraints).unwrap();
        let restored: Vec<ProposalConstraint> = serde_json::from_str(&json).unwrap();
        assert_eq!(constraints, restored);
    }

    // --- AffectedPartyVote ---

    #[test]
    fn consent_voting_no_blocks() {
        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("elders");

        apv.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();
        apv.cast(make_vote("bob", pid, VotePosition::Support)).unwrap();
        apv.cast(make_vote("charlie", pid, VotePosition::StandAside)).unwrap();

        assert!(apv.consent_given);
        assert_eq!(apv.vote_count(), 3);
        assert_eq!(apv.blocker_count(), 0);
    }

    #[test]
    fn consent_voting_with_block() {
        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("workers");

        apv.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();
        apv.cast(make_vote("bob", pid, VotePosition::Block)
            .with_reason("This harms our working conditions"))
            .unwrap();

        assert!(!apv.consent_given);
        assert_eq!(apv.blocker_count(), 1);
        assert_eq!(apv.blockers[0].voter, "bob");
    }

    #[test]
    fn cannot_vote_twice() {
        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("group");
        apv.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();
        assert!(apv.cast(make_vote("alice", pid, VotePosition::Block)).is_err());
    }

    #[test]
    fn recompute_consent() {
        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("group");
        apv.cast(make_vote("alice", pid, VotePosition::Block)).unwrap();
        assert!(!apv.consent_given);

        // Simulate removing the block vote externally and recomputing
        apv.votes.retain(|v| v.voter != "alice");
        apv.recompute_consent();
        assert!(apv.consent_given);
        assert_eq!(apv.blocker_count(), 0);
    }

    #[test]
    fn affected_party_vote_serialization() {
        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("test_group");
        apv.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();

        let json = serde_json::to_string(&apv).unwrap();
        let restored: AffectedPartyVote = serde_json::from_str(&json).unwrap();
        assert_eq!(apv.group_identifier, restored.group_identifier);
        assert_eq!(apv.consent_given, restored.consent_given);
    }

    // --- MediationRecord ---

    #[test]
    fn mediation_lifecycle_happy_path() {
        let pid = Uuid::new_v4();
        let mut med = MediationRecord::new(pid, "elders", vec!["Concerns about impact".into()]);
        assert_eq!(med.status, MediationStatus::Pending);

        med.begin().unwrap();
        assert_eq!(med.status, MediationStatus::InProgress);

        med.add_revision("Added exemption clause for elders");
        assert_eq!(med.revisions.len(), 1);

        med.resolve().unwrap();
        assert_eq!(med.status, MediationStatus::Resolved);
        assert!(med.resolved_at.is_some());
    }

    #[test]
    fn mediation_failure() {
        let pid = Uuid::new_v4();
        let mut med = MediationRecord::new(pid, "workers", vec!["Irreversible harm".into()]);
        med.begin().unwrap();
        med.fail().unwrap();
        assert_eq!(med.status, MediationStatus::Failed);
        assert!(med.resolved_at.is_some());
    }

    #[test]
    fn mediation_invalid_transitions() {
        let pid = Uuid::new_v4();
        let mut med = MediationRecord::new(pid, "group", vec![]);

        // Cannot resolve from Pending
        assert!(med.resolve().is_err());
        // Cannot fail from Pending
        assert!(med.fail().is_err());

        med.begin().unwrap();
        // Cannot begin again
        assert!(med.begin().is_err());
    }

    #[test]
    fn mediation_serialization() {
        let med = MediationRecord::new(Uuid::new_v4(), "g", vec!["concern".into()]);
        let json = serde_json::to_string(&med).unwrap();
        let restored: MediationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(med.group_identifier, restored.group_identifier);
    }

    // --- Constraint evaluation ---

    #[test]
    fn evaluate_all_consent_given() {
        let tag = make_tag("elders", &["alice", "bob"]);
        let constraints = vec![ProposalConstraint::AffectedPartyConsent(tag)];

        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("elders");
        apv.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();
        apv.cast(make_vote("bob", pid, VotePosition::Support)).unwrap();

        assert!(evaluate_affected_party_constraints(&constraints, &[apv]).is_ok());
    }

    #[test]
    fn evaluate_blocked_by_affected_party() {
        let tag = make_tag("workers", &["charlie"]);
        let constraints = vec![ProposalConstraint::AffectedPartyConsent(tag)];

        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("workers");
        apv.cast(make_vote("charlie", pid, VotePosition::Block)).unwrap();

        let result = evaluate_affected_party_constraints(&constraints, &[apv]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            KingdomError::AffectedPartyBlocked { group, blocker_count }
                if group == "workers" && blocker_count == 1
        ));
    }

    #[test]
    fn evaluate_missing_group_vote() {
        let tag = make_tag("elders", &["alice"]);
        let constraints = vec![ProposalConstraint::AffectedPartyConsent(tag)];

        // No group vote provided at all
        let result = evaluate_affected_party_constraints(&constraints, &[]);
        assert!(matches!(
            result.unwrap_err(),
            KingdomError::AffectedPartyNotVoted(g) if g == "elders"
        ));
    }

    #[test]
    fn evaluate_multiple_affected_parties() {
        let tag1 = make_tag("elders", &["alice"]);
        let tag2 = make_tag("workers", &["bob"]);
        let constraints = vec![
            ProposalConstraint::AffectedPartyConsent(tag1),
            ProposalConstraint::AffectedPartyConsent(tag2),
        ];

        let pid = Uuid::new_v4();
        let mut apv1 = AffectedPartyVote::new("elders");
        apv1.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();

        let mut apv2 = AffectedPartyVote::new("workers");
        apv2.cast(make_vote("bob", pid, VotePosition::Support)).unwrap();

        assert!(evaluate_affected_party_constraints(&constraints, &[apv1, apv2]).is_ok());
    }

    #[test]
    fn evaluate_one_of_multiple_blocks() {
        let tag1 = make_tag("elders", &["alice"]);
        let tag2 = make_tag("workers", &["bob"]);
        let constraints = vec![
            ProposalConstraint::AffectedPartyConsent(tag1),
            ProposalConstraint::AffectedPartyConsent(tag2),
        ];

        let pid = Uuid::new_v4();
        let mut apv1 = AffectedPartyVote::new("elders");
        apv1.cast(make_vote("alice", pid, VotePosition::Support)).unwrap();

        let mut apv2 = AffectedPartyVote::new("workers");
        apv2.cast(make_vote("bob", pid, VotePosition::Block)).unwrap();

        let result = evaluate_affected_party_constraints(&constraints, &[apv1, apv2]);
        assert!(result.is_err());
    }

    #[test]
    fn evaluate_non_affected_party_constraints_ignored() {
        let constraints = vec![
            ProposalConstraint::HumanVoteRequired,
            ProposalConstraint::SuperMajorityRequired(0.75),
        ];
        // Should pass — no affected-party constraints to check.
        assert!(evaluate_affected_party_constraints(&constraints, &[]).is_ok());
    }

    #[test]
    fn block_prevents_passage_regardless_of_majority_support() {
        // Core invariant: even if 99% of the community supports,
        // an affected party block halts the proposal.
        let tag = make_tag("minority_group", &["eve"]);
        let constraints = vec![ProposalConstraint::AffectedPartyConsent(tag)];

        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("minority_group");
        apv.cast(make_vote("eve", pid, VotePosition::Block)
            .with_reason("This erases our cultural practices"))
            .unwrap();

        let result = evaluate_affected_party_constraints(&constraints, &[apv]);
        assert!(result.is_err());
    }

    #[test]
    fn deliberation_minimum_not_met() {
        let constraints = vec![ProposalConstraint::DeliberationMinimum(86400)]; // 1 day
        // Opened just now
        let opened = Utc::now();
        let result = check_deliberation_minimum(&constraints, opened);
        assert!(result.is_err());
    }

    #[test]
    fn deliberation_minimum_met() {
        let constraints = vec![ProposalConstraint::DeliberationMinimum(60)]; // 60 seconds
        // Opened 2 minutes ago
        let opened = Utc::now() - chrono::Duration::seconds(120);
        let result = check_deliberation_minimum(&constraints, opened);
        assert!(result.is_ok());
    }

    #[test]
    fn deliberation_no_constraint_always_passes() {
        let constraints = vec![ProposalConstraint::HumanVoteRequired];
        let result = check_deliberation_minimum(&constraints, Utc::now());
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_blockers_all_recorded() {
        let pid = Uuid::new_v4();
        let mut apv = AffectedPartyVote::new("group");
        apv.cast(make_vote("alice", pid, VotePosition::Block)
            .with_reason("Concern A"))
            .unwrap();
        apv.cast(make_vote("bob", pid, VotePosition::Block)
            .with_reason("Concern B"))
            .unwrap();
        apv.cast(make_vote("charlie", pid, VotePosition::Support)).unwrap();

        assert_eq!(apv.blocker_count(), 2);
        assert!(!apv.consent_given);
        assert_eq!(apv.vote_count(), 3);
    }
}
