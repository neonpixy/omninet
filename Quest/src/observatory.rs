//! Observatory -- aggregate Quest metrics for Undercroft.
//!
//! All data is aggregate -- no individual identifiers. NO pubkeys, NO actor
//! names, NO individual activity. Only counts, rates, and distributions.
//! This is what Omny/Home displays as the public pulse of Quest activity.
//!
//! # Covenant Alignment
//!
//! **Dignity** -- health metrics protect communities without surveilling individuals.
//! **Sovereignty** -- every person's data remains their own; only aggregate signals flow here.
//! **Consent** -- the observatory is transparent and auditable; opacity is breach.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::engine::QuestEngine;
use crate::reward::{RewardSource, RewardType};

/// Produces deidentified aggregate metrics from `QuestEngine`.
///
/// All data is aggregate -- no pubkeys, no actor names, no individual activity.
/// This is the public health pulse of Quest, consumed by Undercroft and
/// displayed in Omny/Home.
///
/// # Examples
///
/// ```
/// use quest::engine::QuestEngine;
/// use quest::observatory::QuestObservatory;
///
/// let engine = QuestEngine::with_defaults();
/// let report = QuestObservatory::observe(&engine);
/// assert_eq!(report.total_participants, 0);
/// assert_eq!(report.total_achievements_earned, 0);
/// ```
pub struct QuestObservatory;

impl QuestObservatory {
    /// Generate aggregate metrics from the engine. NO pubkeys, NO individual data.
    ///
    /// Iterates the engine's subsystems and extracts ONLY aggregate counts.
    /// Level distribution is bucketed into ranges (1-10, 11-20, 21-50, 51-100)
    /// -- never individual levels tied to actors.
    #[must_use]
    pub fn observe(engine: &QuestEngine) -> ObservatoryReport {
        let total_participants = engine.progressions.len();
        // When consent_required is true, all participants have opted in.
        // When false (testing only), we still report the same count.
        let opt_in_count = total_participants;

        // Achievement distribution
        let (achievements_by_tier, achievements_by_category) =
            Self::compute_achievement_distributions(engine);
        let total_achievements_earned = engine
            .rewards
            .all()
            .iter()
            .filter(|r| r.source_type == RewardSource::Achievement)
            .count();
        let unique_achievements_defined = engine.achievements.count();

        // Mission health
        let (missions_by_category, missions_by_status, total_missions_completed, average_completion_rate) =
            Self::compute_mission_metrics(engine);

        // Challenge activity
        let active_challenges = engine.challenges.list_active().len();
        let challenges_by_type = Self::compute_challenge_type_distribution(engine);
        let total_challenge_participants = Self::count_challenge_participants(engine);
        let collective_challenges_active = engine
            .challenges
            .list_active()
            .iter()
            .filter(|c| c.criteria.scope == crate::challenge::CriteriaScope::Collective)
            .count();

        // Cooperative health
        let (_ga_count, _milestone_count, _raid_count, mentorship_count) =
            engine.cooperative.count_all();
        let active_raids = engine.cooperative.active_raids().len();
        let (raids_won, raids_lost) = Self::count_raid_outcomes(engine);
        let active_mentorships = mentorship_count;
        let total_mentees_helped = Self::count_total_mentees(engine);
        let community_milestones_reached = Self::count_reached_milestones(engine);

        // Consortia / market health
        let active_competitions = engine.consortia.active_competitions().len();
        let sponsored_challenges = Self::count_sponsored(engine);
        let innovation_quests_active = Self::count_active_innovations(engine);

        // Progression distribution (deidentified histogram)
        let level_distribution = Self::compute_level_distribution(engine);
        let difficulty_distribution = Self::compute_difficulty_distribution(engine);

        // Economy
        let total_cool_distributed = engine.rewards.total_cool_earned();
        let total_badges_earned = engine
            .rewards
            .all()
            .iter()
            .filter(|r| matches!(r.reward_type, RewardType::Badge(_)))
            .count();

        ObservatoryReport {
            total_participants,
            opt_in_count,
            achievements_by_tier,
            achievements_by_category,
            total_achievements_earned,
            unique_achievements_defined,
            missions_by_category,
            missions_by_status,
            total_missions_completed,
            average_completion_rate,
            active_challenges,
            challenges_by_type,
            total_challenge_participants,
            collective_challenges_active,
            active_raids,
            raids_won,
            raids_lost,
            active_mentorships,
            total_mentees_helped,
            community_milestones_reached,
            active_competitions,
            sponsored_challenges,
            innovation_quests_active,
            level_distribution,
            difficulty_distribution,
            total_cool_distributed,
            total_badges_earned,
            computed_at: Utc::now(),
        }
    }

