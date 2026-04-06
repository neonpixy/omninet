use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single vote cast on a proposal.
///
/// From Constellation Art. 4 §1: "Every person and every community shall hold
/// the inalienable right to challenge any structure of governance."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vote {
    pub id: Uuid,
    pub voter: String,
    pub proposal_id: Uuid,
    pub position: VotePosition,
    pub reason: Option<String>,
    pub weight: f64,
    pub delegate_info: Option<DelegateVoteInfo>,
    pub voted_at: DateTime<Utc>,
}

impl Vote {
    /// Create a new vote with default weight of 1.0 and no delegate info.
    pub fn new(voter: impl Into<String>, proposal_id: Uuid, position: VotePosition) -> Self {
        Self {
            id: Uuid::new_v4(),
            voter: voter.into(),
            proposal_id,
            position,
            reason: None,
            weight: 1.0,
            delegate_info: None,
            voted_at: Utc::now(),
        }
    }

    /// Attach a reason explaining this vote (builder pattern).
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the vote weight, used for weighted voting and liquid democracy (builder pattern).
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// Attach delegate info when this vote is cast on behalf of a community (builder pattern).
    pub fn with_delegate_info(mut self, info: DelegateVoteInfo) -> Self {
        self.delegate_info = Some(info);
        self
    }
}

/// How a person votes.
///
/// From Constellation Art. 8 §4: "Dissenting communities retain the right
/// to abstain or propose alternatives."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VotePosition {
    /// In favor.
    Support,
    /// Against.
    Oppose,
    /// Neither for nor against.
    Abstain,
    /// Strong objection — blocks consensus.
    Block,
    /// Defers to the group (doesn't prevent consensus).
    StandAside,
}

impl VotePosition {
    /// Whether this position counts as a vote in favor.
    pub fn is_positive(&self) -> bool {
        matches!(self, VotePosition::Support)
    }

    /// Whether this position counts as a vote against (Oppose or Block).
    pub fn is_negative(&self) -> bool {
        matches!(self, VotePosition::Oppose | VotePosition::Block)
    }

    /// Whether this position blocks consensus (only Block does).
    pub fn prevents_consensus(&self) -> bool {
        matches!(self, VotePosition::Block)
    }
}

/// Information about a vote cast by a delegate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegateVoteInfo {
    pub delegate_id: Uuid,
    pub represents_community: Uuid,
    pub consulted_community: bool,
    pub consultation_result: Option<ConsultationResult>,
}

/// Result of a delegate consulting their community before voting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsultationResult {
    pub responses: u32,
    pub support: u32,
    pub oppose: u32,
    pub abstain: u32,
    pub summary: Option<String>,
    pub consulted_at: DateTime<Utc>,
}

impl ConsultationResult {
    /// The position held by the majority of consulted members.
    pub fn majority_position(&self) -> VotePosition {
        if self.support > self.oppose {
            VotePosition::Support
        } else if self.oppose > self.support {
            VotePosition::Oppose
        } else {
            VotePosition::Abstain
        }
    }
}

/// Aggregated vote counts for a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoteTally {
    pub proposal_id: Uuid,
    pub support: f64,
    pub oppose: f64,
    pub abstain: f64,
    pub block: u32,
    pub stand_aside: u32,
    pub eligible_voters: u32,
    pub votes_cast: u32,
}

impl VoteTally {
    /// Create an empty tally for a proposal with a known number of eligible voters.
    pub fn new(proposal_id: Uuid, eligible_voters: u32) -> Self {
        Self {
            proposal_id,
            support: 0.0,
            oppose: 0.0,
            abstain: 0.0,
            block: 0,
            stand_aside: 0,
            eligible_voters,
            votes_cast: 0,
        }
    }

    /// Build a tally from a set of votes.
    pub fn from_votes(proposal_id: Uuid, votes: &[Vote], eligible_voters: u32) -> Self {
        let mut tally = Self::new(proposal_id, eligible_voters);
        for vote in votes {
            match vote.position {
                VotePosition::Support => tally.support += vote.weight,
                VotePosition::Oppose => tally.oppose += vote.weight,
                VotePosition::Abstain => tally.abstain += vote.weight,
                VotePosition::Block => tally.block += 1,
                VotePosition::StandAside => tally.stand_aside += 1,
            }
            tally.votes_cast += 1;
        }
        tally
    }

    /// Fraction of eligible voters who cast a vote.
    pub fn participation(&self) -> f64 {
        if self.eligible_voters == 0 {
            return 0.0;
        }
        f64::from(self.votes_cast) / f64::from(self.eligible_voters)
    }

    /// Fraction of support among yes+no votes (excludes abstain/standAside).
    pub fn approval_rate(&self) -> f64 {
        let total = self.support + self.oppose;
        if total == 0.0 {
            return 0.0;
        }
        self.support / total
    }

    /// Whether any Block votes were cast.
    pub fn has_blocks(&self) -> bool {
        self.block > 0
    }

    /// Check if quorum requirements are met.
    pub fn meets_quorum(&self, required: &QuorumRequirement) -> bool {
        self.participation() >= required.participation && self.approval_rate() >= required.approval
    }
}

