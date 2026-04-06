//! Achievement system -- things you earn by doing.
//!
//! Achievements are defined by communities and the platform. The `AchievementCriteria`
//! trait is the plugin point: implement it to create custom criteria that evaluate
//! against an `AchievementContext`. Built-in criteria (`CounterCriteria`, `FlagCriteria`,
//! `TimestampCriteria`, `CompositeCriteria`) cover common patterns.
//!
//! The `AchievementRegistry` holds achievement definitions and criteria implementations,
//! providing evaluation and lookup.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::reward::RewardType;

/// Trait for evaluating achievement criteria.
///
/// Implementations must be `Send + Sync` for registry storage.
/// This trait is object-safe and can be used as `Box<dyn AchievementCriteria>`.
///
/// # Example
///
/// ```
/// use quest::achievement::{AchievementCriteria, AchievementContext, CriteriaResult};
///
/// struct PostCountCriteria { target: u64 }
///
/// impl AchievementCriteria for PostCountCriteria {
///     fn id(&self) -> &str { "posts-created" }
///     fn evaluate(&self, ctx: &AchievementContext) -> CriteriaResult {
///         let count = ctx.counters.get("posts_created").copied().unwrap_or(0);
///         if count >= self.target {
///             CriteriaResult::Achieved
///         } else {
///             CriteriaResult::InProgress { current: count, target: self.target }
///         }
///     }
///     fn description(&self) -> &str { "Create posts" }
/// }
/// ```
pub trait AchievementCriteria: Send + Sync {
    /// Unique identifier for this criteria implementation.
    fn id(&self) -> &str;

    /// Evaluate the criteria against the given context.
    fn evaluate(&self, context: &AchievementContext) -> CriteriaResult;

    /// Human-readable description of what this criteria checks.
    fn description(&self) -> &str;
}

/// Context provided to achievement criteria for evaluation.
///
/// Contains counters, timestamps, flags, and free-form metadata about the actor's
/// state. The caller is responsible for populating this from their data sources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AchievementContext {
    /// The actor's public key.
    pub actor: String,
    /// Numeric counters (e.g., "posts_created": 42).
    pub counters: HashMap<String, u64>,
    /// Timestamps of significant events (e.g., "first_post": ...).
    pub timestamps: HashMap<String, DateTime<Utc>>,
    /// Boolean flags (e.g., "has_backup": true).
    pub flags: HashMap<String, bool>,
    /// Free-form metadata.
    pub metadata: HashMap<String, String>,
}

impl AchievementContext {
    /// Create a new context for the given actor.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            ..Default::default()
        }
    }

    /// Set a counter value.
    pub fn with_counter(mut self, key: impl Into<String>, value: u64) -> Self {
        self.counters.insert(key.into(), value);
        self
    }

    /// Set a timestamp.
    pub fn with_timestamp(mut self, key: impl Into<String>, when: DateTime<Utc>) -> Self {
        self.timestamps.insert(key.into(), when);
        self
    }

    /// Set a flag.
    pub fn with_flag(mut self, key: impl Into<String>, value: bool) -> Self {
        self.flags.insert(key.into(), value);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Result of evaluating achievement criteria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CriteriaResult {
    /// The criteria has been fully met.
    Achieved,
    /// Progress has been made but criteria not yet met.
    InProgress {
        /// Current progress value.
        current: u64,
        /// Target value needed to achieve.
        target: u64,
    },
    /// No progress has been made.
    NotStarted,
}

impl CriteriaResult {
    /// Whether the criteria is fully achieved.
    pub fn is_achieved(&self) -> bool {
        matches!(self, Self::Achieved)
    }

    /// Progress as a fraction (0.0 to 1.0). Returns 1.0 for Achieved, 0.0 for NotStarted.
    pub fn progress_fraction(&self) -> f64 {
        match self {
            Self::Achieved => 1.0,
            Self::InProgress { current, target } => {
                if *target == 0 {
                    1.0
                } else {
                    (*current as f64 / *target as f64).min(1.0)
                }
            }
            Self::NotStarted => 0.0,
        }
    }
}

/// An achievement definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// What this achievement represents.
    pub description: String,
    /// Thematic category.
    pub category: AchievementCategory,
    /// ID of the registered `AchievementCriteria` that evaluates this.
    pub criteria_id: String,
    /// Rewards granted when achieved.
    pub rewards: Vec<RewardType>,
    /// Prestige tier.
    pub tier: AchievementTier,
    /// Whether this achievement can be earned multiple times.
    pub repeatable: bool,
    /// Whether this achievement is hidden until earned (surprise achievements).
    pub hidden: bool,
    /// Icon reference (asset ID or name).
    pub icon: String,
}

