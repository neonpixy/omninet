//! # Federation Scope -- Data Boundary for AI Governance
//!
//! From Constellation Art. 3 SS3 -- federation is a data boundary.
//!
//! When communities federate, they form a trust cluster. Advisor's governance
//! delegation scope and skill availability respect these boundaries. A
//! governance delegation only applies to proposals from communities visible
//! within the federation scope. Skills may be restricted to communities
//! that the federated cluster has approved.
//!
//! `FederationScope` is an optional filter applied to governance and skill
//! queries. When empty (unrestricted), all communities are visible -- this
//! is the default and preserves full backward compatibility.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::governance::{GovernanceAIPolicy, GovernanceMode, GovernanceVote, ProposalAnalysis};
use crate::skill::SkillDefinition;

/// Optional federation scope for Advisor operations.
///
/// When set, governance delegation and skill availability are scoped
/// to federated communities. When empty, all communities are visible.
///
/// From Constellation Art. 3 SS3 -- federation is a data boundary.
///
/// # Examples
///
/// ```
/// use advisor::FederationScope;
///
/// // Unrestricted -- sees everything.
/// let open = FederationScope::new();
/// assert!(open.is_unrestricted());
/// assert!(open.is_visible("any_community"));
///
/// // Scoped to specific communities.
/// let scoped = FederationScope::from_communities(["alpha", "beta"]);
/// assert!(!scoped.is_unrestricted());
/// assert!(scoped.is_visible("alpha"));
/// assert!(!scoped.is_visible("gamma"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationScope {
    visible_communities: HashSet<String>,
}

impl Default for FederationScope {
    fn default() -> Self {
        Self::new()
    }
}

impl FederationScope {
    /// Create an unrestricted scope -- all communities are visible.
    pub fn new() -> Self {
        Self {
            visible_communities: HashSet::new(),
        }
    }

    /// Create a scope limited to the given communities.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Whether a community is visible within this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty) or if
    /// the community is in the visible set.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Whether this scope has no restrictions (all communities visible).
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the visible set.
    ///
    /// Returns 0 for unrestricted scopes -- this does NOT mean "no communities",
    /// it means "all communities". Use [`is_unrestricted`](Self::is_unrestricted)
    /// to distinguish.
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty (i.e., unrestricted).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Add a community to the visible set.
    ///
    /// Returns `true` if the community was newly inserted.
    pub fn add_community(&mut self, community_id: impl Into<String>) -> bool {
        self.visible_communities.insert(community_id.into())
    }

    /// Remove a community from the visible set.
    ///
    /// Returns `true` if the community was present and removed.
    pub fn remove_community(&mut self, community_id: &str) -> bool {
        self.visible_communities.remove(community_id)
    }

    // ── Governance scoped methods ──────────────────────────────────────