/// Minimum participation and approval needed for a decision.
///
/// From Constellation Art. 8 §4: "Where full consensus proves impossible,
/// super-majority thresholds ensure broad support."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuorumRequirement {
    /// Fraction of eligible voters who must participate (0.0 to 1.0).
    pub participation: f64,
    /// Fraction of yes/(yes+no) needed to pass (0.0 to 1.0).
    pub approval: f64,
}

impl QuorumRequirement {
    /// Create a quorum requirement, clamping both values to 0.0..=1.0.
    pub fn new(participation: f64, approval: f64) -> Self {
        Self {
            participation: participation.clamp(0.0, 1.0),
            approval: approval.clamp(0.0, 1.0),
        }
    }

    /// Simple majority: 50% participation, 50% approval.
    pub fn majority() -> Self {
        Self::new(0.5, 0.5)
    }

    /// Two-thirds supermajority.
    pub fn supermajority() -> Self {
        Self::new(0.5, 0.667)
    }

    /// Unanimous participation required, 100% approval.
    pub fn unanimous() -> Self {
        Self::new(1.0, 1.0)
    }

    /// One-third threshold (low bar).
    pub fn one_third() -> Self {
        Self::new(0.33, 0.5)
    }
}

impl Default for QuorumRequirement {
    fn default() -> Self {
        Self::majority()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_vote() {
        let pid = Uuid::new_v4();
        let vote = Vote::new("alice", pid, VotePosition::Support)
            .with_reason("Great idea")
            .with_weight(2.0);

        assert_eq!(vote.voter, "alice");
        assert_eq!(vote.position, VotePosition::Support);
        assert_eq!(vote.weight, 2.0);
        assert_eq!(vote.reason.as_deref(), Some("Great idea"));
    }

    #[test]
    fn vote_position_properties() {
        assert!(VotePosition::Support.is_positive());
        assert!(!VotePosition::Oppose.is_positive());
        assert!(VotePosition::Oppose.is_negative());
        assert!(VotePosition::Block.is_negative());
        assert!(VotePosition::Block.prevents_consensus());
        assert!(!VotePosition::Oppose.prevents_consensus());
        assert!(!VotePosition::StandAside.prevents_consensus());
    }

    #[test]
    fn tally_from_votes() {
        let pid = Uuid::new_v4();
        let votes = vec![
            Vote::new("a", pid, VotePosition::Support),
            Vote::new("b", pid, VotePosition::Support),
            Vote::new("c", pid, VotePosition::Oppose),
            Vote::new("d", pid, VotePosition::Abstain),
            Vote::new("e", pid, VotePosition::Block),
        ];

        let tally = VoteTally::from_votes(pid, &votes, 10);
        assert_eq!(tally.support, 2.0);
        assert_eq!(tally.oppose, 1.0);
        assert_eq!(tally.abstain, 1.0);
        assert_eq!(tally.block, 1);
        assert_eq!(tally.votes_cast, 5);
        assert_eq!(tally.participation(), 0.5);
        assert!((tally.approval_rate() - 0.6667).abs() < 0.01);
        assert!(tally.has_blocks());
    }

    #[test]
    fn weighted_tally() {
        let pid = Uuid::new_v4();
        let votes = vec![
            Vote::new("a", pid, VotePosition::Support).with_weight(3.0),
            Vote::new("b", pid, VotePosition::Oppose).with_weight(1.0),
        ];

        let tally = VoteTally::from_votes(pid, &votes, 4);
        assert_eq!(tally.support, 3.0);
        assert_eq!(tally.oppose, 1.0);
        assert_eq!(tally.approval_rate(), 0.75);
    }

    #[test]
    fn quorum_check() {
        let pid = Uuid::new_v4();
        let votes = vec![
            Vote::new("a", pid, VotePosition::Support),
            Vote::new("b", pid, VotePosition::Support),
            Vote::new("c", pid, VotePosition::Support),
            Vote::new("d", pid, VotePosition::Oppose),
            Vote::new("e", pid, VotePosition::Oppose),
        ];

        let tally = VoteTally::from_votes(pid, &votes, 10);
        // 50% participation, 60% approval
        assert!(tally.meets_quorum(&QuorumRequirement::majority()));
        assert!(!tally.meets_quorum(&QuorumRequirement::supermajority()));
    }

    #[test]
    fn empty_tally() {
        let tally = VoteTally::new(Uuid::new_v4(), 0);
        assert_eq!(tally.participation(), 0.0);
        assert_eq!(tally.approval_rate(), 0.0);
    }

    #[test]
    fn consultation_result() {
        let result = ConsultationResult {
            responses: 10,
            support: 7,
            oppose: 3,
            abstain: 0,
            summary: Some("Strong support".into()),
            consulted_at: Utc::now(),
        };
        assert_eq!(result.majority_position(), VotePosition::Support);
    }

    #[test]
    fn quorum_presets() {
        let q = QuorumRequirement::unanimous();
        assert_eq!(q.participation, 1.0);
        assert_eq!(q.approval, 1.0);

        let q = QuorumRequirement::one_third();
        assert_eq!(q.participation, 0.33);
    }
}
