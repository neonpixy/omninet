//! Missions -- guided multi-step experiences with objectives.
//!
//! Missions are the primary structured activity in Quest. Each mission has a series
//! of objectives, optional time limits, and rewards for completion. Missions can be
//! system-defined (onboarding), community-created, personal goals, or
//! consortia-sponsored.
//!
//! # Design Principles
//!
//! - **No dark patterns.** Time limits are optional. Expired missions can be restarted.
//!   Daily/weekly missions have no punishment for skipping.
//! - **Community-defined.** Communities create their own missions via `MissionCreator::Community`.
//! - **Adaptive difficulty.** `AdaptiveDifficulty` suggests mission difficulty based on
//!   completion patterns -- keeps participants in flow state.
//!
//! # Example
//!
//! ```
//! use quest::mission::{Mission, MissionCategory, MissionCreator, MissionScope, MissionEngine, Objective};
//! use quest::progression::Difficulty;
//!
//! let mut engine = MissionEngine::new();
//!
//! let mission = Mission::new("Learn the Basics", "Create your first document in Quill")
//!     .with_category(MissionCategory::Onboarding)
//!     .with_difficulty(Difficulty::Gentle)
//!     .with_objective(Objective::new("open-quill", "Open the Quill program", 1, "quill_opened"))
//!     .with_objective(Objective::new("create-doc", "Create a new document", 1, "docs_created"))
//!     .with_xp_reward(50);
//!
//! engine.add_mission(mission);
//! assert_eq!(engine.mission_count(), 1);
//! ```

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::QuestError;
use crate::progression::{Difficulty, XpAmount};
use crate::reward::RewardType;

// ---------------------------------------------------------------------------
// Mission
// ---------------------------------------------------------------------------

/// A guided experience with one or more objectives to complete.
///
/// Missions are the bread-and-butter of Quest. They give participants clear goals,
/// optional time pressure (never mandatory!), and tangible rewards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// What this mission is about.
    pub description: String,
    /// Classification of the mission.
    pub category: MissionCategory,
    /// Steps to complete. At least one required objective must exist.
    pub objectives: Vec<Objective>,
    /// Items/currency/badges granted on completion.
    pub rewards: Vec<RewardType>,
    /// How hard this mission is.
    pub difficulty: Difficulty,
    /// Optional time limit. `None` means no time pressure -- no dark patterns.
    pub time_limit: Option<Duration>,
    /// Whether this mission can be done more than once.
    pub repeatable: bool,
    /// If repeatable, minimum time between attempts.
    pub cooldown: Option<Duration>,
    /// IDs (as strings) of missions that must be completed first.
    pub prerequisites: Vec<String>,
    /// Who created this mission.
    pub created_by: MissionCreator,
    /// Who can see and start this mission.
    pub scope: MissionScope,
    /// XP awarded on completion.
    pub xp_reward: XpAmount,
}

impl Mission {
    /// Create a new mission with sensible defaults.
    ///
    /// Starts as `MissionCategory::Personal`, `Difficulty::Normal`, global scope,
    /// system-created, non-repeatable, no time limit.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            category: MissionCategory::Personal,
            objectives: Vec::new(),
            rewards: Vec::new(),
            difficulty: Difficulty::Normal,
            time_limit: None,
            repeatable: false,
            cooldown: None,
            prerequisites: Vec::new(),
            created_by: MissionCreator::System,
            scope: MissionScope::Global,
            xp_reward: 0,
        }
    }

    /// Set the mission category.
    pub fn with_category(mut self, category: MissionCategory) -> Self {
        self.category = category;
        self
    }

    /// Add an objective.
    pub fn with_objective(mut self, objective: Objective) -> Self {
        self.objectives.push(objective);
        self
    }

    /// Add a reward.
    pub fn with_reward(mut self, reward: RewardType) -> Self {
        self.rewards.push(reward);
        self
    }

    /// Set the difficulty.
    pub fn with_difficulty(mut self, difficulty: Difficulty) -> Self {
        self.difficulty = difficulty;
        self
    }

    /// Set an optional time limit.
    pub fn with_time_limit(mut self, limit: Duration) -> Self {
        self.time_limit = Some(limit);
        self
    }

    /// Make this mission repeatable with an optional cooldown.
    pub fn with_repeatable(mut self, cooldown: Option<Duration>) -> Self {
        self.repeatable = true;
        self.cooldown = cooldown;
        self
    }

    /// Add a prerequisite mission ID.
    pub fn with_prerequisite(mut self, mission_id: impl Into<String>) -> Self {
        self.prerequisites.push(mission_id.into());
        self
    }

    /// Set who created this mission.
    pub fn with_creator(mut self, creator: MissionCreator) -> Self {
        self.created_by = creator;
        self
    }

    /// Set the mission scope.
    pub fn with_scope(mut self, scope: MissionScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set the XP reward.
    pub fn with_xp_reward(mut self, xp: XpAmount) -> Self {
        self.xp_reward = xp;
        self
    }

    /// Whether all required objectives have been met given progress data.
    pub fn is_complete(&self, progress: &HashMap<String, u64>) -> bool {
        self.objectives
            .iter()
            .filter(|o| !o.optional)
            .all(|o| progress.get(&o.id).copied().unwrap_or(0) >= o.target)
    }

    /// Count of required (non-optional) objectives.
    pub fn required_objective_count(&self) -> usize {
        self.objectives.iter().filter(|o| !o.optional).count()
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// What kind of mission this is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionCategory {
    /// First-time experiences -- learn the basics.
    Onboarding,
    /// Refreshes daily. No punishment for skipping!
    Daily,
    /// Refreshes weekly. No punishment for skipping!
    Weekly,
    /// Time-scoped themes (seasons, events).
    Seasonal,
    /// Self-defined goals.
    Personal,
    /// Community-created missions.
    Community,
    /// Specific to a Throne program (Studio, Abacus, Quill, etc.).
    Program(String),
    /// Explore features -- ties into Oracle's DisclosureTracker.
    Discovery,
    /// Anything else.
    Custom(String),
}

/// Who created this mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionCreator {
    /// Built-in system missions (onboarding, etc.).
    System,
    /// Created by a community.
    Community {
        /// The community that created this mission.
        community_id: Uuid,
    },
    /// A personal goal set by the participant.
    Personal {
        /// The participant's public key.
        actor: String,
    },
    /// Sponsored by a business consortium.
    Consortia {
        /// The sponsoring consortium.
        consortia_id: Uuid,
    },
}

