use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::vote::{QuorumRequirement, Vote, VotePosition, VoteTally};

/// A proposal for community decision.
///
/// From Constellation Art. 3 §1: "Every community shall hold the lawful authority
/// to enact agreements, declarations, compacts, and collective decisions."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Proposal {
    pub id: Uuid,
    pub author: String,
    pub deciding_body: DecidingBody,
    pub title: String,
    pub body: String,
    pub proposal_type: ProposalType,
    pub decision_process: String,
    pub quorum: QuorumRequirement,
    pub status: ProposalStatus,
    pub votes: Vec<Vote>,
    pub discussion: Vec<DiscussionPost>,
    pub outcome: Option<ProposalOutcome>,
    pub created_at: DateTime<Utc>,
    pub voting_opens: Option<DateTime<Utc>>,
    pub voting_closes: Option<DateTime<Utc>>,
}

impl Proposal {
    /// Create a new proposal in Draft status with standard defaults.
    pub fn new(
        author: impl Into<String>,
        deciding_body: DecidingBody,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            author: author.into(),
            deciding_body,
            title: title.into(),
            body: body.into(),
            proposal_type: ProposalType::Standard,
            decision_process: "direct_vote".into(),
            quorum: QuorumRequirement::majority(),
            status: ProposalStatus::Draft,
            votes: Vec::new(),
            discussion: Vec::new(),
            outcome: None,
            created_at: Utc::now(),
            voting_opens: None,
            voting_closes: None,
        }
    }

    /// Set the proposal type (builder pattern).
    pub fn with_type(mut self, proposal_type: ProposalType) -> Self {
        self.proposal_type = proposal_type;
        self
    }

    /// Set the decision process by name (builder pattern).
    pub fn with_decision_process(mut self, process: impl Into<String>) -> Self {
        self.decision_process = process.into();
        self
    }

    /// Set the quorum requirement (builder pattern).
    pub fn with_quorum(mut self, quorum: QuorumRequirement) -> Self {
        self.quorum = quorum;
        self
    }

    /// Open discussion period.
    pub fn open_discussion(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != ProposalStatus::Draft {
            return Err(crate::KingdomError::InvalidProposalStatus {
                expected: "Draft".into(),
                actual: format!("{:?}", self.status),
            });
        }
        self.status = ProposalStatus::Discussion;
        Ok(())
    }

    /// Open voting period.
    pub fn open_voting(
        &mut self,
        closes_at: DateTime<Utc>,
    ) -> Result<(), crate::KingdomError> {
        if !matches!(
            self.status,
            ProposalStatus::Draft | ProposalStatus::Discussion
        ) {
            return Err(crate::KingdomError::InvalidProposalStatus {
                expected: "Draft or Discussion".into(),
                actual: format!("{:?}", self.status),
            });
        }
        self.status = ProposalStatus::Voting;
        self.voting_opens = Some(Utc::now());
        self.voting_closes = Some(closes_at);
        Ok(())
    }

    /// Add a discussion post.
    pub fn add_discussion(&mut self, post: DiscussionPost) {
        self.discussion.push(post);
    }

    /// Cast a vote. Prevents duplicates.
    pub fn add_vote(&mut self, vote: Vote) -> Result<(), crate::KingdomError> {
        if self.status != ProposalStatus::Voting {
            return Err(crate::KingdomError::VotingNotOpen);
        }
        if self.has_voted(&vote.voter) {
            return Err(crate::KingdomError::AlreadyVoted(vote.voter.clone()));
        }
        self.votes.push(vote);
        Ok(())
    }

    /// Whether a voter has already cast a vote on this proposal.
    pub fn has_voted(&self, pubkey: &str) -> bool {
        self.votes.iter().any(|v| v.voter == pubkey)
    }

    /// Look up the vote cast by a specific voter.
    pub fn vote_for(&self, pubkey: &str) -> Option<&Vote> {
        self.votes.iter().find(|v| v.voter == pubkey)
    }

    /// Get a tally of current votes.
    pub fn tally(&self, eligible_voters: u32) -> VoteTally {
        VoteTally::from_votes(self.id, &self.votes, eligible_voters)
    }

    /// Resolve the proposal with an outcome.
    pub fn resolve(&mut self, outcome: ProposalOutcome) -> Result<(), crate::KingdomError> {
        if self.status != ProposalStatus::Voting {
            return Err(crate::KingdomError::InvalidProposalStatus {
                expected: "Voting".into(),
                actual: format!("{:?}", self.status),
            });
        }
        self.status = ProposalStatus::Resolved;
        self.outcome = Some(outcome);
        Ok(())
    }

    /// Withdraw the proposal (author only).
    pub fn withdraw(&mut self) -> Result<(), crate::KingdomError> {
        if matches!(
            self.status,
            ProposalStatus::Resolved | ProposalStatus::Withdrawn
        ) {
            return Err(crate::KingdomError::InvalidProposalStatus {
                expected: "Draft, Discussion, or Voting".into(),
                actual: format!("{:?}", self.status),
            });
        }
        self.status = ProposalStatus::Withdrawn;
        Ok(())
    }

    /// Whether the proposal is still in Draft and can be edited.
    pub fn can_edit(&self) -> bool {
        self.status == ProposalStatus::Draft
    }

    /// Whether voting is currently open on this proposal.
    pub fn is_voting_open(&self) -> bool {
        self.status == ProposalStatus::Voting
    }

    /// Count how many votes have been cast for a specific position.
    pub fn vote_count(&self, position: VotePosition) -> usize {
        self.votes.iter().filter(|v| v.position == position).count()
    }
}

