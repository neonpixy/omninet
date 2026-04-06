//! Consortia -- business and enterprise competition features.
//!
//! Consortia compete through sponsored challenges, market competitions, and
//! innovation quests. This module provides the structures for tracking these
//! activities and computing standings.
//!
//! # Design Principles
//!
//! - **Opt-in and transparent.** Competition metrics have explicit weights.
//!   Standings are computed from real data, not opaque algorithms.
//! - **Sponsored does not mean manipulated.** Sponsored challenges use the same
//!   rules as community challenges -- sponsorship adds a Cool pool and branding,
//!   not special treatment.
//! - **Innovation over extraction.** Innovation quests reward building something
//!   new, not gaming metrics.
//!
//! # Example
//!
//! ```
//! use quest::consortia::{
//!     ConsortiaLeaderboard, SponsoredChallenge, ConsortiumSponsor,
//!     RewardDistribution, MarketCompetition, CompetitionSeason,
//! };
//! use chrono::{Utc, Duration};
//! use uuid::Uuid;
//!
//! let mut board = ConsortiaLeaderboard::new();
//! assert_eq!(board.count(), 0);
//! ```

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::challenge::ChallengeStatus;
use crate::error::QuestError;
use crate::progression::XpAmount;
use crate::reward::RewardType;

// ---------------------------------------------------------------------------
// SponsoredChallenge
// ---------------------------------------------------------------------------

/// A challenge backed by a consortium's Cool pool.
///
/// Sponsorship adds funding and optional branding to an existing challenge.
/// It does not change the challenge rules or give the sponsor any advantage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SponsoredChallenge {
    /// The underlying challenge (by reference ID).
    pub challenge_id: Uuid,
    /// Who is sponsoring.
    pub sponsor: ConsortiumSponsor,
    /// Total Cool available for participant rewards.
    pub cool_pool: u64,
    /// How Cool is distributed among completers.
    pub distribution: RewardDistribution,
    /// Optional visual branding for the challenge.
    pub branding: Option<ChallengeBranding>,
}

impl SponsoredChallenge {
    /// Create a new sponsored challenge.
    pub fn new(challenge_id: Uuid, sponsor: ConsortiumSponsor, cool_pool: u64) -> Self {
        Self {
            challenge_id,
            sponsor,
            cool_pool,
            distribution: RewardDistribution::AllCompleters,
            branding: None,
        }
    }

    /// Set the reward distribution strategy.
    pub fn with_distribution(mut self, distribution: RewardDistribution) -> Self {
        self.distribution = distribution;
        self
    }

    /// Set branding for the challenge.
    pub fn with_branding(mut self, branding: ChallengeBranding) -> Self {
        self.branding = Some(branding);
        self
    }
}

/// The entity sponsoring a challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsortiumSponsor {
    /// The consortium's ID.
    pub consortia_id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// The sponsor's public key.
    pub pubkey: String,
}

impl ConsortiumSponsor {
    /// Create a new sponsor.
    pub fn new(
        consortia_id: Uuid,
        name: impl Into<String>,
        pubkey: impl Into<String>,
    ) -> Self {
        Self {
            consortia_id,
            name: name.into(),
            pubkey: pubkey.into(),
        }
    }
}

/// How Cool from a sponsored challenge is distributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewardDistribution {
    /// Everyone who completes gets an equal share.
    EqualSplit,
    /// More progress = larger share.
    ProportionalToProgress,
    /// Only the top N performers. ONLY for opt-in competitive challenges.
    TopN {
        /// How many top performers receive rewards.
        n: u32,
    },
    /// Everyone who finishes gets the full reward amount.
    AllCompleters,
}

/// Visual branding for a sponsored challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeBranding {
    /// Prefix shown in the challenge title (e.g., "Sponsored by Acme Guild").
    pub title_prefix: String,
    /// Optional icon reference.
    pub icon: Option<String>,
    /// Optional brand color.
    pub color: Option<String>,
}

impl ChallengeBranding {
    /// Create branding with a title prefix.
    pub fn new(title_prefix: impl Into<String>) -> Self {
        Self {
            title_prefix: title_prefix.into(),
            icon: None,
            color: None,
        }
    }

    /// Set an icon.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set a brand color.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

// ---------------------------------------------------------------------------
// MarketCompetition
// ---------------------------------------------------------------------------

/// A structured competition between consortia in a specific sector.
///
/// Competitions run for defined seasons and track multiple weighted metrics.
/// Standings are transparent and deterministic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCompetition {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Industry sector (e.g., "design", "education", "commerce").
    pub sector: String,
    /// Consortia participating in this competition.
    pub participants: Vec<CompetitorEntry>,
    /// What is being measured and how it's weighted.
    pub metrics: Vec<CompetitionMetric>,
    /// The time window for this competition.
    pub season: CompetitionSeason,
    /// Lifecycle state.
    pub status: ChallengeStatus,
}

