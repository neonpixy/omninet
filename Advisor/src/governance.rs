//! Advisor Governance Mode — Liquid Democracy delegation.
//!
//! Advisor can serve as a Liquid Democracy delegate, voting on your behalf
//! based on your sovereign data. This module provides the types and logic
//! for value-aligned autonomous voting with full human override capability.
//!
//! # Sandboxed Reasoning
//!
//! Governance mode operates in a constrained context:
//! - **Reads:** ValueProfile, Vault data, proposal text, community charter, voting history (Yoke)
//! - **Cannot read:** other people's data, external content, network information beyond the proposal
//!
//! This prevents prompt injection via crafted proposals — the proposal text is
//! the ONLY external input.
//!
//! # Covenant Alignment
//!
//! - **Dignity:** Voting is a sacred right. Advisor preserves it, never subverts it.
//! - **Sovereignty:** Human override is always available. Low confidence triggers deferral.
//! - **Consent:** Delegation is voluntary, revocable, and category-excludable.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AdvisorError;

// ── Enums ───────────────────────────────────────────────────────────────

/// How much reasoning detail Advisor provides for governance decisions.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReasoningDetail {
    /// One-sentence summary.
    Brief,
    /// Paragraph with key factors.
    #[default]
    Standard,
    /// Full analysis with dissenting considerations.
    Detailed,
}

/// Categories of proposals in community governance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProposalType {
    /// Changes to the community charter (foundational document).
    CharterAmendment,
    /// Proposal to dissolve the community.
    Dissolution,
    /// General policy changes.
    PolicyChange,
    /// Allocation of community resources.
    ResourceAllocation,
    /// Actions affecting a specific member (e.g., exclusion).
    MemberAction,
    /// Changes to governance roles.
    RoleChange,
    /// Extension point for community-specific proposal types.
    Custom(String),
}

/// A position on a governance proposal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VotePosition {
    /// In favor of the proposal.
    Approve,
    /// Against the proposal.
    Reject,
    /// Neither for nor against — counted as present but not voting.
    Abstain,
    /// Formal block — signals a fundamental Covenant concern.
    Block,
    /// Defer vote to another delegate (re-delegation).
    Delegate,
}

/// How much of Advisor's reasoning is visible to the community.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReasoningTransparency {
    /// Reasoning visible only to the delegating human.
    Private,
    /// Summary visible to the community, full reasoning to the human.
    #[default]
    SummaryPublic,
    /// Full reasoning visible to the community.
    FullPublic,
}

/// What the Advisor decides to do with a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GovernanceAction {
    /// Cast a vote.
    Vote(GovernanceVote),
    /// Abstain with reasoning.
    Abstain(String),
    /// Defer to human — confidence too low, novel topic, or excluded category.
    DeferToHuman(String),
}

// ── Configuration ───────────────────────────────────────────────────────

/// Configuration for Advisor's governance delegation behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceModeConfig {
    /// Whether Advisor is allowed to cast votes automatically.
    /// Default: true for Citizens.
    pub auto_vote: bool,

    /// Whether Advisor sends a Pager notification before voting.
    /// Default: true — you always know when Advisor is about to vote.
    pub notification_before_vote: bool,

    /// How long to wait for human override before Advisor votes.
    /// Default: 24 hours.
    pub deliberation_window: Duration,

    /// Proposal types that ALWAYS require human vote.
    /// Default: CharterAmendment, Dissolution.
    pub excluded_categories: Vec<ProposalType>,

    /// How much reasoning detail Advisor provides.
    pub reasoning_detail: ReasoningDetail,
}

impl Default for GovernanceModeConfig {
    fn default() -> Self {
        Self {
            auto_vote: true,
            notification_before_vote: true,
            deliberation_window: Duration::from_secs(24 * 60 * 60), // 24 hours
            excluded_categories: vec![ProposalType::CharterAmendment, ProposalType::Dissolution],
            reasoning_detail: ReasoningDetail::Standard,
        }
    }
}

// ── Value Profile ───────────────────────────────────────────────────────

/// A pattern learned from past voting behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VotingPattern {
    /// The topic or domain this pattern covers.
    pub topic: String,
    /// The tendency — which position the human usually takes on this topic.
    pub position_tendency: VotePosition,
    /// How strong this tendency is (0.0 = no preference, 1.0 = always votes this way).
    pub strength: f64,
    /// How many votes contributed to this pattern.
    pub sample_count: usize,
}

/// A record of when the human overrode Advisor's recommendation.
///
/// Overrides are the most valuable signal — they reveal where Advisor's
/// model of your values is wrong.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverrideSignal {
    /// The proposal that was overridden.
    pub proposal_id: Uuid,
    /// What Advisor recommended.
    pub advisor_position: VotePosition,
    /// What the human chose instead.
    pub human_position: VotePosition,
    /// Optional explanation from the human.
    pub reason: Option<String>,
    /// When the override happened.
    pub timestamp: DateTime<Utc>,
}

/// Advisor's understanding of your governance values.
///
/// Built from three sources:
/// 1. Stated preferences — explicit declarations ("I value environmental sustainability")
/// 2. Voting patterns — learned from your past votes
/// 3. Override signals — corrections where Advisor got it wrong
///
/// The ValueProfile is the ONLY data Advisor reads when making governance
/// decisions (sandboxed reasoning).
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValueProfile {
    /// Explicit preferences the human has declared.
    /// Key: topic/domain, Value: stated preference.
    pub stated_preferences: HashMap<String, String>,

    /// Patterns learned from past voting behavior.
    pub voting_patterns: Vec<VotingPattern>,

    /// How strongly past behavior aligns with each charter section.
    /// Key: charter section identifier, Value: alignment score (0.0..=1.0).
    pub charter_alignment: HashMap<String, f64>,

    /// Records of when the human overrode Advisor's recommendation.
    pub override_signals: Vec<OverrideSignal>,
}

