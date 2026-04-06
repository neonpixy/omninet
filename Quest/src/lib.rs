//! # Quest -- Gamification & Progression
//!
//! The guided path. Quest makes sovereignty engaging, not intimidating.
//! Missions, achievements, skill trees, and progression that reward meaningful
//! participation -- not engagement metrics, not addiction loops.
//!
//! ## Design Principles
//!
//! - **No dark patterns.** Streaks have forgiveness. Missing days doesn't punish.
//!   The Covenant governs.
//! - **Opt-in everything.** `QuestConfig::consent_required` is `true` by default.
//! - **Personal bests, not leaderboards.** You compete against yourself.
//! - **Trait-based extensibility.** `AchievementCriteria` is the plugin point.
//!   Communities can define their own.
//!
//! ## Architecture
//!
//! Quest is pure data structures and logic. Zero async, zero platform dependencies.
//! Integration with Yoke (history), Fortune (Cool rewards), Oracle (guidance),
//! and Kingdom (governance) happens via the Engine or the app layer.

pub mod error;
pub mod config;
pub mod reward;
pub mod achievement;
pub mod progression;
pub mod mission;
pub mod challenge;
pub mod consortia;
pub mod cooperative;
pub mod engine;
pub mod federation_scope;
pub mod observatory;

// Re-exports — foundation modules
pub use error::QuestError;
pub use config::QuestConfig;
pub use reward::{Badge, BadgeTier, Reward, RewardLedger, RewardSource, RewardType};
pub use achievement::{
    Achievement, AchievementCategory, AchievementContext, AchievementCriteria,
    AchievementProgress, AchievementRegistry, AchievementStatus, AchievementTier,
    CompositeCriteria, CounterCriteria, CriteriaResult, FlagCriteria, TimestampCriteria,
};
pub use progression::{
    Difficulty, DifficultySnapshot, FlowCalibration, PersonalBest, Progression, SkillNode,
    SkillTree, Streak, XpAmount,
};
pub use mission::{
    Mission, MissionCategory, MissionCreator, MissionEngine, MissionProgress, MissionScope,
    MissionStatus, Objective,
};
pub use challenge::{
    Challenge, ChallengeBoard, ChallengeCriteria, ChallengeEntry, ChallengeParticipant,
    ChallengeStatus, ChallengeType, CriteriaScope,
};
pub use consortia::ConsortiaLeaderboard;
pub use cooperative::CooperativeBoard;
pub use federation_scope::FederationScope;
pub use engine::{QuestEngine, QuestStatus, QuestSummary};
pub use observatory::{ObservatoryReport, QuestObservatory};