impl MarketCompetition {
    /// Create a new market competition.
    pub fn new(
        name: impl Into<String>,
        sector: impl Into<String>,
        season: CompetitionSeason,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            sector: sector.into(),
            participants: Vec::new(),
            metrics: Vec::new(),
            season,
            status: ChallengeStatus::Upcoming,
        }
    }

    /// Add a metric to the competition.
    pub fn with_metric(mut self, metric: CompetitionMetric) -> Self {
        self.metrics.push(metric);
        self
    }

    /// Set the status.
    pub fn with_status(mut self, status: ChallengeStatus) -> Self {
        self.status = status;
        self
    }

    /// Add a competitor entry.
    pub fn add_participant(&mut self, entry: CompetitorEntry) {
        self.participants.push(entry);
    }

    /// Compute weighted standings: `Vec<(consortia_id, weighted_score)>` sorted descending.
    ///
    /// Each metric's score is normalized by the weight. If `higher_is_better` is `false`,
    /// the score contribution is inverted (lower raw score = higher weighted contribution).
    pub fn standings(&self) -> Vec<(Uuid, f64)> {
        let mut results: Vec<(Uuid, f64)> = self
            .participants
            .iter()
            .map(|p| {
                let weighted_score: f64 = self
                    .metrics
                    .iter()
                    .map(|metric| {
                        let raw = p.scores.get(&metric.id).copied().unwrap_or(0.0);
                        if metric.higher_is_better {
                            raw * metric.weight
                        } else {
                            // Lower is better: invert by using 1/(1+raw) so it stays positive
                            (1.0 / (1.0 + raw)) * metric.weight
                        }
                    })
                    .sum();
                (p.consortia_id, weighted_score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

/// A consortium's entry in a market competition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitorEntry {
    /// The consortium's ID.
    pub consortia_id: Uuid,
    /// Human-readable name.
    pub consortia_name: String,
    /// Per-metric scores: metric_id -> score.
    pub scores: HashMap<String, f64>,
    /// When the consortium joined.
    pub joined_at: DateTime<Utc>,
}

impl CompetitorEntry {
    /// Create a new competitor entry.
    pub fn new(consortia_id: Uuid, name: impl Into<String>) -> Self {
        Self {
            consortia_id,
            consortia_name: name.into(),
            scores: HashMap::new(),
            joined_at: Utc::now(),
        }
    }

    /// Set a score for a metric.
    pub fn with_score(mut self, metric_id: impl Into<String>, score: f64) -> Self {
        self.scores.insert(metric_id.into(), score);
        self
    }
}

/// A weighted metric used in a market competition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionMetric {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// What this metric measures.
    pub description: String,
    /// Relative weight (0.0-1.0). All weights in a competition should sum to 1.0.
    pub weight: f64,
    /// Whether a higher score is better (true) or lower is better (false).
    pub higher_is_better: bool,
}

impl CompetitionMetric {
    /// Create a new metric where higher is better.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        weight: f64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            weight,
            higher_is_better: true,
        }
    }

    /// Set whether higher is better.
    pub fn with_higher_is_better(mut self, higher: bool) -> Self {
        self.higher_is_better = higher;
        self
    }
}

/// The time window for a competition season.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompetitionSeason {
    /// Season name (e.g., "Spring 2026").
    pub name: String,
    /// When the season starts.
    pub starts_at: DateTime<Utc>,
    /// When the season ends.
    pub ends_at: DateTime<Utc>,
}

impl CompetitionSeason {
    /// Create a new season.
    pub fn new(
        name: impl Into<String>,
        starts_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> Self {
        Self {
            name: name.into(),
            starts_at,
            ends_at,
        }
    }
}

// ---------------------------------------------------------------------------
// InnovationQuest
// ---------------------------------------------------------------------------

/// An innovation quest -- build something new around a theme.
///
/// Consortia submit .idea references that are evaluated against weighted criteria.
/// This rewards genuine creation, not metric gaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnovationQuest {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// What this quest is about.
    pub description: String,
    /// The innovation theme (e.g., "accessibility", "sustainability").
    pub theme: String,
    /// How submissions are evaluated.
    pub criteria: Vec<InnovationCriterion>,
    /// Rewards for winners.
    pub rewards: Vec<RewardType>,
    /// XP awarded for participation/winning.
    pub xp_reward: XpAmount,
    /// When submissions open.
    pub starts_at: DateTime<Utc>,
    /// When submissions close.
    pub ends_at: DateTime<Utc>,
    /// All submissions received.
    pub submissions: Vec<InnovationSubmission>,
}

impl InnovationQuest {
    /// Create a new innovation quest.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        theme: impl Into<String>,
        starts_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            theme: theme.into(),
            criteria: Vec::new(),
            rewards: Vec::new(),
            xp_reward: 0,
            starts_at,
            ends_at,
            submissions: Vec::new(),
        }
    }

    /// Add an evaluation criterion.
    pub fn with_criterion(mut self, criterion: InnovationCriterion) -> Self {
        self.criteria.push(criterion);
        self
    }

    /// Add a reward.
    pub fn with_reward(mut self, reward: RewardType) -> Self {
        self.rewards.push(reward);
        self
    }

    /// Set XP reward.
    pub fn with_xp_reward(mut self, xp: XpAmount) -> Self {
        self.xp_reward = xp;
        self
    }

    /// Submit an entry to this quest.
    pub fn submit(&mut self, submission: InnovationSubmission) {
        self.submissions.push(submission);
    }

    /// Compute weighted scores for all submissions and return sorted (highest first).
    pub fn ranked_submissions(&self) -> Vec<(Uuid, f64)> {
        let mut scores: Vec<(Uuid, f64)> = self
            .submissions
            .iter()
            .map(|sub| {
                let total: f64 = self
                    .criteria
                    .iter()
                    .map(|c| {
                        let raw = sub.scores.get(&c.name).copied().unwrap_or(0.0);
                        raw * c.weight
                    })
                    .sum();
                (sub.id, total)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}

/// A criterion for evaluating innovation submissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnovationCriterion {
    /// Name of the criterion (also used as the key in submission scores).
    pub name: String,
    /// What this criterion evaluates.
    pub description: String,
    /// Relative weight (0.0-1.0).
    pub weight: f64,
}

impl InnovationCriterion {
    /// Create a new criterion.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        weight: f64,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            weight,
        }
    }
}