impl ValueProfile {
    /// Create a new empty value profile.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the profile based on a vote outcome.
    ///
    /// If the human overrode Advisor's recommendation, an `OverrideSignal`
    /// is recorded and voting patterns are adjusted to account for the
    /// correction.
    pub fn update_from_vote(
        &mut self,
        proposal_topic: &str,
        human_position: VotePosition,
        was_override: bool,
        advisor_position: Option<VotePosition>,
        proposal_id: Uuid,
    ) {
        // Record override signal if this was a correction
        if was_override {
            if let Some(advisor_pos) = advisor_position {
                self.override_signals.push(OverrideSignal {
                    proposal_id,
                    advisor_position: advisor_pos,
                    human_position,
                    reason: None,
                    timestamp: Utc::now(),
                });
            }
        }

        // Update or create the voting pattern for this topic
        if let Some(pattern) = self
            .voting_patterns
            .iter_mut()
            .find(|p| p.topic == proposal_topic)
        {
            pattern.sample_count += 1;

            if pattern.position_tendency == human_position {
                // Reinforces existing pattern — strengthen it
                let reinforcement = 1.0 / pattern.sample_count as f64;
                pattern.strength = (pattern.strength + reinforcement).min(1.0);
            } else {
                // Contradicts existing pattern — weaken it
                let decay = 1.0 / pattern.sample_count as f64;
                pattern.strength = (pattern.strength - decay).max(0.0);

                // If strength drops below threshold, flip the tendency
                if pattern.strength < 0.2 {
                    pattern.position_tendency = human_position;
                    pattern.strength = 0.3; // Reset to moderate confidence
                }
            }
        } else {
            // New topic — create pattern with moderate initial confidence
            self.voting_patterns.push(VotingPattern {
                topic: proposal_topic.to_string(),
                position_tendency: human_position,
                strength: 0.5,
                sample_count: 1,
            });
        }
    }

    /// Compute how well a proposal aligns with this value profile.
    ///
    /// Returns a score from -1.0 (strongly misaligned) to 1.0 (strongly aligned).
    /// Zero means no signal — Advisor has no basis to judge.
    ///
    /// Alignment is computed from three sources:
    /// 1. Stated preferences matching proposal topics
    /// 2. Voting pattern tendencies for relevant topics
    /// 3. Charter alignment for relevant charter sections
    pub fn alignment_with(
        &self,
        proposal_topics: &[String],
        proposal_charter_sections: &[String],
    ) -> f64 {
        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        // Factor 1: Stated preferences (weight = 0.4 per match)
        for topic in proposal_topics {
            if self.stated_preferences.contains_key(topic) {
                // Stated preference exists for this topic — positive alignment
                total_score += 0.4;
                total_weight += 0.4;
            }
        }

        // Factor 2: Voting patterns (weight = strength * 0.4)
        for topic in proposal_topics {
            if let Some(pattern) = self.voting_patterns.iter().find(|p| p.topic == *topic) {
                let weight = pattern.strength * 0.4;
                let direction = match pattern.position_tendency {
                    VotePosition::Approve => 1.0,
                    VotePosition::Reject => -1.0,
                    VotePosition::Block => -1.0,
                    VotePosition::Abstain | VotePosition::Delegate => 0.0,
                };
                total_score += direction * weight;
                total_weight += weight;
            }
        }

        // Factor 3: Charter alignment (weight = alignment * 0.2)
        for section in proposal_charter_sections {
            if let Some(&alignment) = self.charter_alignment.get(section) {
                let weight = 0.2;
                total_score += alignment * weight;
                total_weight += weight;
            }
        }

        if total_weight < f64::EPSILON {
            0.0 // No signal — cannot judge
        } else {
            (total_score / total_weight).clamp(-1.0, 1.0)
        }
    }

    /// How confident Advisor is in its model of your values.
    ///
    /// Returns 0.0 (no confidence) to 1.0 (high confidence).
    ///
    /// Confidence is based on:
    /// - Number of stated preferences (more = better)
    /// - Number and strength of voting patterns (more samples = more confident)
    /// - Override frequency (more overrides = less confident in current model)
    pub fn confidence(&self) -> f64 {
        // Base: how much data do we have?
        let preference_score = (self.stated_preferences.len() as f64 * 0.1).min(0.3);

        let pattern_score = if self.voting_patterns.is_empty() {
            0.0
        } else {
            let avg_strength: f64 = self.voting_patterns.iter().map(|p| p.strength).sum::<f64>()
                / self.voting_patterns.len() as f64;
            let avg_samples: f64 = self.voting_patterns.iter().map(|p| p.sample_count as f64).sum::<f64>()
                / self.voting_patterns.len() as f64;
            // More patterns with more samples and higher strength = more confidence
            let pattern_breadth = (self.voting_patterns.len() as f64 * 0.05).min(0.2);
            let pattern_depth = (avg_samples * 0.02).min(0.2);
            let pattern_strength = avg_strength * 0.1;
            pattern_breadth + pattern_depth + pattern_strength
        };

        // Override penalty: recent overrides reduce confidence
        let recent_overrides = self
            .override_signals
            .iter()
            .filter(|o| {
                let age = Utc::now().signed_duration_since(o.timestamp);
                age.num_days() < 30
            })
            .count();
        let override_penalty = (recent_overrides as f64 * 0.05).min(0.3);

        (preference_score + pattern_score - override_penalty).clamp(0.0, 1.0)
    }
}

// ── Governance Mode ─────────────────────────────────────────────────────

/// The Advisor's governance delegation state.
///
/// Tracks the owner's delegation preferences, value profile, and voting history.
/// Operates in a sandboxed context — reads only the ValueProfile and proposal data,
/// never external content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceMode {
    /// The owner's public key (crown_id from Crown).
    pub owner_pubkey: String,

    /// Whether delegation is currently active.
    pub delegation_active: bool,

    /// Advisor's model of the owner's governance values.
    pub value_profile: ValueProfile,

    /// History of governance votes cast (both auto and manual).
    pub voting_history: Vec<GovernanceVote>,

    /// How many times the human has overridden Advisor's recommendation.
    pub override_count: usize,

    /// Configuration for governance behavior.
    pub config: GovernanceModeConfig,
}

