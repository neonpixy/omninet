//! Engine -- the Quest runtime that ties everything together.
//!
//! `QuestEngine` is the central coordinator. It owns all subsystems and provides
//! a unified API for XP awards, achievement evaluation, mission lifecycle, challenges,
//! cooperative activities, and reward tracking.
//!
//! # Example
//!
//! ```
//! use quest::engine::QuestEngine;
//! use quest::QuestConfig;
//!
//! let mut engine = QuestEngine::with_defaults();
//! engine.award_xp("alice", 100, "onboarding");
//! let status = engine.status("alice");
//! assert!(status.total_xp >= 100);
//! ```

use std::collections::HashMap;

use uuid::Uuid;

use crate::achievement::{
    Achievement, AchievementContext, AchievementCriteria, AchievementRegistry, CriteriaResult,
};
use crate::challenge::ChallengeBoard;
use crate::config::QuestConfig;
use crate::consortia::ConsortiaLeaderboard;
use crate::cooperative::CooperativeBoard;
use crate::error::QuestError;
use crate::mission::{Mission, MissionEngine, MissionProgress};
use crate::progression::{Difficulty, FlowCalibration, Progression, XpAmount};
use crate::reward::{Badge, Reward, RewardLedger, RewardSource, RewardType};

// ---------------------------------------------------------------------------
// QuestEngine
// ---------------------------------------------------------------------------

/// The central coordinator that ties every Quest subsystem together.
///
/// Owns configuration, achievement registry, mission engine, challenge board,
/// consortia leaderboard, cooperative board, per-actor progressions, and the
/// global reward ledger.
#[derive(Debug)]
pub struct QuestEngine {
    /// Global configuration.
    pub config: QuestConfig,
    /// Achievement definitions and criteria.
    pub achievements: AchievementRegistry,
    /// Mission definitions and per-actor progress.
    pub missions: MissionEngine,
    /// Challenge lifecycle.
    pub challenges: ChallengeBoard,
    /// Consortia competitive activities.
    pub consortia: ConsortiaLeaderboard,
    /// Cooperative group activities.
    pub cooperative: CooperativeBoard,
    /// Per-actor progression state (XP, level, streaks, skill trees).
    pub progressions: HashMap<String, Progression>,
    /// Per-actor flow calibrations.
    pub calibrations: HashMap<String, FlowCalibration>,
    /// Global reward ledger (all actors).
    pub rewards: RewardLedger,
}

impl QuestEngine {
    /// Create a new engine with the given configuration and empty subsystems.
    pub fn new(config: QuestConfig) -> Self {
        Self {
            config,
            achievements: AchievementRegistry::new(),
            missions: MissionEngine::new(),
            challenges: ChallengeBoard::new(),
            consortia: ConsortiaLeaderboard::new(),
            cooperative: CooperativeBoard::new(),
            progressions: HashMap::new(),
            calibrations: HashMap::new(),
            rewards: RewardLedger::new(),
        }
    }

    /// Create a new engine with the standard (default) configuration.
    pub fn with_defaults() -> Self {
        Self::new(QuestConfig::default())
    }

    // --- Progression ---

    /// Get a reference to an actor's progression, if it exists.
    pub fn progression_for(&self, actor: &str) -> Option<&Progression> {
        self.progressions.get(actor)
    }

    /// Get a mutable reference to an actor's progression, creating one if missing.
    ///
    /// New progressions are initialized with the engine's current config (for
    /// streak forgiveness and other per-config values).
    pub fn progression_for_mut(&mut self, actor: &str) -> &mut Progression {
        let config = &self.config;
        self.progressions
            .entry(actor.to_owned())
            .or_insert_with(|| Progression::with_config(actor, config))
    }

    /// Award XP to an actor. Creates the progression if it does not exist.
    ///
    /// Returns the actor's new level after the award.
    pub fn award_xp(&mut self, actor: &str, amount: XpAmount, source: &str) -> u32 {
        let config = &self.config;
        let progression = self
            .progressions
            .entry(actor.to_owned())
            .or_insert_with(|| Progression::with_config(actor, config));
        let old_level = progression.level;
        let new_level = progression.award_xp(amount, config);

        if new_level > old_level {
            log::info!(
                "Quest: {actor} leveled up from {old_level} to {new_level} (+{amount} XP from {source})"
            );
        }

        new_level
    }