/// A submission to an innovation quest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnovationSubmission {
    /// Unique identifier.
    pub id: Uuid,
    /// The submitting consortium.
    pub consortia_id: Uuid,
    /// Reference to the .idea being submitted.
    pub idea_ref: String,
    /// When the submission was made.
    pub submitted_at: DateTime<Utc>,
    /// Per-criterion scores (0.0-10.0), keyed by criterion name.
    pub scores: HashMap<String, f64>,
    /// Feedback comments from evaluators.
    pub feedback: Vec<String>,
}

impl InnovationSubmission {
    /// Create a new submission.
    pub fn new(
        consortia_id: Uuid,
        idea_ref: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            consortia_id,
            idea_ref: idea_ref.into(),
            submitted_at: Utc::now(),
            scores: HashMap::new(),
            feedback: Vec::new(),
        }
    }

    /// Set a score for a criterion.
    pub fn with_score(mut self, criterion: impl Into<String>, score: f64) -> Self {
        self.scores.insert(criterion.into(), score);
        self
    }

    /// Add feedback.
    pub fn with_feedback(mut self, comment: impl Into<String>) -> Self {
        self.feedback.push(comment.into());
        self
    }
}

// ---------------------------------------------------------------------------
// ConsortiaLeaderboard
// ---------------------------------------------------------------------------

/// Manages all consortia competitive activities.
///
/// Holds market competitions, sponsored challenges, and innovation quests.
/// Provides lookup, scoring, and standings computation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsortiaLeaderboard {
    competitions: Vec<MarketCompetition>,
    sponsored: Vec<SponsoredChallenge>,
    innovations: Vec<InnovationQuest>,
}

impl ConsortiaLeaderboard {
    /// Create an empty leaderboard.
    pub fn new() -> Self {
        Self {
            competitions: Vec::new(),
            sponsored: Vec::new(),
            innovations: Vec::new(),
        }
    }

    /// Register a market competition.
    pub fn add_competition(&mut self, competition: MarketCompetition) {
        self.competitions.push(competition);
    }

    /// Register a sponsored challenge.
    pub fn add_sponsored_challenge(&mut self, challenge: SponsoredChallenge) {
        self.sponsored.push(challenge);
    }

    /// Register an innovation quest.
    pub fn add_innovation_quest(&mut self, quest: InnovationQuest) {
        self.innovations.push(quest);
    }

    /// Look up a competition by ID.
    pub fn get_competition(&self, id: Uuid) -> Option<&MarketCompetition> {
        self.competitions.iter().find(|c| c.id == id)
    }

    /// Look up a sponsored challenge by challenge ID.
    pub fn get_sponsored(&self, challenge_id: Uuid) -> Option<&SponsoredChallenge> {
        self.sponsored.iter().find(|s| s.challenge_id == challenge_id)
    }

    /// Look up an innovation quest by ID.
    pub fn get_innovation(&self, id: Uuid) -> Option<&InnovationQuest> {
        self.innovations.iter().find(|i| i.id == id)
    }

    /// All currently active competitions.
    pub fn active_competitions(&self) -> Vec<&MarketCompetition> {
        self.competitions
            .iter()
            .filter(|c| c.status == ChallengeStatus::Active)
            .collect()
    }

    /// Submit an innovation entry.
    pub fn submit_innovation(
        &mut self,
        quest_id: Uuid,
        submission: InnovationSubmission,
    ) -> Result<(), QuestError> {
        let quest = self
            .innovations
            .iter_mut()
            .find(|i| i.id == quest_id)
            .ok_or_else(|| QuestError::NotFound(format!("innovation quest {quest_id}")))?;

        quest.submit(submission);
        Ok(())
    }

    /// Score an innovation submission.
    pub fn score_innovation(
        &mut self,
        quest_id: Uuid,
        submission_id: Uuid,
        scores: HashMap<String, f64>,
    ) -> Result<(), QuestError> {
        let quest = self
            .innovations
            .iter_mut()
            .find(|i| i.id == quest_id)
            .ok_or_else(|| QuestError::NotFound(format!("innovation quest {quest_id}")))?;

        let submission = quest
            .submissions
            .iter_mut()
            .find(|s| s.id == submission_id)
            .ok_or_else(|| {
                QuestError::NotFound(format!("submission {submission_id}"))
            })?;

        submission.scores = scores;
        Ok(())
    }

    /// Compute weighted standings for a competition.
    pub fn competition_standings(&self, competition_id: Uuid) -> Result<Vec<(Uuid, f64)>, QuestError> {
        let competition = self
            .get_competition(competition_id)
            .ok_or_else(|| QuestError::NotFound(format!("competition {competition_id}")))?;

        Ok(competition.standings())
    }

    /// Total count of all consortia activities.
    pub fn count(&self) -> usize {
        self.competitions.len() + self.sponsored.len() + self.innovations.len()
    }

    // --- Federation-scoped queries ---

    /// All competitions that have at least one participant from a visible consortia.
    ///
    /// When the scope is unrestricted, all competitions are returned.
    /// When scoped, a competition is included if any of its participants'
    /// `consortia_id` is visible in the scope.
    pub fn competitions_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&MarketCompetition> {
        if scope.is_unrestricted() {
            return self.competitions.iter().collect();
        }
        self.competitions
            .iter()
            .filter(|c| {
                c.participants.is_empty()
                    || c.participants
                        .iter()
                        .any(|p| scope.is_visible_uuid(&p.consortia_id))
            })
            .collect()
    }