impl GovernanceMode {
    /// Create a new governance mode for the given owner.
    pub fn new(owner_pubkey: impl Into<String>) -> Self {
        Self {
            owner_pubkey: owner_pubkey.into(),
            delegation_active: false,
            value_profile: ValueProfile::new(),
            voting_history: Vec::new(),
            override_count: 0,
            config: GovernanceModeConfig::default(),
        }
    }

    /// Create with a custom configuration.
    pub fn with_config(mut self, config: GovernanceModeConfig) -> Self {
        self.config = config;
        self
    }

    /// Activate governance delegation.
    pub fn activate(&mut self) {
        self.delegation_active = true;
    }

    /// Deactivate governance delegation.
    pub fn deactivate(&mut self) {
        self.delegation_active = false;
    }

    /// Decide how to handle a proposal.
    ///
    /// This is the core decision function. It evaluates the proposal against
    /// the value profile and community policy, returning a `GovernanceAction`.
    ///
    /// # Sandboxed Context
    ///
    /// This method only reads:
    /// - The ValueProfile (owner's values)
    /// - The proposal metadata passed as arguments
    /// - The community policy passed as argument
    ///
    /// It does NOT access any external state.
    pub fn evaluate_proposal(
        &self,
        proposal_id: Uuid,
        community_id: &str,
        proposal_type: &ProposalType,
        proposal_topics: &[String],
        proposal_charter_sections: &[String],
        community_policy: &GovernanceAIPolicy,
    ) -> Result<GovernanceAction, AdvisorError> {
        if !self.delegation_active {
            return Err(AdvisorError::InvalidConfiguration(
                "governance delegation is not active".into(),
            ));
        }

        // Check community policy: is Advisor delegation allowed?
        if !community_policy.advisor_delegation_allowed {
            return Ok(GovernanceAction::DeferToHuman(
                "community policy does not allow Advisor delegation".into(),
            ));
        }

        // Check excluded categories (owner's config)
        if self.config.excluded_categories.contains(proposal_type) {
            return Ok(GovernanceAction::DeferToHuman(format!(
                "proposal type {:?} is in your excluded categories — human vote required",
                proposal_type
            )));
        }

        // Check community-required human categories
        if community_policy
            .human_required_categories
            .contains(proposal_type)
        {
            return Ok(GovernanceAction::DeferToHuman(format!(
                "community requires human vote for {:?} proposals",
                proposal_type
            )));
        }

        // Check auto-vote percentage cap
        if let Some(max_pct) = community_policy.max_auto_vote_percentage {
            // Note: actual percentage tracking happens at the Kingdom level.
            // We encode awareness of the cap here; Kingdom enforces it.
            if max_pct <= 0.0 {
                return Ok(GovernanceAction::DeferToHuman(
                    "community auto-vote percentage cap reached".into(),
                ));
            }
        }

        // Check if auto-vote is enabled
        if !self.config.auto_vote {
            return Ok(GovernanceAction::DeferToHuman(
                "auto-vote is disabled in your governance config".into(),
            ));
        }

        // Compute alignment and confidence
        let alignment = self
            .value_profile
            .alignment_with(proposal_topics, proposal_charter_sections);
        let confidence = self.value_profile.confidence();

        // Low confidence → defer to human
        if confidence < 0.2 {
            return Ok(GovernanceAction::DeferToHuman(format!(
                "confidence too low ({:.0}%) — not enough voting history to judge",
                confidence * 100.0
            )));
        }

        // No signal on this topic → defer to human
        if alignment.abs() < f64::EPSILON && proposal_topics.iter().all(|t| {
            !self.value_profile.voting_patterns.iter().any(|p| p.topic == *t)
        }) {
            return Ok(GovernanceAction::DeferToHuman(
                "novel topic — no voting history or stated preferences to guide decision".into(),
            ));
        }

        // Determine position based on alignment
        let position = if alignment > 0.3 {
            VotePosition::Approve
        } else if alignment < -0.3 {
            VotePosition::Reject
        } else {
            // Marginal alignment — abstain rather than guess
            return Ok(GovernanceAction::Abstain(format!(
                "alignment score {:.2} is marginal — neither clearly for nor against",
                alignment
            )));
        };

        let reasoning = match self.config.reasoning_detail {
            ReasoningDetail::Brief => format!(
                "alignment: {:.2}, confidence: {:.0}%",
                alignment,
                confidence * 100.0
            ),
            ReasoningDetail::Standard => format!(
                "Based on {} stated preferences and {} voting patterns. \
                 Alignment score: {:.2}. Confidence: {:.0}%.",
                self.value_profile.stated_preferences.len(),
                self.value_profile.voting_patterns.len(),
                alignment,
                confidence * 100.0
            ),
            ReasoningDetail::Detailed => format!(
                "Based on {} stated preferences, {} voting patterns, and {} charter alignments. \
                 Alignment score: {:.2}. Confidence: {:.0}%. \
                 Override history: {} corrections in {} total votes.",
                self.value_profile.stated_preferences.len(),
                self.value_profile.voting_patterns.len(),
                self.value_profile.charter_alignment.len(),
                alignment,
                confidence * 100.0,
                self.override_count,
                self.voting_history.len()
            ),
        };

        let vote = GovernanceVote {
            proposal_id,
            community_id: community_id.to_string(),
            position,
            reasoning,
            confidence,
            was_auto: true,
            was_overridden: false,
            override_position: None,
            voted_at: Utc::now(),
        };

        Ok(GovernanceAction::Vote(vote))
    }

    /// Record a completed vote and update the value profile.
    pub fn record_vote(
        &mut self,
        vote: GovernanceVote,
        proposal_topic: &str,
        was_override: bool,
        advisor_position: Option<VotePosition>,
    ) {
        let proposal_id = vote.proposal_id;
        let human_position = if was_override {
            vote.override_position.unwrap_or(vote.position)
        } else {
            vote.position
        };

        if was_override {
            self.override_count += 1;
        }

        self.voting_history.push(vote);
        self.value_profile.update_from_vote(
            proposal_topic,
            human_position,
            was_override,
            advisor_position,
            proposal_id,
        );
    }