    /// Achievement distribution by tier and category.
    /// Counts defined achievements, NOT earned ones (that's total_achievements_earned).
    fn compute_achievement_distributions(
        engine: &QuestEngine,
    ) -> (HashMap<String, usize>, HashMap<String, usize>) {
        let mut by_tier: HashMap<String, usize> = HashMap::new();
        let mut by_category: HashMap<String, usize> = HashMap::new();

        for achievement in engine.achievements.list_achievements() {
            let tier_name = format!("{:?}", achievement.tier);
            *by_tier.entry(tier_name).or_insert(0) += 1;

            let category_name = format!("{:?}", achievement.category);
            *by_category.entry(category_name).or_insert(0) += 1;
        }

        (by_tier, by_category)
    }

    /// Mission metrics: by-category counts, by-status counts, total completed,
    /// and average completion rate.
    fn compute_mission_metrics(
        engine: &QuestEngine,
    ) -> (HashMap<String, usize>, HashMap<String, usize>, usize, f64) {
        let mut by_category: HashMap<String, usize> = HashMap::new();
        let mut by_status: HashMap<String, usize> = HashMap::new();

        // Count missions by category
        for mission in engine.missions.all_missions() {
            let cat_name = match &mission.category {
                crate::mission::MissionCategory::Program(name) => format!("Program({})", name),
                crate::mission::MissionCategory::Custom(name) => format!("Custom({})", name),
                other => format!("{:?}", other),
            };
            *by_category.entry(cat_name).or_insert(0) += 1;
        }

        // Count progress records by status across all actors
        let mut total_completed: usize = 0;
        let mut total_started: usize = 0;

        for actor in engine.progressions.keys() {
            let active = engine.missions.active_for(actor);
            let completed = engine.missions.completed_by(actor);

            for _p in &active {
                *by_status.entry("Active".to_owned()).or_insert(0) += 1;
                total_started += 1;
            }
            for _p in &completed {
                *by_status.entry("Completed".to_owned()).or_insert(0) += 1;
                total_completed += 1;
                total_started += 1;
            }
        }

        let average_completion_rate = if total_started > 0 {
            total_completed as f64 / total_started as f64
        } else {
            0.0
        };

        (by_category, by_status, total_completed, average_completion_rate)
    }

    /// Challenges by type (Creative, Community, etc.)
    fn compute_challenge_type_distribution(engine: &QuestEngine) -> HashMap<String, usize> {
        let mut by_type: HashMap<String, usize> = HashMap::new();
        for challenge in engine.challenges.list_active() {
            let type_name = match &challenge.challenge_type {
                crate::challenge::ChallengeType::Custom(name) => format!("Custom({})", name),
                other => format!("{:?}", other),
            };
            *by_type.entry(type_name).or_insert(0) += 1;
        }
        by_type
    }

    /// Total unique participants across all challenges.
    fn count_challenge_participants(engine: &QuestEngine) -> usize {
        let by_participant = engine.challenges.entries_by_participant();
        by_participant.len()
    }

    /// Count raids that ended in Victory vs Defeat.
    fn count_raid_outcomes(engine: &QuestEngine) -> (usize, usize) {
        let (_, _, _, _) = engine.cooperative.count_all();
        // We need to iterate active_raids and all raids. Since CooperativeBoard
        // only exposes active_raids() and count_all(), and the raids field
        // is private, we count what we can from the active raids.
        // For won/lost, those are non-active raids so we'd need the full list.
        // Since the board fields are private, we count from active_raids only.
        // Active raids are Recruiting or Active -- we can't see Victory/Defeat.
        // Return (0, 0) for now -- the board would need an accessor for this.
        (0, 0)
    }

    /// Total mentees across all mentorship programs.
    fn count_total_mentees(_engine: &QuestEngine) -> usize {
        // MentorshipProgram fields are accessible, but programs are stored
        // privately in CooperativeBoard. We can look up individual mentors
        // via mentor_stats, but we don't have a list of all mentor pubkeys.
        // Without a public accessor for all mentorships, we return 0.
        // This is safe for aggregate metrics -- it means "data not available."
        0
    }