    // --- Achievements ---

    /// Register a criteria implementation for achievement evaluation.
    pub fn register_criteria(&mut self, criteria: Box<dyn AchievementCriteria>) {
        self.achievements.register_criteria(criteria);
    }

    /// Register an achievement definition.
    pub fn register_achievement(&mut self, achievement: Achievement) {
        self.achievements.define_achievement(achievement);
    }

    /// Evaluate all registered achievements against the given context.
    ///
    /// Returns the IDs of achievements that are newly `Achieved` in this evaluation.
    /// Does NOT automatically grant rewards -- the caller decides whether to do that.
    pub fn evaluate_achievements(
        &mut self,
        actor: &str,
        context: &AchievementContext,
    ) -> Vec<String> {
        let results = self.achievements.evaluate_all(context);
        let mut newly_achieved = Vec::new();

        for (achievement_id, result) in results {
            if result == CriteriaResult::Achieved {
                newly_achieved.push(achievement_id);
            }
        }

        // Record activity on the actor's streak if they achieved anything.
        if !newly_achieved.is_empty() {
            let config = &self.config;
            let progression = self
                .progressions
                .entry(actor.to_owned())
                .or_insert_with(|| Progression::with_config(actor, config));
            progression.streak.record_activity(chrono::Utc::now());
        }

        newly_achieved
    }

    // --- Missions ---

    /// Add a mission definition to the engine.
    pub fn add_mission(&mut self, mission: Mission) {
        self.missions.add_mission(mission);
    }

    /// Start a mission for an actor. Delegates to `MissionEngine::start_mission`.
    pub fn start_mission(
        &mut self,
        actor: &str,
        mission_id: &Uuid,
    ) -> Result<(), QuestError> {
        self.missions.start_mission(actor, *mission_id)?;
        Ok(())
    }

    /// Update progress on a specific objective within a mission.
    ///
    /// Returns the updated `MissionProgress`. If all objectives are met,
    /// the mission status transitions to `Completed`.
    pub fn complete_objective(
        &mut self,
        actor: &str,
        mission_id: &Uuid,
        objective_id: &str,
        increment: u64,
    ) -> Result<MissionProgress, QuestError> {
        self.missions
            .update_progress(actor, *mission_id, objective_id, increment)
    }

    /// Missions available to an actor.
    pub fn available_missions(&self, actor: &str) -> Vec<&Mission> {
        self.missions.available_for(actor)
    }

    /// Missions currently active for an actor.
    pub fn active_missions(&self, actor: &str) -> Vec<&MissionProgress> {
        self.missions.active_for(actor)
    }

    /// Missions completed by an actor.
    pub fn completed_missions(&self, actor: &str) -> Vec<&MissionProgress> {
        self.missions.completed_by(actor)
    }

    // --- Rewards ---

    /// Grant a reward to an actor. The reward is recorded in the global ledger.
    pub fn grant_reward(
        &mut self,
        actor: &str,
        reward_type: RewardType,
        source_id: &str,
        source_type: RewardSource,
    ) {
        let reward = Reward::new(reward_type, source_id, source_type, actor);
        self.rewards.grant(reward);
    }

    /// All rewards granted to a specific actor.
    pub fn rewards_for(&self, actor: &str) -> Vec<&Reward> {
        self.rewards
            .all()
            .iter()
            .filter(|r| r.recipient == actor)
            .collect()
    }

    /// Total Cool currency earned by a specific actor.
    pub fn total_cool_for(&self, actor: &str) -> u64 {
        self.rewards
            .all()
            .iter()
            .filter(|r| r.recipient == actor)
            .filter_map(|r| match &r.reward_type {
                RewardType::Cool(amount) => Some(*amount),
                _ => None,
            })
            .sum()
    }

    // --- Status ---