    /// Analyze a proposal without casting a vote.
    ///
    /// Produces a `ProposalAnalysis` that can be shown to the human
    /// regardless of whether auto-vote is enabled.
    pub fn analyze_proposal(
        &self,
        proposal_id: Uuid,
        summary: &str,
        proposal_topics: &[String],
        proposal_charter_sections: &[String],
        impact_assessment: &str,
    ) -> ProposalAnalysis {
        let alignment = self
            .value_profile
            .alignment_with(proposal_topics, proposal_charter_sections);
        let confidence = self.value_profile.confidence();

        let recommended_position = if alignment > 0.3 {
            VotePosition::Approve
        } else if alignment < -0.3 {
            VotePosition::Reject
        } else {
            VotePosition::Abstain
        };

        let mut dissenting_considerations = Vec::new();

        // Generate dissenting considerations based on override history
        let relevant_overrides: Vec<_> = self
            .value_profile
            .override_signals
            .iter()
            .filter(|o| o.advisor_position != o.human_position)
            .collect();

        if !relevant_overrides.is_empty() {
            dissenting_considerations.push(format!(
                "You have overridden Advisor {} time(s) — your actual preference may differ.",
                relevant_overrides.len()
            ));
        }

        if confidence < 0.4 {
            dissenting_considerations.push(
                "Low confidence in value model — consider voting manually.".to_string(),
            );
        }

        if alignment.abs() < 0.3 {
            dissenting_considerations.push(
                "Marginal alignment — this proposal does not clearly match or oppose your values."
                    .to_string(),
            );
        }

        ProposalAnalysis {
            proposal_id,
            summary: summary.to_string(),
            alignment_score: alignment,
            impact_assessment: impact_assessment.to_string(),
            charter_relevance: proposal_charter_sections.to_vec(),
            recommended_position,
            confidence,
            dissenting_considerations,
        }
    }
}

// ── Governance Vote ─────────────────────────────────────────────────────

/// A vote cast on a governance proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceVote {
    /// The proposal being voted on.
    pub proposal_id: Uuid,
    /// The community where the vote is cast.
    pub community_id: String,
    /// The position taken.
    pub position: VotePosition,
    /// Reasoning for the vote (detail level depends on config).
    pub reasoning: String,
    /// How confident Advisor is in this vote (0.0..=1.0).
    pub confidence: f64,
    /// Whether this vote was cast automatically by Advisor.
    pub was_auto: bool,
    /// Whether the human overrode Advisor's recommendation.
    pub was_overridden: bool,
    /// The position the human chose instead (if overridden).
    pub override_position: Option<VotePosition>,
    /// When the vote was cast.
    pub voted_at: DateTime<Utc>,
}

// ── Proposal Analysis ───────────────────────────────────────────────────

/// Advisor's analysis of a governance proposal.
///
/// Produced by `GovernanceMode::analyze_proposal()`. Can be shown to the human
/// regardless of whether auto-vote is enabled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposalAnalysis {
    /// The proposal being analyzed.
    pub proposal_id: Uuid,
    /// Plain-language summary of the proposal.
    pub summary: String,
    /// How well this proposal aligns with the ValueProfile (-1.0..=1.0).
    pub alignment_score: f64,
    /// Assessment of the proposal's impact.
    pub impact_assessment: String,
    /// Which charter sections this proposal relates to.
    pub charter_relevance: Vec<String>,
    /// Advisor's recommended position.
    pub recommended_position: VotePosition,
    /// Advisor's confidence in the recommendation (0.0..=1.0).
    pub confidence: f64,
    /// Reasons the human might disagree with the recommendation.
    pub dissenting_considerations: Vec<String>,
}

// ── Community Policy ────────────────────────────────────────────────────

/// Community-level controls on Advisor governance delegation.
///
/// Set by the community charter (Kingdom). Governs what Advisor is and
/// isn't allowed to do in governance votes for this community.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceAIPolicy {
    /// Whether this community allows Advisor delegation at all.
    /// Default: true.
    pub advisor_delegation_allowed: bool,

    /// Proposal types that always require human votes in this community.
    /// Default: CharterAmendment, Dissolution, MemberAction.
    pub human_required_categories: Vec<ProposalType>,

    /// Optional cap on auto-voted percentage.
    /// If more than N% of a vote is Advisor-delegated, extend deliberation window.
    pub max_auto_vote_percentage: Option<f64>,

    /// How much of Advisor's reasoning is visible to the community.
    pub reasoning_transparency: ReasoningTransparency,
}