/// Who can see and start this mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionScope {
    /// Everyone on the network.
    Global,
    /// Scoped to a specific community.
    Community {
        /// The community this mission belongs to.
        community_id: Uuid,
    },
    /// Only visible to its creator.
    Personal,
    /// Scoped to a specific Throne program.
    Program {
        /// The program name (e.g., "Studio", "Abacus").
        program_name: String,
    },
}

// ---------------------------------------------------------------------------
// Objective
// ---------------------------------------------------------------------------

/// A single step within a mission.
///
/// Objectives track progress toward a numeric target on a named metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    /// Unique identifier within the mission.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// How many to count (e.g., 1 = "do it once", 10 = "do it ten times").
    pub target: u64,
    /// What to count (e.g., "posts_created", "votes_cast").
    pub metric: String,
    /// Sequence order. 0 means any order is fine.
    pub order: u32,
    /// Whether this objective is a bonus (not required for completion).
    pub optional: bool,
}

impl Objective {
    /// Create a new required objective.
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        target: u64,
        metric: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            target,
            metric: metric.into(),
            order: 0,
            optional: false,
        }
    }

    /// Set the sequence order (0 = any order).
    pub fn with_order(mut self, order: u32) -> Self {
        self.order = order;
        self
    }

    /// Mark this objective as optional (bonus).
    pub fn as_optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

// ---------------------------------------------------------------------------
// MissionProgress
// ---------------------------------------------------------------------------

/// A participant's progress on a specific mission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionProgress {
    /// Which mission this tracks.
    pub mission_id: Uuid,
    /// The participant's public key.
    pub actor: String,
    /// Current status.
    pub status: MissionStatus,
    /// Per-objective progress: objective_id -> current count.
    pub objective_progress: HashMap<String, u64>,
    /// When the mission was started.
    pub started_at: Option<DateTime<Utc>>,
    /// When the mission was completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// How many times this mission has been attempted.
    pub attempts: u32,
}

impl MissionProgress {
    /// Create new progress for an actor starting a mission.
    pub fn new(mission_id: Uuid, actor: impl Into<String>) -> Self {
        Self {
            mission_id,
            actor: actor.into(),
            status: MissionStatus::Active,
            objective_progress: HashMap::new(),
            started_at: Some(Utc::now()),
            completed_at: None,
            attempts: 1,
        }
    }
}

/// The state of a participant's mission attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissionStatus {
    /// Mission is available but not started.
    Available,
    /// Mission is in progress.
    Active,
    /// All required objectives have been met.
    Completed,
    /// Time limit reached. No penalty -- can restart.
    Expired,
    /// Participant chose to stop. No penalty.
    Abandoned,
}

// ---------------------------------------------------------------------------
// MissionEngine
// ---------------------------------------------------------------------------

/// Manages missions and participant progress.
///
/// The engine is the central coordination point: it stores mission definitions,
/// tracks per-actor progress, and enforces rules (prerequisites, cooldowns,
/// time limits).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MissionEngine {
    missions: Vec<Mission>,
    /// actor -> their progress records (one per mission attempt).
    progress: HashMap<String, Vec<MissionProgress>>,
}

impl MissionEngine {
    /// Create an empty mission engine.
    pub fn new() -> Self {
        Self {
            missions: Vec::new(),
            progress: HashMap::new(),
        }
    }

    /// Register a new mission definition.
    pub fn add_mission(&mut self, mission: Mission) {
        self.missions.push(mission);
    }