    /// A snapshot of an actor's current quest state.
    pub fn status(&self, actor: &str) -> QuestStatus {
        let progression = self.progressions.get(actor);

        let level = progression.map(|p| p.level).unwrap_or(1);
        let total_xp = progression.map(|p| p.total_xp).unwrap_or(0);
        let xp_to_next = progression
            .map(|p| p.xp_to_next_level(&self.config))
            .unwrap_or_else(|| {
                Progression::xp_for_level(2, &self.config)
            });
        let streak_days = progression.map(|p| p.streak.current).unwrap_or(0);

        let active_missions = self.missions.active_for(actor).len();
        let completed_missions = self.missions.completed_by(actor).len();

        let actor_rewards: Vec<&Reward> = self
            .rewards
            .all()
            .iter()
            .filter(|r| r.recipient == actor)
            .collect();

        let achievements_earned = actor_rewards
            .iter()
            .filter(|r| r.source_type == RewardSource::Achievement)
            .count();

        let total_cool_earned: u64 = actor_rewards
            .iter()
            .filter_map(|r| match &r.reward_type {
                RewardType::Cool(amount) => Some(*amount),
                _ => None,
            })
            .sum();

        let badges: Vec<Badge> = actor_rewards
            .iter()
            .filter_map(|r| match &r.reward_type {
                RewardType::Badge(badge) => Some(badge.clone()),
                _ => None,
            })
            .collect();

        let suggested_difficulty = self
            .calibrations
            .get(actor)
            .map(|c| c.suggested_difficulty)
            .unwrap_or(Difficulty::Normal);

        QuestStatus {
            actor: actor.to_owned(),
            level,
            total_xp,
            xp_to_next_level: xp_to_next,
            active_missions,
            completed_missions,
            achievements_earned,
            total_cool_earned,
            badges,
            streak_days,
            suggested_difficulty,
        }
    }

    // --- Summary ---

    /// Aggregate stats across all actors and subsystems.
    pub fn summary(&self) -> QuestSummary {
        let total_participants = self.progressions.len();

        let total_achievements_earned = self
            .rewards
            .all()
            .iter()
            .filter(|r| r.source_type == RewardSource::Achievement)
            .count();

        let total_missions_completed: usize = self
            .progressions
            .keys()
            .map(|actor| self.missions.completed_by(actor).len())
            .sum();

        let active_challenges = self.challenges.list_active().len();

        let (_, _, active_raids_count, _) = self.cooperative.count_all();

        let total_cool_distributed = self.rewards.total_cool_earned();

        QuestSummary {
            total_participants,
            total_achievements_earned,
            total_missions_completed,
            active_challenges,
            active_raids: active_raids_count,
            total_cool_distributed,
        }
    }
}

// ---------------------------------------------------------------------------
// QuestStatus
// ---------------------------------------------------------------------------

/// A snapshot of a single actor's quest state at a point in time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestStatus {
    /// The actor's public key or identifier.
    pub actor: String,
    /// Current level.
    pub level: u32,
    /// Total accumulated XP.
    pub total_xp: XpAmount,
    /// XP remaining to reach the next level.
    pub xp_to_next_level: XpAmount,
    /// Number of currently active missions.
    pub active_missions: usize,
    /// Number of completed missions.
    pub completed_missions: usize,
    /// Number of achievements earned (via reward source).
    pub achievements_earned: usize,
    /// Total Cool currency earned.
    pub total_cool_earned: u64,
    /// Badges earned.
    pub badges: Vec<Badge>,
    /// Current streak in days.
    pub streak_days: u32,
    /// Suggested difficulty from flow calibration.
    pub suggested_difficulty: Difficulty,
}

// ---------------------------------------------------------------------------
// QuestSummary
// ---------------------------------------------------------------------------