/// What body is deciding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DecidingBody {
    /// A single community votes on this proposal.
    Community(Uuid),
    /// A union's members vote on this proposal.
    Union(Uuid),
    /// A consortium (federation) votes on this proposal.
    Consortium(Uuid),
}

impl DecidingBody {
    /// The ID of the body this proposal belongs to, regardless of variant.
    pub fn id(&self) -> &Uuid {
        match self {
            DecidingBody::Community(id)
            | DecidingBody::Union(id)
            | DecidingBody::Consortium(id) => id,
        }
    }
}

/// Category of proposal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProposalType {
    /// General-purpose proposal.
    Standard,
    /// Financial or resource allocation decision.
    Treasury,
    /// Change to community rules or governance structure.
    Policy,
    /// Amendment to the community charter.
    Amendment,
    /// Election of members to governance roles.
    Election,
    /// Decision about membership (admission, removal, role change).
    Membership,
    /// Decision about joining, leaving, or modifying a federation.
    Federation,
    /// Urgent decision requiring expedited process.
    Emergency,
}

/// Lifecycle of a proposal.
///
/// From Constellation Art. 3 §4: "For a collective act to carry lawful standing,
/// it shall be undertaken with open process, informed participation."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProposalStatus {
    /// Being drafted, can still be edited.
    Draft,
    /// Open for discussion, not yet votable.
    Discussion,
    /// Voting is open.
    Voting,
    /// Decision reached.
    Resolved,
    /// Pulled by the author.
    Withdrawn,
    /// Under challenge (see challenge.rs).
    Challenged,
}

/// The result of a resolved proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposalOutcome {
    pub result: crate::decision::ProposalResult,
    pub eligible_voters: u32,
    pub votes_cast: u32,
    pub support_votes: f64,
    pub oppose_votes: f64,
    pub block_votes: u32,
    pub quorum_met: bool,
    pub decided_at: DateTime<Utc>,
    pub notes: Option<String>,
}

impl ProposalOutcome {
    /// Build an outcome from a vote tally, decision result, and quorum requirement.
    pub fn from_tally(
        tally: &VoteTally,
        result: crate::decision::ProposalResult,
        quorum: &QuorumRequirement,
    ) -> Self {
        Self {
            result,
            eligible_voters: tally.eligible_voters,
            votes_cast: tally.votes_cast,
            support_votes: tally.support,
            oppose_votes: tally.oppose,
            block_votes: tally.block,
            quorum_met: tally.meets_quorum(quorum),
            decided_at: Utc::now(),
            notes: None,
        }
    }

    /// Fraction of eligible voters who participated.
    pub fn participation_rate(&self) -> f64 {
        if self.eligible_voters == 0 {
            return 0.0;
        }
        f64::from(self.votes_cast) / f64::from(self.eligible_voters)
    }

    /// Fraction of support among yes+no votes (excludes abstentions).
    pub fn support_rate(&self) -> f64 {
        let total = self.support_votes + self.oppose_votes;
        if total == 0.0 {
            return 0.0;
        }
        self.support_votes / total
    }
}

/// A post in a proposal's discussion thread.
///
/// From Convocation Art. 5 §2: "Convocations shall employ equitable and culturally
/// appropriate methods of speaking, listening, sharing, and deciding."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscussionPost {
    pub id: Uuid,
    pub author: String,
    pub content: String,
    pub reply_to: Option<Uuid>,
    pub posted_at: DateTime<Utc>,
}