impl Achievement {
    /// Create a new achievement definition.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        category: AchievementCategory,
        criteria_id: impl Into<String>,
        tier: AchievementTier,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category,
            criteria_id: criteria_id.into(),
            rewards: Vec::new(),
            tier,
            repeatable: false,
            hidden: false,
            icon: String::new(),
        }
    }

    /// Add a reward to this achievement.
    pub fn with_reward(mut self, reward: RewardType) -> Self {
        self.rewards.push(reward);
        self
    }

    /// Mark this achievement as repeatable.
    pub fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    /// Mark this achievement as hidden (surprise).
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Set the icon reference.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

/// Thematic categories for achievements.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementCategory {
    /// Making things (designs, documents, presentations).
    Creation,
    /// Community participation and interaction.
    Social,
    /// Voting, proposals, delegation.
    Governance,
    /// Buying, selling, operating a business.
    Commerce,
    /// Discovering features, visiting areas.
    Exploration,
    /// Skill development and learning.
    Mastery,
    /// Helping others learn and grow.
    Mentorship,
    /// Identity setup, encryption, recovery.
    Sovereignty,
    /// Community-defined category.
    Custom(String),
}

/// Prestige tiers for achievements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementTier {
    /// Accessible -- everyone can get these with basic participation.
    Bronze,
    /// Takes sustained effort.
    Silver,
    /// Significant commitment required.
    Gold,
    /// Mastery level.
    Platinum,
    /// Exceptional and rare.
    Legendary,
}

/// Per-participant progress toward an achievement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementProgress {
    /// Which achievement this tracks.
    pub achievement_id: String,
    /// The actor's public key.
    pub actor: String,
    /// Current status.
    pub status: AchievementStatus,
    /// Current progress value.
    pub current_progress: u64,
    /// Target value needed.
    pub target: u64,
    /// When progress first started.
    pub started_at: Option<DateTime<Utc>>,
    /// When the achievement was completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// How many times completed (for repeatable achievements).
    pub times_completed: u32,
}

impl AchievementProgress {
    /// Create new progress tracking for an achievement.
    pub fn new(
        achievement_id: impl Into<String>,
        actor: impl Into<String>,
        target: u64,
    ) -> Self {
        Self {
            achievement_id: achievement_id.into(),
            actor: actor.into(),
            status: AchievementStatus::Locked,
            current_progress: 0,
            target,
            started_at: None,
            completed_at: None,
            times_completed: 0,
        }
    }

    /// Update progress from a criteria result.
    pub fn update(&mut self, result: &CriteriaResult) {
        match result {
            CriteriaResult::Achieved => {
                if self.status != AchievementStatus::Completed {
                    self.status = AchievementStatus::Completed;
                    self.current_progress = self.target;
                    self.completed_at = Some(Utc::now());
                    self.times_completed += 1;
                }
            }
            CriteriaResult::InProgress { current, target } => {
                if self.status == AchievementStatus::Locked {
                    self.status = AchievementStatus::InProgress;
                    self.started_at = Some(Utc::now());
                }
                self.current_progress = *current;
                self.target = *target;
            }
            CriteriaResult::NotStarted => {
                // Don't regress -- stay at current status
            }
        }
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress_fraction(&self) -> f64 {
        if self.target == 0 {
            return 1.0;
        }
        (self.current_progress as f64 / self.target as f64).min(1.0)
    }
}

/// Status of an achievement for a participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementStatus {
    /// Not yet started.
    Locked,
    /// Some progress made.
    InProgress,
    /// Fully completed.
    Completed,
}

/// Registry of achievement definitions and criteria implementations.
///
/// The registry is the central place to define achievements and the criteria
/// that evaluate them. Communities register their own criteria and achievements.
pub struct AchievementRegistry {
    criteria: HashMap<String, Box<dyn AchievementCriteria>>,
    achievements: Vec<Achievement>,
}

impl std::fmt::Debug for AchievementRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AchievementRegistry")
            .field("criteria_count", &self.criteria.len())
            .field("achievement_count", &self.achievements.len())
            .finish()
    }
}

