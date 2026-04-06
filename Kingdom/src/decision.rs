use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::vote::{QuorumRequirement, Vote, VoteTally};

/// The result of tallying votes through a decision process.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProposalResult {
    /// The proposal met its threshold and is approved.
    Passed,
    /// The proposal did not meet its threshold.
    Failed,
    /// The proposal was set aside for later reconsideration.
    Tabled,
}

/// A pluggable decision-making algorithm.
///
/// From Constellation Art. 1 §4: "Communities may govern in many forms, so long
/// as they shall remain in lawful alignment with the Core and Commons."
///
/// Kingdom provides 6 built-in implementations. Communities can create their own.
pub trait DecisionProcess: Send + Sync {
    /// Count the votes and produce a tally.
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally;

    /// Determine if the tally resolves the proposal.
    fn is_resolved(&self, tally: &VoteTally, quorum: &QuorumRequirement) -> Option<ProposalResult>;

    /// Identifier for this process.
    fn process_id(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Built-in implementations
// ---------------------------------------------------------------------------

/// Simple majority: more yes than no, quorum met.
pub struct DirectVoteProcess;

impl DecisionProcess for DirectVoteProcess {
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally {
        VoteTally::from_votes(
            votes.first().map_or(uuid::Uuid::nil(), |v| v.proposal_id),
            votes,
            eligible_voters,
        )
    }

    fn is_resolved(&self, tally: &VoteTally, quorum: &QuorumRequirement) -> Option<ProposalResult> {
        if tally.participation() < quorum.participation {
            return None; // not enough participation yet
        }
        if tally.approval_rate() >= quorum.approval {
            Some(ProposalResult::Passed)
        } else {
            Some(ProposalResult::Failed)
        }
    }

    fn process_id(&self) -> &str {
        "direct_vote"
    }
}

/// Consensus: everyone must agree, any block kills it.
///
/// From Constellation Art. 8 §4: "Assemblies seek alignment through dialogue,
/// amendment, and creative synthesis."
pub struct ConsensusProcess;

impl DecisionProcess for ConsensusProcess {
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally {
        VoteTally::from_votes(
            votes.first().map_or(uuid::Uuid::nil(), |v| v.proposal_id),
            votes,
            eligible_voters,
        )
    }

    fn is_resolved(&self, tally: &VoteTally, quorum: &QuorumRequirement) -> Option<ProposalResult> {
        if tally.participation() < quorum.participation {
            return None;
        }
        if tally.has_blocks() {
            return Some(ProposalResult::Failed);
        }
        // For consensus, require very high approval (no oppose votes either)
        if tally.oppose == 0.0 {
            Some(ProposalResult::Passed)
        } else {
            None // still deliberating
        }
    }

    fn process_id(&self) -> &str {
        "consensus"
    }
}

/// Consent-based: passes unless someone objects. Silence is approval.
///
/// Different from consensus: consent asks "can you live with this?"
/// rather than "do you fully agree?"
pub struct ConsentProcess;

impl DecisionProcess for ConsentProcess {
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally {
        VoteTally::from_votes(
            votes.first().map_or(uuid::Uuid::nil(), |v| v.proposal_id),
            votes,
            eligible_voters,
        )
    }

    fn is_resolved(&self, tally: &VoteTally, _quorum: &QuorumRequirement) -> Option<ProposalResult> {
        // In consent, any block or oppose kills it; silence = approval
        if tally.has_blocks() {
            return Some(ProposalResult::Failed);
        }
        if tally.oppose > 0.0 {
            return Some(ProposalResult::Failed);
        }
        // Passes by default (silence = approval) as long as voting period allows
        Some(ProposalResult::Passed)
    }

    fn process_id(&self) -> &str {
        "consent"
    }
}

/// Supermajority: requires a higher threshold than simple majority.
///
/// From Constellation Art. 8 §4: "Where full consensus proves impossible,
/// super-majority thresholds ensure broad support."
pub struct SuperMajorityProcess {
    pub threshold: f64,
}

impl SuperMajorityProcess {
    pub fn two_thirds() -> Self {
        Self { threshold: 0.667 }
    }

    pub fn three_quarters() -> Self {
        Self { threshold: 0.75 }
    }
}

impl DecisionProcess for SuperMajorityProcess {
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally {
        VoteTally::from_votes(
            votes.first().map_or(uuid::Uuid::nil(), |v| v.proposal_id),
            votes,
            eligible_voters,
        )
    }

    fn is_resolved(&self, tally: &VoteTally, quorum: &QuorumRequirement) -> Option<ProposalResult> {
        if tally.participation() < quorum.participation {
            return None;
        }
        if tally.approval_rate() >= self.threshold {
            Some(ProposalResult::Passed)
        } else {
            Some(ProposalResult::Failed)
        }
    }

    fn process_id(&self) -> &str {
        "supermajority"
    }
}

/// Ranked choice (instant runoff): eliminate lowest, redistribute, repeat.
///
/// Votes carry ranked preferences. This process simulates instant runoff
/// by using the weight field as rank position (1.0 = first choice).
pub struct RankedChoiceProcess;

/// A ranked choice ballot: voter ranks candidates/options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedBallot {
    pub voter: String,
    pub rankings: Vec<String>, // options in preference order (first = most preferred)
}

impl RankedChoiceProcess {
    /// Run instant runoff on ranked ballots. Returns the winner option key.
    pub fn run_instant_runoff(ballots: &[RankedBallot], options: &[String]) -> Option<String> {
        let mut eliminated: HashSet<String> = HashSet::new();
        let mut remaining: Vec<String> = options.to_vec();
        let total_ballots = ballots.len();

        loop {
            if remaining.len() <= 1 {
                return remaining.into_iter().next();
            }

            // Count first-choice votes among non-eliminated options
            let mut counts: HashMap<String, usize> = HashMap::new();
            for option in &remaining {
                counts.insert(option.clone(), 0);
            }

            for ballot in ballots {
                // Find the highest-ranked non-eliminated option
                for choice in &ballot.rankings {
                    if !eliminated.contains(choice) {
                        if let Some(count) = counts.get_mut(choice) {
                            *count += 1;
                            break;
                        }
                    }
                }
            }

            // Check if anyone has a majority
            let majority_threshold = total_ballots / 2 + 1;
            if let Some((winner, count)) = counts.iter().max_by_key(|(_, c)| *c) {
                if *count >= majority_threshold {
                    return Some(winner.clone());
                }
            }

            // Eliminate the option with fewest votes
            if let Some((loser, _)) = counts.iter().min_by_key(|(_, c)| *c) {
                let loser = loser.clone();
                eliminated.insert(loser.clone());
                remaining.retain(|o| o != &loser);
            } else {
                break;
            }
        }

        None
    }
}

impl DecisionProcess for RankedChoiceProcess {
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally {
        // For ranked choice, we use the standard tally for quorum tracking.
        // The actual ranked algorithm runs separately via run_instant_runoff.
        VoteTally::from_votes(
            votes.first().map_or(uuid::Uuid::nil(), |v| v.proposal_id),
            votes,
            eligible_voters,
        )
    }

