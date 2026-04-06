//! Community health aggregation.
//!
//! Extracts aggregate metrics from Kingdom's Community type. NEVER stores
//! pubkeys, member identities, or any individually identifiable information.
//! Only counts, rates, and structural signals.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use bulwark::CollectiveHealthPulse;
use kingdom::{Community, Proposal, ProposalStatus};

/// Aggregated health metrics for a single community.
///
/// All data is deidentified: member count, role distribution (by role name,
/// not by pubkey), and governance activity (proposal counts, vote totals).
/// NO pubkeys, NO member identities, NO individual activity.
///
/// # Examples
///
/// ```
/// use kingdom::{Community, CommunityBasis};
/// use undercroft::CommunityHealth;
///
/// let community = Community::new("Village Alpha", CommunityBasis::Place);
/// let health = CommunityHealth::from_community(&community, &[], None);
/// assert_eq!(health.member_count, 0);
/// assert_eq!(health.community_name, "Village Alpha");
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunityHealth {
    /// The community's UUID.
    pub community_id: Uuid,
    /// The community's display name.
    pub community_name: String,
    /// Total member count (no identities).
    pub member_count: usize,
    /// Serialized community status (e.g. "Active", "Forming").
    pub active_status: String,
    /// Distribution of roles: role name -> count. NO pubkeys.
    pub role_distribution: HashMap<String, usize>,
    /// Governance activity aggregates.
    pub governance: GovernanceActivity,
    /// Collective health score from Bulwark (0-19), if available.
    pub collective_health_score: Option<u32>,
    /// Collective health status from Bulwark (e.g. "Thriving"), if available.
    pub collective_health_status: Option<String>,
    /// When this snapshot was computed.
    pub computed_at: DateTime<Utc>,
}

/// Aggregated governance activity for a community.
///
/// Counts of proposals, votes, and participation rates. No individual
/// voter identities or vote positions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GovernanceActivity {
    /// Number of proposals currently in Draft, Discussion, or Voting status.
    pub active_proposals: usize,
    /// Number of proposals that have been resolved.
    pub resolved_proposals: usize,
    /// Total votes cast across all proposals.
    pub total_votes_cast: usize,
    /// Average participation rate across resolved proposals (0.0-1.0).
    pub average_participation: f64,
}

impl CommunityHealth {
    /// Build community health from a Kingdom Community, its proposals, and
    /// an optional Bulwark collective health pulse.
    ///
    /// This extracts ONLY aggregate counts and rates. The members Vec in
    /// Community contains pubkeys -- we deliberately count them without
    /// storing any.
    ///
    /// # Arguments
    ///
    /// * `community` - The Kingdom community to analyze.
    /// * `proposals` - All proposals for this community.
    /// * `pulse` - Optional Bulwark collective health data.
    ///
    /// # Examples
    ///
    /// ```
    /// use kingdom::{Community, CommunityBasis};
    /// use undercroft::CommunityHealth;
    ///
    /// let mut community = Community::new("Test", CommunityBasis::Digital);
    /// community.add_founder("alice_pubkey");
    /// community.activate().unwrap();
    /// community.add_member("bob_pubkey", None).unwrap();
    ///
    /// let health = CommunityHealth::from_community(&community, &[], None);
    /// assert_eq!(health.member_count, 2);
    /// // Verify NO pubkeys in output
    /// let json = serde_json::to_string(&health).unwrap();
    /// assert!(!json.contains("alice_pubkey"));
    /// assert!(!json.contains("bob_pubkey"));
    /// ```
    #[must_use]
    pub fn from_community(
        community: &Community,
        proposals: &[Proposal],
        pulse: Option<&CollectiveHealthPulse>,
    ) -> Self {
        // Count members per role -- NOT their pubkeys.
        let mut role_distribution = HashMap::new();
        for member in &community.members {
            let role_name = format!("{:?}", member.role);
            *role_distribution.entry(role_name).or_insert(0) += 1;
        }

        // Aggregate governance activity.
        let governance = Self::compute_governance(proposals);

        // Extract Bulwark health if available.
        let (collective_health_score, collective_health_status) = match pulse {
            Some(p) => (
                Some(p.factors.total_score()),
                Some(format!("{:?}", p.status)),
            ),
            None => (None, None),
        };

        Self {
            community_id: community.id,
            community_name: community.name.clone(),
            member_count: community.member_count(),
            active_status: format!("{:?}", community.status),
            role_distribution,
            governance,
            collective_health_score,
            collective_health_status,
            computed_at: Utc::now(),
        }
    }

