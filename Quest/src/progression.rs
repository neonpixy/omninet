//! XP, levels, skill trees, streaks, and flow calibration -- personal growth.
//!
//! Progression tracks a participant's growth over time. XP and levels provide
//! a sense of advancement. Skill trees map to Throne programs. Personal bests
//! compare you against yourself, never others. Streaks have forgiveness built in
//! because the Covenant forbids punishing rest.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::QuestConfig;

/// Type alias for experience point amounts.
pub type XpAmount = u64;

/// A participant's progression state.
///
/// Tracks XP, level, skill trees, personal bests, and streaks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progression {
    /// The participant's public key.
    pub actor: String,
    /// Total accumulated XP.
    pub total_xp: XpAmount,
    /// Current level (computed from total_xp and config).
    pub level: u32,
    /// Skill trees the participant is progressing through.
    pub skill_trees: HashMap<String, SkillTree>,
    /// Personal best records.
    pub personal_bests: Vec<PersonalBest>,
    /// Activity streak tracking.
    pub streak: Streak,
    /// When the participant joined Quest.
    pub joined_at: DateTime<Utc>,
}

impl Progression {
    /// Create a new progression tracker for a participant.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            total_xp: 0,
            level: 1,
            skill_trees: HashMap::new(),
            personal_bests: Vec::new(),
            streak: Streak::new(2), // default forgiveness
            joined_at: Utc::now(),
        }
    }

    /// Create a new progression tracker with a specific config (for streak forgiveness).
    pub fn with_config(actor: impl Into<String>, config: &QuestConfig) -> Self {
        Self {
            actor: actor.into(),
            total_xp: 0,
            level: 1,
            skill_trees: HashMap::new(),
            personal_bests: Vec::new(),
            streak: Streak::new(config.streak_forgiveness_days),
            joined_at: Utc::now(),
        }
    }

    /// Award XP and recalculate level.
    ///
    /// Returns the new level (which may be the same if not enough XP for a level-up).
    pub fn award_xp(&mut self, amount: XpAmount, config: &QuestConfig) -> u32 {
        self.total_xp = self.total_xp.saturating_add(amount);
        self.level = Self::level_for_xp(self.total_xp, config);
        self.level
    }

    /// Calculate what level corresponds to a given total XP amount.
    ///
    /// Uses the formula: level N requires `base * scaling^(N-1)` cumulative XP.
    pub fn level_for_xp(total_xp: XpAmount, config: &QuestConfig) -> u32 {
        let mut cumulative: f64 = 0.0;
        let mut level: u32 = 1;

        while level < config.max_level {
            let xp_for_next = Self::xp_for_level_internal(level + 1, config);
            cumulative += xp_for_next;
            if (total_xp as f64) < cumulative {
                break;
            }
            level += 1;
        }

        level
    }

    /// How much XP is needed to reach a specific level (from the previous level).
    ///
    /// Level 1 needs 0 XP (you start there). Level 2 needs `base` XP.
    /// Level N needs `base * scaling^(N-2)` XP.
    pub fn xp_for_level(level: u32, config: &QuestConfig) -> XpAmount {
        if level <= 1 {
            return 0;
        }
        Self::xp_for_level_internal(level, config) as XpAmount
    }

    fn xp_for_level_internal(level: u32, config: &QuestConfig) -> f64 {
        if level <= 1 {
            return 0.0;
        }
        config.xp_per_level_base as f64 * config.xp_level_scaling.powi((level as i32) - 2)
    }

    /// Total XP needed to reach the next level from current position.
    pub fn xp_to_next_level(&self, config: &QuestConfig) -> XpAmount {
        if self.level >= config.max_level {
            return 0;
        }
        let cumulative_for_current = self.cumulative_xp_for_level(self.level, config);
        let cumulative_for_next = self.cumulative_xp_for_level(self.level + 1, config);
        let needed = cumulative_for_next.saturating_sub(cumulative_for_current);
        let earned_toward_next = self.total_xp.saturating_sub(cumulative_for_current);
        needed.saturating_sub(earned_toward_next)
    }

    /// Progress toward the next level as a fraction (0.0 to 1.0).
    pub fn progress_to_next_level(&self, config: &QuestConfig) -> f64 {
        if self.level >= config.max_level {
            return 1.0;
        }
        let cumulative_for_current = self.cumulative_xp_for_level(self.level, config);
        let cumulative_for_next = self.cumulative_xp_for_level(self.level + 1, config);
        let range = cumulative_for_next.saturating_sub(cumulative_for_current);
        if range == 0 {
            return 1.0;
        }
        let earned = self.total_xp.saturating_sub(cumulative_for_current);
        (earned as f64 / range as f64).min(1.0)
    }

    /// Cumulative XP needed to reach a given level.
    fn cumulative_xp_for_level(&self, level: u32, config: &QuestConfig) -> XpAmount {
        let mut total: f64 = 0.0;
        for l in 2..=level {
            total += Self::xp_for_level_internal(l, config);
        }
        total as XpAmount
    }

    /// Add a skill tree to track.
    pub fn add_skill_tree(&mut self, tree: SkillTree) {
        self.skill_trees.insert(tree.id.clone(), tree);
    }

    /// Record a new personal best if it exceeds the previous record.
    ///
    /// Returns `true` if this is a new personal best.
    pub fn record_personal_best(
        &mut self,
        metric: impl Into<String>,
        value: u64,
        window_days: u32,
    ) -> bool {
        let metric = metric.into();
        let previous = self
            .personal_bests
            .iter()
            .filter(|pb| pb.metric == metric)
            .map(|pb| pb.value)
            .max();

        if previous.is_some_and(|prev| value <= prev) {
            return false;
        }

        self.personal_bests.push(PersonalBest {
            metric,
            value,
            previous_best: previous,
            achieved_at: Utc::now(),
            window_days,
        });
        true
    }

    /// Check the current personal best for a metric.
    pub fn check_personal_best(&self, metric: &str) -> Option<&PersonalBest> {
        self.personal_bests
            .iter()
            .filter(|pb| pb.metric == metric)
            .max_by_key(|pb| pb.value)
    }
}