    /// Filter voting history to only votes cast in visible communities.
    ///
    /// When the scope is unrestricted, all votes pass through.
    pub fn filter_votes_scoped<'a>(
        &self,
        votes: &'a [GovernanceVote],
    ) -> Vec<&'a GovernanceVote> {
        if self.is_unrestricted() {
            return votes.iter().collect();
        }
        votes
            .iter()
            .filter(|v| self.visible_communities.contains(v.community_id.as_str()))
            .collect()
    }

    /// Filter a set of proposal analyses to only those for visible communities.
    ///
    /// Proposal analyses carry charter_relevance but not community_id directly,
    /// so this filters by the community_ids provided alongside each analysis.
    pub fn filter_analyses_scoped<'a>(
        &self,
        analyses: &'a [(String, ProposalAnalysis)],
    ) -> Vec<&'a (String, ProposalAnalysis)> {
        if self.is_unrestricted() {
            return analyses.iter().collect();
        }
        analyses
            .iter()
            .filter(|(community_id, _)| self.visible_communities.contains(community_id.as_str()))
            .collect()
    }

    /// Check whether governance delegation is allowed for a given community
    /// under this federation scope.
    ///
    /// Returns `false` if the community is not visible in the scope.
    /// This is a pre-check before calling `GovernanceMode::evaluate_proposal`.
    pub fn can_delegate_in(&self, community_id: &str) -> bool {
        self.is_visible(community_id)
    }

    /// Count votes cast in visible communities.
    pub fn vote_count_scoped(&self, votes: &[GovernanceVote]) -> usize {
        if self.is_unrestricted() {
            return votes.len();
        }
        votes
            .iter()
            .filter(|v| self.visible_communities.contains(v.community_id.as_str()))
            .count()
    }

    /// Compute the override rate within the visible federation scope.
    ///
    /// Returns the fraction of votes that were overridden (0.0..=1.0).
    /// Returns `None` if no votes exist in the scope.
    pub fn override_rate_scoped(&self, mode: &GovernanceMode) -> Option<f64> {
        let visible_votes = self.filter_votes_scoped(&mode.voting_history);
        if visible_votes.is_empty() {
            return None;
        }
        let overridden = visible_votes.iter().filter(|v| v.was_overridden).count();
        Some(overridden as f64 / visible_votes.len() as f64)
    }

    /// Filter community AI policies to only those from visible communities.
    ///
    /// Useful for understanding what governance constraints apply across a
    /// federation. Returns the community_id and policy pairs.
    pub fn filter_policies_scoped<'a>(
        &self,
        policies: &'a [(String, GovernanceAIPolicy)],
    ) -> Vec<&'a (String, GovernanceAIPolicy)> {
        if self.is_unrestricted() {
            return policies.iter().collect();
        }
        policies
            .iter()
            .filter(|(community_id, _)| self.visible_communities.contains(community_id.as_str()))
            .collect()
    }

    // ── Skill scoped methods ───────────────────────────────────────────

    /// Filter skills to only those available in visible communities.
    ///
    /// Each skill is paired with the community that registered it.
    /// Unrestricted scope passes everything through.
    pub fn filter_skills_scoped<'a>(
        &self,
        skills: &'a [(String, SkillDefinition)],
    ) -> Vec<&'a (String, SkillDefinition)> {
        if self.is_unrestricted() {
            return skills.iter().collect();
        }
        skills
            .iter()
            .filter(|(community_id, _)| self.visible_communities.contains(community_id.as_str()))
            .collect()
    }

    /// Count skills available in the visible federation scope.
    pub fn skill_count_scoped(&self, skills: &[(String, SkillDefinition)]) -> usize {
        if self.is_unrestricted() {
            return skills.len();
        }
        skills
            .iter()
            .filter(|(community_id, _)| self.visible_communities.contains(community_id.as_str()))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{GovernanceMode, GovernanceVote, VotePosition};
    use crate::skill::SkillDefinition;
    use chrono::Utc;
    use uuid::Uuid;

    // ── Core FederationScope ─────────────────────────────────────────

    #[test]
    fn unrestricted_scope_sees_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
        assert!(scope.is_visible("any_community"));
        assert!(scope.is_visible("another_community"));
    }

    #[test]
    fn scoped_scope_filters_communities() {
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        assert!(!scope.is_unrestricted());
        assert!(!scope.is_empty());
        assert_eq!(scope.len(), 2);
        assert!(scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert!(!scope.is_visible("gamma"));
        assert!(!scope.is_visible("delta"));
    }

    #[test]
    fn default_is_unrestricted() {
        let scope = FederationScope::default();
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn add_and_remove_communities() {
        let mut scope = FederationScope::new();
        assert!(scope.is_unrestricted());

        assert!(scope.add_community("alpha"));
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("alpha"));
        assert!(!scope.is_visible("beta"));

        // Adding again returns false.
        assert!(!scope.add_community("alpha"));

        assert!(scope.remove_community("alpha"));
        assert!(scope.is_unrestricted());

        // Removing nonexistent returns false.
        assert!(!scope.remove_community("alpha"));
    }

    #[test]
    fn empty_communities_iterator() {
        let scope = FederationScope::from_communities(Vec::<String>::new());
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn serialization_roundtrip() {
        let scope = FederationScope::from_communities(["alpha", "beta", "gamma"]);
        let json = serde_json::to_string(&scope).expect("serialize");
        let deserialized: FederationScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn unrestricted_serialization_roundtrip() {
        let scope = FederationScope::new();
        let json = serde_json::to_string(&scope).expect("serialize");
        let deserialized: FederationScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, deserialized);
        assert!(deserialized.is_unrestricted());
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FederationScope>();
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn test_vote(community_id: &str, overridden: bool) -> GovernanceVote {
        GovernanceVote {
            proposal_id: Uuid::new_v4(),
            community_id: community_id.to_string(),
            position: VotePosition::Approve,
            reasoning: "test".into(),
            confidence: 0.8,
            was_auto: true,
            was_overridden: overridden,
            override_position: if overridden {
                Some(VotePosition::Reject)
            } else {
                None
            },
            voted_at: Utc::now(),
        }
    }

    fn test_governance_mode_with_votes() -> GovernanceMode {
        let mut mode = GovernanceMode::new("cpub1test");
        mode.activate();
        mode.voting_history.push(test_vote("alpha", false));
        mode.voting_history.push(test_vote("alpha", true));
        mode.voting_history.push(test_vote("beta", false));
        mode.voting_history.push(test_vote("gamma", false));
        mode.voting_history.push(test_vote("gamma", true));
        mode
    }

    // ── Governance scoped methods ───────────────────────────────────

    #[test]
    fn filter_votes_unrestricted_returns_all() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::new();
        let filtered = scope.filter_votes_scoped(&mode.voting_history);
        assert_eq!(filtered.len(), 5);
    }

    #[test]
    fn filter_votes_scoped_filters_by_community() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        let filtered = scope.filter_votes_scoped(&mode.voting_history);
        assert_eq!(filtered.len(), 3); // 2 alpha + 1 beta
    }

    #[test]
    fn filter_votes_scoped_excludes_invisible() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::from_communities(["gamma"]);
        let filtered = scope.filter_votes_scoped(&mode.voting_history);
        assert_eq!(filtered.len(), 2); // 2 gamma
    }

    #[test]
    fn can_delegate_in_visible_community() {
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        assert!(scope.can_delegate_in("alpha"));
        assert!(scope.can_delegate_in("beta"));
        assert!(!scope.can_delegate_in("gamma"));
    }

    #[test]
    fn can_delegate_in_unrestricted() {
        let scope = FederationScope::new();
        assert!(scope.can_delegate_in("any_community"));
    }

    #[test]
    fn vote_count_scoped_unrestricted() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::new();
        assert_eq!(scope.vote_count_scoped(&mode.voting_history), 5);
    }

    #[test]
    fn vote_count_scoped_filtered() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::from_communities(["alpha"]);
        assert_eq!(scope.vote_count_scoped(&mode.voting_history), 2);
    }

    #[test]
    fn override_rate_scoped_no_votes() {
        let mode = GovernanceMode::new("cpub1test");
        let scope = FederationScope::new();
        assert!(scope.override_rate_scoped(&mode).is_none());
    }

    #[test]
    fn override_rate_scoped_unrestricted() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::new();
        let rate = scope.override_rate_scoped(&mode).unwrap();
        // 2 overridden out of 5 total = 0.4
        assert!((rate - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn override_rate_scoped_filtered() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::from_communities(["alpha"]);
        let rate = scope.override_rate_scoped(&mode).unwrap();
        // alpha has 2 votes, 1 overridden = 0.5
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn override_rate_scoped_no_visible_votes() {
        let mode = test_governance_mode_with_votes();
        let scope = FederationScope::from_communities(["nonexistent"]);
        assert!(scope.override_rate_scoped(&mode).is_none());
    }

    #[test]
    fn filter_policies_scoped_unrestricted() {
        let policies = vec![
            ("alpha".to_string(), GovernanceAIPolicy::default()),
            ("beta".to_string(), GovernanceAIPolicy::default()),
        ];
        let scope = FederationScope::new();
        let filtered = scope.filter_policies_scoped(&policies);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_policies_scoped_filtered() {
        let policies = vec![
            ("alpha".to_string(), GovernanceAIPolicy::default()),
            ("beta".to_string(), GovernanceAIPolicy::default()),
            ("gamma".to_string(), GovernanceAIPolicy::default()),
        ];
        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        let filtered = scope.filter_policies_scoped(&policies);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].0, "alpha");
        assert_eq!(filtered[1].0, "gamma");
    }

    #[test]
    fn filter_analyses_scoped_unrestricted() {
        let analyses = vec![
            (
                "alpha".to_string(),
                ProposalAnalysis {
                    proposal_id: Uuid::new_v4(),
                    summary: "test".into(),
                    alignment_score: 0.5,
                    impact_assessment: "low".into(),
                    charter_relevance: vec![],
                    recommended_position: VotePosition::Approve,
                    confidence: 0.8,
                    dissenting_considerations: vec![],
                },
            ),
        ];
        let scope = FederationScope::new();
        let filtered = scope.filter_analyses_scoped(&analyses);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_analyses_scoped_excludes_invisible() {
        let analyses = vec![
            (
                "alpha".to_string(),
                ProposalAnalysis {
                    proposal_id: Uuid::new_v4(),
                    summary: "test".into(),
                    alignment_score: 0.5,
                    impact_assessment: "low".into(),
                    charter_relevance: vec![],
                    recommended_position: VotePosition::Approve,
                    confidence: 0.8,
                    dissenting_considerations: vec![],
                },
            ),
            (
                "beta".to_string(),
                ProposalAnalysis {
                    proposal_id: Uuid::new_v4(),
                    summary: "test2".into(),
                    alignment_score: 0.3,
                    impact_assessment: "high".into(),
                    charter_relevance: vec![],
                    recommended_position: VotePosition::Reject,
                    confidence: 0.6,
                    dissenting_considerations: vec![],
                },
            ),
        ];
        let scope = FederationScope::from_communities(["beta"]);
        let filtered = scope.filter_analyses_scoped(&analyses);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, "beta");
    }

    // ── Skill scoped methods ────────────────────────────────────────

    #[test]
    fn filter_skills_scoped_unrestricted() {
        let skills = vec![
            (
                "alpha".to_string(),
                SkillDefinition::new("skill.a", "Skill A", "First skill"),
            ),
            (
                "beta".to_string(),
                SkillDefinition::new("skill.b", "Skill B", "Second skill"),
            ),
        ];
        let scope = FederationScope::new();
        let filtered = scope.filter_skills_scoped(&skills);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_skills_scoped_filters_by_community() {
        let skills = vec![
            (
                "alpha".to_string(),
                SkillDefinition::new("skill.a", "Skill A", "First"),
            ),
            (
                "beta".to_string(),
                SkillDefinition::new("skill.b", "Skill B", "Second"),
            ),
            (
                "gamma".to_string(),
                SkillDefinition::new("skill.c", "Skill C", "Third"),
            ),
        ];
        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        let filtered = scope.filter_skills_scoped(&skills);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].1.id, "skill.a");
        assert_eq!(filtered[1].1.id, "skill.c");
    }

    #[test]
    fn skill_count_scoped_unrestricted() {
        let skills = vec![
            (
                "alpha".to_string(),
                SkillDefinition::new("s1", "S1", "One"),
            ),
            (
                "beta".to_string(),
                SkillDefinition::new("s2", "S2", "Two"),
            ),
        ];
        let scope = FederationScope::new();
        assert_eq!(scope.skill_count_scoped(&skills), 2);
    }

    #[test]
    fn skill_count_scoped_filtered() {
        let skills = vec![
            (
                "alpha".to_string(),
                SkillDefinition::new("s1", "S1", "One"),
            ),
            (
                "beta".to_string(),
                SkillDefinition::new("s2", "S2", "Two"),
            ),
            (
                "gamma".to_string(),
                SkillDefinition::new("s3", "S3", "Three"),
            ),
        ];
        let scope = FederationScope::from_communities(["beta"]);
        assert_eq!(scope.skill_count_scoped(&skills), 1);
    }
}