    /// Remove a mission by ID. Returns `true` if found and removed.
    pub fn remove_mission(&mut self, mission_id: Uuid) -> bool {
        let before = self.missions.len();
        self.missions.retain(|m| m.id != mission_id);
        self.missions.len() < before
    }

    /// Look up a mission by ID.
    pub fn get_mission(&self, mission_id: Uuid) -> Option<&Mission> {
        self.missions.iter().find(|m| m.id == mission_id)
    }

    /// Start a mission for a participant.
    ///
    /// Checks prerequisites and cooldown. Returns the new progress record.
    pub fn start_mission(
        &mut self,
        actor: impl Into<String>,
        mission_id: Uuid,
    ) -> Result<MissionProgress, QuestError> {
        let actor = actor.into();

        let mission = self
            .get_mission(mission_id)
            .ok_or_else(|| QuestError::NotFound(format!("mission {mission_id}")))?
            .clone();

        // Check prerequisites
        for prereq in &mission.prerequisites {
            let completed = self
                .completed_by(&actor)
                .iter()
                .any(|p| p.mission_id.to_string() == *prereq);
            if !completed {
                return Err(QuestError::NotEligible(format!(
                    "prerequisite mission {prereq} not completed"
                )));
            }
        }

        // Check if already active
        let actor_progress = self.progress.entry(actor.clone()).or_default();
        let already_active = actor_progress
            .iter()
            .any(|p| p.mission_id == mission_id && p.status == MissionStatus::Active);
        if already_active {
            return Err(QuestError::InvalidState(
                "mission already active".into(),
            ));
        }

        // Check cooldown for repeatable missions
        if mission.repeatable {
            if let Some(cooldown) = mission.cooldown {
                let last_completed = actor_progress
                    .iter()
                    .filter(|p| p.mission_id == mission_id && p.status == MissionStatus::Completed)
                    .filter_map(|p| p.completed_at)
                    .max();
                if let Some(last) = last_completed {
                    let elapsed = Utc::now() - last;
                    if elapsed < cooldown {
                        let remaining = (cooldown - elapsed).num_seconds().max(0) as u64;
                        return Err(QuestError::CooldownActive {
                            remaining_seconds: remaining,
                        });
                    }
                }
            }
        } else {
            // Non-repeatable: check if already completed
            let already_completed = actor_progress
                .iter()
                .any(|p| p.mission_id == mission_id && p.status == MissionStatus::Completed);
            if already_completed {
                return Err(QuestError::InvalidState(
                    "mission already completed and is not repeatable".into(),
                ));
            }
        }

        let progress = MissionProgress::new(mission_id, &actor);
        actor_progress.push(progress.clone());
        Ok(progress)
    }

    /// Update progress on a specific objective within a mission.
    ///
    /// Increments the objective's counter by `increment`. If all required objectives
    /// are now met, the mission status transitions to `Completed`.
    pub fn update_progress(
        &mut self,
        actor: &str,
        mission_id: Uuid,
        objective_id: &str,
        increment: u64,
    ) -> Result<MissionProgress, QuestError> {
        let mission = self
            .get_mission(mission_id)
            .ok_or_else(|| QuestError::NotFound(format!("mission {mission_id}")))?
            .clone();

        // Validate objective exists
        if !mission.objectives.iter().any(|o| o.id == objective_id) {
            return Err(QuestError::NotFound(format!(
                "objective {objective_id} in mission {mission_id}"
            )));
        }

        let actor_progress = self
            .progress
            .get_mut(actor)
            .ok_or_else(|| QuestError::NotFound(format!("no progress for actor {actor}")))?;

        let progress = actor_progress
            .iter_mut()
            .find(|p| p.mission_id == mission_id && p.status == MissionStatus::Active)
            .ok_or_else(|| {
                QuestError::InvalidState(format!(
                    "no active progress for mission {mission_id}"
                ))
            })?;

        // Check time limit
        if let (Some(limit), Some(started)) = (mission.time_limit, progress.started_at) {
            if Utc::now() - started > limit {
                progress.status = MissionStatus::Expired;
                return Ok(progress.clone());
            }
        }

        // Increment objective
        let counter = progress
            .objective_progress
            .entry(objective_id.to_string())
            .or_insert(0);
        *counter = counter.saturating_add(increment);

        // Check completion
        if mission.is_complete(&progress.objective_progress) {
            progress.status = MissionStatus::Completed;
            progress.completed_at = Some(Utc::now());
        }

        Ok(progress.clone())
    }

    /// Mark a mission as completed manually (e.g., for system-granted completions).
    pub fn complete_mission(
        &mut self,
        actor: &str,
        mission_id: Uuid,
    ) -> Result<MissionProgress, QuestError> {
        let actor_progress = self
            .progress
            .get_mut(actor)
            .ok_or_else(|| QuestError::NotFound(format!("no progress for actor {actor}")))?;

        let progress = actor_progress
            .iter_mut()
            .find(|p| p.mission_id == mission_id && p.status == MissionStatus::Active)
            .ok_or_else(|| {
                QuestError::InvalidState(format!(
                    "no active progress for mission {mission_id}"
                ))
            })?;

        progress.status = MissionStatus::Completed;
        progress.completed_at = Some(Utc::now());
        Ok(progress.clone())
    }