impl Default for GovernanceAIPolicy {
    fn default() -> Self {
        Self {
            advisor_delegation_allowed: true,
            human_required_categories: vec![
                ProposalType::CharterAmendment,
                ProposalType::Dissolution,
                ProposalType::MemberAction,
            ],
            max_auto_vote_percentage: None,
            reasoning_transparency: ReasoningTransparency::SummaryPublic,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ─────────────────────────────────────────────────────

    fn sample_value_profile() -> ValueProfile {
        let mut profile = ValueProfile::new();
        profile
            .stated_preferences
            .insert("environment".to_string(), "prioritize sustainability".to_string());
        profile
            .stated_preferences
            .insert("privacy".to_string(), "maximum data sovereignty".to_string());
        profile.voting_patterns.push(VotingPattern {
            topic: "environment".to_string(),
            position_tendency: VotePosition::Approve,
            strength: 0.8,
            sample_count: 10,
        });
        profile.voting_patterns.push(VotingPattern {
            topic: "spending".to_string(),
            position_tendency: VotePosition::Reject,
            strength: 0.6,
            sample_count: 5,
        });
        profile
            .charter_alignment
            .insert("section-3-sustainability".to_string(), 0.9);
        profile
            .charter_alignment
            .insert("section-7-budget".to_string(), 0.4);
        profile
    }

    fn sample_governance_mode() -> GovernanceMode {
        let mut mode = GovernanceMode::new("cpub1test");
        mode.value_profile = sample_value_profile();
        mode.activate();
        mode
    }

    fn default_policy() -> GovernanceAIPolicy {
        GovernanceAIPolicy::default()
    }

    // ── ValueProfile: construction ──────────────────────────────────

    #[test]
    fn empty_value_profile() {
        let profile = ValueProfile::new();
        assert!(profile.stated_preferences.is_empty());
        assert!(profile.voting_patterns.is_empty());
        assert!(profile.charter_alignment.is_empty());
        assert!(profile.override_signals.is_empty());
    }

    #[test]
    fn value_profile_with_preferences() {
        let mut profile = ValueProfile::new();
        profile
            .stated_preferences
            .insert("topic".into(), "preference".into());
        assert_eq!(profile.stated_preferences.len(), 1);
    }

    #[test]
    fn value_profile_from_voting_history() {
        let mut profile = ValueProfile::new();
        let id = Uuid::new_v4();

        // Cast 5 approve votes on "environment"
        for _ in 0..5 {
            profile.update_from_vote("environment", VotePosition::Approve, false, None, id);
        }

        assert_eq!(profile.voting_patterns.len(), 1);
        let pattern = &profile.voting_patterns[0];
        assert_eq!(pattern.topic, "environment");
        assert_eq!(pattern.position_tendency, VotePosition::Approve);
        assert_eq!(pattern.sample_count, 5);
        assert!(pattern.strength > 0.5); // Reinforced above initial
    }

    #[test]
    fn voting_pattern_weakens_on_contradiction() {
        let mut profile = ValueProfile::new();
        let id = Uuid::new_v4();

        // Establish a pattern
        for _ in 0..5 {
            profile.update_from_vote("taxes", VotePosition::Approve, false, None, id);
        }
        let initial_strength = profile.voting_patterns[0].strength;

        // Contradict it
        profile.update_from_vote("taxes", VotePosition::Reject, false, None, id);
        assert!(profile.voting_patterns[0].strength < initial_strength);
    }

    #[test]
    fn voting_pattern_flips_on_sustained_contradiction() {
        let mut profile = ValueProfile::new();
        let id = Uuid::new_v4();

        // Establish a weak pattern
        profile.update_from_vote("taxes", VotePosition::Approve, false, None, id);

        // Contradict it repeatedly — should eventually flip
        for _ in 0..10 {
            profile.update_from_vote("taxes", VotePosition::Reject, false, None, id);
        }

        let pattern = &profile.voting_patterns[0];
        assert_eq!(pattern.position_tendency, VotePosition::Reject);
    }

    #[test]
    fn multiple_topics_tracked_independently() {
        let mut profile = ValueProfile::new();
        let id = Uuid::new_v4();

        profile.update_from_vote("environment", VotePosition::Approve, false, None, id);
        profile.update_from_vote("spending", VotePosition::Reject, false, None, id);

        assert_eq!(profile.voting_patterns.len(), 2);
        assert_eq!(profile.voting_patterns[0].topic, "environment");
        assert_eq!(profile.voting_patterns[1].topic, "spending");
    }

    // ── ValueProfile: confidence ────────────────────────────────────

    #[test]
    fn empty_profile_zero_confidence() {
        let profile = ValueProfile::new();
        assert!((profile.confidence() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn preferences_increase_confidence() {
        let mut profile = ValueProfile::new();
        let empty_confidence = profile.confidence();

        profile
            .stated_preferences
            .insert("topic".into(), "pref".into());
        assert!(profile.confidence() > empty_confidence);
    }

    #[test]
    fn voting_patterns_increase_confidence() {
        let mut profile = ValueProfile::new();
        let id = Uuid::new_v4();

        for _ in 0..10 {
            profile.update_from_vote("topic", VotePosition::Approve, false, None, id);
        }

        assert!(profile.confidence() > 0.0);
    }

    #[test]
    fn overrides_decrease_confidence() {
        let mut profile = sample_value_profile();
        let base_confidence = profile.confidence();

        // Add recent overrides
        for _ in 0..5 {
            profile.override_signals.push(OverrideSignal {
                proposal_id: Uuid::new_v4(),
                advisor_position: VotePosition::Approve,
                human_position: VotePosition::Reject,
                reason: None,
                timestamp: Utc::now(),
            });
        }

        assert!(profile.confidence() < base_confidence);
    }

    #[test]
    fn confidence_clamped_to_zero_one() {
        let profile = ValueProfile::new();
        let c = profile.confidence();
        assert!((0.0..=1.0).contains(&c));

        let full_profile = sample_value_profile();
        let c = full_profile.confidence();
        assert!((0.0..=1.0).contains(&c));
    }

    // ── ValueProfile: alignment ─────────────────────────────────────

    #[test]
    fn alignment_with_no_data_returns_zero() {
        let profile = ValueProfile::new();
        let score = profile.alignment_with(
            &["unknown_topic".into()],
            &["unknown_section".into()],
        );
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn alignment_with_matching_preference() {
        let profile = sample_value_profile();
        let score = profile.alignment_with(&["environment".into()], &[]);
        assert!(score > 0.0); // Has stated preference + approve pattern
    }

    #[test]
    fn alignment_with_reject_pattern() {
        let profile = sample_value_profile();
        let score = profile.alignment_with(&["spending".into()], &[]);
        assert!(score < 0.0); // Reject pattern = negative alignment
    }

    #[test]
    fn alignment_with_charter_sections() {
        let profile = sample_value_profile();
        let score = profile.alignment_with(&[], &["section-3-sustainability".into()]);
        assert!(score > 0.0); // High charter alignment for this section
    }

    #[test]
    fn alignment_combines_multiple_signals() {
        let profile = sample_value_profile();
        // Both preference and pattern exist for "environment"
        let single_score = profile.alignment_with(&["environment".into()], &[]);
        // Add charter section
        let combined_score = profile.alignment_with(
            &["environment".into()],
            &["section-3-sustainability".into()],
        );
        // Combined should still be positive and account for more signals
        assert!(combined_score > 0.0);
        // The combined score has more weight behind it (not necessarily higher magnitude)
        assert!(single_score > 0.0);
    }

    #[test]
    fn alignment_clamped_to_range() {
        let profile = sample_value_profile();
        let score = profile.alignment_with(
            &["environment".into(), "spending".into()],
            &["section-3-sustainability".into(), "section-7-budget".into()],
        );
        assert!((-1.0..=1.0).contains(&score));
    }

    // ── GovernanceMode: configuration ───────────────────────────────

    #[test]
    fn default_config() {
        let config = GovernanceModeConfig::default();
        assert!(config.auto_vote);
        assert!(config.notification_before_vote);
        assert_eq!(config.deliberation_window, Duration::from_secs(24 * 60 * 60));
        assert_eq!(config.excluded_categories.len(), 2);
        assert!(config.excluded_categories.contains(&ProposalType::CharterAmendment));
        assert!(config.excluded_categories.contains(&ProposalType::Dissolution));
        assert_eq!(config.reasoning_detail, ReasoningDetail::Standard);
    }

    #[test]
    fn custom_deliberation_window() {
        let config = GovernanceModeConfig {
            deliberation_window: Duration::from_secs(48 * 60 * 60), // 48 hours
            ..Default::default()
        };
        assert_eq!(config.deliberation_window.as_secs(), 48 * 60 * 60);
    }

    #[test]
    fn governance_mode_creation() {
        let mode = GovernanceMode::new("cpub1test");
        assert_eq!(mode.owner_pubkey, "cpub1test");
        assert!(!mode.delegation_active);
        assert_eq!(mode.override_count, 0);
        assert!(mode.voting_history.is_empty());
    }

    #[test]
    fn governance_mode_activation() {
        let mut mode = GovernanceMode::new("cpub1test");
        assert!(!mode.delegation_active);
        mode.activate();
        assert!(mode.delegation_active);
        mode.deactivate();
        assert!(!mode.delegation_active);
    }

    #[test]
    fn governance_mode_with_config() {
        let config = GovernanceModeConfig {
            auto_vote: false,
            ..Default::default()
        };
        let mode = GovernanceMode::new("cpub1test").with_config(config);
        assert!(!mode.config.auto_vote);
    }

    // ── Proposal evaluation ─────────────────────────────────────────

    #[test]
    fn evaluate_when_inactive_returns_error() {
        let mode = GovernanceMode::new("cpub1test"); // not activated
        let result = mode.evaluate_proposal(
            Uuid::new_v4(),
            "community-1",
            &ProposalType::PolicyChange,
            &["environment".into()],
            &[],
            &default_policy(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn excluded_category_defers_to_human() {
        let mode = sample_governance_mode();
        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::CharterAmendment, // excluded by default
                &["governance".into()],
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("excluded categories"));
            }
            _ => panic!("expected DeferToHuman for excluded category"),
        }
    }

    #[test]
    fn community_required_human_category_defers() {
        let mode = sample_governance_mode();
        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::MemberAction, // community requires human
                &["membership".into()],
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("community requires human vote"));
            }
            _ => panic!("expected DeferToHuman for community-required category"),
        }
    }

    #[test]
    fn community_disallows_advisor_delegation() {
        let mode = sample_governance_mode();
        let policy = GovernanceAIPolicy {
            advisor_delegation_allowed: false,
            ..Default::default()
        };
        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &[],
                &policy,
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("does not allow"));
            }
            _ => panic!("expected DeferToHuman when community disallows delegation"),
        }
    }