impl Default for AchievementRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AchievementRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            criteria: HashMap::new(),
            achievements: Vec::new(),
        }
    }

    /// Register a criteria implementation. Replaces any existing criteria with the same ID.
    pub fn register_criteria(&mut self, criteria: Box<dyn AchievementCriteria>) {
        let id = criteria.id().to_owned();
        self.criteria.insert(id, criteria);
    }

    /// Define an achievement. The achievement's `criteria_id` must reference
    /// a registered criteria (checked at evaluation time, not at definition time).
    pub fn define_achievement(&mut self, achievement: Achievement) {
        self.achievements.push(achievement);
    }

    /// Evaluate a single achievement against the given context.
    ///
    /// Returns `None` if the achievement or its criteria is not found.
    pub fn evaluate(
        &self,
        achievement_id: &str,
        context: &AchievementContext,
    ) -> Option<CriteriaResult> {
        let achievement = self.achievements.iter().find(|a| a.id == achievement_id)?;
        let criteria = self.criteria.get(&achievement.criteria_id)?;
        Some(criteria.evaluate(context))
    }

    /// Evaluate all achievements against the given context.
    ///
    /// Returns a vec of (achievement_id, result) pairs. Skips achievements
    /// whose criteria is not registered.
    pub fn evaluate_all(&self, context: &AchievementContext) -> Vec<(String, CriteriaResult)> {
        self.achievements
            .iter()
            .filter_map(|a| {
                let criteria = self.criteria.get(&a.criteria_id)?;
                Some((a.id.clone(), criteria.evaluate(context)))
            })
            .collect()
    }

    /// Look up an achievement by ID.
    pub fn get_achievement(&self, id: &str) -> Option<&Achievement> {
        self.achievements.iter().find(|a| a.id == id)
    }

    /// All defined achievements.
    pub fn list_achievements(&self) -> &[Achievement] {
        &self.achievements
    }

    /// Achievements in a specific category.
    pub fn by_category(&self, category: &AchievementCategory) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| &a.category == category)
            .collect()
    }

    /// Achievements at a specific tier.
    pub fn by_tier(&self, tier: AchievementTier) -> Vec<&Achievement> {
        self.achievements
            .iter()
            .filter(|a| a.tier == tier)
            .collect()
    }

    /// Total number of defined achievements.
    pub fn count(&self) -> usize {
        self.achievements.len()
    }

    /// Number of registered criteria implementations.
    pub fn criteria_count(&self) -> usize {
        self.criteria.len()
    }
}

// ---------------------------------------------------------------------------
// Built-in criteria implementations
// ---------------------------------------------------------------------------

/// Criteria that checks a counter against a target value.
///
/// Achieved when `context.counters[counter_key] >= target`.
pub struct CounterCriteria {
    id: String,
    counter_key: String,
    target: u64,
}

impl CounterCriteria {
    /// Create a new counter criteria.
    pub fn new(id: impl Into<String>, counter_key: impl Into<String>, target: u64) -> Self {
        Self {
            id: id.into(),
            counter_key: counter_key.into(),
            target,
        }
    }
}

impl AchievementCriteria for CounterCriteria {
    fn id(&self) -> &str {
        &self.id
    }

    fn evaluate(&self, context: &AchievementContext) -> CriteriaResult {
        let current = context.counters.get(&self.counter_key).copied().unwrap_or(0);
        if current >= self.target {
            CriteriaResult::Achieved
        } else if current > 0 {
            CriteriaResult::InProgress {
                current,
                target: self.target,
            }
        } else {
            CriteriaResult::NotStarted
        }
    }

    fn description(&self) -> &str {
        &self.counter_key
    }
}

/// Criteria that checks a boolean flag.
///
/// Achieved when `context.flags[flag_key] == expected`.
pub struct FlagCriteria {
    id: String,
    flag_key: String,
    expected: bool,
}

impl FlagCriteria {
    /// Create a new flag criteria.
    pub fn new(id: impl Into<String>, flag_key: impl Into<String>, expected: bool) -> Self {
        Self {
            id: id.into(),
            flag_key: flag_key.into(),
            expected,
        }
    }
}

impl AchievementCriteria for FlagCriteria {
    fn id(&self) -> &str {
        &self.id
    }

    fn evaluate(&self, context: &AchievementContext) -> CriteriaResult {
        match context.flags.get(&self.flag_key) {
            Some(value) if *value == self.expected => CriteriaResult::Achieved,
            Some(_) => CriteriaResult::InProgress {
                current: 0,
                target: 1,
            },
            None => CriteriaResult::NotStarted,
        }
    }

    fn description(&self) -> &str {
        &self.flag_key
    }
}

/// Criteria that checks for the existence of a timestamp.
///
/// Achieved when `context.timestamps[timestamp_key]` exists.
pub struct TimestampCriteria {
    id: String,
    timestamp_key: String,
}

impl TimestampCriteria {
    /// Create a new timestamp criteria.
    pub fn new(id: impl Into<String>, timestamp_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            timestamp_key: timestamp_key.into(),
        }
    }
}

impl AchievementCriteria for TimestampCriteria {
    fn id(&self) -> &str {
        &self.id
    }

    fn evaluate(&self, context: &AchievementContext) -> CriteriaResult {
        if context.timestamps.contains_key(&self.timestamp_key) {
            CriteriaResult::Achieved
        } else {
            CriteriaResult::NotStarted
        }
    }

    fn description(&self) -> &str {
        &self.timestamp_key
    }
}

/// Criteria that combines multiple other criteria.
///
/// When `require_all` is `true`, all referenced criteria must be achieved.
/// When `false`, any one criteria being achieved is sufficient.
///
/// For `InProgress`, reports aggregate progress across all sub-criteria.
pub struct CompositeCriteria {
    id: String,
    criteria_ids: Vec<String>,
    require_all: bool,
}