    /// Active competitions with at least one participant from a visible consortia.
    ///
    /// Combines the active filter with federation scoping.
    pub fn active_competitions_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&MarketCompetition> {
        if scope.is_unrestricted() {
            return self.active_competitions();
        }
        self.competitions
            .iter()
            .filter(|c| {
                c.status == ChallengeStatus::Active
                    && (c.participants.is_empty()
                        || c.participants
                            .iter()
                            .any(|p| scope.is_visible_uuid(&p.consortia_id)))
            })
            .collect()
    }

    /// Innovation quests that have at least one submission from a visible consortia.
    ///
    /// When the scope is unrestricted, all innovation quests are returned.
    /// When scoped, a quest is included if it has no submissions yet (open to
    /// federated participants) or has at least one submission from a visible consortia.
    pub fn innovations_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&InnovationQuest> {
        if scope.is_unrestricted() {
            return self.innovations.iter().collect();
        }
        self.innovations
            .iter()
            .filter(|q| {
                q.submissions.is_empty()
                    || q.submissions
                        .iter()
                        .any(|s| scope.is_visible_uuid(&s.consortia_id))
            })
            .collect()
    }

    /// Compute standings for a competition, filtered to visible consortia.
    ///
    /// Only participants whose `consortia_id` is visible in the scope are
    /// included in the standings. When unrestricted, returns all standings.
    pub fn competition_standings_scoped(
        &self,
        competition_id: Uuid,
        scope: &crate::federation_scope::FederationScope,
    ) -> Result<Vec<(Uuid, f64)>, QuestError> {
        let competition = self
            .get_competition(competition_id)
            .ok_or_else(|| QuestError::NotFound(format!("competition {competition_id}")))?;

        if scope.is_unrestricted() {
            return Ok(competition.standings());
        }

        let mut results: Vec<(Uuid, f64)> = competition
            .participants
            .iter()
            .filter(|p| scope.is_visible_uuid(&p.consortia_id))
            .map(|p| {
                let weighted_score: f64 = competition
                    .metrics
                    .iter()
                    .map(|metric| {
                        let raw = p.scores.get(&metric.id).copied().unwrap_or(0.0);
                        if metric.higher_is_better {
                            raw * metric.weight
                        } else {
                            (1.0 / (1.0 + raw)) * metric.weight
                        }
                    })
                    .sum();
                (p.consortia_id, weighted_score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results)
    }

    /// Total count of consortia activities visible within the federation scope.
    ///
    /// Counts competitions with visible participants, all sponsored challenges
    /// (sponsorship is cross-cutting), and innovations with visible submissions.
    pub fn count_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> usize {
        if scope.is_unrestricted() {
            return self.count();
        }
        self.competitions_scoped(scope).len()
            + self.sponsored.len()
            + self.innovations_scoped(scope).len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample_season() -> CompetitionSeason {
        let now = Utc::now();
        CompetitionSeason::new("Spring 2026", now, now + Duration::days(90))
    }

    fn sample_sponsor() -> ConsortiumSponsor {
        ConsortiumSponsor::new(Uuid::new_v4(), "Acme Guild", "cpub1acme")
    }

    fn sample_competition() -> MarketCompetition {
        MarketCompetition::new("Design Excellence", "design", sample_season())
            .with_metric(CompetitionMetric::new(
                "quality",
                "Quality",
                "Design quality score",
                0.6,
            ))
            .with_metric(
                CompetitionMetric::new(
                    "accessibility",
                    "Accessibility",
                    "WCAG compliance",
                    0.4,
                ),
            )
            .with_status(ChallengeStatus::Active)
    }

    fn sample_innovation_quest() -> InnovationQuest {
        let now = Utc::now();
        InnovationQuest::new(
            "Accessibility Jam",
            "Build the most accessible product",
            "accessibility",
            now,
            now + Duration::days(30),
        )
        .with_criterion(InnovationCriterion::new("design", "Visual design quality", 0.5))
        .with_criterion(InnovationCriterion::new("a11y", "Accessibility score", 0.5))
        .with_xp_reward(500)
    }

    // --- ConsortiumSponsor ---

    #[test]
    fn sponsor_new() {
        let id = Uuid::new_v4();
        let s = ConsortiumSponsor::new(id, "Guild", "cpub1x");
        assert_eq!(s.consortia_id, id);
        assert_eq!(s.name, "Guild");
        assert_eq!(s.pubkey, "cpub1x");
    }

    #[test]
    fn sponsor_serde_round_trip() {
        let s = sample_sponsor();
        let json = serde_json::to_string(&s).unwrap();
        let restored: ConsortiumSponsor = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, s);
    }

    // --- SponsoredChallenge ---

    #[test]
    fn sponsored_challenge_new() {
        let challenge_id = Uuid::new_v4();
        let sc = SponsoredChallenge::new(challenge_id, sample_sponsor(), 1000);
        assert_eq!(sc.challenge_id, challenge_id);
        assert_eq!(sc.cool_pool, 1000);
        assert_eq!(sc.distribution, RewardDistribution::AllCompleters);
        assert!(sc.branding.is_none());
    }

    #[test]
    fn sponsored_challenge_builder() {
        let sc = SponsoredChallenge::new(Uuid::new_v4(), sample_sponsor(), 5000)
            .with_distribution(RewardDistribution::EqualSplit)
            .with_branding(
                ChallengeBranding::new("Sponsored by Acme")
                    .with_icon("acme-icon")
                    .with_color("#FF5500"),
            );

        assert_eq!(sc.distribution, RewardDistribution::EqualSplit);
        assert!(sc.branding.is_some());
        let branding = sc.branding.unwrap();
        assert_eq!(branding.title_prefix, "Sponsored by Acme");
        assert_eq!(branding.icon, Some("acme-icon".into()));
        assert_eq!(branding.color, Some("#FF5500".into()));
    }

    #[test]
    fn sponsored_challenge_serde_round_trip() {
        let sc = SponsoredChallenge::new(Uuid::new_v4(), sample_sponsor(), 1000);
        let json = serde_json::to_string(&sc).unwrap();
        let restored: SponsoredChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.challenge_id, sc.challenge_id);
        assert_eq!(restored.cool_pool, 1000);
    }

    // --- RewardDistribution ---

    #[test]
    fn reward_distribution_serde_round_trip() {
        let distributions = [
            RewardDistribution::EqualSplit,
            RewardDistribution::ProportionalToProgress,
            RewardDistribution::TopN { n: 3 },
            RewardDistribution::AllCompleters,
        ];
        for d in &distributions {
            let json = serde_json::to_string(d).unwrap();
            let restored: RewardDistribution = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, d);
        }
    }