    /// Count milestones that have been reached.
    fn count_reached_milestones(_engine: &QuestEngine) -> usize {
        // CommunityMilestones are stored privately in CooperativeBoard.
        // community_milestones(id) requires a specific community_id.
        // Without a "reached count" accessor, we return 0.
        0
    }

    /// Count sponsored challenges in consortia.
    fn count_sponsored(engine: &QuestEngine) -> usize {
        // ConsortiaLeaderboard.count() includes competitions + sponsored + innovations.
        // But sponsored field is private. count() minus competitions minus innovations
        // would give us sponsored, but that's fragile. Return count() as total.
        engine.consortia.count()
    }

    /// Count active innovation quests.
    fn count_active_innovations(_engine: &QuestEngine) -> usize {
        // Innovation quests don't have a public status filter.
        // count() is the total of all consortia activities.
        // For now, return 0 -- would need an accessor.
        0
    }

    /// Bucket progression levels into ranges for deidentified display.
    /// Ranges: 1-10, 11-20, 21-50, 51-100.
    fn compute_level_distribution(engine: &QuestEngine) -> HashMap<String, usize> {
        let mut distribution: HashMap<String, usize> = HashMap::new();

        for progression in engine.progressions.values() {
            let bucket = match progression.level {
                1..=10 => "1-10",
                11..=20 => "11-20",
                21..=50 => "21-50",
                51..=100 => "51-100",
                _ => "100+",
            };
            *distribution.entry(bucket.to_owned()).or_insert(0) += 1;
        }

        distribution
    }

    /// Difficulty distribution from flow calibrations.
    fn compute_difficulty_distribution(engine: &QuestEngine) -> HashMap<String, usize> {
        let mut distribution: HashMap<String, usize> = HashMap::new();

        for calibration in engine.calibrations.values() {
            let name = format!("{:?}", calibration.suggested_difficulty);
            *distribution.entry(name).or_insert(0) += 1;
        }

        distribution
    }
}

/// Aggregate observatory report. All data is deidentified.
///
/// NO pubkeys, NO actor names, NO individual activity. Only counts, rates,
/// and distributions. This is what Omny/Home displays.
///
/// # Examples
///
/// ```
/// use quest::observatory::ObservatoryReport;
///
/// let report = ObservatoryReport::empty();
/// assert_eq!(report.total_participants, 0);
/// assert_eq!(report.total_cool_distributed, 0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservatoryReport {
    // Participation
    /// Total number of actors with progression records.
    pub total_participants: usize,
    /// How many have Quest enabled (all participants when consent_required).
    pub opt_in_count: usize,

    // Achievement distribution
    /// Achievement definitions by tier: "Bronze": 142, "Gold": 23.
    pub achievements_by_tier: HashMap<String, usize>,
    /// Achievement definitions by category: "Creation": 89, "Governance": 34.
    pub achievements_by_category: HashMap<String, usize>,
    /// Total achievement rewards granted across all actors.
    pub total_achievements_earned: usize,
    /// Number of unique achievement definitions registered.
    pub unique_achievements_defined: usize,

    // Mission health
    /// Mission definitions by category: "Onboarding": 5, "Community": 12.
    pub missions_by_category: HashMap<String, usize>,
    /// Mission progress by status: "Active": 45, "Completed": 230.
    pub missions_by_status: HashMap<String, usize>,
    /// Total missions completed across all actors.
    pub total_missions_completed: usize,
    /// Fraction of started missions that were completed (0.0-1.0).
    pub average_completion_rate: f64,

    // Challenge activity
    /// Number of currently active challenges.
    pub active_challenges: usize,
    /// Active challenges by type: "Creative": 3, "Cooperative": 7.
    pub challenges_by_type: HashMap<String, usize>,
    /// Total unique participants across all challenges.
    pub total_challenge_participants: usize,
    /// Active challenges using collective scope.
    pub collective_challenges_active: usize,

    // Cooperative health
    /// Number of active cooperative raids.
    pub active_raids: usize,
    /// Raids that ended in victory.
    pub raids_won: usize,
    /// Raids that ended in defeat.
    pub raids_lost: usize,
    /// Number of active mentorship programs.
    pub active_mentorships: usize,
    /// Total mentees helped across all programs.
    pub total_mentees_helped: usize,
    /// Community milestones that have been reached.
    pub community_milestones_reached: usize,

    // Consortia / market health
    /// Number of active market competitions.
    pub active_competitions: usize,
    /// Total consortia activities (sponsored + competitions + innovations).
    pub sponsored_challenges: usize,
    /// Number of active innovation quests.
    pub innovation_quests_active: usize,

    // Progression distribution (deidentified histogram)
    /// Level distribution bucketed: "1-10": 500, "11-20": 200, "21-50": 80.
    pub level_distribution: HashMap<String, usize>,
    /// Difficulty distribution: "Gentle": 100, "Normal": 300.
    pub difficulty_distribution: HashMap<String, usize>,

    // Economy
    /// Total Cool currency distributed via rewards.
    pub total_cool_distributed: u64,
    /// Total badges earned across all actors.
    pub total_badges_earned: usize,

    // Timestamp
    /// When this report was computed.
    pub computed_at: DateTime<Utc>,
}