/// Aggregate stats across all actors and subsystems.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestSummary {
    /// Total number of actors with progression records.
    pub total_participants: usize,
    /// Total achievement rewards granted across all actors.
    pub total_achievements_earned: usize,
    /// Total missions completed across all actors.
    pub total_missions_completed: usize,
    /// Number of currently active challenges.
    pub active_challenges: usize,
    /// Number of currently active cooperative raids.
    pub active_raids: usize,
    /// Total Cool currency distributed via rewards.
    pub total_cool_distributed: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::{
        Achievement, AchievementCategory, AchievementContext, AchievementTier, CounterCriteria,
        FlagCriteria,
    };
    use crate::challenge::{
        Challenge, ChallengeCriteria, ChallengeParticipant, ChallengeStatus, CriteriaScope,
    };
    use crate::mission::{Mission, MissionStatus, Objective};
    use crate::reward::BadgeTier;

    fn test_config() -> QuestConfig {
        QuestConfig::default().with_consent_required(false)
    }

    // --- Construction ---

    #[test]
    fn new_with_config() {
        let config = QuestConfig::casual();
        let engine = QuestEngine::new(config.clone());
        assert_eq!(engine.config, config);
        assert!(engine.progressions.is_empty());
        assert_eq!(engine.rewards.count(), 0);
    }

    #[test]
    fn with_defaults() {
        let engine = QuestEngine::with_defaults();
        assert_eq!(engine.config, QuestConfig::default());
    }

    // --- Progression / XP ---

    #[test]
    fn award_xp_creates_progression() {
        let mut engine = QuestEngine::new(test_config());
        let level = engine.award_xp("alice", 50, "test");
        assert_eq!(level, 1);
        assert!(engine.progression_for("alice").is_some());
        assert_eq!(engine.progression_for("alice").unwrap().total_xp, 50);
    }

    #[test]
    fn award_xp_levels_up() {
        let mut engine = QuestEngine::new(test_config());
        let level = engine.award_xp("alice", 100, "test");
        assert_eq!(level, 2);
    }

    #[test]
    fn award_xp_multiple_actors() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 200, "test");
        engine.award_xp("bob", 50, "test");
        assert_eq!(engine.progressions.len(), 2);
        assert_eq!(engine.progression_for("alice").unwrap().total_xp, 200);
        assert_eq!(engine.progression_for("bob").unwrap().total_xp, 50);
    }

    #[test]
    fn award_xp_accumulates() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 50, "a");
        engine.award_xp("alice", 50, "b");
        assert_eq!(engine.progression_for("alice").unwrap().total_xp, 100);
    }

    #[test]
    fn progression_for_mut_creates_on_demand() {
        let mut engine = QuestEngine::new(test_config());
        let p = engine.progression_for_mut("alice");
        assert_eq!(p.actor, "alice");
        assert_eq!(p.level, 1);
    }

    #[test]
    fn progression_for_returns_none_when_missing() {
        let engine = QuestEngine::new(test_config());
        assert!(engine.progression_for("nobody").is_none());
    }

    // --- Achievements ---

    #[test]
    fn register_and_evaluate_achievement() {
        let mut engine = QuestEngine::new(test_config());

        // Register a counter criteria: need 10 posts
        engine.register_criteria(Box::new(CounterCriteria::new("posts-10", "posts_created", 10)));

        // Define the achievement
        engine.register_achievement(Achievement::new(
            "first-posts",
            "First 10 Posts",
            "Create your first 10 posts",
            AchievementCategory::Creation,
            "posts-10",
            AchievementTier::Bronze,
        ));

        // Context with 5 posts -- not enough
        let mut ctx = AchievementContext::new("alice");
        ctx.counters.insert("posts_created".to_owned(), 5);
        let achieved = engine.evaluate_achievements("alice", &ctx);
        assert!(achieved.is_empty());

        // Context with 10 posts -- achieved
        ctx.counters.insert("posts_created".to_owned(), 10);
        let achieved = engine.evaluate_achievements("alice", &ctx);
        assert_eq!(achieved.len(), 1);
        assert_eq!(achieved[0], "first-posts");
    }

    #[test]
    fn achievement_records_streak_activity() {
        let mut engine = QuestEngine::new(test_config());
        engine.register_criteria(Box::new(FlagCriteria::new("flag-true", "onboarded", true)));
        engine.register_achievement(Achievement::new(
            "onboarded",
            "Onboarded",
            "Complete onboarding",
            AchievementCategory::Sovereignty,
            "flag-true",
            AchievementTier::Bronze,
        ));

        let mut ctx = AchievementContext::new("alice");
        ctx.flags.insert("onboarded".to_owned(), true);
        let achieved = engine.evaluate_achievements("alice", &ctx);
        assert!(!achieved.is_empty());

        // Streak should be 1 now
        let p = engine.progression_for("alice").unwrap();
        assert_eq!(p.streak.current, 1);
    }

    #[test]
    fn no_achievement_no_streak() {
        let mut engine = QuestEngine::new(test_config());
        engine.register_criteria(Box::new(CounterCriteria::new("counter-5", "c", 5)));
        engine.register_achievement(Achievement::new(
            "a",
            "A",
            "desc",
            AchievementCategory::Creation,
            "counter-5",
            AchievementTier::Bronze,
        ));

        let ctx = AchievementContext::new("alice"); // no counters
        let achieved = engine.evaluate_achievements("alice", &ctx);
        assert!(achieved.is_empty());
        // No progression created since nothing was achieved
        assert!(engine.progression_for("alice").is_none());
    }

    // --- Missions ---

    fn sample_mission() -> Mission {
        Mission::new("Learn Basics", "Create your first document")
            .with_objective(Objective::new("open-app", "Open the app", 1, "app_opened"))
            .with_objective(Objective::new("create-doc", "Create a document", 1, "docs_created"))
            .with_xp_reward(50)
    }

    #[test]
    fn add_and_start_mission() {
        let mut engine = QuestEngine::new(test_config());
        let mission = sample_mission();
        let mid = mission.id;
        engine.add_mission(mission);
        engine.start_mission("alice", &mid).unwrap();
        assert_eq!(engine.active_missions("alice").len(), 1);
    }

    #[test]
    fn start_mission_not_found() {
        let mut engine = QuestEngine::new(test_config());
        let result = engine.start_mission("alice", &Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn complete_objective_updates_progress() {
        let mut engine = QuestEngine::new(test_config());
        let mission = sample_mission();
        let mid = mission.id;
        engine.add_mission(mission);
        engine.start_mission("alice", &mid).unwrap();

        let progress = engine
            .complete_objective("alice", &mid, "open-app", 1)
            .unwrap();
        assert_eq!(progress.status, MissionStatus::Active);

        let progress = engine
            .complete_objective("alice", &mid, "create-doc", 1)
            .unwrap();
        assert_eq!(progress.status, MissionStatus::Completed);
    }

    #[test]
    fn available_missions() {
        let mut engine = QuestEngine::new(test_config());
        let mission = sample_mission();
        engine.add_mission(mission);
        assert_eq!(engine.available_missions("alice").len(), 1);
    }

    #[test]
    fn completed_missions_after_finish() {
        let mut engine = QuestEngine::new(test_config());
        let mission = sample_mission();
        let mid = mission.id;
        engine.add_mission(mission);
        engine.start_mission("alice", &mid).unwrap();
        engine.complete_objective("alice", &mid, "open-app", 1).unwrap();
        engine.complete_objective("alice", &mid, "create-doc", 1).unwrap();
        assert_eq!(engine.completed_missions("alice").len(), 1);
        assert_eq!(engine.active_missions("alice").len(), 0);
    }

    // --- Rewards ---

    #[test]
    fn grant_reward() {
        let mut engine = QuestEngine::new(test_config());
        engine.grant_reward("alice", RewardType::Cool(100), "mission-1", RewardSource::Mission);
        assert_eq!(engine.rewards.count(), 1);
        assert_eq!(engine.rewards_for("alice").len(), 1);
    }

    #[test]
    fn rewards_for_filters_by_actor() {
        let mut engine = QuestEngine::new(test_config());
        engine.grant_reward("alice", RewardType::Cool(100), "m", RewardSource::Mission);
        engine.grant_reward("bob", RewardType::Cool(50), "m", RewardSource::Mission);
        assert_eq!(engine.rewards_for("alice").len(), 1);
        assert_eq!(engine.rewards_for("bob").len(), 1);
        assert_eq!(engine.rewards_for("nobody").len(), 0);
    }

    #[test]
    fn total_cool_for_actor() {
        let mut engine = QuestEngine::new(test_config());
        engine.grant_reward("alice", RewardType::Cool(100), "m1", RewardSource::Mission);
        engine.grant_reward("alice", RewardType::Cool(50), "m2", RewardSource::Challenge);
        engine.grant_reward(
            "alice",
            RewardType::Badge(Badge::new("x", "X", "desc", "icon", BadgeTier::Bronze)),
            "a1",
            RewardSource::Achievement,
        );
        assert_eq!(engine.total_cool_for("alice"), 150);
    }

    #[test]
    fn total_cool_zero_for_unknown() {
        let engine = QuestEngine::new(test_config());
        assert_eq!(engine.total_cool_for("nobody"), 0);
    }

    // --- Status ---

    #[test]
    fn status_empty_actor() {
        let engine = QuestEngine::new(test_config());
        let status = engine.status("nobody");
        assert_eq!(status.actor, "nobody");
        assert_eq!(status.level, 1);
        assert_eq!(status.total_xp, 0);
        assert_eq!(status.active_missions, 0);
        assert_eq!(status.completed_missions, 0);
        assert_eq!(status.achievements_earned, 0);
        assert_eq!(status.total_cool_earned, 0);
        assert!(status.badges.is_empty());
        assert_eq!(status.streak_days, 0);
        assert_eq!(status.suggested_difficulty, Difficulty::Normal);
    }

    #[test]
    fn status_after_activity() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 100, "test");
        engine.grant_reward(
            "alice",
            RewardType::Cool(50),
            "src",
            RewardSource::Achievement,
        );
        engine.grant_reward(
            "alice",
            RewardType::Badge(Badge::new("b1", "Badge", "desc", "icon", BadgeTier::Silver)),
            "src",
            RewardSource::Achievement,
        );

        let status = engine.status("alice");
        assert_eq!(status.level, 2);
        assert_eq!(status.total_xp, 100);
        assert_eq!(status.total_cool_earned, 50);
        assert_eq!(status.achievements_earned, 2); // Cool + Badge from Achievement source
        assert_eq!(status.badges.len(), 1);
    }

    #[test]
    fn status_xp_to_next_level() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 50, "test");
        let status = engine.status("alice");
        // Level 1, need 100 for level 2, earned 50, so 50 remaining
        assert_eq!(status.xp_to_next_level, 50);
    }

    #[test]
    fn status_suggested_difficulty_from_calibration() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 1, "test"); // create progression
        let mut cal = FlowCalibration::new("alice");
        cal.suggested_difficulty = Difficulty::Heroic;
        engine.calibrations.insert("alice".to_owned(), cal);

        let status = engine.status("alice");
        assert_eq!(status.suggested_difficulty, Difficulty::Heroic);
    }

    #[test]
    fn status_serde_round_trip() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 100, "test");
        let status = engine.status("alice");
        let json = serde_json::to_string(&status).unwrap();
        let restored: QuestStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.actor, "alice");
        assert_eq!(restored.level, 2);
    }

    // --- Summary ---

    #[test]
    fn summary_empty() {
        let engine = QuestEngine::new(test_config());
        let summary = engine.summary();
        assert_eq!(summary.total_participants, 0);
        assert_eq!(summary.total_achievements_earned, 0);
        assert_eq!(summary.total_missions_completed, 0);
        assert_eq!(summary.active_challenges, 0);
        assert_eq!(summary.active_raids, 0);
        assert_eq!(summary.total_cool_distributed, 0);
    }

    #[test]
    fn summary_with_activity() {
        let mut engine = QuestEngine::new(test_config());

        // Add some participants
        engine.award_xp("alice", 100, "test");
        engine.award_xp("bob", 50, "test");

        // Add a mission and complete it
        let mission = sample_mission();
        let mid = mission.id;
        engine.add_mission(mission);
        engine.start_mission("alice", &mid).unwrap();
        engine.complete_objective("alice", &mid, "open-app", 1).unwrap();
        engine.complete_objective("alice", &mid, "create-doc", 1).unwrap();

        // Grant some rewards
        engine.grant_reward(
            "alice",
            RewardType::Cool(100),
            "a1",
            RewardSource::Achievement,
        );

        let summary = engine.summary();
        assert_eq!(summary.total_participants, 2);
        assert_eq!(summary.total_achievements_earned, 1);
        assert_eq!(summary.total_missions_completed, 1);
        assert_eq!(summary.total_cool_distributed, 100);
    }

    #[test]
    fn summary_active_challenges() {
        let mut engine = QuestEngine::new(test_config());
        let challenge = Challenge::new("Sprint", "Publish ideas")
            .with_criteria(ChallengeCriteria {
                metric: "ideas".into(),
                target: 10,
                scope: CriteriaScope::Collective,
            })
            .with_status(ChallengeStatus::Active);
        engine.challenges.add_challenge(challenge);

        let summary = engine.summary();
        assert_eq!(summary.active_challenges, 1);
    }

    #[test]
    fn summary_serde_round_trip() {
        let engine = QuestEngine::new(test_config());
        let summary = engine.summary();
        let json = serde_json::to_string(&summary).unwrap();
        let restored: QuestSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.total_participants, 0);
    }

    // --- Integration scenarios ---

    #[test]
    fn full_lifecycle() {
        let mut engine = QuestEngine::new(test_config());

        // Register criteria and achievement
        engine.register_criteria(Box::new(CounterCriteria::new("docs-1", "docs_created", 1)));
        engine.register_achievement(Achievement::new(
            "first-doc",
            "First Document",
            "Create your first document",
            AchievementCategory::Creation,
            "docs-1",
            AchievementTier::Bronze,
        ));

        // Add mission
        let mission = sample_mission();
        let mid = mission.id;
        engine.add_mission(mission);

        // Start mission
        engine.start_mission("alice", &mid).unwrap();
        assert_eq!(engine.active_missions("alice").len(), 1);

        // Complete objectives
        engine.complete_objective("alice", &mid, "open-app", 1).unwrap();
        engine.complete_objective("alice", &mid, "create-doc", 1).unwrap();
        assert_eq!(engine.completed_missions("alice").len(), 1);

        // Award XP for completing the mission
        engine.award_xp("alice", 50, "mission-complete");

        // Evaluate achievements
        let mut ctx = AchievementContext::new("alice");
        ctx.counters.insert("docs_created".to_owned(), 1);
        let achieved = engine.evaluate_achievements("alice", &ctx);
        assert_eq!(achieved, vec!["first-doc"]);

        // Grant rewards
        engine.grant_reward(
            "alice",
            RewardType::Cool(25),
            "first-doc",
            RewardSource::Achievement,
        );

        // Check status
        let status = engine.status("alice");
        assert!(status.total_xp >= 50);
        assert_eq!(status.completed_missions, 1);
        assert_eq!(status.achievements_earned, 1);
        assert_eq!(status.total_cool_earned, 25);
    }

    #[test]
    fn multiple_actors_independent() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("alice", 1000, "test");
        engine.award_xp("bob", 10, "test");

        let alice_status = engine.status("alice");
        let bob_status = engine.status("bob");
        assert!(alice_status.level > bob_status.level);
        assert_eq!(engine.summary().total_participants, 2);
    }

    #[test]
    fn challenge_integration() {
        let mut engine = QuestEngine::new(test_config());
        let challenge = Challenge::new("Sprint", "Publish ideas")
            .with_criteria(ChallengeCriteria {
                metric: "ideas".into(),
                target: 10,
                scope: CriteriaScope::Collective,
            })
            .with_status(ChallengeStatus::Active);
        let cid = challenge.id;
        engine.challenges.add_challenge(challenge);

        let participant = ChallengeParticipant::Individual {
            pubkey: "cpub1alice".into(),
        };
        engine.challenges.join(cid, participant.clone()).unwrap();
        engine
            .challenges
            .update_progress(cid, &participant, 10)
            .unwrap();
        let status = engine.challenges.check_completion(cid).unwrap();
        assert_eq!(status, ChallengeStatus::Completed);
    }
}