    /// Compute governance activity from a slice of proposals.
    fn compute_governance(proposals: &[Proposal]) -> GovernanceActivity {
        let active_proposals = proposals
            .iter()
            .filter(|p| {
                matches!(
                    p.status,
                    ProposalStatus::Draft | ProposalStatus::Discussion | ProposalStatus::Voting
                )
            })
            .count();

        let resolved_proposals = proposals
            .iter()
            .filter(|p| p.status == ProposalStatus::Resolved)
            .count();

        let total_votes_cast: usize = proposals.iter().map(|p| p.votes.len()).sum();

        // Average participation across resolved proposals that have outcomes.
        let participation_rates: Vec<f64> = proposals
            .iter()
            .filter_map(|p| p.outcome.as_ref())
            .map(|o| o.participation_rate())
            .collect();

        let average_participation = if participation_rates.is_empty() {
            0.0
        } else {
            let sum: f64 = participation_rates.iter().sum();
            sum / participation_rates.len() as f64
        };

        GovernanceActivity {
            active_proposals,
            resolved_proposals,
            total_votes_cast,
            average_participation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bulwark::{
        CollectiveContentHealth, CollectiveCommunicationPattern, CollectiveHealthFactors,
        CrossMembershipLevel, EngagementDistribution, PowerDistribution,
    };
    use kingdom::{
        CommunityBasis, CommunityRole, DecidingBody, ProposalOutcome,
        Vote, VotePosition,
        decision::ProposalResult,
    };

    fn test_community() -> Community {
        let mut c = Community::new("Test Village", CommunityBasis::Place);
        c.add_founder("alice_pk_001");
        c.add_founder("bob_pk_002");
        c.activate().unwrap();
        c.add_member("charlie_pk_003", Some("alice_pk_001".into()))
            .unwrap();
        c.update_member_role("charlie_pk_003", CommunityRole::Member)
            .unwrap();
        c
    }

    #[test]
    fn from_empty_community() {
        let c = Community::new("Empty", CommunityBasis::Digital);
        let health = CommunityHealth::from_community(&c, &[], None);

        assert_eq!(health.community_name, "Empty");
        assert_eq!(health.member_count, 0);
        assert_eq!(health.active_status, "Forming");
        assert!(health.role_distribution.is_empty());
        assert!(health.collective_health_score.is_none());
        assert!(health.collective_health_status.is_none());
    }

    #[test]
    fn from_community_with_members() {
        let c = test_community();
        let health = CommunityHealth::from_community(&c, &[], None);

        assert_eq!(health.member_count, 3);
        assert_eq!(health.active_status, "Active");

        // Role distribution
        assert_eq!(health.role_distribution.get("Founder"), Some(&2));
        assert_eq!(health.role_distribution.get("Member"), Some(&1));
        assert!(!health.role_distribution.contains_key("Elder"));
    }

    #[test]
    fn no_pubkeys_in_serialized_output() {
        let c = test_community();
        let health = CommunityHealth::from_community(&c, &[], None);
        let json = serde_json::to_string(&health).unwrap();

        // Verify NO pubkeys leak into the serialized output.
        assert!(!json.contains("alice_pk_001"));
        assert!(!json.contains("bob_pk_002"));
        assert!(!json.contains("charlie_pk_003"));
    }

    #[test]
    fn governance_activity_from_proposals() {
        let c = test_community();
        let cid = c.id;

        // Create a draft proposal
        let draft = Proposal::new(
            "author1",
            DecidingBody::Community(cid),
            "Draft One",
            "Body",
        );

        // Create a resolved proposal with votes
        let mut resolved = Proposal::new(
            "author2",
            DecidingBody::Community(cid),
            "Resolved One",
            "Body",
        );
        let closes = Utc::now() + chrono::Duration::days(7);
        resolved.open_voting(closes).unwrap();
        resolved
            .add_vote(Vote::new("voter1", resolved.id, VotePosition::Support))
            .unwrap();
        resolved
            .add_vote(Vote::new("voter2", resolved.id, VotePosition::Support))
            .unwrap();
        resolved
            .add_vote(Vote::new("voter3", resolved.id, VotePosition::Oppose))
            .unwrap();

        let tally = resolved.tally(5);
        let outcome = ProposalOutcome::from_tally(
            &tally,
            ProposalResult::Passed,
            &resolved.quorum,
        );
        resolved.resolve(outcome).unwrap();

        let health = CommunityHealth::from_community(&c, &[draft, resolved], None);

        assert_eq!(health.governance.active_proposals, 1); // draft
        assert_eq!(health.governance.resolved_proposals, 1);
        assert_eq!(health.governance.total_votes_cast, 3);
        assert!(health.governance.average_participation > 0.0);
    }

    #[test]
    fn with_bulwark_health_pulse() {
        let c = test_community();
        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::BroadlyEngaged,
            communication: CollectiveCommunicationPattern::Healthy,
            cross_membership: CrossMembershipLevel::WellConnected,
            power_distribution: PowerDistribution::Distributed,
            content_health: CollectiveContentHealth::Positive,
        };
        let pulse = CollectiveHealthPulse::compute(c.id, factors, 3);

        let health = CommunityHealth::from_community(&c, &[], Some(&pulse));

        assert_eq!(health.collective_health_score, Some(0));
        assert_eq!(
            health.collective_health_status.as_deref(),
            Some("Thriving")
        );
    }

    #[test]
    fn governance_with_no_resolved_proposals() {
        let c = test_community();
        let draft = Proposal::new(
            "author",
            DecidingBody::Community(c.id),
            "Title",
            "Body",
        );

        let health = CommunityHealth::from_community(&c, &[draft], None);
        assert_eq!(health.governance.active_proposals, 1);
        assert_eq!(health.governance.resolved_proposals, 0);
        assert_eq!(health.governance.total_votes_cast, 0);
        assert_eq!(health.governance.average_participation, 0.0);
    }

    #[test]
    fn serde_round_trip() {
        let c = test_community();
        let health = CommunityHealth::from_community(&c, &[], None);
        let json = serde_json::to_string(&health).unwrap();
        let restored: CommunityHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.community_id, health.community_id);
        assert_eq!(restored.community_name, health.community_name);
        assert_eq!(restored.member_count, health.member_count);
        assert_eq!(restored.active_status, health.active_status);
        assert_eq!(restored.role_distribution, health.role_distribution);
    }
}