impl DiscussionPost {
    pub fn new(author: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            author: author.into(),
            content: content.into(),
            reply_to: None,
            posted_at: Utc::now(),
        }
    }

    pub fn reply(author: impl Into<String>, content: impl Into<String>, reply_to: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            author: author.into(),
            content: content.into(),
            reply_to: Some(reply_to),
            posted_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_proposal() -> Proposal {
        Proposal::new(
            "alice",
            DecidingBody::Community(Uuid::new_v4()),
            "Build a garden",
            "Let's create a community garden in the vacant lot",
        )
    }

    #[test]
    fn proposal_lifecycle_happy_path() {
        let mut p = test_proposal();
        assert!(p.can_edit());
        assert_eq!(p.status, ProposalStatus::Draft);

        p.open_discussion().unwrap();
        assert_eq!(p.status, ProposalStatus::Discussion);
        assert!(!p.can_edit());

        let closes = Utc::now() + chrono::Duration::days(7);
        p.open_voting(closes).unwrap();
        assert!(p.is_voting_open());

        p.add_vote(Vote::new("alice", p.id, VotePosition::Support)).unwrap();
        p.add_vote(Vote::new("bob", p.id, VotePosition::Support)).unwrap();
        p.add_vote(Vote::new("charlie", p.id, VotePosition::Oppose)).unwrap();

        assert_eq!(p.votes.len(), 3);
        assert_eq!(p.vote_count(VotePosition::Support), 2);

        let tally = p.tally(5);
        let outcome = ProposalOutcome::from_tally(
            &tally,
            crate::decision::ProposalResult::Passed,
            &p.quorum,
        );
        p.resolve(outcome).unwrap();
        assert_eq!(p.status, ProposalStatus::Resolved);
        assert!(p.outcome.is_some());
    }

    #[test]
    fn cannot_vote_when_not_open() {
        let mut p = test_proposal();
        let vote = Vote::new("alice", p.id, VotePosition::Support);
        assert!(p.add_vote(vote).is_err());
    }

    #[test]
    fn cannot_vote_twice() {
        let mut p = test_proposal();
        let closes = Utc::now() + chrono::Duration::days(7);
        p.open_voting(closes).unwrap();

        p.add_vote(Vote::new("alice", p.id, VotePosition::Support)).unwrap();
        assert!(p.add_vote(Vote::new("alice", p.id, VotePosition::Oppose)).is_err());
    }

    #[test]
    fn withdraw_proposal() {
        let mut p = test_proposal();
        p.withdraw().unwrap();
        assert_eq!(p.status, ProposalStatus::Withdrawn);

        // Can't withdraw again
        assert!(p.withdraw().is_err());
    }

    #[test]
    fn cannot_resolve_non_voting_proposal() {
        let mut p = test_proposal();
        let outcome = ProposalOutcome {
            result: crate::decision::ProposalResult::Passed,
            eligible_voters: 5,
            votes_cast: 3,
            support_votes: 2.0,
            oppose_votes: 1.0,
            block_votes: 0,
            quorum_met: true,
            decided_at: Utc::now(),
            notes: None,
        };
        assert!(p.resolve(outcome).is_err());
    }

    #[test]
    fn discussion_posts() {
        let mut p = test_proposal();
        p.open_discussion().unwrap();

        let post = DiscussionPost::new("alice", "I think this is a great idea!");
        let post_id = post.id;
        p.add_discussion(post);

        let reply = DiscussionPost::reply("bob", "I agree!", post_id);
        p.add_discussion(reply);

        assert_eq!(p.discussion.len(), 2);
        assert_eq!(p.discussion[1].reply_to, Some(post_id));
    }

    #[test]
    fn proposal_outcome_rates() {
        let outcome = ProposalOutcome {
            result: crate::decision::ProposalResult::Passed,
            eligible_voters: 10,
            votes_cast: 8,
            support_votes: 6.0,
            oppose_votes: 2.0,
            block_votes: 0,
            quorum_met: true,
            decided_at: Utc::now(),
            notes: None,
        };

        assert_eq!(outcome.participation_rate(), 0.8);
        assert_eq!(outcome.support_rate(), 0.75);
    }

    #[test]
    fn deciding_body_variants() {
        let cid = Uuid::new_v4();
        let body = DecidingBody::Community(cid);
        assert_eq!(*body.id(), cid);

        let uid = Uuid::new_v4();
        let body = DecidingBody::Union(uid);
        assert_eq!(*body.id(), uid);
    }

    #[test]
    fn proposal_builder_pattern() {
        let p = Proposal::new("alice", DecidingBody::Community(Uuid::new_v4()), "T", "B")
            .with_type(ProposalType::Amendment)
            .with_decision_process("supermajority")
            .with_quorum(QuorumRequirement::supermajority());

        assert_eq!(p.proposal_type, ProposalType::Amendment);
        assert_eq!(p.decision_process, "supermajority");
        assert_eq!(p.quorum.approval, 0.667);
    }

    #[test]
    fn proposal_serialization_roundtrip() {
        let p = test_proposal();
        let json = serde_json::to_string(&p).unwrap();
        let restored: Proposal = serde_json::from_str(&json).unwrap();
        assert_eq!(p.title, restored.title);
        assert_eq!(p.author, restored.author);
    }

    #[test]
    fn skip_discussion_go_straight_to_voting() {
        let mut p = test_proposal();
        let closes = Utc::now() + chrono::Duration::days(1);
        // Can go directly from Draft to Voting
        p.open_voting(closes).unwrap();
        assert!(p.is_voting_open());
    }
}