/// A skill tree representing mastery of a program or domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTree {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// What this skill tree covers.
    pub description: String,
    /// Which Throne program this maps to (Studio, Abacus, etc.).
    pub program: String,
    /// All nodes in the tree.
    pub nodes: Vec<SkillNode>,
    /// IDs of unlocked nodes.
    pub unlocked: HashSet<String>,
}

impl SkillTree {
    /// Create a new skill tree.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        program: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            program: program.into(),
            nodes: Vec::new(),
            unlocked: HashSet::new(),
        }
    }

    /// Add a node to the tree.
    pub fn with_node(mut self, node: SkillNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// Unlock a node by ID. Returns `true` if newly unlocked.
    ///
    /// Does not check prerequisites -- the caller should check `available_nodes()`
    /// before calling this.
    pub fn unlock(&mut self, node_id: impl Into<String>) -> bool {
        self.unlocked.insert(node_id.into())
    }

    /// Whether a node is unlocked.
    pub fn is_unlocked(&self, node_id: &str) -> bool {
        self.unlocked.contains(node_id)
    }

    /// Nodes whose prerequisites are all met but that are not yet unlocked.
    pub fn available_nodes(&self) -> Vec<&SkillNode> {
        self.nodes
            .iter()
            .filter(|node| {
                !self.unlocked.contains(&node.id)
                    && node
                        .prerequisites
                        .iter()
                        .all(|prereq| self.unlocked.contains(prereq))
            })
            .collect()
    }

    /// Mastery percentage (unlocked / total nodes).
    pub fn mastery_percent(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        self.unlocked.len() as f64 / self.nodes.len() as f64 * 100.0
    }
}

/// A single node in a skill tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    /// Unique identifier within the tree.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// What unlocking this node represents.
    pub description: String,
    /// XP required to unlock (in addition to prerequisites).
    pub xp_required: XpAmount,
    /// IDs of nodes that must be unlocked first.
    pub prerequisites: Vec<String>,
    /// Depth in the tree (0 = root).
    pub tier: u32,
}

impl SkillNode {
    /// Create a new skill node.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        xp_required: XpAmount,
        tier: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            xp_required,
            prerequisites: Vec::new(),
            tier,
        }
    }

    /// Add a prerequisite node ID.
    pub fn with_prerequisite(mut self, prereq: impl Into<String>) -> Self {
        self.prerequisites.push(prereq.into());
        self
    }
}