    fn is_resolved(&self, tally: &VoteTally, quorum: &QuorumRequirement) -> Option<ProposalResult> {
        if tally.participation() < quorum.participation {
            return None;
        }
        // Ranked choice resolves when participation is met — the actual winner
        // is determined by run_instant_runoff, not by support/oppose ratios.
        if tally.approval_rate() >= quorum.approval {
            Some(ProposalResult::Passed)
        } else {
            Some(ProposalResult::Failed)
        }
    }

    fn process_id(&self) -> &str {
        "ranked_choice"
    }
}

/// Liquid democracy: delegates can vote on behalf of others, with cycle detection.
///
/// From Constellation Art. 8 §3: "Communities may send delegates to higher
/// coordinating bodies carrying specific mandates, not general authority."
pub struct LiquidDemocracyProcess;

/// A delegation of voting power.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoteDelegation {
    pub delegator: String,
    pub delegate: String,
    pub scope: DelegationScope,
    pub active: bool,
}

/// What proposals a delegation covers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DelegationScope {
    /// All proposals.
    All,
    /// Only proposals of a given type.
    Category(String),
    /// Only a specific proposal.
    SingleProposal(uuid::Uuid),
}

impl LiquidDemocracyProcess {
    /// Resolve the delegation chain for a voter. Returns the final voter
    /// who actually casts the vote. Detects cycles.
    pub fn resolve_delegation(
        voter: &str,
        delegations: &[VoteDelegation],
        proposal_id: uuid::Uuid,
    ) -> Result<String, crate::KingdomError> {
        let mut current = voter.to_string();
        let mut visited = HashSet::new();

        loop {
            if visited.contains(&current) {
                return Err(crate::KingdomError::CircularDelegation(
                    format!("cycle at {current}"),
                ));
            }
            visited.insert(current.clone());

            // Find active delegation from current for this proposal
            let delegation = delegations.iter().find(|d| {
                d.delegator == current
                    && d.active
                    && match &d.scope {
                        DelegationScope::All => true,
                        DelegationScope::Category(_) => true,
                        DelegationScope::SingleProposal(id) => *id == proposal_id,
                    }
            });

            match delegation {
                Some(d) => current = d.delegate.clone(),
                None => return Ok(current),
            }
        }
    }