    /// Abandon a mission. No penalty -- the participant simply chose to stop.
    pub fn abandon_mission(
        &mut self,
        actor: &str,
        mission_id: Uuid,
    ) -> Result<(), QuestError> {
        let actor_progress = self
            .progress
            .get_mut(actor)
            .ok_or_else(|| QuestError::NotFound(format!("no progress for actor {actor}")))?;

        let progress = actor_progress
            .iter_mut()
            .find(|p| p.mission_id == mission_id && p.status == MissionStatus::Active)
            .ok_or_else(|| {
                QuestError::InvalidState(format!(
                    "no active progress for mission {mission_id}"
                ))
            })?;

        progress.status = MissionStatus::Abandoned;
        Ok(())
    }

    /// Missions available to an actor (not already active or completed non-repeatable).
    pub fn available_for(&self, actor: &str) -> Vec<&Mission> {
        let actor_progress = self.progress.get(actor);
        self.missions
            .iter()
            .filter(|m| {
                // Check prerequisites
                let prereqs_met = m.prerequisites.iter().all(|prereq| {
                    actor_progress
                        .map(|progress| {
                            progress.iter().any(|p| {
                                p.mission_id.to_string() == *prereq
                                    && p.status == MissionStatus::Completed
                            })
                        })
                        .unwrap_or(false)
                });
                if !prereqs_met && !m.prerequisites.is_empty() {
                    return false;
                }

                // Check not already active
                let already_active = actor_progress
                    .map(|progress| {
                        progress
                            .iter()
                            .any(|p| p.mission_id == m.id && p.status == MissionStatus::Active)
                    })
                    .unwrap_or(false);
                if already_active {
                    return false;
                }

                // Check not already completed (for non-repeatable)
                if !m.repeatable {
                    let already_completed = actor_progress
                        .map(|progress| {
                            progress.iter().any(|p| {
                                p.mission_id == m.id && p.status == MissionStatus::Completed
                            })
                        })
                        .unwrap_or(false);
                    if already_completed {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Missions currently in progress for an actor.
    pub fn active_for(&self, actor: &str) -> Vec<&MissionProgress> {
        self.progress
            .get(actor)
            .map(|progress| {
                progress
                    .iter()
                    .filter(|p| p.status == MissionStatus::Active)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Missions completed by an actor.
    pub fn completed_by(&self, actor: &str) -> Vec<&MissionProgress> {
        self.progress
            .get(actor)
            .map(|progress| {
                progress
                    .iter()
                    .filter(|p| p.status == MissionStatus::Completed)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// All missions in a given category.
    pub fn by_category(&self, category: &MissionCategory) -> Vec<&Mission> {
        self.missions
            .iter()
            .filter(|m| &m.category == category)
            .collect()
    }

    /// All missions in a given scope.
    pub fn by_scope(&self, scope: &MissionScope) -> Vec<&Mission> {
        self.missions
            .iter()
            .filter(|m| &m.scope == scope)
            .collect()
    }

    /// Total number of registered missions.
    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    /// All mission definitions.
    pub fn all_missions(&self) -> &[Mission] {
        &self.missions
    }
}

// ---------------------------------------------------------------------------
// AdaptiveDifficulty
// ---------------------------------------------------------------------------

/// Tracks an actor's completion patterns and suggests appropriate difficulty.
///
/// Keeps participants in a flow state by suggesting harder missions when they're
/// breezing through everything, and easier ones when they're struggling.
/// Uses completion rate and average completion time as signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveDifficulty {
    /// The participant's public key.
    pub actor: String,
    /// Fraction of recent missions completed (0.0 to 1.0).
    pub completion_rate: f64,
    /// Average time to complete missions, if known.
    pub average_completion_time: Option<Duration>,
    /// The difficulty we'd recommend for their next mission.
    pub suggested_difficulty: Difficulty,
    /// Total missions completed (for rate calculation).
    completed_count: u32,
    /// Total missions attempted (for rate calculation).
    attempted_count: u32,
    /// Sum of completion durations in seconds (for average calculation).
    total_completion_secs: u64,
}

impl AdaptiveDifficulty {
    /// Create a new adaptive difficulty tracker starting at Normal.
    pub fn new(actor: impl Into<String>) -> Self {
        Self {
            actor: actor.into(),
            completion_rate: 0.0,
            average_completion_time: None,
            suggested_difficulty: Difficulty::Normal,
            completed_count: 0,
            attempted_count: 0,
            total_completion_secs: 0,
        }
    }

    /// Record a mission outcome and recalculate suggestions.
    ///
    /// - `completed`: whether the mission was completed (vs. abandoned/expired).
    /// - `time_taken`: how long the mission took, if completed.
    pub fn update(&mut self, completed: bool, time_taken: Option<Duration>) {
        self.attempted_count += 1;
        if completed {
            self.completed_count += 1;
            if let Some(duration) = time_taken {
                self.total_completion_secs =
                    self.total_completion_secs.saturating_add(duration.num_seconds().max(0) as u64);
                self.average_completion_time = Some(Duration::seconds(
                    (self.total_completion_secs / u64::from(self.completed_count)) as i64,
                ));
            }
        }

        self.completion_rate = if self.attempted_count > 0 {
            f64::from(self.completed_count) / f64::from(self.attempted_count)
        } else {
            0.0
        };

        self.suggested_difficulty = self.suggest();
    }

    /// Suggest a difficulty based on current completion patterns.
    ///
    /// - Above 80% completion: suggest harder.
    /// - Below 40% completion: suggest easier.
    /// - Between: stay where you are.
    pub fn suggest(&self) -> Difficulty {
        if self.completion_rate > 0.8 {
            self.suggested_difficulty.harder()
        } else if self.completion_rate < 0.4 {
            self.suggested_difficulty.easier()
        } else {
            self.suggested_difficulty
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mission() -> Mission {
        Mission::new("Test Mission", "A test mission")
            .with_category(MissionCategory::Onboarding)
            .with_difficulty(Difficulty::Gentle)
            .with_objective(Objective::new("obj-1", "Do the thing", 1, "things_done"))
            .with_xp_reward(100)
    }

    fn sample_mission_with_id(id: Uuid) -> Mission {
        let mut m = sample_mission();
        m.id = id;
        m
    }

    // --- Mission construction ---

    #[test]
    fn mission_new() {
        let m = Mission::new("Hello", "World");
        assert_eq!(m.name, "Hello");
        assert_eq!(m.description, "World");
        assert_eq!(m.category, MissionCategory::Personal);
        assert_eq!(m.difficulty, Difficulty::Normal);
        assert!(m.objectives.is_empty());
        assert!(m.rewards.is_empty());
        assert!(m.time_limit.is_none());
        assert!(!m.repeatable);
        assert!(m.cooldown.is_none());
        assert!(m.prerequisites.is_empty());
        assert_eq!(m.xp_reward, 0);
    }

    #[test]
    fn mission_builder() {
        let m = Mission::new("Build", "A thing")
            .with_category(MissionCategory::Daily)
            .with_difficulty(Difficulty::Heroic)
            .with_objective(Objective::new("a", "Do A", 5, "metric_a"))
            .with_reward(RewardType::Cool(50))
            .with_time_limit(Duration::hours(24))
            .with_repeatable(Some(Duration::hours(12)))
            .with_prerequisite("prereq-1")
            .with_creator(MissionCreator::System)
            .with_scope(MissionScope::Global)
            .with_xp_reward(200);

        assert_eq!(m.category, MissionCategory::Daily);
        assert_eq!(m.difficulty, Difficulty::Heroic);
        assert_eq!(m.objectives.len(), 1);
        assert_eq!(m.rewards.len(), 1);
        assert!(m.time_limit.is_some());
        assert!(m.repeatable);
        assert!(m.cooldown.is_some());
        assert_eq!(m.prerequisites.len(), 1);
        assert_eq!(m.xp_reward, 200);
    }

    #[test]
    fn mission_is_complete_all_met() {
        let m = sample_mission();
        let mut progress = HashMap::new();
        progress.insert("obj-1".to_string(), 1);
        assert!(m.is_complete(&progress));
    }

    #[test]
    fn mission_is_complete_not_met() {
        let m = sample_mission();
        let progress = HashMap::new();
        assert!(!m.is_complete(&progress));
    }

    #[test]
    fn mission_is_complete_optional_not_required() {
        let m = Mission::new("Test", "desc")
            .with_objective(Objective::new("req", "Required", 1, "m"))
            .with_objective(Objective::new("opt", "Optional", 1, "m").as_optional());

        let mut progress = HashMap::new();
        progress.insert("req".to_string(), 1);
        // Optional not done, but mission is still complete
        assert!(m.is_complete(&progress));
    }

    #[test]
    fn mission_required_objective_count() {
        let m = Mission::new("Test", "desc")
            .with_objective(Objective::new("a", "A", 1, "m"))
            .with_objective(Objective::new("b", "B", 1, "m").as_optional())
            .with_objective(Objective::new("c", "C", 1, "m"));

        assert_eq!(m.required_objective_count(), 2);
    }

    #[test]
    fn mission_serde_round_trip() {
        let m = sample_mission();
        let json = serde_json::to_string(&m).unwrap();
        let restored: Mission = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, m.id);
        assert_eq!(restored.name, "Test Mission");
        assert_eq!(restored.objectives.len(), 1);
    }

    // --- MissionCategory ---

    #[test]
    fn mission_category_serde_round_trip() {
        let categories = vec![
            MissionCategory::Onboarding,
            MissionCategory::Daily,
            MissionCategory::Weekly,
            MissionCategory::Seasonal,
            MissionCategory::Personal,
            MissionCategory::Community,
            MissionCategory::Program("Studio".into()),
            MissionCategory::Discovery,
            MissionCategory::Custom("special".into()),
        ];
        for cat in &categories {
            let json = serde_json::to_string(cat).unwrap();
            let restored: MissionCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, cat);
        }
    }

    // --- MissionCreator ---

    #[test]
    fn mission_creator_variants() {
        let system = MissionCreator::System;
        let community = MissionCreator::Community {
            community_id: Uuid::new_v4(),
        };
        let personal = MissionCreator::Personal {
            actor: "cpub1alice".into(),
        };
        let consortia = MissionCreator::Consortia {
            consortia_id: Uuid::new_v4(),
        };

        // All should serialize
        for creator in [&system, &community, &personal, &consortia] {
            let json = serde_json::to_string(creator).unwrap();
            assert!(!json.is_empty());
        }
    }

    // --- MissionScope ---

    #[test]
    fn mission_scope_serde_round_trip() {
        let scopes = vec![
            MissionScope::Global,
            MissionScope::Community {
                community_id: Uuid::new_v4(),
            },
            MissionScope::Personal,
            MissionScope::Program {
                program_name: "Abacus".into(),
            },
        ];
        for scope in &scopes {
            let json = serde_json::to_string(scope).unwrap();
            let restored: MissionScope = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, scope);
        }
    }

    // --- Objective ---

    #[test]
    fn objective_new() {
        let o = Objective::new("obj-1", "Do something", 5, "things_done");
        assert_eq!(o.id, "obj-1");
        assert_eq!(o.target, 5);
        assert_eq!(o.metric, "things_done");
        assert_eq!(o.order, 0);
        assert!(!o.optional);
    }

    #[test]
    fn objective_optional_and_ordered() {
        let o = Objective::new("bonus", "Bonus task", 1, "bonus_metric")
            .with_order(3)
            .as_optional();
        assert_eq!(o.order, 3);
        assert!(o.optional);
    }

    #[test]
    fn objective_serde_round_trip() {
        let o = Objective::new("test", "Test", 10, "metric").with_order(2);
        let json = serde_json::to_string(&o).unwrap();
        let restored: Objective = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, o);
    }

    // --- MissionProgress ---

    #[test]
    fn mission_progress_new() {
        let id = Uuid::new_v4();
        let p = MissionProgress::new(id, "alice");
        assert_eq!(p.mission_id, id);
        assert_eq!(p.actor, "alice");
        assert_eq!(p.status, MissionStatus::Active);
        assert!(p.started_at.is_some());
        assert!(p.completed_at.is_none());
        assert_eq!(p.attempts, 1);
    }

    #[test]
    fn mission_progress_serde_round_trip() {
        let p = MissionProgress::new(Uuid::new_v4(), "bob");
        let json = serde_json::to_string(&p).unwrap();
        let restored: MissionProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.mission_id, p.mission_id);
        assert_eq!(restored.actor, "bob");
    }

    // --- MissionStatus ---

    #[test]
    fn mission_status_serde_round_trip() {
        let statuses = [
            MissionStatus::Available,
            MissionStatus::Active,
            MissionStatus::Completed,
            MissionStatus::Expired,
            MissionStatus::Abandoned,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let restored: MissionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, s);
        }
    }

    // --- MissionEngine ---

    #[test]
    fn engine_empty() {
        let engine = MissionEngine::new();
        assert_eq!(engine.mission_count(), 0);
    }

    #[test]
    fn engine_add_and_get_mission() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        assert_eq!(engine.mission_count(), 1);
        assert!(engine.get_mission(id).is_some());
    }

    #[test]
    fn engine_remove_mission() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        assert!(engine.remove_mission(id));
        assert_eq!(engine.mission_count(), 0);
        assert!(!engine.remove_mission(id)); // already removed
    }

    #[test]
    fn engine_start_mission() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);

        let progress = engine.start_mission("alice", id).unwrap();
        assert_eq!(progress.status, MissionStatus::Active);
        assert_eq!(engine.active_for("alice").len(), 1);
    }

    #[test]
    fn engine_start_mission_not_found() {
        let mut engine = MissionEngine::new();
        let result = engine.start_mission("alice", Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn engine_start_mission_already_active() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        let result = engine.start_mission("alice", id);
        assert!(result.is_err());
    }

    #[test]
    fn engine_start_non_repeatable_completed() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();
        engine.complete_mission("alice", id).unwrap();

        let result = engine.start_mission("alice", id);
        assert!(result.is_err());
    }

    #[test]
    fn engine_start_repeatable_after_completion() {
        let mut engine = MissionEngine::new();
        let m = sample_mission().with_repeatable(None);
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();
        engine.complete_mission("alice", id).unwrap();

        // Repeatable with no cooldown -- should succeed
        let result = engine.start_mission("alice", id);
        assert!(result.is_ok());
    }

    #[test]
    fn engine_prerequisites() {
        let mut engine = MissionEngine::new();
        let prereq_id = Uuid::new_v4();
        let prereq = sample_mission_with_id(prereq_id);
        let main = sample_mission().with_prerequisite(prereq_id.to_string());
        let main_id = main.id;

        engine.add_mission(prereq);
        engine.add_mission(main);

        // Can't start main without completing prereq
        let result = engine.start_mission("alice", main_id);
        assert!(result.is_err());

        // Complete the prereq
        engine.start_mission("alice", prereq_id).unwrap();
        engine.complete_mission("alice", prereq_id).unwrap();

        // Now it should work
        let result = engine.start_mission("alice", main_id);
        assert!(result.is_ok());
    }

    #[test]
    fn engine_update_progress() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        let p = engine.update_progress("alice", id, "obj-1", 1).unwrap();
        assert_eq!(p.status, MissionStatus::Completed);
        assert!(p.completed_at.is_some());
    }

    #[test]
    fn engine_update_progress_partial() {
        let mut engine = MissionEngine::new();
        let m = Mission::new("Multi", "Multiple objectives")
            .with_objective(Objective::new("a", "A", 3, "metric_a"))
            .with_objective(Objective::new("b", "B", 2, "metric_b"));
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        let p = engine.update_progress("alice", id, "a", 2).unwrap();
        assert_eq!(p.status, MissionStatus::Active);

        let p = engine.update_progress("alice", id, "a", 1).unwrap();
        assert_eq!(p.status, MissionStatus::Active); // b still not done

        let p = engine.update_progress("alice", id, "b", 2).unwrap();
        assert_eq!(p.status, MissionStatus::Completed);
    }

    #[test]
    fn engine_update_progress_invalid_objective() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        let result = engine.update_progress("alice", id, "nonexistent", 1);
        assert!(result.is_err());
    }