/// A personal best record -- you compete against yourself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalBest {
    /// What was measured (e.g., "words_written", "designs_created").
    pub metric: String,
    /// The new best value.
    pub value: u64,
    /// Previous best value, if any.
    pub previous_best: Option<u64>,
    /// When this record was set.
    pub achieved_at: DateTime<Utc>,
    /// Comparison window in days.
    pub window_days: u32,
}

/// Activity streak tracking with built-in forgiveness.
///
/// The Covenant forbids punishing rest. Missing a day or two is human,
/// not failure. `forgiveness_days` provides a grace period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Streak {
    /// Current streak in days.
    pub current: u32,
    /// All-time longest streak.
    pub longest: u32,
    /// When the participant was last active.
    pub last_active: Option<DateTime<Utc>>,
    /// Grace period before streak is affected. Default: 2.
    /// The Covenant forbids punishing rest.
    pub forgiveness_days: u32,
}

impl Streak {
    /// Create a new streak tracker with the given forgiveness window.
    pub fn new(forgiveness_days: u32) -> Self {
        Self {
            current: 0,
            longest: 0,
            last_active: None,
            forgiveness_days,
        }
    }

    /// Record activity at the given time.
    ///
    /// If activity is within the forgiveness window, the streak continues.
    /// If it's been too long, the streak resets to 1.
    pub fn record_activity(&mut self, now: DateTime<Utc>) {
        match self.last_active {
            None => {
                // First activity ever
                self.current = 1;
            }
            Some(last) => {
                let days_since = (now - last).num_days();
                if days_since <= 0 {
                    // Same day -- no change to streak count
                    self.last_active = Some(now);
                    return;
                } else if days_since <= 1 + self.forgiveness_days as i64 {
                    // Within grace period -- streak continues
                    self.current += 1;
                } else {
                    // Too long -- streak resets
                    self.current = 1;
                }
            }
        }

        self.longest = self.longest.max(self.current);
        self.last_active = Some(now);
    }

    /// Whether the streak is currently active (within forgiveness window).
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        match self.last_active {
            None => false,
            Some(last) => {
                let days_since = (now - last).num_days();
                days_since <= 1 + self.forgiveness_days as i64
            }
        }
    }

    /// Days remaining before the grace period expires.
    ///
    /// Returns 0 if already expired or no activity recorded.
    pub fn days_until_grace_expires(&self, now: DateTime<Utc>) -> u32 {
        match self.last_active {
            None => 0,
            Some(last) => {
                let days_since = (now - last).num_days();
                let grace_total = 1 + self.forgiveness_days as i64;
                let remaining = grace_total - days_since;
                if remaining > 0 {
                    remaining as u32
                } else {
                    0
                }
            }
        }
    }
}

/// Flow state calibration -- difficulty adaptation.
///
/// Adjusts suggested difficulty based on completion rates. If a participant
/// is completing everything easily (>90%), suggest harder. If struggling
/// (<30%), suggest easier. This keeps people in their flow state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowCalibration {
    /// The participant's public key.
    pub actor: String,
    /// Recent activity rate (missions/challenges completed per day).
    pub velocity: f64,
    /// Current suggested difficulty.
    pub suggested_difficulty: Difficulty,
    /// History of difficulty snapshots for trend analysis.
    pub history: Vec<DifficultySnapshot>,
}

impl FlowCalibration {
    /// Create a new flow calibration starting at Normal difficulty.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            velocity: 0.0,
            suggested_difficulty: Difficulty::Normal,
            history: Vec::new(),
        }
    }

    /// Update calibration based on recent completion rate.
    ///
    /// `completion_rate` should be between 0.0 and 1.0.
    /// `adaptation_rate` controls how aggressively difficulty changes (from config).
    pub fn update(&mut self, completion_rate: f64, adaptation_rate: f64) {
        let clamped_rate = completion_rate.clamp(0.0, 1.0);
        let now = Utc::now();

        // Record snapshot
        self.history.push(DifficultySnapshot {
            difficulty: self.suggested_difficulty,
            completion_rate: clamped_rate,
            measured_at: now,
        });

        // Adjust difficulty based on completion rate
        self.suggested_difficulty = Self::suggest_difficulty_from_rate(clamped_rate, adaptation_rate, self.suggested_difficulty);
    }

    /// Suggest a difficulty level based on completion rate.
    ///
    /// - Above 90%: suggest harder
    /// - Below 30%: suggest easier
    /// - Between: stay current
    ///
    /// `adaptation_rate` modulates how likely a change is. Higher = more responsive.
    pub fn suggest_difficulty(completion_rate: f64) -> Difficulty {
        Self::suggest_difficulty_from_rate(completion_rate, 1.0, Difficulty::Normal)
    }

    fn suggest_difficulty_from_rate(
        completion_rate: f64,
        adaptation_rate: f64,
        current: Difficulty,
    ) -> Difficulty {
        // Only change if the signal is strong enough (modulated by adaptation_rate)
        let threshold = 1.0 - adaptation_rate.clamp(0.0, 1.0);

        if completion_rate > 0.9 && completion_rate > threshold {
            current.harder()
        } else if completion_rate < 0.3 && (1.0 - completion_rate) > threshold {
            current.easier()
        } else {
            current
        }
    }
}