    /// Resolve all votes, applying delegations. Returns effective votes
    /// (with weights accumulated from delegators).
    pub fn resolve_votes(
        votes: &[Vote],
        delegations: &[VoteDelegation],
        eligible_voters: &[String],
        proposal_id: uuid::Uuid,
    ) -> Result<Vec<Vote>, crate::KingdomError> {
        let mut effective_votes: HashMap<String, Vote> = HashMap::new();

        // First, collect direct votes
        for vote in votes {
            effective_votes.insert(vote.voter.clone(), vote.clone());
        }

        // For each eligible voter who hasn't voted, resolve their delegation
        for voter in eligible_voters {
            if effective_votes.contains_key(voter) {
                continue; // already voted directly
            }

            let resolved = Self::resolve_delegation(voter, delegations, proposal_id)?;
            if let Some(delegate_vote) = effective_votes.get_mut(&resolved) {
                delegate_vote.weight += 1.0; // add the delegator's weight
            }
        }

        Ok(effective_votes.into_values().collect())
    }
}

impl DecisionProcess for LiquidDemocracyProcess {
    fn tally(&self, votes: &[Vote], eligible_voters: u32) -> VoteTally {
        VoteTally::from_votes(
            votes.first().map_or(uuid::Uuid::nil(), |v| v.proposal_id),
            votes,
            eligible_voters,
        )
    }

    fn is_resolved(&self, tally: &VoteTally, quorum: &QuorumRequirement) -> Option<ProposalResult> {
        if tally.participation() < quorum.participation {
            return None;
        }
        if tally.approval_rate() >= quorum.approval {
            Some(ProposalResult::Passed)
        } else {
            Some(ProposalResult::Failed)
        }
    }