impl CompositeCriteria {
    /// Create a composite criteria requiring all sub-criteria.
    pub fn all(id: impl Into<String>, criteria_ids: Vec<String>) -> Self {
        Self {
            id: id.into(),
            criteria_ids,
            require_all: true,
        }
    }

    /// Create a composite criteria requiring any one sub-criteria.
    pub fn any(id: impl Into<String>, criteria_ids: Vec<String>) -> Self {
        Self {
            id: id.into(),
            criteria_ids,
            require_all: false,
        }
    }
}

impl AchievementCriteria for CompositeCriteria {
    fn id(&self) -> &str {
        &self.id
    }

    fn evaluate(&self, _context: &AchievementContext) -> CriteriaResult {
        // Composite evaluation requires access to the registry, which this trait
        // doesn't provide. The registry's evaluate method handles composites
        // by evaluating sub-criteria individually. This standalone evaluate
        // returns NotStarted as a sentinel -- the registry must handle composition.
        //
        // This design keeps the trait object-safe and avoids circular references.
        // The AchievementRegistry.evaluate_all() checks for composite criteria
        // and resolves them by evaluating each sub-criteria.
        CriteriaResult::NotStarted
    }

    fn description(&self) -> &str {
        if self.require_all {
            "all required criteria"
        } else {
            "any required criteria"
        }
    }
}

impl CompositeCriteria {
    /// Evaluate this composite against already-computed sub-results.
    ///
    /// This is called by the registry after evaluating each sub-criteria individually.
    pub fn evaluate_with_results(&self, results: &[CriteriaResult]) -> CriteriaResult {
        if results.is_empty() {
            return CriteriaResult::NotStarted;
        }

        let achieved_count = results.iter().filter(|r| r.is_achieved()).count();
        let total = results.len() as u64;

        if self.require_all {
            if achieved_count == results.len() {
                CriteriaResult::Achieved
            } else if achieved_count > 0 {
                CriteriaResult::InProgress {
                    current: achieved_count as u64,
                    target: total,
                }
            } else {
                // Check if any sub-criteria is in progress
                let any_in_progress = results
                    .iter()
                    .any(|r| matches!(r, CriteriaResult::InProgress { .. }));
                if any_in_progress {
                    CriteriaResult::InProgress {
                        current: 0,
                        target: total,
                    }
                } else {
                    CriteriaResult::NotStarted
                }
            }
        } else {
            // Any
            if achieved_count > 0 {
                CriteriaResult::Achieved
            } else {
                let any_in_progress = results
                    .iter()
                    .any(|r| matches!(r, CriteriaResult::InProgress { .. }));
                if any_in_progress {
                    CriteriaResult::InProgress {
                        current: 0,
                        target: 1,
                    }
                } else {
                    CriteriaResult::NotStarted
                }
            }
        }
    }

    /// The sub-criteria IDs this composite references.
    pub fn criteria_ids(&self) -> &[String] {
        &self.criteria_ids
    }

    /// Whether all sub-criteria are required (true) or any (false).
    pub fn requires_all(&self) -> bool {
        self.require_all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- AchievementContext tests ---

    #[test]
    fn context_builder() {
        let ctx = AchievementContext::new("cpub1alice")
            .with_counter("posts", 42)
            .with_flag("has_backup", true)
            .with_timestamp("first_post", Utc::now())
            .with_metadata("theme", "dark");

        assert_eq!(ctx.actor, "cpub1alice");
        assert_eq!(ctx.counters["posts"], 42);
        assert!(ctx.flags["has_backup"]);
        assert!(ctx.timestamps.contains_key("first_post"));
        assert_eq!(ctx.metadata["theme"], "dark");
    }

    #[test]
    fn context_default() {
        let ctx = AchievementContext::default();
        assert!(ctx.actor.is_empty());
        assert!(ctx.counters.is_empty());
        assert!(ctx.flags.is_empty());
        assert!(ctx.timestamps.is_empty());
        assert!(ctx.metadata.is_empty());
    }

    #[test]
    fn context_serde_round_trip() {
        let ctx = AchievementContext::new("cpub1bob")
            .with_counter("designs", 10)
            .with_flag("verified", true);
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: AchievementContext = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.actor, "cpub1bob");
        assert_eq!(restored.counters["designs"], 10);
        assert!(restored.flags["verified"]);
    }

    // --- CriteriaResult tests ---

    #[test]
    fn criteria_result_is_achieved() {
        assert!(CriteriaResult::Achieved.is_achieved());
        assert!(
            !CriteriaResult::InProgress {
                current: 5,
                target: 10
            }
            .is_achieved()
        );
        assert!(!CriteriaResult::NotStarted.is_achieved());
    }