/// Difficulty levels for missions and challenges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Difficulty {
    /// Easy, relaxed pace.
    Gentle,
    /// Standard difficulty.
    Normal,
    /// Challenging but achievable.
    Ambitious,
    /// Maximum challenge.
    Heroic,
}

impl Difficulty {
    /// Move to the next harder difficulty, capping at Heroic.
    pub fn harder(self) -> Self {
        match self {
            Self::Gentle => Self::Normal,
            Self::Normal => Self::Ambitious,
            Self::Ambitious => Self::Heroic,
            Self::Heroic => Self::Heroic,
        }
    }

    /// Move to the next easier difficulty, flooring at Gentle.
    pub fn easier(self) -> Self {
        match self {
            Self::Gentle => Self::Gentle,
            Self::Normal => Self::Gentle,
            Self::Ambitious => Self::Normal,
            Self::Heroic => Self::Ambitious,
        }
    }
}

/// A snapshot of difficulty and completion rate at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifficultySnapshot {
    /// The difficulty at this point.
    pub difficulty: Difficulty,
    /// Completion rate (0.0 to 1.0).
    pub completion_rate: f64,
    /// When this was measured.
    pub measured_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    fn default_config() -> QuestConfig {
        QuestConfig::default()
    }

    // --- Progression tests ---

    #[test]
    fn progression_creation() {
        let p = Progression::new("cpub1alice");
        assert_eq!(p.actor, "cpub1alice");
        assert_eq!(p.total_xp, 0);
        assert_eq!(p.level, 1);
        assert!(p.skill_trees.is_empty());
        assert!(p.personal_bests.is_empty());
    }

    #[test]
    fn progression_with_config() {
        let config = QuestConfig::casual();
        let p = Progression::with_config("alice", &config);
        assert_eq!(p.streak.forgiveness_days, 5); // casual has 5
    }

    #[test]
    fn award_xp_basic() {
        let config = default_config();
        let mut p = Progression::new("alice");
        p.award_xp(50, &config);
        assert_eq!(p.total_xp, 50);
        assert_eq!(p.level, 1); // 100 XP needed for level 2
    }

    #[test]
    fn award_xp_level_up() {
        let config = default_config();
        let mut p = Progression::new("alice");
        p.award_xp(100, &config); // exactly enough for level 2
        assert_eq!(p.level, 2);
    }

    #[test]
    fn award_xp_multiple_levels() {
        let config = default_config();
        let mut p = Progression::new("alice");
        // Level 2: 100, Level 3: 150 (100*1.5), total: 250
        p.award_xp(250, &config);
        assert_eq!(p.level, 3);
    }

    #[test]
    fn award_xp_saturating() {
        let config = default_config();
        let mut p = Progression::new("alice");
        p.total_xp = u64::MAX - 10;
        p.award_xp(100, &config);
        assert_eq!(p.total_xp, u64::MAX);
    }

    #[test]
    fn xp_for_level_1_is_zero() {
        let config = default_config();
        assert_eq!(Progression::xp_for_level(1, &config), 0);
    }

    #[test]
    fn xp_for_level_2_is_base() {
        let config = default_config();
        assert_eq!(Progression::xp_for_level(2, &config), 100);
    }

    #[test]
    fn xp_for_level_3_is_scaled() {
        let config = default_config();
        // base * scaling^(3-2) = 100 * 1.5 = 150
        assert_eq!(Progression::xp_for_level(3, &config), 150);
    }

    #[test]
    fn level_for_xp_zero() {
        let config = default_config();
        assert_eq!(Progression::level_for_xp(0, &config), 1);
    }

    #[test]
    fn level_for_xp_exact_boundary() {
        let config = default_config();
        assert_eq!(Progression::level_for_xp(100, &config), 2);
    }

    #[test]
    fn level_for_xp_between_levels() {
        let config = default_config();
        assert_eq!(Progression::level_for_xp(99, &config), 1);
        assert_eq!(Progression::level_for_xp(101, &config), 2);
    }

    #[test]
    fn xp_to_next_level() {
        let config = default_config();
        let mut p = Progression::new("alice");
        // At level 1, 0 XP, need 100 to reach level 2
        assert_eq!(p.xp_to_next_level(&config), 100);

        p.award_xp(50, &config);
        assert_eq!(p.xp_to_next_level(&config), 50);

        p.award_xp(50, &config); // now at level 2
        // Need 150 for level 3
        assert_eq!(p.xp_to_next_level(&config), 150);
    }

    #[test]
    fn xp_to_next_level_at_max() {
        let config = QuestConfig::default().with_max_level(2);
        let mut p = Progression::new("alice");
        p.award_xp(100, &config);
        assert_eq!(p.level, 2);
        assert_eq!(p.xp_to_next_level(&config), 0);
    }

    #[test]
    fn progress_to_next_level_fraction() {
        let config = default_config();
        let mut p = Progression::new("alice");
        assert_eq!(p.progress_to_next_level(&config), 0.0);

        p.award_xp(50, &config);
        assert_eq!(p.progress_to_next_level(&config), 0.5);

        p.award_xp(50, &config);
        // Now at level 2, 0 progress toward level 3
        assert_eq!(p.progress_to_next_level(&config), 0.0);
    }

    #[test]
    fn progress_to_next_level_at_max() {
        let config = QuestConfig::default().with_max_level(1);
        let p = Progression::new("alice");
        assert_eq!(p.progress_to_next_level(&config), 1.0);
    }

    #[test]
    fn add_skill_tree() {
        let mut p = Progression::new("alice");
        let tree = SkillTree::new("studio-basics", "Studio Basics", "Learn Studio", "Studio");
        p.add_skill_tree(tree);
        assert_eq!(p.skill_trees.len(), 1);
        assert!(p.skill_trees.contains_key("studio-basics"));
    }

    #[test]
    fn personal_best_new_record() {
        let mut p = Progression::new("alice");
        assert!(p.record_personal_best("words_written", 1000, 30));
        assert_eq!(p.check_personal_best("words_written").unwrap().value, 1000);
    }

    #[test]
    fn personal_best_not_beaten() {
        let mut p = Progression::new("alice");
        p.record_personal_best("words_written", 1000, 30);
        assert!(!p.record_personal_best("words_written", 500, 30));
        assert!(!p.record_personal_best("words_written", 1000, 30)); // equal doesn't count
    }

    #[test]
    fn personal_best_beaten() {
        let mut p = Progression::new("alice");
        p.record_personal_best("words_written", 1000, 30);
        assert!(p.record_personal_best("words_written", 1500, 30));

        let best = p.check_personal_best("words_written").unwrap();
        assert_eq!(best.value, 1500);
        assert_eq!(best.previous_best, Some(1000));
    }

    #[test]
    fn personal_best_different_metrics() {
        let mut p = Progression::new("alice");
        p.record_personal_best("words", 100, 30);
        p.record_personal_best("designs", 50, 30);
        assert_eq!(p.check_personal_best("words").unwrap().value, 100);
        assert_eq!(p.check_personal_best("designs").unwrap().value, 50);
        assert!(p.check_personal_best("nonexistent").is_none());
    }

    #[test]
    fn progression_serde_round_trip() {
        let config = default_config();
        let mut p = Progression::new("alice");
        p.award_xp(200, &config);
        p.record_personal_best("posts", 42, 30);
        let json = serde_json::to_string(&p).unwrap();
        let restored: Progression = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.actor, "alice");
        assert_eq!(restored.total_xp, 200);
        assert_eq!(restored.level, p.level);
    }

    // --- SkillTree tests ---

    #[test]
    fn skill_tree_creation() {
        let tree = SkillTree::new("design", "Design Skills", "Master design", "Studio");
        assert_eq!(tree.id, "design");
        assert!(tree.nodes.is_empty());
        assert!(tree.unlocked.is_empty());
    }

    #[test]
    fn skill_tree_with_nodes() {
        let tree = SkillTree::new("design", "Design", "", "Studio")
            .with_node(SkillNode::new("basics", "Basics", "Learn basics", 50, 0))
            .with_node(
                SkillNode::new("advanced", "Advanced", "Advanced techniques", 200, 1)
                    .with_prerequisite("basics"),
            );
        assert_eq!(tree.nodes.len(), 2);
    }

    #[test]
    fn skill_tree_unlock() {
        let mut tree = SkillTree::new("design", "Design", "", "Studio")
            .with_node(SkillNode::new("basics", "Basics", "", 50, 0));

        assert!(!tree.is_unlocked("basics"));
        assert!(tree.unlock("basics")); // first unlock returns true
        assert!(tree.is_unlocked("basics"));
        assert!(!tree.unlock("basics")); // second unlock returns false (already unlocked)
    }

    #[test]
    fn skill_tree_available_nodes() {
        let mut tree = SkillTree::new("design", "Design", "", "Studio")
            .with_node(SkillNode::new("basics", "Basics", "", 50, 0))
            .with_node(
                SkillNode::new("intermediate", "Intermediate", "", 100, 1)
                    .with_prerequisite("basics"),
            )
            .with_node(
                SkillNode::new("advanced", "Advanced", "", 200, 2)
                    .with_prerequisite("intermediate"),
            );

        // Initially only basics is available (no prereqs)
        let available = tree.available_nodes();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, "basics");

        // After unlocking basics, intermediate becomes available
        tree.unlock("basics");
        let available = tree.available_nodes();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, "intermediate");

        // After unlocking intermediate, advanced becomes available
        tree.unlock("intermediate");
        let available = tree.available_nodes();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, "advanced");

        // After unlocking everything, nothing available
        tree.unlock("advanced");
        assert!(tree.available_nodes().is_empty());
    }

    #[test]
    fn skill_tree_mastery_percent() {
        let mut tree = SkillTree::new("design", "Design", "", "Studio")
            .with_node(SkillNode::new("a", "A", "", 50, 0))
            .with_node(SkillNode::new("b", "B", "", 50, 0))
            .with_node(SkillNode::new("c", "C", "", 50, 0))
            .with_node(SkillNode::new("d", "D", "", 50, 0));

        assert_eq!(tree.mastery_percent(), 0.0);
        tree.unlock("a");
        assert_eq!(tree.mastery_percent(), 25.0);
        tree.unlock("b");
        assert_eq!(tree.mastery_percent(), 50.0);
        tree.unlock("c");
        tree.unlock("d");
        assert_eq!(tree.mastery_percent(), 100.0);
    }

    #[test]
    fn skill_tree_mastery_empty() {
        let tree = SkillTree::new("empty", "Empty", "", "Studio");
        assert_eq!(tree.mastery_percent(), 0.0);
    }

    #[test]
    fn skill_tree_serde_round_trip() {
        let mut tree = SkillTree::new("design", "Design", "desc", "Studio")
            .with_node(SkillNode::new("a", "A", "", 50, 0));
        tree.unlock("a");
        let json = serde_json::to_string(&tree).unwrap();
        let restored: SkillTree = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "design");
        assert!(restored.is_unlocked("a"));
    }

    // --- SkillNode tests ---

    #[test]
    fn skill_node_creation() {
        let node = SkillNode::new("basics", "Basics", "Learn the basics", 50, 0);
        assert_eq!(node.id, "basics");
        assert_eq!(node.xp_required, 50);
        assert_eq!(node.tier, 0);
        assert!(node.prerequisites.is_empty());
    }

    #[test]
    fn skill_node_with_prerequisites() {
        let node = SkillNode::new("advanced", "Advanced", "", 200, 2)
            .with_prerequisite("basics")
            .with_prerequisite("intermediate");
        assert_eq!(node.prerequisites.len(), 2);
    }

    #[test]
    fn skill_node_serde_round_trip() {
        let node = SkillNode::new("test", "Test", "desc", 100, 1)
            .with_prerequisite("prereq1");
        let json = serde_json::to_string(&node).unwrap();
        let restored: SkillNode = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "test");
        assert_eq!(restored.prerequisites, vec!["prereq1"]);
    }

    // --- Streak tests ---

    #[test]
    fn streak_creation() {
        let s = Streak::new(2);
        assert_eq!(s.current, 0);
        assert_eq!(s.longest, 0);
        assert!(s.last_active.is_none());
        assert_eq!(s.forgiveness_days, 2);
    }

    #[test]
    fn streak_first_activity() {
        let mut s = Streak::new(2);
        s.record_activity(Utc::now());
        assert_eq!(s.current, 1);
        assert_eq!(s.longest, 1);
        assert!(s.last_active.is_some());
    }

    #[test]
    fn streak_consecutive_days() {
        let mut s = Streak::new(2);
        let day1 = Utc::now();
        let day2 = day1 + Duration::days(1);
        let day3 = day2 + Duration::days(1);

        s.record_activity(day1);
        assert_eq!(s.current, 1);
        s.record_activity(day2);
        assert_eq!(s.current, 2);
        s.record_activity(day3);
        assert_eq!(s.current, 3);
        assert_eq!(s.longest, 3);
    }

    #[test]
    fn streak_same_day_no_increment() {
        let mut s = Streak::new(2);
        let now = Utc::now();
        s.record_activity(now);
        s.record_activity(now);
        assert_eq!(s.current, 1);
    }

    #[test]
    fn streak_forgiveness_within_grace() {
        let mut s = Streak::new(2);
        let day1 = Utc::now();
        let day4 = day1 + Duration::days(3); // skipped 2 days, within 2-day grace

        s.record_activity(day1);
        s.record_activity(day4);
        assert_eq!(s.current, 2); // streak continues!
    }

    #[test]
    fn streak_forgiveness_exceeded() {
        let mut s = Streak::new(2);
        let day1 = Utc::now();
        let day5 = day1 + Duration::days(4); // skipped 3 days, exceeds 2-day grace

        s.record_activity(day1);
        s.record_activity(day5);
        assert_eq!(s.current, 1); // streak reset
        assert_eq!(s.longest, 1); // longest preserved from day1
    }

    #[test]
    fn streak_longest_preserved() {
        let mut s = Streak::new(0); // no forgiveness for this test
        let day1 = Utc::now();

        s.record_activity(day1);
        s.record_activity(day1 + Duration::days(1));
        s.record_activity(day1 + Duration::days(2));
        assert_eq!(s.longest, 3);

        // Break the streak
        s.record_activity(day1 + Duration::days(10));
        assert_eq!(s.current, 1);
        assert_eq!(s.longest, 3); // still 3
    }

    #[test]
    fn streak_is_active() {
        let mut s = Streak::new(2);
        let now = Utc::now();

        assert!(!s.is_active(now)); // no activity yet

        s.record_activity(now);
        assert!(s.is_active(now));
        assert!(s.is_active(now + Duration::days(1)));
        assert!(s.is_active(now + Duration::days(3))); // within 2-day grace
        assert!(!s.is_active(now + Duration::days(4))); // grace exceeded
    }

    #[test]
    fn streak_days_until_grace_expires() {
        let mut s = Streak::new(2);
        let now = Utc::now();

        assert_eq!(s.days_until_grace_expires(now), 0); // no activity

        s.record_activity(now);
        assert_eq!(s.days_until_grace_expires(now), 3); // 1 + 2 forgiveness
        assert_eq!(s.days_until_grace_expires(now + Duration::days(1)), 2);
        assert_eq!(s.days_until_grace_expires(now + Duration::days(2)), 1);
        assert_eq!(s.days_until_grace_expires(now + Duration::days(3)), 0);
        assert_eq!(s.days_until_grace_expires(now + Duration::days(4)), 0);
    }

    #[test]
    fn streak_serde_round_trip() {
        let mut s = Streak::new(3);
        s.record_activity(Utc::now());
        let json = serde_json::to_string(&s).unwrap();
        let restored: Streak = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.current, 1);
        assert_eq!(restored.forgiveness_days, 3);
    }

    // --- FlowCalibration tests ---

    #[test]
    fn flow_calibration_creation() {
        let fc = FlowCalibration::new("alice");
        assert_eq!(fc.actor, "alice");
        assert_eq!(fc.suggested_difficulty, Difficulty::Normal);
        assert!(fc.history.is_empty());
    }

    #[test]
    fn flow_calibration_high_completion_increases_difficulty() {
        let mut fc = FlowCalibration::new("alice");
        fc.update(0.95, 0.5); // very high completion, moderate adaptation
        assert_eq!(fc.suggested_difficulty, Difficulty::Ambitious);
        assert_eq!(fc.history.len(), 1);
    }

    #[test]
    fn flow_calibration_low_completion_decreases_difficulty() {
        let mut fc = FlowCalibration::new("alice");
        fc.suggested_difficulty = Difficulty::Ambitious;
        fc.update(0.2, 0.5); // low completion
        assert_eq!(fc.suggested_difficulty, Difficulty::Normal);
    }

    #[test]
    fn flow_calibration_moderate_completion_stays() {
        let mut fc = FlowCalibration::new("alice");
        fc.update(0.6, 0.5); // moderate completion
        assert_eq!(fc.suggested_difficulty, Difficulty::Normal);
    }

    #[test]
    fn flow_calibration_suggest_difficulty_static() {
        assert_eq!(FlowCalibration::suggest_difficulty(0.95), Difficulty::Ambitious);
        assert_eq!(FlowCalibration::suggest_difficulty(0.5), Difficulty::Normal);
        assert_eq!(FlowCalibration::suggest_difficulty(0.1), Difficulty::Gentle);
    }

    #[test]
    fn flow_calibration_clamps_rate() {
        let mut fc = FlowCalibration::new("alice");
        fc.update(1.5, 0.5); // over 1.0
        // Should not panic, clamped to 1.0
        assert_eq!(fc.history.len(), 1);
    }

    #[test]
    fn flow_calibration_serde_round_trip() {
        let mut fc = FlowCalibration::new("alice");
        fc.update(0.95, 0.5);
        let json = serde_json::to_string(&fc).unwrap();
        let restored: FlowCalibration = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.actor, "alice");
        assert_eq!(restored.suggested_difficulty, Difficulty::Ambitious);
    }

    // --- Difficulty tests ---

    #[test]
    fn difficulty_harder() {
        assert_eq!(Difficulty::Gentle.harder(), Difficulty::Normal);
        assert_eq!(Difficulty::Normal.harder(), Difficulty::Ambitious);
        assert_eq!(Difficulty::Ambitious.harder(), Difficulty::Heroic);
        assert_eq!(Difficulty::Heroic.harder(), Difficulty::Heroic); // capped
    }

    #[test]
    fn difficulty_easier() {
        assert_eq!(Difficulty::Heroic.easier(), Difficulty::Ambitious);
        assert_eq!(Difficulty::Ambitious.easier(), Difficulty::Normal);
        assert_eq!(Difficulty::Normal.easier(), Difficulty::Gentle);
        assert_eq!(Difficulty::Gentle.easier(), Difficulty::Gentle); // floored
    }

    #[test]
    fn difficulty_serde_round_trip() {
        let difficulties = [
            Difficulty::Gentle,
            Difficulty::Normal,
            Difficulty::Ambitious,
            Difficulty::Heroic,
        ];
        for d in &difficulties {
            let json = serde_json::to_string(d).unwrap();
            let restored: Difficulty = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, d);
        }
    }

    // --- DifficultySnapshot tests ---

    #[test]
    fn difficulty_snapshot_serde_round_trip() {
        let snap = DifficultySnapshot {
            difficulty: Difficulty::Ambitious,
            completion_rate: 0.85,
            measured_at: Utc::now(),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let restored: DifficultySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.difficulty, Difficulty::Ambitious);
    }

    // --- PersonalBest tests ---

    #[test]
    fn personal_best_serde_round_trip() {
        let pb = PersonalBest {
            metric: "words_written".into(),
            value: 5000,
            previous_best: Some(3000),
            achieved_at: Utc::now(),
            window_days: 30,
        };
        let json = serde_json::to_string(&pb).unwrap();
        let restored: PersonalBest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.metric, "words_written");
        assert_eq!(restored.value, 5000);
        assert_eq!(restored.previous_best, Some(3000));
    }
}