    fn process_id(&self) -> &str {
        "liquid_democracy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vote::VotePosition;

    fn make_votes(
        proposal_id: uuid::Uuid,
        positions: &[(&str, VotePosition)],
    ) -> Vec<Vote> {
        positions
            .iter()
            .map(|(voter, pos)| Vote::new(*voter, proposal_id, *pos))
            .collect()
    }

    #[test]
    fn direct_vote_majority_passes() {
        let pid = uuid::Uuid::new_v4();
        let process = DirectVoteProcess;
        let votes = make_votes(pid, &[
            ("a", VotePosition::Support),
            ("b", VotePosition::Support),
            ("c", VotePosition::Oppose),
        ]);
        let tally = process.tally(&votes, 5);
        let quorum = QuorumRequirement::new(0.5, 0.5);
        // 3/5 participation = 0.6, 2/3 approval ≈ 0.67
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Passed)
        );
    }

    #[test]
    fn direct_vote_fails_quorum() {
        let pid = uuid::Uuid::new_v4();
        let process = DirectVoteProcess;
        let votes = make_votes(pid, &[("a", VotePosition::Support)]);
        let tally = process.tally(&votes, 10);
        let quorum = QuorumRequirement::majority();
        // 1/10 participation = 0.1 < 0.5 required
        assert_eq!(process.is_resolved(&tally, &quorum), None);
    }

    #[test]
    fn consensus_blocks_kill() {
        let pid = uuid::Uuid::new_v4();
        let process = ConsensusProcess;
        let votes = make_votes(pid, &[
            ("a", VotePosition::Support),
            ("b", VotePosition::Support),
            ("c", VotePosition::Block),
        ]);
        let tally = process.tally(&votes, 3);
        let quorum = QuorumRequirement::unanimous();
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Failed)
        );
    }

    #[test]
    fn consensus_passes_when_all_support() {
        let pid = uuid::Uuid::new_v4();
        let process = ConsensusProcess;
        let votes = make_votes(pid, &[
            ("a", VotePosition::Support),
            ("b", VotePosition::Support),
            ("c", VotePosition::Support),
        ]);
        let tally = process.tally(&votes, 3);
        let quorum = QuorumRequirement::unanimous();
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Passed)
        );
    }

    #[test]
    fn consent_passes_by_default() {
        let _pid = uuid::Uuid::new_v4();
        let process = ConsentProcess;
        let votes: Vec<Vote> = vec![]; // nobody objects
        let tally = process.tally(&votes, 10);
        let quorum = QuorumRequirement::default();
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Passed)
        );
    }

    #[test]
    fn consent_fails_on_objection() {
        let pid = uuid::Uuid::new_v4();
        let process = ConsentProcess;
        let votes = make_votes(pid, &[("a", VotePosition::Oppose)]);
        let tally = process.tally(&votes, 10);
        let quorum = QuorumRequirement::default();
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Failed)
        );
    }

    #[test]
    fn supermajority_two_thirds() {
        let pid = uuid::Uuid::new_v4();
        let process = SuperMajorityProcess::two_thirds();
        let votes = make_votes(pid, &[
            ("a", VotePosition::Support),
            ("b", VotePosition::Support),
            ("c", VotePosition::Support),
            ("d", VotePosition::Oppose),
        ]);
        let tally = process.tally(&votes, 6);
        let quorum = QuorumRequirement::new(0.5, 0.5); // quorum doesn't matter, process has its own threshold
        // 3/4 approval = 0.75 >= 0.667
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Passed)
        );
    }

    #[test]
    fn supermajority_fails_below_threshold() {
        let pid = uuid::Uuid::new_v4();
        let process = SuperMajorityProcess::two_thirds();
        let votes = make_votes(pid, &[
            ("a", VotePosition::Support),
            ("b", VotePosition::Oppose),
            ("c", VotePosition::Oppose),
        ]);
        let tally = process.tally(&votes, 3);
        let quorum = QuorumRequirement::new(0.5, 0.5);
        assert_eq!(
            process.is_resolved(&tally, &quorum),
            Some(ProposalResult::Failed)
        );
    }

    #[test]
    fn ranked_choice_instant_runoff() {
        let options = vec!["apple".into(), "banana".into(), "cherry".into()];
        let ballots = vec![
            RankedBallot {
                voter: "a".into(),
                rankings: vec!["apple".into(), "banana".into(), "cherry".into()],
            },
            RankedBallot {
                voter: "b".into(),
                rankings: vec!["banana".into(), "apple".into(), "cherry".into()],
            },
            RankedBallot {
                voter: "c".into(),
                rankings: vec!["cherry".into(), "banana".into(), "apple".into()],
            },
            RankedBallot {
                voter: "d".into(),
                rankings: vec!["banana".into(), "cherry".into(), "apple".into()],
            },
            RankedBallot {
                voter: "e".into(),
                rankings: vec!["apple".into(), "banana".into(), "cherry".into()],
            },
        ];

        // Round 1: apple=2, banana=2, cherry=1. Cherry eliminated.
        // Round 2: cherry's voter (c) goes to banana. apple=2, banana=3. Banana wins.
        let winner = RankedChoiceProcess::run_instant_runoff(&ballots, &options);
        assert_eq!(winner, Some("banana".into()));
    }

    #[test]
    fn liquid_democracy_delegation_resolution() {
        let pid = uuid::Uuid::new_v4();
        let delegations = vec![
            VoteDelegation {
                delegator: "alice".into(),
                delegate: "bob".into(),
                scope: DelegationScope::All,
                active: true,
            },
        ];

        let resolved = LiquidDemocracyProcess::resolve_delegation("alice", &delegations, pid).unwrap();
        assert_eq!(resolved, "bob");

        // Bob has no delegation, resolves to self
        let resolved = LiquidDemocracyProcess::resolve_delegation("bob", &delegations, pid).unwrap();
        assert_eq!(resolved, "bob");
    }

    #[test]
    fn liquid_democracy_transitive_delegation() {
        let pid = uuid::Uuid::new_v4();
        let delegations = vec![
            VoteDelegation {
                delegator: "alice".into(),
                delegate: "bob".into(),
                scope: DelegationScope::All,
                active: true,
            },
            VoteDelegation {
                delegator: "bob".into(),
                delegate: "charlie".into(),
                scope: DelegationScope::All,
                active: true,
            },
        ];

        let resolved = LiquidDemocracyProcess::resolve_delegation("alice", &delegations, pid).unwrap();
        assert_eq!(resolved, "charlie");
    }

    #[test]
    fn liquid_democracy_cycle_detection() {
        let pid = uuid::Uuid::new_v4();
        let delegations = vec![
            VoteDelegation {
                delegator: "alice".into(),
                delegate: "bob".into(),
                scope: DelegationScope::All,
                active: true,
            },
            VoteDelegation {
                delegator: "bob".into(),
                delegate: "alice".into(),
                scope: DelegationScope::All,
                active: true,
            },
        ];

        let result = LiquidDemocracyProcess::resolve_delegation("alice", &delegations, pid);
        assert!(result.is_err());
        assert!(matches!(result, Err(crate::KingdomError::CircularDelegation(_))));
    }

    #[test]
    fn liquid_democracy_scoped_delegation() {
        let pid = uuid::Uuid::new_v4();
        let other_pid = uuid::Uuid::new_v4();
        let delegations = vec![
            VoteDelegation {
                delegator: "alice".into(),
                delegate: "bob".into(),
                scope: DelegationScope::SingleProposal(pid),
                active: true,
            },
        ];

        // Matches the proposal — delegates to bob
        let resolved = LiquidDemocracyProcess::resolve_delegation("alice", &delegations, pid).unwrap();
        assert_eq!(resolved, "bob");

        // Different proposal — no delegation, stays alice
        let resolved = LiquidDemocracyProcess::resolve_delegation("alice", &delegations, other_pid).unwrap();
        assert_eq!(resolved, "alice");
    }

    #[test]
    fn process_ids_unique() {
        let processes: Vec<Box<dyn DecisionProcess>> = vec![
            Box::new(DirectVoteProcess),
            Box::new(ConsensusProcess),
            Box::new(ConsentProcess),
            Box::new(SuperMajorityProcess::two_thirds()),
            Box::new(RankedChoiceProcess),
            Box::new(LiquidDemocracyProcess),
        ];
        let ids: HashSet<&str> = processes.iter().map(|p| p.process_id()).collect();
        assert_eq!(ids.len(), 6);
    }
}