    #[test]
    fn criteria_result_progress_fraction() {
        assert_eq!(CriteriaResult::Achieved.progress_fraction(), 1.0);
        assert_eq!(CriteriaResult::NotStarted.progress_fraction(), 0.0);
        assert_eq!(
            CriteriaResult::InProgress {
                current: 5,
                target: 10
            }
            .progress_fraction(),
            0.5
        );
        assert_eq!(
            CriteriaResult::InProgress {
                current: 15,
                target: 10
            }
            .progress_fraction(),
            1.0
        );
        // Zero target edge case
        assert_eq!(
            CriteriaResult::InProgress {
                current: 0,
                target: 0
            }
            .progress_fraction(),
            1.0
        );
    }

    #[test]
    fn criteria_result_serde_round_trip() {
        let results = [
            CriteriaResult::Achieved,
            CriteriaResult::InProgress {
                current: 3,
                target: 10,
            },
            CriteriaResult::NotStarted,
        ];
        for result in &results {
            let json = serde_json::to_string(result).unwrap();
            let restored: CriteriaResult = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, result);
        }
    }

    // --- Achievement tests ---

    #[test]
    fn achievement_creation() {
        let ach = Achievement::new(
            "first-post",
            "First Post",
            "Create your first post",
            AchievementCategory::Creation,
            "counter-posts-1",
            AchievementTier::Bronze,
        );
        assert_eq!(ach.id, "first-post");
        assert!(!ach.repeatable);
        assert!(!ach.hidden);
        assert!(ach.rewards.is_empty());
    }

    #[test]
    fn achievement_builder() {
        let ach = Achievement::new(
            "prolific",
            "Prolific Creator",
            "Create 100 posts",
            AchievementCategory::Creation,
            "counter-posts-100",
            AchievementTier::Gold,
        )
        .with_reward(RewardType::Cool(500))
        .with_reward(RewardType::Title("Prolific".into()))
        .with_icon("icon-prolific")
        .repeatable()
        .hidden();

        assert_eq!(ach.rewards.len(), 2);
        assert!(ach.repeatable);
        assert!(ach.hidden);
        assert_eq!(ach.icon, "icon-prolific");
    }

    #[test]
    fn achievement_serde_round_trip() {
        let ach = Achievement::new(
            "test",
            "Test",
            "A test",
            AchievementCategory::Exploration,
            "criteria-1",
            AchievementTier::Silver,
        )
        .with_reward(RewardType::Cool(100));
        let json = serde_json::to_string(&ach).unwrap();
        let restored: Achievement = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "test");
        assert_eq!(restored.category, AchievementCategory::Exploration);
    }

    // --- AchievementCategory tests ---

    #[test]
    fn category_custom() {
        let cat = AchievementCategory::Custom("gardening".into());
        let json = serde_json::to_string(&cat).unwrap();
        let restored: AchievementCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, cat);
    }

    #[test]
    fn category_all_variants_serde() {
        let categories = [
            AchievementCategory::Creation,
            AchievementCategory::Social,
            AchievementCategory::Governance,
            AchievementCategory::Commerce,
            AchievementCategory::Exploration,
            AchievementCategory::Mastery,
            AchievementCategory::Mentorship,
            AchievementCategory::Sovereignty,
            AchievementCategory::Custom("test".into()),
        ];
        for cat in &categories {
            let json = serde_json::to_string(cat).unwrap();
            let restored: AchievementCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, cat);
        }
    }

    // --- AchievementProgress tests ---

    #[test]
    fn progress_creation() {
        let progress = AchievementProgress::new("ach-1", "cpub1alice", 100);
        assert_eq!(progress.status, AchievementStatus::Locked);
        assert_eq!(progress.current_progress, 0);
        assert_eq!(progress.target, 100);
        assert_eq!(progress.times_completed, 0);
        assert!(progress.started_at.is_none());
        assert!(progress.completed_at.is_none());
    }

    #[test]
    fn progress_update_in_progress() {
        let mut progress = AchievementProgress::new("ach-1", "cpub1alice", 100);
        progress.update(&CriteriaResult::InProgress {
            current: 30,
            target: 100,
        });
        assert_eq!(progress.status, AchievementStatus::InProgress);
        assert_eq!(progress.current_progress, 30);
        assert!(progress.started_at.is_some());
        assert!(progress.completed_at.is_none());
    }

    #[test]
    fn progress_update_achieved() {
        let mut progress = AchievementProgress::new("ach-1", "cpub1alice", 100);
        progress.update(&CriteriaResult::Achieved);
        assert_eq!(progress.status, AchievementStatus::Completed);
        assert_eq!(progress.times_completed, 1);
        assert!(progress.completed_at.is_some());
    }

    #[test]
    fn progress_no_regression_on_not_started() {
        let mut progress = AchievementProgress::new("ach-1", "cpub1alice", 100);
        progress.update(&CriteriaResult::InProgress {
            current: 50,
            target: 100,
        });
        progress.update(&CriteriaResult::NotStarted);
        // Status should not regress
        assert_eq!(progress.status, AchievementStatus::InProgress);
        assert_eq!(progress.current_progress, 50);
    }

    #[test]
    fn progress_fraction() {
        let mut progress = AchievementProgress::new("ach-1", "cpub1alice", 100);
        assert_eq!(progress.progress_fraction(), 0.0);
        progress.current_progress = 50;
        assert_eq!(progress.progress_fraction(), 0.5);
        progress.current_progress = 100;
        assert_eq!(progress.progress_fraction(), 1.0);
        progress.current_progress = 150;
        assert_eq!(progress.progress_fraction(), 1.0); // capped at 1.0
    }

    #[test]
    fn progress_fraction_zero_target() {
        let progress = AchievementProgress::new("ach-1", "cpub1alice", 0);
        assert_eq!(progress.progress_fraction(), 1.0);
    }

    #[test]
    fn progress_serde_round_trip() {
        let mut progress = AchievementProgress::new("ach-1", "cpub1alice", 100);
        progress.update(&CriteriaResult::InProgress {
            current: 42,
            target: 100,
        });
        let json = serde_json::to_string(&progress).unwrap();
        let restored: AchievementProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.achievement_id, "ach-1");
        assert_eq!(restored.current_progress, 42);
        assert_eq!(restored.status, AchievementStatus::InProgress);
    }

    // --- CounterCriteria tests ---

    #[test]
    fn counter_criteria_not_started() {
        let criteria = CounterCriteria::new("c1", "posts", 10);
        let ctx = AchievementContext::new("alice");
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::NotStarted);
    }

    #[test]
    fn counter_criteria_in_progress() {
        let criteria = CounterCriteria::new("c1", "posts", 10);
        let ctx = AchievementContext::new("alice").with_counter("posts", 5);
        assert_eq!(
            criteria.evaluate(&ctx),
            CriteriaResult::InProgress {
                current: 5,
                target: 10
            }
        );
    }

    #[test]
    fn counter_criteria_achieved() {
        let criteria = CounterCriteria::new("c1", "posts", 10);
        let ctx = AchievementContext::new("alice").with_counter("posts", 10);
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::Achieved);
    }

    #[test]
    fn counter_criteria_exceeds_target() {
        let criteria = CounterCriteria::new("c1", "posts", 10);
        let ctx = AchievementContext::new("alice").with_counter("posts", 99);
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::Achieved);
    }

    // --- FlagCriteria tests ---

    #[test]
    fn flag_criteria_not_started() {
        let criteria = FlagCriteria::new("f1", "has_backup", true);
        let ctx = AchievementContext::new("alice");
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::NotStarted);
    }

    #[test]
    fn flag_criteria_achieved() {
        let criteria = FlagCriteria::new("f1", "has_backup", true);
        let ctx = AchievementContext::new("alice").with_flag("has_backup", true);
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::Achieved);
    }

    #[test]
    fn flag_criteria_wrong_value() {
        let criteria = FlagCriteria::new("f1", "has_backup", true);
        let ctx = AchievementContext::new("alice").with_flag("has_backup", false);
        assert_eq!(
            criteria.evaluate(&ctx),
            CriteriaResult::InProgress {
                current: 0,
                target: 1
            }
        );
    }

    #[test]
    fn flag_criteria_expects_false() {
        let criteria = FlagCriteria::new("f1", "has_ads", false);
        let ctx = AchievementContext::new("alice").with_flag("has_ads", false);
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::Achieved);
    }

    // --- TimestampCriteria tests ---

    #[test]
    fn timestamp_criteria_not_started() {
        let criteria = TimestampCriteria::new("t1", "first_post");
        let ctx = AchievementContext::new("alice");
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::NotStarted);
    }

    #[test]
    fn timestamp_criteria_achieved() {
        let criteria = TimestampCriteria::new("t1", "first_post");
        let ctx = AchievementContext::new("alice").with_timestamp("first_post", Utc::now());
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::Achieved);
    }

    // --- CompositeCriteria tests ---

    #[test]
    fn composite_all_achieved() {
        let comp = CompositeCriteria::all(
            "comp-all",
            vec!["c1".into(), "c2".into()],
        );
        let results = vec![CriteriaResult::Achieved, CriteriaResult::Achieved];
        assert_eq!(comp.evaluate_with_results(&results), CriteriaResult::Achieved);
    }

    #[test]
    fn composite_all_partial() {
        let comp = CompositeCriteria::all(
            "comp-all",
            vec!["c1".into(), "c2".into()],
        );
        let results = vec![
            CriteriaResult::Achieved,
            CriteriaResult::InProgress {
                current: 5,
                target: 10,
            },
        ];
        assert_eq!(
            comp.evaluate_with_results(&results),
            CriteriaResult::InProgress {
                current: 1,
                target: 2
            }
        );
    }

    #[test]
    fn composite_all_none() {
        let comp = CompositeCriteria::all(
            "comp-all",
            vec!["c1".into(), "c2".into()],
        );
        let results = vec![CriteriaResult::NotStarted, CriteriaResult::NotStarted];
        assert_eq!(
            comp.evaluate_with_results(&results),
            CriteriaResult::NotStarted
        );
    }

    #[test]
    fn composite_any_one_achieved() {
        let comp = CompositeCriteria::any(
            "comp-any",
            vec!["c1".into(), "c2".into()],
        );
        let results = vec![CriteriaResult::Achieved, CriteriaResult::NotStarted];
        assert_eq!(comp.evaluate_with_results(&results), CriteriaResult::Achieved);
    }

    #[test]
    fn composite_any_none_achieved() {
        let comp = CompositeCriteria::any(
            "comp-any",
            vec!["c1".into(), "c2".into()],
        );
        let results = vec![CriteriaResult::NotStarted, CriteriaResult::NotStarted];
        assert_eq!(
            comp.evaluate_with_results(&results),
            CriteriaResult::NotStarted
        );
    }

    #[test]
    fn composite_any_in_progress() {
        let comp = CompositeCriteria::any(
            "comp-any",
            vec!["c1".into(), "c2".into()],
        );
        let results = vec![
            CriteriaResult::InProgress {
                current: 3,
                target: 10,
            },
            CriteriaResult::NotStarted,
        ];
        assert_eq!(
            comp.evaluate_with_results(&results),
            CriteriaResult::InProgress {
                current: 0,
                target: 1
            }
        );
    }

    #[test]
    fn composite_empty_results() {
        let comp = CompositeCriteria::all("comp", Vec::new());
        assert_eq!(
            comp.evaluate_with_results(&[]),
            CriteriaResult::NotStarted
        );
    }

    #[test]
    fn composite_accessors() {
        let comp = CompositeCriteria::all(
            "comp",
            vec!["a".into(), "b".into()],
        );
        assert_eq!(comp.criteria_ids(), &["a", "b"]);
        assert!(comp.requires_all());

        let comp2 = CompositeCriteria::any("comp2", vec!["x".into()]);
        assert!(!comp2.requires_all());
    }

    // --- AchievementRegistry tests ---

    #[test]
    fn registry_empty() {
        let reg = AchievementRegistry::new();
        assert_eq!(reg.count(), 0);
        assert_eq!(reg.criteria_count(), 0);
    }

    #[test]
    fn registry_register_and_define() {
        let mut reg = AchievementRegistry::new();
        reg.register_criteria(Box::new(CounterCriteria::new("posts-10", "posts", 10)));
        reg.define_achievement(Achievement::new(
            "first-ten",
            "First Ten",
            "Create 10 posts",
            AchievementCategory::Creation,
            "posts-10",
            AchievementTier::Bronze,
        ));
        assert_eq!(reg.count(), 1);
        assert_eq!(reg.criteria_count(), 1);
    }

    #[test]
    fn registry_evaluate() {
        let mut reg = AchievementRegistry::new();
        reg.register_criteria(Box::new(CounterCriteria::new("posts-10", "posts", 10)));
        reg.define_achievement(Achievement::new(
            "first-ten",
            "First Ten",
            "Create 10 posts",
            AchievementCategory::Creation,
            "posts-10",
            AchievementTier::Bronze,
        ));

        let ctx = AchievementContext::new("cpub1alice").with_counter("posts", 5);
        let result = reg.evaluate("first-ten", &ctx).unwrap();
        assert_eq!(
            result,
            CriteriaResult::InProgress {
                current: 5,
                target: 10
            }
        );

        let ctx2 = AchievementContext::new("cpub1alice").with_counter("posts", 10);
        let result2 = reg.evaluate("first-ten", &ctx2).unwrap();
        assert_eq!(result2, CriteriaResult::Achieved);
    }

    #[test]
    fn registry_evaluate_nonexistent() {
        let reg = AchievementRegistry::new();
        let ctx = AchievementContext::new("alice");
        assert!(reg.evaluate("nope", &ctx).is_none());
    }

    #[test]
    fn registry_evaluate_missing_criteria() {
        let mut reg = AchievementRegistry::new();
        reg.define_achievement(Achievement::new(
            "orphan",
            "Orphan",
            "No criteria",
            AchievementCategory::Exploration,
            "nonexistent-criteria",
            AchievementTier::Bronze,
        ));
        let ctx = AchievementContext::new("alice");
        assert!(reg.evaluate("orphan", &ctx).is_none());
    }

    #[test]
    fn registry_evaluate_all() {
        let mut reg = AchievementRegistry::new();
        reg.register_criteria(Box::new(CounterCriteria::new("posts-1", "posts", 1)));
        reg.register_criteria(Box::new(FlagCriteria::new("backup", "has_backup", true)));

        reg.define_achievement(Achievement::new(
            "first-post",
            "First Post",
            "Make a post",
            AchievementCategory::Creation,
            "posts-1",
            AchievementTier::Bronze,
        ));
        reg.define_achievement(Achievement::new(
            "backed-up",
            "Backed Up",
            "Set up backup",
            AchievementCategory::Sovereignty,
            "backup",
            AchievementTier::Bronze,
        ));

        let ctx = AchievementContext::new("alice")
            .with_counter("posts", 5)
            .with_flag("has_backup", true);
        let results = reg.evaluate_all(&ctx);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_achieved()));
    }

    #[test]
    fn registry_get_achievement() {
        let mut reg = AchievementRegistry::new();
        reg.define_achievement(Achievement::new(
            "test",
            "Test",
            "A test",
            AchievementCategory::Exploration,
            "c1",
            AchievementTier::Silver,
        ));
        assert!(reg.get_achievement("test").is_some());
        assert!(reg.get_achievement("nope").is_none());
    }

    #[test]
    fn registry_by_category() {
        let mut reg = AchievementRegistry::new();
        reg.define_achievement(Achievement::new(
            "a1",
            "A1",
            "",
            AchievementCategory::Creation,
            "c1",
            AchievementTier::Bronze,
        ));
        reg.define_achievement(Achievement::new(
            "a2",
            "A2",
            "",
            AchievementCategory::Social,
            "c2",
            AchievementTier::Bronze,
        ));
        reg.define_achievement(Achievement::new(
            "a3",
            "A3",
            "",
            AchievementCategory::Creation,
            "c3",
            AchievementTier::Silver,
        ));

        assert_eq!(reg.by_category(&AchievementCategory::Creation).len(), 2);
        assert_eq!(reg.by_category(&AchievementCategory::Social).len(), 1);
        assert_eq!(reg.by_category(&AchievementCategory::Commerce).len(), 0);
    }

    #[test]
    fn registry_by_tier() {
        let mut reg = AchievementRegistry::new();
        reg.define_achievement(Achievement::new(
            "a1",
            "A1",
            "",
            AchievementCategory::Creation,
            "c1",
            AchievementTier::Bronze,
        ));
        reg.define_achievement(Achievement::new(
            "a2",
            "A2",
            "",
            AchievementCategory::Social,
            "c2",
            AchievementTier::Legendary,
        ));

        assert_eq!(reg.by_tier(AchievementTier::Bronze).len(), 1);
        assert_eq!(reg.by_tier(AchievementTier::Legendary).len(), 1);
        assert_eq!(reg.by_tier(AchievementTier::Gold).len(), 0);
    }

    #[test]
    fn registry_criteria_replacement() {
        let mut reg = AchievementRegistry::new();
        reg.register_criteria(Box::new(CounterCriteria::new("c1", "posts", 10)));
        reg.register_criteria(Box::new(CounterCriteria::new("c1", "posts", 5)));
        assert_eq!(reg.criteria_count(), 1);

        // The replacement should use target 5
        reg.define_achievement(Achievement::new(
            "test",
            "Test",
            "",
            AchievementCategory::Creation,
            "c1",
            AchievementTier::Bronze,
        ));
        let ctx = AchievementContext::new("alice").with_counter("posts", 5);
        assert_eq!(reg.evaluate("test", &ctx), Some(CriteriaResult::Achieved));
    }

    #[test]
    fn registry_debug() {
        let reg = AchievementRegistry::new();
        let debug = format!("{reg:?}");
        assert!(debug.contains("AchievementRegistry"));
    }

    #[test]
    fn registry_default() {
        let reg = AchievementRegistry::default();
        assert_eq!(reg.count(), 0);
    }

    // --- Object safety test ---

    #[test]
    fn criteria_trait_is_object_safe() {
        // This test verifies the trait can be used as a trait object.
        let criteria: Box<dyn AchievementCriteria> =
            Box::new(CounterCriteria::new("c1", "posts", 10));
        let ctx = AchievementContext::new("alice").with_counter("posts", 15);
        assert_eq!(criteria.evaluate(&ctx), CriteriaResult::Achieved);
        assert_eq!(criteria.id(), "c1");
        assert!(!criteria.description().is_empty());
    }

    #[test]
    fn criteria_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CounterCriteria>();
        assert_send_sync::<FlagCriteria>();
        assert_send_sync::<TimestampCriteria>();
        assert_send_sync::<CompositeCriteria>();
    }

    // --- AchievementTier tests ---

    #[test]
    fn achievement_tier_serde_round_trip() {
        let tiers = [
            AchievementTier::Bronze,
            AchievementTier::Silver,
            AchievementTier::Gold,
            AchievementTier::Platinum,
            AchievementTier::Legendary,
        ];
        for tier in &tiers {
            let json = serde_json::to_string(tier).unwrap();
            let restored: AchievementTier = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, tier);
        }
    }

    // --- AchievementStatus tests ---

    #[test]
    fn achievement_status_serde_round_trip() {
        let statuses = [
            AchievementStatus::Locked,
            AchievementStatus::InProgress,
            AchievementStatus::Completed,
        ];
        for status in &statuses {
            let json = serde_json::to_string(status).unwrap();
            let restored: AchievementStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, status);
        }
    }
}