    #[test]
    fn engine_update_progress_no_active_mission() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);

        let result = engine.update_progress("alice", id, "obj-1", 1);
        assert!(result.is_err());
    }

    #[test]
    fn engine_complete_mission() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        let p = engine.complete_mission("alice", id).unwrap();
        assert_eq!(p.status, MissionStatus::Completed);
        assert!(p.completed_at.is_some());
    }

    #[test]
    fn engine_abandon_mission() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        engine.abandon_mission("alice", id).unwrap();
        assert!(engine.active_for("alice").is_empty());
    }

    #[test]
    fn engine_abandon_no_penalty() {
        // Abandoning should not affect ability to see available missions
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();
        engine.abandon_mission("alice", id).unwrap();

        // Should still be available (non-repeatable but abandoned, not completed)
        let available = engine.available_for("alice");
        assert_eq!(available.len(), 1);
    }

    #[test]
    fn engine_available_for_empty() {
        let engine = MissionEngine::new();
        assert!(engine.available_for("alice").is_empty());
    }

    #[test]
    fn engine_available_for_filters_active() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        assert!(engine.available_for("alice").is_empty());
    }

    #[test]
    fn engine_available_for_filters_completed_non_repeatable() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();
        engine.complete_mission("alice", id).unwrap();

        assert!(engine.available_for("alice").is_empty());
    }

    #[test]
    fn engine_available_for_shows_repeatable_completed() {
        let mut engine = MissionEngine::new();
        let m = sample_mission().with_repeatable(None);
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();
        engine.complete_mission("alice", id).unwrap();

        assert_eq!(engine.available_for("alice").len(), 1);
    }

    #[test]
    fn engine_completed_by() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();
        engine.complete_mission("alice", id).unwrap();

        assert_eq!(engine.completed_by("alice").len(), 1);
        assert!(engine.completed_by("bob").is_empty());
    }

    #[test]
    fn engine_by_category() {
        let mut engine = MissionEngine::new();
        engine.add_mission(sample_mission()); // Onboarding
        engine.add_mission(
            Mission::new("Daily", "A daily")
                .with_category(MissionCategory::Daily),
        );

        assert_eq!(engine.by_category(&MissionCategory::Onboarding).len(), 1);
        assert_eq!(engine.by_category(&MissionCategory::Daily).len(), 1);
        assert!(engine.by_category(&MissionCategory::Seasonal).is_empty());
    }

    #[test]
    fn engine_by_scope() {
        let mut engine = MissionEngine::new();
        engine.add_mission(sample_mission()); // Global
        engine.add_mission(
            Mission::new("Local", "Community-scoped")
                .with_scope(MissionScope::Personal),
        );

        assert_eq!(engine.by_scope(&MissionScope::Global).len(), 1);
        assert_eq!(engine.by_scope(&MissionScope::Personal).len(), 1);
    }

    #[test]
    fn engine_serde_round_trip() {
        let mut engine = MissionEngine::new();
        engine.add_mission(sample_mission());
        let json = serde_json::to_string(&engine).unwrap();
        let restored: MissionEngine = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.mission_count(), 1);
    }

    #[test]
    fn engine_multiple_actors() {
        let mut engine = MissionEngine::new();
        let m = sample_mission();
        let id = m.id;
        engine.add_mission(m);

        engine.start_mission("alice", id).unwrap();
        engine.start_mission("bob", id).unwrap();

        assert_eq!(engine.active_for("alice").len(), 1);
        assert_eq!(engine.active_for("bob").len(), 1);
    }

    // --- AdaptiveDifficulty ---

    #[test]
    fn adaptive_difficulty_new() {
        let ad = AdaptiveDifficulty::new("alice");
        assert_eq!(ad.actor, "alice");
        assert_eq!(ad.completion_rate, 0.0);
        assert!(ad.average_completion_time.is_none());
        assert_eq!(ad.suggested_difficulty, Difficulty::Normal);
    }

    #[test]
    fn adaptive_difficulty_high_completion_increases() {
        let mut ad = AdaptiveDifficulty::new("alice");
        // Complete 9 out of 10
        for _ in 0..9 {
            ad.update(true, Some(Duration::minutes(5)));
        }
        ad.update(false, None);
        // 90% completion rate -> should suggest harder
        assert!(ad.completion_rate >= 0.8);
        assert_ne!(ad.suggested_difficulty, Difficulty::Gentle);
    }

    #[test]
    fn adaptive_difficulty_low_completion_decreases() {
        let mut ad = AdaptiveDifficulty::new("alice");
        // Complete 1 out of 10
        ad.update(true, Some(Duration::minutes(5)));
        for _ in 0..9 {
            ad.update(false, None);
        }
        // 10% completion rate -> should suggest easier
        assert!(ad.completion_rate < 0.4);
    }

    #[test]
    fn adaptive_difficulty_average_time() {
        let mut ad = AdaptiveDifficulty::new("alice");
        ad.update(true, Some(Duration::minutes(10)));
        ad.update(true, Some(Duration::minutes(20)));
        // Average should be 15 minutes = 900 seconds
        let avg = ad.average_completion_time.unwrap();
        assert_eq!(avg.num_seconds(), 900);
    }

    #[test]
    fn adaptive_difficulty_no_time_for_failures() {
        let mut ad = AdaptiveDifficulty::new("alice");
        ad.update(false, None);
        ad.update(false, None);
        assert!(ad.average_completion_time.is_none());
    }

    #[test]
    fn adaptive_difficulty_suggest() {
        let mut ad = AdaptiveDifficulty::new("alice");
        // Before any data, completion_rate is 0.0 which triggers "suggest easier"
        // (0.0 < 0.4). Starting at Normal, easier = Gentle.
        assert_eq!(ad.suggest(), Difficulty::Gentle);

        // Simulate perfect completion -- should push difficulty up
        for _ in 0..5 {
            ad.update(true, None);
        }
        // 100% completion rate -> suggested_difficulty has been ratcheted up
        assert!(ad.completion_rate > 0.8);
        // Should not be at the bottom
        assert_ne!(ad.suggested_difficulty, Difficulty::Gentle);
    }

    #[test]
    fn adaptive_difficulty_serde_round_trip() {
        let mut ad = AdaptiveDifficulty::new("alice");
        ad.update(true, Some(Duration::minutes(5)));
        let json = serde_json::to_string(&ad).unwrap();
        let restored: AdaptiveDifficulty = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.actor, "alice");
        assert_eq!(restored.completion_rate, ad.completion_rate);
    }

    // --- No dark patterns verification ---

    #[test]
    fn expired_mission_no_penalty() {
        // Expired missions should allow restart
        let mut engine = MissionEngine::new();
        let m = sample_mission()
            .with_time_limit(Duration::seconds(0)) // expires immediately
            .with_repeatable(None);
        let id = m.id;
        engine.add_mission(m);
        engine.start_mission("alice", id).unwrap();

        // Update should detect expiration
        let p = engine.update_progress("alice", id, "obj-1", 1).unwrap();
        assert_eq!(p.status, MissionStatus::Expired);

        // Should be able to start again (repeatable)
        let result = engine.start_mission("alice", id);
        assert!(result.is_ok());
    }

    #[test]
    fn no_skip_penalty_daily_mission() {
        // Daily missions should be available regardless of history
        let mut engine = MissionEngine::new();
        let m = Mission::new("Daily", "Do daily thing")
            .with_category(MissionCategory::Daily)
            .with_repeatable(None)
            .with_objective(Objective::new("x", "X", 1, "m"));
        let id = m.id;
        engine.add_mission(m);

        // Skip it entirely -- no penalty
        // It should remain available
        assert_eq!(engine.available_for("alice").len(), 1);

        // Complete it
        engine.start_mission("alice", id).unwrap();
        engine.complete_mission("alice", id).unwrap();

        // Still available because it's repeatable
        assert_eq!(engine.available_for("alice").len(), 1);
    }
}