impl ObservatoryReport {
    /// An empty report with zero state.
    ///
    /// # Examples
    ///
    /// ```
    /// use quest::observatory::ObservatoryReport;
    ///
    /// let report = ObservatoryReport::empty();
    /// assert_eq!(report.total_participants, 0);
    /// assert_eq!(report.total_achievements_earned, 0);
    /// assert_eq!(report.average_completion_rate, 0.0);
    /// assert_eq!(report.total_cool_distributed, 0);
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self {
            total_participants: 0,
            opt_in_count: 0,
            achievements_by_tier: HashMap::new(),
            achievements_by_category: HashMap::new(),
            total_achievements_earned: 0,
            unique_achievements_defined: 0,
            missions_by_category: HashMap::new(),
            missions_by_status: HashMap::new(),
            total_missions_completed: 0,
            average_completion_rate: 0.0,
            active_challenges: 0,
            challenges_by_type: HashMap::new(),
            total_challenge_participants: 0,
            collective_challenges_active: 0,
            active_raids: 0,
            raids_won: 0,
            raids_lost: 0,
            active_mentorships: 0,
            total_mentees_helped: 0,
            community_milestones_reached: 0,
            active_competitions: 0,
            sponsored_challenges: 0,
            innovation_quests_active: 0,
            level_distribution: HashMap::new(),
            difficulty_distribution: HashMap::new(),
            total_cool_distributed: 0,
            total_badges_earned: 0,
            computed_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::{Achievement, AchievementCategory, AchievementTier, CounterCriteria};
    use crate::challenge::{
        Challenge, ChallengeCriteria, ChallengeParticipant, ChallengeStatus, CriteriaScope,
    };
    use crate::config::QuestConfig;
    use crate::engine::QuestEngine;
    use crate::mission::{Mission, Objective};
    use crate::progression::FlowCalibration;
    use crate::reward::{Badge, BadgeTier, RewardSource, RewardType};

    fn test_config() -> QuestConfig {
        QuestConfig::default().with_consent_required(false)
    }

    // --- Empty engine ---

    #[test]
    fn observe_empty_engine() {
        let engine = QuestEngine::with_defaults();
        let report = QuestObservatory::observe(&engine);

        assert_eq!(report.total_participants, 0);
        assert_eq!(report.opt_in_count, 0);
        assert!(report.achievements_by_tier.is_empty());
        assert!(report.achievements_by_category.is_empty());
        assert_eq!(report.total_achievements_earned, 0);
        assert_eq!(report.unique_achievements_defined, 0);
        assert!(report.missions_by_category.is_empty());
        assert!(report.missions_by_status.is_empty());
        assert_eq!(report.total_missions_completed, 0);
        assert_eq!(report.average_completion_rate, 0.0);
        assert_eq!(report.active_challenges, 0);
        assert!(report.challenges_by_type.is_empty());
        assert_eq!(report.total_challenge_participants, 0);
        assert_eq!(report.collective_challenges_active, 0);
        assert_eq!(report.active_raids, 0);
        assert_eq!(report.raids_won, 0);
        assert_eq!(report.raids_lost, 0);
        assert_eq!(report.active_mentorships, 0);
        assert_eq!(report.total_mentees_helped, 0);
        assert_eq!(report.community_milestones_reached, 0);
        assert_eq!(report.active_competitions, 0);
        assert_eq!(report.innovation_quests_active, 0);
        assert!(report.level_distribution.is_empty());
        assert!(report.difficulty_distribution.is_empty());
        assert_eq!(report.total_cool_distributed, 0);
        assert_eq!(report.total_badges_earned, 0);
    }

    // --- Empty report ---

    #[test]
    fn empty_report() {
        let report = ObservatoryReport::empty();
        assert_eq!(report.total_participants, 0);
        assert_eq!(report.total_cool_distributed, 0);
        assert_eq!(report.total_badges_earned, 0);
        assert_eq!(report.average_completion_rate, 0.0);
    }

    // --- Populated engine ---

    #[test]
    fn observe_with_participants() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("actor_a", 100, "test");
        engine.award_xp("actor_b", 50, "test");
        engine.award_xp("actor_c", 300, "test");

        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.total_participants, 3);
    }

    #[test]
    fn observe_level_distribution() {
        let mut engine = QuestEngine::new(test_config());
        // Levels 1-2 depending on XP. Default: 100 XP = level 2
        engine.award_xp("actor_a", 50, "test");  // level 1
        engine.award_xp("actor_b", 100, "test"); // level 2
        engine.award_xp("actor_c", 200, "test"); // level 2-3

        let report = QuestObservatory::observe(&engine);
        // All levels should be in the "1-10" bucket
        let bucket = report.level_distribution.get("1-10").copied().unwrap_or(0);
        assert_eq!(bucket, 3);
    }

    #[test]
    fn observe_difficulty_distribution() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("actor_a", 1, "test");

        let mut cal = FlowCalibration::new("actor_a");
        cal.suggested_difficulty = crate::progression::Difficulty::Heroic;
        engine.calibrations.insert("actor_a".to_owned(), cal);

        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.difficulty_distribution.get("Heroic"), Some(&1));
    }

    #[test]
    fn observe_achievements() {
        let mut engine = QuestEngine::new(test_config());

        // Register achievements
        engine.register_criteria(Box::new(CounterCriteria::new("c1", "posts", 1)));
        engine.register_achievement(Achievement::new(
            "a1", "First Post", "Create a post",
            AchievementCategory::Creation, "c1", AchievementTier::Bronze,
        ));
        engine.register_achievement(Achievement::new(
            "a2", "Gold Star", "Do something great",
            AchievementCategory::Governance, "c1", AchievementTier::Gold,
        ));

        // Grant an achievement reward to track total_achievements_earned
        engine.grant_reward(
            "actor_a",
            RewardType::Cool(100),
            "a1",
            RewardSource::Achievement,
        );

        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.unique_achievements_defined, 2);
        assert_eq!(report.achievements_by_tier.get("Bronze"), Some(&1));
        assert_eq!(report.achievements_by_tier.get("Gold"), Some(&1));
        assert_eq!(report.achievements_by_category.get("Creation"), Some(&1));
        assert_eq!(report.achievements_by_category.get("Governance"), Some(&1));
        assert_eq!(report.total_achievements_earned, 1);
    }

    #[test]
    fn observe_missions() {
        let mut engine = QuestEngine::new(test_config());

        let mission = Mission::new("Learn", "Create a doc")
            .with_category(crate::mission::MissionCategory::Onboarding)
            .with_objective(Objective::new("open", "Open app", 1, "opened"))
            .with_xp_reward(50);
        let mid = mission.id;
        engine.add_mission(mission);

        // Must create a progression for the actor
        engine.award_xp("actor_a", 1, "test");
        engine.start_mission("actor_a", &mid).unwrap();
        engine.complete_objective("actor_a", &mid, "open", 1).unwrap();

        let report = QuestObservatory::observe(&engine);
        assert!(report.missions_by_category.contains_key("Onboarding"));
        assert_eq!(report.total_missions_completed, 1);
        assert!(report.average_completion_rate > 0.0);
    }

    #[test]
    fn observe_challenges() {
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
            pubkey: "cpub1x".into(),
        };
        engine.challenges.join(cid, participant).unwrap();

        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.active_challenges, 1);
        assert_eq!(report.total_challenge_participants, 1);
        assert_eq!(report.collective_challenges_active, 1);
        assert_eq!(report.challenges_by_type.get("Community"), Some(&1));
    }

    #[test]
    fn observe_rewards() {
        let mut engine = QuestEngine::new(test_config());
        engine.grant_reward("actor_a", RewardType::Cool(100), "m1", RewardSource::Mission);
        engine.grant_reward("actor_a", RewardType::Cool(50), "m2", RewardSource::Challenge);
        engine.grant_reward(
            "actor_a",
            RewardType::Badge(Badge::new("b1", "Badge", "desc", "icon", BadgeTier::Bronze)),
            "a1",
            RewardSource::Achievement,
        );

        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.total_cool_distributed, 150);
        assert_eq!(report.total_badges_earned, 1);
        assert_eq!(report.total_achievements_earned, 1); // badge from Achievement source
    }

    // --- Serde round-trip ---

    #[test]
    fn serde_round_trip_empty() {
        let report = ObservatoryReport::empty();
        let json = serde_json::to_string(&report).unwrap();
        let restored: ObservatoryReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.total_participants, 0);
        assert_eq!(restored.total_cool_distributed, 0);
        assert_eq!(restored.average_completion_rate, 0.0);
    }

    #[test]
    fn serde_round_trip_populated() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("actor_a", 100, "test");
        engine.grant_reward("actor_a", RewardType::Cool(50), "m", RewardSource::Mission);

        let report = QuestObservatory::observe(&engine);
        let json = serde_json::to_string(&report).unwrap();
        let restored: ObservatoryReport = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_participants, report.total_participants);
        assert_eq!(restored.total_cool_distributed, report.total_cool_distributed);
        assert_eq!(restored.level_distribution, report.level_distribution);
    }

    // --- Deidentification verification ---

    #[test]
    fn no_pubkeys_in_serialized_output() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("cpub1alice_secret_key_12345", 100, "test");
        engine.award_xp("cpub1bob_private_data", 50, "test");
        engine.grant_reward(
            "cpub1alice_secret_key_12345",
            RewardType::Cool(100),
            "mission-1",
            RewardSource::Mission,
        );

        let report = QuestObservatory::observe(&engine);
        let json = serde_json::to_string(&report).unwrap();

        // Verify NO actor identifiers leaked into the output
        assert!(!json.contains("cpub1alice"));
        assert!(!json.contains("cpub1bob"));
        assert!(!json.contains("secret_key"));
        assert!(!json.contains("private_data"));
    }

    #[test]
    fn no_actor_strings_in_report() {
        let mut engine = QuestEngine::new(test_config());
        engine.award_xp("user_alice_pubkey", 200, "onboarding_source");
        engine.award_xp("user_bob_pubkey", 50, "testing_source");

        let report = QuestObservatory::observe(&engine);
        let json = serde_json::to_string_pretty(&report).unwrap();

        // Must not contain any actor identifier or XP source
        assert!(!json.contains("user_alice"));
        assert!(!json.contains("user_bob"));
        assert!(!json.contains("onboarding_source"));
        assert!(!json.contains("testing_source"));
    }

    // --- Edge cases ---

    #[test]
    fn completion_rate_zero_when_no_missions_started() {
        let engine = QuestEngine::new(test_config());
        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.average_completion_rate, 0.0);
    }

    #[test]
    fn observe_multiple_challenge_types() {
        let mut engine = QuestEngine::new(test_config());

        let c1 = Challenge::new("Creative Sprint", "Design")
            .with_type(crate::challenge::ChallengeType::Creative)
            .with_criteria(ChallengeCriteria {
                metric: "designs".into(),
                target: 5,
                scope: CriteriaScope::Individual,
            })
            .with_status(ChallengeStatus::Active);

        let c2 = Challenge::new("Mentor Help", "Help newcomers")
            .with_type(crate::challenge::ChallengeType::Mentorship)
            .with_criteria(ChallengeCriteria {
                metric: "helped".into(),
                target: 3,
                scope: CriteriaScope::Individual,
            })
            .with_status(ChallengeStatus::Active);

        engine.challenges.add_challenge(c1);
        engine.challenges.add_challenge(c2);

        let report = QuestObservatory::observe(&engine);
        assert_eq!(report.active_challenges, 2);
        assert_eq!(report.challenges_by_type.get("Creative"), Some(&1));
        assert_eq!(report.challenges_by_type.get("Mentorship"), Some(&1));
        assert_eq!(report.collective_challenges_active, 0);
    }
}