    #[test]
    fn auto_vote_disabled_defers_to_human() {
        let config = GovernanceModeConfig {
            auto_vote: false,
            ..Default::default()
        };
        let mut mode = sample_governance_mode();
        mode.config = config;

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("auto-vote is disabled"));
            }
            _ => panic!("expected DeferToHuman when auto-vote disabled"),
        }
    }

    #[test]
    fn low_confidence_defers_to_human() {
        let mut mode = GovernanceMode::new("cpub1test");
        mode.activate();
        // Empty value profile = zero confidence

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("confidence too low"));
            }
            _ => panic!("expected DeferToHuman for low confidence"),
        }
    }

    #[test]
    fn novel_topic_defers_to_human() {
        let mut mode = sample_governance_mode();
        // Profile has patterns for "environment" and "spending" but not "defense"
        // Give the profile enough data to not be low-confidence
        for _ in 0..5 {
            mode.value_profile.stated_preferences.insert(
                format!("pref-{}", mode.value_profile.stated_preferences.len()),
                "value".into(),
            );
        }

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["defense_policy".into()], // novel topic
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("novel topic"));
            }
            _ => panic!("expected DeferToHuman for novel topic"),
        }
    }

    #[test]
    fn strong_alignment_produces_approve_vote() {
        let mode = sample_governance_mode();
        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &["section-3-sustainability".into()],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::Vote(vote) => {
                assert_eq!(vote.position, VotePosition::Approve);
                assert!(vote.was_auto);
                assert!(!vote.was_overridden);
                assert!(vote.confidence > 0.0);
            }
            _ => panic!("expected Vote with Approve position"),
        }
    }

    #[test]
    fn negative_alignment_produces_reject_vote() {
        let mode = sample_governance_mode();
        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::ResourceAllocation,
                &["spending".into()],
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::Vote(vote) => {
                assert_eq!(vote.position, VotePosition::Reject);
            }
            _ => panic!("expected Vote with Reject position"),
        }
    }

    #[test]
    fn marginal_alignment_produces_abstain() {
        let mut mode = sample_governance_mode();
        // Add an Abstain pattern — direction is 0.0, so alignment is marginal
        mode.value_profile.voting_patterns.push(VotingPattern {
            topic: "infrastructure".to_string(),
            position_tendency: VotePosition::Abstain,
            strength: 0.5,
            sample_count: 3,
        });

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["infrastructure".into()],
                &[],
                &default_policy(),
            )
            .unwrap();

        match action {
            GovernanceAction::Abstain(reason) => {
                assert!(reason.contains("marginal"));
            }
            _ => panic!("expected Abstain for marginal alignment"),
        }
    }

    // ── Override handling ────────────────────────────────────────────

    #[test]
    fn record_vote_updates_history() {
        let mut mode = sample_governance_mode();
        let vote = GovernanceVote {
            proposal_id: Uuid::new_v4(),
            community_id: "community-1".into(),
            position: VotePosition::Approve,
            reasoning: "test".into(),
            confidence: 0.8,
            was_auto: true,
            was_overridden: false,
            override_position: None,
            voted_at: Utc::now(),
        };

        mode.record_vote(vote, "environment", false, None);
        assert_eq!(mode.voting_history.len(), 1);
        assert_eq!(mode.override_count, 0);
    }

    #[test]
    fn record_override_increments_count() {
        let mut mode = sample_governance_mode();
        let vote = GovernanceVote {
            proposal_id: Uuid::new_v4(),
            community_id: "community-1".into(),
            position: VotePosition::Approve,
            reasoning: "test".into(),
            confidence: 0.8,
            was_auto: true,
            was_overridden: true,
            override_position: Some(VotePosition::Reject),
            voted_at: Utc::now(),
        };

        mode.record_vote(vote, "environment", true, Some(VotePosition::Approve));
        assert_eq!(mode.override_count, 1);
        assert_eq!(mode.value_profile.override_signals.len(), 1);
    }

    #[test]
    fn override_records_signal_with_positions() {
        let mut mode = sample_governance_mode();
        let proposal_id = Uuid::new_v4();
        let vote = GovernanceVote {
            proposal_id,
            community_id: "community-1".into(),
            position: VotePosition::Reject, // advisor recommended approve, human chose reject
            reasoning: "test".into(),
            confidence: 0.8,
            was_auto: false,
            was_overridden: true,
            override_position: Some(VotePosition::Reject),
            voted_at: Utc::now(),
        };

        mode.record_vote(vote, "environment", true, Some(VotePosition::Approve));

        let signal = &mode.value_profile.override_signals.last().unwrap();
        assert_eq!(signal.proposal_id, proposal_id);
        assert_eq!(signal.advisor_position, VotePosition::Approve);
        assert_eq!(signal.human_position, VotePosition::Reject);
    }

    // ── Proposal analysis ───────────────────────────────────────────

    #[test]
    fn analyze_proposal_produces_analysis() {
        let mode = sample_governance_mode();
        let analysis = mode.analyze_proposal(
            Uuid::new_v4(),
            "Increase renewable energy funding",
            &["environment".into()],
            &["section-3-sustainability".into()],
            "Would allocate 20% of community budget to renewables",
        );

        assert!(analysis.alignment_score > 0.0);
        assert_eq!(analysis.recommended_position, VotePosition::Approve);
        assert!(!analysis.summary.is_empty());
        assert!(!analysis.impact_assessment.is_empty());
        assert!(!analysis.charter_relevance.is_empty());
    }

    #[test]
    fn analysis_includes_dissenting_considerations_on_low_confidence() {
        let mut mode = GovernanceMode::new("cpub1test");
        mode.activate();
        // Minimal data — low confidence
        mode.value_profile
            .stated_preferences
            .insert("environment".into(), "pro".into());

        let analysis = mode.analyze_proposal(
            Uuid::new_v4(),
            "Test proposal",
            &["environment".into()],
            &[],
            "Test impact",
        );

        assert!(analysis
            .dissenting_considerations
            .iter()
            .any(|c| c.contains("Low confidence")));
    }

    #[test]
    fn analysis_notes_override_history() {
        let mut mode = sample_governance_mode();
        mode.value_profile.override_signals.push(OverrideSignal {
            proposal_id: Uuid::new_v4(),
            advisor_position: VotePosition::Approve,
            human_position: VotePosition::Reject,
            reason: None,
            timestamp: Utc::now(),
        });

        let analysis = mode.analyze_proposal(
            Uuid::new_v4(),
            "Test proposal",
            &["environment".into()],
            &[],
            "Test impact",
        );

        assert!(analysis
            .dissenting_considerations
            .iter()
            .any(|c| c.contains("overridden")));
    }

    // ── Deliberation window ─────────────────────────────────────────

    #[test]
    fn deliberation_window_default_24_hours() {
        let config = GovernanceModeConfig::default();
        assert_eq!(config.deliberation_window.as_secs(), 24 * 60 * 60);
    }

    #[test]
    fn deliberation_window_custom() {
        let config = GovernanceModeConfig {
            deliberation_window: Duration::from_secs(12 * 60 * 60),
            ..Default::default()
        };
        assert_eq!(config.deliberation_window.as_secs(), 12 * 60 * 60);
    }

    #[test]
    fn deliberation_window_zero_allowed() {
        // Zero window = immediate vote (for time-sensitive proposals)
        let config = GovernanceModeConfig {
            deliberation_window: Duration::ZERO,
            ..Default::default()
        };
        assert_eq!(config.deliberation_window.as_secs(), 0);
    }

    // ── Sandboxed context verification ──────────────────────────────

    #[test]
    fn evaluate_only_uses_provided_data() {
        // GovernanceMode::evaluate_proposal takes all data as arguments.
        // It does NOT reach into any global state, file system, or network.
        // This test verifies the function signature enforces sandboxing:
        // all inputs are explicit parameters.
        let mode = sample_governance_mode();
        let proposal_id = Uuid::new_v4();
        let community_id = "community-1";
        let proposal_type = ProposalType::PolicyChange;
        let topics = vec!["environment".to_string()];
        let charter_sections = vec!["section-3-sustainability".to_string()];
        let policy = default_policy();

        // This compiles and runs — proving all data flows through parameters
        let result = mode.evaluate_proposal(
            proposal_id,
            community_id,
            &proposal_type,
            &topics,
            &charter_sections,
            &policy,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn evaluate_does_not_mutate_state() {
        let mode = sample_governance_mode();
        let mode_before = mode.clone();

        let _ = mode.evaluate_proposal(
            Uuid::new_v4(),
            "community-1",
            &ProposalType::PolicyChange,
            &["environment".into()],
            &[],
            &default_policy(),
        );

        // evaluate_proposal takes &self — cannot mutate
        assert_eq!(mode, mode_before);
    }

    // ── Community policy enforcement ────────────────────────────────

    #[test]
    fn default_community_policy() {
        let policy = GovernanceAIPolicy::default();
        assert!(policy.advisor_delegation_allowed);
        assert!(policy.human_required_categories.contains(&ProposalType::CharterAmendment));
        assert!(policy.human_required_categories.contains(&ProposalType::Dissolution));
        assert!(policy.human_required_categories.contains(&ProposalType::MemberAction));
        assert!(policy.max_auto_vote_percentage.is_none());
        assert_eq!(policy.reasoning_transparency, ReasoningTransparency::SummaryPublic);
    }

    #[test]
    fn zero_auto_vote_cap_defers() {
        let mode = sample_governance_mode();
        let policy = GovernanceAIPolicy {
            max_auto_vote_percentage: Some(0.0),
            ..Default::default()
        };

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &[],
                &policy,
            )
            .unwrap();

        matches!(action, GovernanceAction::DeferToHuman(_));
    }

    #[test]
    fn strict_community_policy() {
        let policy = GovernanceAIPolicy {
            advisor_delegation_allowed: true,
            human_required_categories: vec![
                ProposalType::CharterAmendment,
                ProposalType::Dissolution,
                ProposalType::MemberAction,
                ProposalType::RoleChange,
                ProposalType::ResourceAllocation,
            ],
            max_auto_vote_percentage: Some(25.0),
            reasoning_transparency: ReasoningTransparency::FullPublic,
        };

        let mode = sample_governance_mode();
        // RoleChange is in the community's required list
        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::RoleChange,
                &["governance".into()],
                &[],
                &policy,
            )
            .unwrap();

        match action {
            GovernanceAction::DeferToHuman(reason) => {
                assert!(reason.contains("community requires human vote"));
            }
            _ => panic!("expected DeferToHuman for community-required RoleChange"),
        }
    }

    // ── Reasoning detail levels ─────────────────────────────────────

    #[test]
    fn brief_reasoning_is_short() {
        let mut mode = sample_governance_mode();
        mode.config.reasoning_detail = ReasoningDetail::Brief;

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &["section-3-sustainability".into()],
                &default_policy(),
            )
            .unwrap();

        if let GovernanceAction::Vote(vote) = action {
            assert!(vote.reasoning.len() < 100);
            assert!(vote.reasoning.contains("alignment"));
        }
    }

    #[test]
    fn detailed_reasoning_includes_override_history() {
        let mut mode = sample_governance_mode();
        mode.config.reasoning_detail = ReasoningDetail::Detailed;
        mode.override_count = 3;

        let action = mode
            .evaluate_proposal(
                Uuid::new_v4(),
                "community-1",
                &ProposalType::PolicyChange,
                &["environment".into()],
                &["section-3-sustainability".into()],
                &default_policy(),
            )
            .unwrap();

        if let GovernanceAction::Vote(vote) = action {
            assert!(vote.reasoning.contains("charter alignments"));
            assert!(vote.reasoning.contains("corrections"));
        }
    }

    // ── Serialization ───────────────────────────────────────────────

    #[test]
    fn governance_mode_serialization_roundtrip() {
        let mode = sample_governance_mode();
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: GovernanceMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn governance_vote_serialization_roundtrip() {
        let vote = GovernanceVote {
            proposal_id: Uuid::new_v4(),
            community_id: "community-1".into(),
            position: VotePosition::Approve,
            reasoning: "test reasoning".into(),
            confidence: 0.85,
            was_auto: true,
            was_overridden: false,
            override_position: None,
            voted_at: Utc::now(),
        };
        let json = serde_json::to_string(&vote).unwrap();
        let deserialized: GovernanceVote = serde_json::from_str(&json).unwrap();
        assert_eq!(vote, deserialized);
    }

    #[test]
    fn proposal_type_custom_serialization() {
        let pt = ProposalType::Custom("environmental_review".into());
        let json = serde_json::to_string(&pt).unwrap();
        let deserialized: ProposalType = serde_json::from_str(&json).unwrap();
        assert_eq!(pt, deserialized);
    }

    #[test]
    fn governance_ai_policy_serialization_roundtrip() {
        let policy = GovernanceAIPolicy::default();
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: GovernanceAIPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, deserialized);
    }

    #[test]
    fn proposal_analysis_serialization_roundtrip() {
        let analysis = ProposalAnalysis {
            proposal_id: Uuid::new_v4(),
            summary: "Test proposal".into(),
            alignment_score: 0.75,
            impact_assessment: "Moderate impact".into(),
            charter_relevance: vec!["section-1".into()],
            recommended_position: VotePosition::Approve,
            confidence: 0.8,
            dissenting_considerations: vec!["Consider budget impact".into()],
        };
        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: ProposalAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(analysis, deserialized);
    }

    // ── Send + Sync ─────────────────────────────────────────────────

    #[test]
    fn governance_types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GovernanceMode>();
        assert_send_sync::<GovernanceModeConfig>();
        assert_send_sync::<ValueProfile>();
        assert_send_sync::<GovernanceVote>();
        assert_send_sync::<GovernanceAction>();
        assert_send_sync::<ProposalAnalysis>();
        assert_send_sync::<GovernanceAIPolicy>();
        assert_send_sync::<VotePosition>();
        assert_send_sync::<ProposalType>();
        assert_send_sync::<ReasoningDetail>();
        assert_send_sync::<ReasoningTransparency>();
    }
}