    // --- ChallengeBranding ---

    #[test]
    fn branding_new() {
        let b = ChallengeBranding::new("Sponsored");
        assert_eq!(b.title_prefix, "Sponsored");
        assert!(b.icon.is_none());
        assert!(b.color.is_none());
    }

    #[test]
    fn branding_serde_round_trip() {
        let b = ChallengeBranding::new("Acme")
            .with_icon("logo")
            .with_color("#000");
        let json = serde_json::to_string(&b).unwrap();
        let restored: ChallengeBranding = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, b);
    }

    // --- CompetitionSeason ---

    #[test]
    fn season_new() {
        let s = sample_season();
        assert_eq!(s.name, "Spring 2026");
        assert!(s.ends_at > s.starts_at);
    }

    #[test]
    fn season_serde_round_trip() {
        let s = sample_season();
        let json = serde_json::to_string(&s).unwrap();
        let restored: CompetitionSeason = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, s);
    }

    // --- CompetitionMetric ---

    #[test]
    fn metric_new() {
        let m = CompetitionMetric::new("quality", "Quality", "Quality score", 0.7);
        assert_eq!(m.id, "quality");
        assert_eq!(m.weight, 0.7);
        assert!(m.higher_is_better);
    }

    #[test]
    fn metric_lower_is_better() {
        let m = CompetitionMetric::new("latency", "Latency", "Response time", 0.3)
            .with_higher_is_better(false);
        assert!(!m.higher_is_better);
    }

    #[test]
    fn metric_serde_round_trip() {
        let m = CompetitionMetric::new("x", "X", "desc", 0.5);
        let json = serde_json::to_string(&m).unwrap();
        let restored: CompetitionMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "x");
        assert_eq!(restored.weight, 0.5);
    }

    // --- CompetitorEntry ---

    #[test]
    fn competitor_entry_new() {
        let id = Uuid::new_v4();
        let e = CompetitorEntry::new(id, "Guild Alpha");
        assert_eq!(e.consortia_id, id);
        assert_eq!(e.consortia_name, "Guild Alpha");
        assert!(e.scores.is_empty());
    }

    #[test]
    fn competitor_entry_with_scores() {
        let e = CompetitorEntry::new(Uuid::new_v4(), "Guild")
            .with_score("quality", 8.5)
            .with_score("speed", 7.0);
        assert_eq!(e.scores.len(), 2);
        assert_eq!(e.scores["quality"], 8.5);
    }

    #[test]
    fn competitor_entry_serde_round_trip() {
        let e = CompetitorEntry::new(Uuid::new_v4(), "Guild")
            .with_score("q", 9.0);
        let json = serde_json::to_string(&e).unwrap();
        let restored: CompetitorEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.consortia_id, e.consortia_id);
        assert_eq!(restored.scores["q"], 9.0);
    }

    // --- MarketCompetition ---

    #[test]
    fn competition_new() {
        let c = sample_competition();
        assert_eq!(c.name, "Design Excellence");
        assert_eq!(c.sector, "design");
        assert_eq!(c.metrics.len(), 2);
        assert_eq!(c.status, ChallengeStatus::Active);
    }

    #[test]
    fn competition_add_participant() {
        let mut c = sample_competition();
        c.add_participant(CompetitorEntry::new(Uuid::new_v4(), "Guild A"));
        assert_eq!(c.participants.len(), 1);
    }

    #[test]
    fn competition_standings_empty() {
        let c = sample_competition();
        assert!(c.standings().is_empty());
    }

    #[test]
    fn competition_standings_sorted() {
        let mut c = sample_competition();
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        c.add_participant(
            CompetitorEntry::new(id_a, "A")
                .with_score("quality", 6.0)
                .with_score("accessibility", 8.0),
        );
        c.add_participant(
            CompetitorEntry::new(id_b, "B")
                .with_score("quality", 9.0)
                .with_score("accessibility", 7.0),
        );

        let standings = c.standings();
        assert_eq!(standings.len(), 2);
        // B: 9.0*0.6 + 7.0*0.4 = 5.4 + 2.8 = 8.2
        // A: 6.0*0.6 + 8.0*0.4 = 3.6 + 3.2 = 6.8
        assert_eq!(standings[0].0, id_b);
        assert_eq!(standings[1].0, id_a);
        assert!((standings[0].1 - 8.2).abs() < 0.001);
        assert!((standings[1].1 - 6.8).abs() < 0.001);
    }

    #[test]
    fn competition_standings_lower_is_better() {
        let season = sample_season();
        let mut c = MarketCompetition::new("Efficiency", "ops", season)
            .with_metric(
                CompetitionMetric::new("latency", "Latency", "ms", 1.0)
                    .with_higher_is_better(false),
            )
            .with_status(ChallengeStatus::Active);

        let id_fast = Uuid::new_v4();
        let id_slow = Uuid::new_v4();

        c.add_participant(
            CompetitorEntry::new(id_fast, "Fast").with_score("latency", 10.0),
        );
        c.add_participant(
            CompetitorEntry::new(id_slow, "Slow").with_score("latency", 100.0),
        );

        let standings = c.standings();
        // Lower latency = higher score via 1/(1+raw)
        // Fast: 1/(1+10) = 0.0909
        // Slow: 1/(1+100) = 0.0099
        assert_eq!(standings[0].0, id_fast);
    }

    #[test]
    fn competition_serde_round_trip() {
        let c = sample_competition();
        let json = serde_json::to_string(&c).unwrap();
        let restored: MarketCompetition = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, c.id);
        assert_eq!(restored.metrics.len(), 2);
    }

    // --- InnovationCriterion ---

    #[test]
    fn innovation_criterion_new() {
        let c = InnovationCriterion::new("design", "Design quality", 0.5);
        assert_eq!(c.name, "design");
        assert_eq!(c.weight, 0.5);
    }

    #[test]
    fn innovation_criterion_serde_round_trip() {
        let c = InnovationCriterion::new("a", "b", 0.3);
        let json = serde_json::to_string(&c).unwrap();
        let restored: InnovationCriterion = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "a");
    }

    // --- InnovationSubmission ---

    #[test]
    fn submission_new() {
        let cid = Uuid::new_v4();
        let s = InnovationSubmission::new(cid, "idea://my-design");
        assert_eq!(s.consortia_id, cid);
        assert_eq!(s.idea_ref, "idea://my-design");
        assert!(s.scores.is_empty());
        assert!(s.feedback.is_empty());
    }

    #[test]
    fn submission_with_scores_and_feedback() {
        let s = InnovationSubmission::new(Uuid::new_v4(), "idea://x")
            .with_score("design", 8.5)
            .with_score("a11y", 9.0)
            .with_feedback("Great work on accessibility");

        assert_eq!(s.scores.len(), 2);
        assert_eq!(s.feedback.len(), 1);
    }

    #[test]
    fn submission_serde_round_trip() {
        let s = InnovationSubmission::new(Uuid::new_v4(), "idea://test")
            .with_score("design", 7.0);
        let json = serde_json::to_string(&s).unwrap();
        let restored: InnovationSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, s.id);
        assert_eq!(restored.scores["design"], 7.0);
    }

    // --- InnovationQuest ---

    #[test]
    fn innovation_quest_new() {
        let q = sample_innovation_quest();
        assert_eq!(q.name, "Accessibility Jam");
        assert_eq!(q.theme, "accessibility");
        assert_eq!(q.criteria.len(), 2);
        assert_eq!(q.xp_reward, 500);
        assert!(q.submissions.is_empty());
    }

    #[test]
    fn innovation_quest_submit() {
        let mut q = sample_innovation_quest();
        let sub = InnovationSubmission::new(Uuid::new_v4(), "idea://entry1");
        q.submit(sub);
        assert_eq!(q.submissions.len(), 1);
    }

    #[test]
    fn innovation_quest_ranked_submissions() {
        let mut q = sample_innovation_quest();

        let sub1 = InnovationSubmission::new(Uuid::new_v4(), "idea://1")
            .with_score("design", 8.0)
            .with_score("a11y", 6.0);
        let sub2 = InnovationSubmission::new(Uuid::new_v4(), "idea://2")
            .with_score("design", 7.0)
            .with_score("a11y", 10.0);

        let id1 = sub1.id;
        let id2 = sub2.id;
        q.submit(sub1);
        q.submit(sub2);

        let ranked = q.ranked_submissions();
        // sub1: 8.0*0.5 + 6.0*0.5 = 7.0
        // sub2: 7.0*0.5 + 10.0*0.5 = 8.5
        assert_eq!(ranked[0].0, id2);
        assert_eq!(ranked[1].0, id1);
        assert!((ranked[0].1 - 8.5).abs() < 0.001);
        assert!((ranked[1].1 - 7.0).abs() < 0.001);
    }

    #[test]
    fn innovation_quest_ranked_empty() {
        let q = sample_innovation_quest();
        assert!(q.ranked_submissions().is_empty());
    }

    #[test]
    fn innovation_quest_serde_round_trip() {
        let q = sample_innovation_quest();
        let json = serde_json::to_string(&q).unwrap();
        let restored: InnovationQuest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, q.id);
        assert_eq!(restored.theme, "accessibility");
    }

    // --- ConsortiaLeaderboard ---

    #[test]
    fn leaderboard_empty() {
        let lb = ConsortiaLeaderboard::new();
        assert_eq!(lb.count(), 0);
        assert!(lb.active_competitions().is_empty());
    }

    #[test]
    fn leaderboard_add_competition() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition());
        assert_eq!(lb.count(), 1);
        assert_eq!(lb.active_competitions().len(), 1);
    }

    #[test]
    fn leaderboard_add_sponsored() {
        let mut lb = ConsortiaLeaderboard::new();
        let sc = SponsoredChallenge::new(Uuid::new_v4(), sample_sponsor(), 1000);
        let cid = sc.challenge_id;
        lb.add_sponsored_challenge(sc);
        assert_eq!(lb.count(), 1);
        assert!(lb.get_sponsored(cid).is_some());
    }

    #[test]
    fn leaderboard_add_innovation() {
        let mut lb = ConsortiaLeaderboard::new();
        let q = sample_innovation_quest();
        let id = q.id;
        lb.add_innovation_quest(q);
        assert_eq!(lb.count(), 1);
        assert!(lb.get_innovation(id).is_some());
    }

    #[test]
    fn leaderboard_get_competition() {
        let mut lb = ConsortiaLeaderboard::new();
        let c = sample_competition();
        let id = c.id;
        lb.add_competition(c);
        assert!(lb.get_competition(id).is_some());
        assert!(lb.get_competition(Uuid::new_v4()).is_none());
    }

    #[test]
    fn leaderboard_submit_innovation() {
        let mut lb = ConsortiaLeaderboard::new();
        let q = sample_innovation_quest();
        let quest_id = q.id;
        lb.add_innovation_quest(q);

        let sub = InnovationSubmission::new(Uuid::new_v4(), "idea://test");
        lb.submit_innovation(quest_id, sub).unwrap();
        assert_eq!(lb.get_innovation(quest_id).unwrap().submissions.len(), 1);
    }

    #[test]
    fn leaderboard_submit_innovation_not_found() {
        let mut lb = ConsortiaLeaderboard::new();
        let sub = InnovationSubmission::new(Uuid::new_v4(), "idea://x");
        let result = lb.submit_innovation(Uuid::new_v4(), sub);
        assert!(result.is_err());
    }

    #[test]
    fn leaderboard_score_innovation() {
        let mut lb = ConsortiaLeaderboard::new();
        let q = sample_innovation_quest();
        let quest_id = q.id;
        lb.add_innovation_quest(q);

        let sub = InnovationSubmission::new(Uuid::new_v4(), "idea://x");
        let sub_id = sub.id;
        lb.submit_innovation(quest_id, sub).unwrap();

        let mut scores = HashMap::new();
        scores.insert("design".into(), 9.0);
        scores.insert("a11y".into(), 8.0);
        lb.score_innovation(quest_id, sub_id, scores).unwrap();

        let q = lb.get_innovation(quest_id).unwrap();
        assert_eq!(q.submissions[0].scores["design"], 9.0);
    }

    #[test]
    fn leaderboard_score_innovation_not_found() {
        let mut lb = ConsortiaLeaderboard::new();
        let result = lb.score_innovation(Uuid::new_v4(), Uuid::new_v4(), HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn leaderboard_competition_standings() {
        let mut lb = ConsortiaLeaderboard::new();
        let mut c = sample_competition();
        let comp_id = c.id;

        c.add_participant(
            CompetitorEntry::new(Uuid::new_v4(), "A")
                .with_score("quality", 10.0)
                .with_score("accessibility", 10.0),
        );
        lb.add_competition(c);

        let standings = lb.competition_standings(comp_id).unwrap();
        assert_eq!(standings.len(), 1);
        // 10*0.6 + 10*0.4 = 10.0
        assert!((standings[0].1 - 10.0).abs() < 0.001);
    }

    #[test]
    fn leaderboard_competition_standings_not_found() {
        let lb = ConsortiaLeaderboard::new();
        let result = lb.competition_standings(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn leaderboard_count_all_types() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition());
        lb.add_sponsored_challenge(SponsoredChallenge::new(
            Uuid::new_v4(),
            sample_sponsor(),
            500,
        ));
        lb.add_innovation_quest(sample_innovation_quest());
        assert_eq!(lb.count(), 3);
    }

    #[test]
    fn leaderboard_serde_round_trip() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition());
        lb.add_innovation_quest(sample_innovation_quest());
        let json = serde_json::to_string(&lb).unwrap();
        let restored: ConsortiaLeaderboard = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.count(), 2);
    }

    // --- Transparency checks ---

    #[test]
    fn standings_deterministic() {
        let mut c = sample_competition();
        let id = Uuid::new_v4();
        c.add_participant(
            CompetitorEntry::new(id, "G")
                .with_score("quality", 5.0)
                .with_score("accessibility", 5.0),
        );

        let s1 = c.standings();
        let s2 = c.standings();
        assert_eq!(s1[0].0, s2[0].0);
        assert_eq!(s1[0].1, s2[0].1);
    }

    #[test]
    fn missing_scores_treated_as_zero() {
        let mut c = sample_competition();
        // Participant with no scores
        c.add_participant(CompetitorEntry::new(Uuid::new_v4(), "Empty"));
        let standings = c.standings();
        assert_eq!(standings.len(), 1);
        assert_eq!(standings[0].1, 0.0);
    }

    // --- Federation-scoped queries ---

    #[test]
    fn competitions_scoped_unrestricted() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(lb.competitions_scoped(&scope).len(), 1);
    }

    #[test]
    fn competitions_scoped_filters_by_consortia() {
        let mut lb = ConsortiaLeaderboard::new();

        let cid_visible = Uuid::new_v4();
        let cid_hidden = Uuid::new_v4();

        let mut comp1 = sample_competition();
        comp1.add_participant(CompetitorEntry::new(cid_visible, "Visible Guild"));

        let mut comp2 = sample_competition();
        comp2.add_participant(CompetitorEntry::new(cid_hidden, "Hidden Guild"));

        lb.add_competition(comp1);
        lb.add_competition(comp2);

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid_visible.to_string(),
        ]);
        let visible = lb.competitions_scoped(&scope);
        assert_eq!(visible.len(), 1);
        assert!(visible[0]
            .participants
            .iter()
            .any(|p| p.consortia_id == cid_visible));
    }

    #[test]
    fn competitions_scoped_includes_empty_participant_list() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition()); // no participants

        let scope = crate::federation_scope::FederationScope::from_communities([
            "some-community",
        ]);
        // Competitions with no participants are included (open to all)
        assert_eq!(lb.competitions_scoped(&scope).len(), 1);
    }

    #[test]
    fn active_competitions_scoped_unrestricted() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition()); // Active

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(lb.active_competitions_scoped(&scope).len(), 1);
    }

    #[test]
    fn active_competitions_scoped_filters() {
        let mut lb = ConsortiaLeaderboard::new();

        let cid = Uuid::new_v4();

        let mut active = sample_competition();
        active.add_participant(CompetitorEntry::new(cid, "Guild"));

        let mut inactive = MarketCompetition::new("Upcoming", "tech", sample_season())
            .with_status(ChallengeStatus::Upcoming);
        inactive.add_participant(CompetitorEntry::new(cid, "Guild"));

        lb.add_competition(active);
        lb.add_competition(inactive);

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid.to_string(),
        ]);
        let visible = lb.active_competitions_scoped(&scope);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].status, ChallengeStatus::Active);
    }

    #[test]
    fn innovations_scoped_unrestricted() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_innovation_quest(sample_innovation_quest());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(lb.innovations_scoped(&scope).len(), 1);
    }

    #[test]
    fn innovations_scoped_filters_by_submission_consortia() {
        let mut lb = ConsortiaLeaderboard::new();
        let cid_visible = Uuid::new_v4();
        let cid_hidden = Uuid::new_v4();

        let mut q1 = sample_innovation_quest();
        q1.submit(InnovationSubmission::new(cid_visible, "idea://1"));

        let mut q2 = sample_innovation_quest();
        q2.submit(InnovationSubmission::new(cid_hidden, "idea://2"));

        lb.add_innovation_quest(q1);
        lb.add_innovation_quest(q2);

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid_visible.to_string(),
        ]);
        let visible = lb.innovations_scoped(&scope);
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn innovations_scoped_includes_empty_submissions() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_innovation_quest(sample_innovation_quest()); // no submissions

        let scope = crate::federation_scope::FederationScope::from_communities([
            "some-community",
        ]);
        assert_eq!(lb.innovations_scoped(&scope).len(), 1);
    }

    #[test]
    fn competition_standings_scoped_unrestricted() {
        let mut lb = ConsortiaLeaderboard::new();
        let mut c = sample_competition();
        let comp_id = c.id;
        c.add_participant(
            CompetitorEntry::new(Uuid::new_v4(), "A")
                .with_score("quality", 8.0)
                .with_score("accessibility", 8.0),
        );
        lb.add_competition(c);

        let scope = crate::federation_scope::FederationScope::new();
        let standings = lb.competition_standings_scoped(comp_id, &scope).unwrap();
        assert_eq!(standings.len(), 1);
    }

    #[test]
    fn competition_standings_scoped_filters_participants() {
        let mut lb = ConsortiaLeaderboard::new();
        let cid_visible = Uuid::new_v4();
        let cid_hidden = Uuid::new_v4();

        let mut c = sample_competition();
        let comp_id = c.id;
        c.add_participant(
            CompetitorEntry::new(cid_visible, "Visible")
                .with_score("quality", 8.0)
                .with_score("accessibility", 8.0),
        );
        c.add_participant(
            CompetitorEntry::new(cid_hidden, "Hidden")
                .with_score("quality", 10.0)
                .with_score("accessibility", 10.0),
        );
        lb.add_competition(c);

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid_visible.to_string(),
        ]);
        let standings = lb.competition_standings_scoped(comp_id, &scope).unwrap();
        assert_eq!(standings.len(), 1);
        assert_eq!(standings[0].0, cid_visible);
    }

    #[test]
    fn competition_standings_scoped_not_found() {
        let lb = ConsortiaLeaderboard::new();
        let scope = crate::federation_scope::FederationScope::new();
        let result = lb.competition_standings_scoped(Uuid::new_v4(), &scope);
        assert!(result.is_err());
    }

    #[test]
    fn count_scoped_unrestricted() {
        let mut lb = ConsortiaLeaderboard::new();
        lb.add_competition(sample_competition());
        lb.add_sponsored_challenge(SponsoredChallenge::new(
            Uuid::new_v4(),
            sample_sponsor(),
            1000,
        ));
        lb.add_innovation_quest(sample_innovation_quest());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(lb.count_scoped(&scope), 3);
    }

    #[test]
    fn count_scoped_filters() {
        let mut lb = ConsortiaLeaderboard::new();
        let cid = Uuid::new_v4();

        // Competition with visible participant
        let mut comp = sample_competition();
        comp.add_participant(CompetitorEntry::new(cid, "Guild"));
        lb.add_competition(comp);

        // Competition with hidden participant
        let mut comp2 = sample_competition();
        comp2.add_participant(CompetitorEntry::new(Uuid::new_v4(), "Other"));
        lb.add_competition(comp2);

        // Sponsored challenge (always included)
        lb.add_sponsored_challenge(SponsoredChallenge::new(
            Uuid::new_v4(),
            sample_sponsor(),
            1000,
        ));

        // Innovation with visible submission
        let mut q = sample_innovation_quest();
        q.submit(InnovationSubmission::new(cid, "idea://x"));
        lb.add_innovation_quest(q);

        // Innovation with hidden submission
        let mut q2 = sample_innovation_quest();
        q2.submit(InnovationSubmission::new(Uuid::new_v4(), "idea://y"));
        lb.add_innovation_quest(q2);

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid.to_string(),
        ]);
        // 1 competition + 1 sponsored + 1 innovation = 3
        assert_eq!(lb.count_scoped(&scope), 3);
    }
}
