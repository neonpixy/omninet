//! Cooperative -- collaborative achievement systems.
//!
//! The heart of Quest's "cooperative > competitive" philosophy. Group achievements,
//! community milestones, cooperative raids, and mentorship programs reward lifting
//! others up.
//!
//! # Design Principles
//!
//! - **Together is better.** Group achievements, raids, and mentorship all reward
//!   collaboration, not individual dominance.
//! - **No punitive defeat.** A raid "Defeat" is "we'll get 'em next time," not shame.
//!   No lost progress, no penalties.
//! - **Mentorship earns XP.** The mentor earns bonus XP when their mentees achieve.
//!   Teaching is a first-class progression path.
//!
//! # Example
//!
//! ```
//! use quest::cooperative::{CooperativeBoard, GroupAchievement, Contributor, GroupAchievementStatus};
//! use uuid::Uuid;
//!
//! let mut board = CooperativeBoard::new();
//!
//! let achievement = GroupAchievement::new(
//!     Uuid::new_v4(),
//!     "Community Quilt",
//!     "Create 500 ideas together",
//!     500,
//!     "ideas_created",
//! ).with_xp_reward(1000);
//!
//! board.add_group_achievement(achievement);
//! let (achievements, milestones, raids, mentorships) = board.count_all();
//! assert_eq!(achievements, 1);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::QuestError;
use crate::progression::XpAmount;
use crate::reward::{Badge, RewardType};

// ---------------------------------------------------------------------------
// GroupAchievement
// ---------------------------------------------------------------------------

/// A shared achievement for a community.
///
/// Every member contributes toward a collective target. When the target is met,
/// everyone who contributed earns the rewards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAchievement {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// What this achievement is about.
    pub description: String,
    /// The community working toward this.
    pub community_id: Uuid,
    /// Numeric target to reach.
    pub target: u64,
    /// What to count (e.g., "ideas_created", "votes_cast").
    pub metric: String,
    /// Current collective progress.
    pub current: u64,
    /// Rewards granted to contributors on completion.
    pub rewards: Vec<RewardType>,
    /// XP awarded on completion.
    pub xp_reward: XpAmount,
    /// Individual contributions.
    pub contributors: Vec<Contributor>,
    /// Lifecycle state.
    pub status: GroupAchievementStatus,
    /// When this achievement was created.
    pub created_at: DateTime<Utc>,
    /// When this achievement was completed, if it has been.
    pub completed_at: Option<DateTime<Utc>>,
}

impl GroupAchievement {
    /// Create a new group achievement.
    pub fn new(
        community_id: Uuid,
        name: impl Into<String>,
        description: impl Into<String>,
        target: u64,
        metric: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            community_id,
            target,
            metric: metric.into(),
            current: 0,
            rewards: Vec::new(),
            xp_reward: 0,
            contributors: Vec::new(),
            status: GroupAchievementStatus::Active,
            created_at: Utc::now(),
            completed_at: None,
        }
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

    /// Record a contribution from a participant.
    ///
    /// Updates the contributor's running total or creates a new entry.
    /// Returns `true` if the achievement is now complete.
    pub fn contribute(&mut self, pubkey: &str, amount: u64) -> bool {
        self.current = self.current.saturating_add(amount);

        if let Some(c) = self.contributors.iter_mut().find(|c| c.pubkey == pubkey) {
            c.contribution = c.contribution.saturating_add(amount);
            c.last_contributed = Utc::now();
        } else {
            self.contributors.push(Contributor {
                pubkey: pubkey.to_string(),
                contribution: amount,
                last_contributed: Utc::now(),
            });
        }

        if self.current >= self.target && self.status == GroupAchievementStatus::Active {
            self.status = GroupAchievementStatus::Completed;
            self.completed_at = Some(Utc::now());
            return true;
        }

        false
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress_fraction(&self) -> f64 {
        if self.target == 0 {
            return 1.0;
        }
        (self.current as f64 / self.target as f64).min(1.0)
    }
}

/// Lifecycle state of a group achievement.
///
/// Defined locally to avoid circular dependency with achievement.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GroupAchievementStatus {
    /// In progress -- contributions accepted.
    Active,
    /// Target met.
    Completed,
    /// Time expired (no penalty, can be recreated).
    Expired,
}

/// A single participant's contribution to a group activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    /// The participant's public key.
    pub pubkey: String,
    /// Cumulative contribution amount.
    pub contribution: u64,
    /// When the last contribution was made.
    pub last_contributed: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// CommunityMilestone
// ---------------------------------------------------------------------------

/// A numeric threshold that, once reached, triggers a celebration.
///
/// Milestones are passive counters -- they fire when a community metric
/// crosses a threshold. Think "100 members!", "1,000 ideas published!".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityMilestone {
    /// Unique identifier.
    pub id: Uuid,
    /// The community this milestone belongs to.
    pub community_id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// What this milestone represents.
    pub description: String,
    /// The threshold value.
    pub threshold: u64,
    /// What to measure (e.g., "member_count", "ideas_published").
    pub metric: String,
    /// Current value of the metric.
    pub current: u64,
    /// Whether the threshold has been crossed.
    pub reached: bool,
    /// When the milestone was reached.
    pub reached_at: Option<DateTime<Utc>>,
    /// Optional celebration to display.
    pub celebration: Option<Celebration>,
}

impl CommunityMilestone {
    /// Create a new milestone.
    pub fn new(
        community_id: Uuid,
        name: impl Into<String>,
        description: impl Into<String>,
        threshold: u64,
        metric: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            community_id,
            name: name.into(),
            description: description.into(),
            threshold,
            metric: metric.into(),
            current: 0,
            reached: false,
            reached_at: None,
            celebration: None,
        }
    }

    /// Set a celebration for when the milestone is reached.
    pub fn with_celebration(mut self, celebration: Celebration) -> Self {
        self.celebration = Some(celebration);
        self
    }

    /// Update the current value and check if the milestone is reached.
    ///
    /// Returns `true` if the milestone was newly reached (first crossing).
    pub fn check(&mut self, current_value: u64) -> bool {
        self.current = current_value;
        if !self.reached && current_value >= self.threshold {
            self.reached = true;
            self.reached_at = Some(Utc::now());
            return true;
        }
        false
    }
}

/// A celebration displayed when a milestone is reached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Celebration {
    /// The celebration message.
    pub message: String,
    /// Optional badge awarded to community members.
    pub badge: Option<Badge>,
    /// Optional Cool reward for community members.
    pub cool_reward: Option<u64>,
}

impl Celebration {
    /// Create a celebration with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            badge: None,
            cool_reward: None,
        }
    }

    /// Add a badge to the celebration.
    pub fn with_badge(mut self, badge: Badge) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Add a Cool reward.
    pub fn with_cool_reward(mut self, amount: u64) -> Self {
        self.cool_reward = Some(amount);
        self
    }
}

// ---------------------------------------------------------------------------
// CooperativeRaid
// ---------------------------------------------------------------------------

/// A time-limited cooperative challenge with a "boss" target.
///
/// The community works together to reach a target before time runs out.
/// "Defeat" is encouraging ("we'll get 'em next time!"), never punishing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooperativeRaid {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// What this raid is about.
    pub description: String,
    /// The community hosting the raid.
    pub community_id: Uuid,
    /// The target to reach (the "boss health").
    pub target: u64,
    /// Current collective progress.
    pub current: u64,
    /// What to count.
    pub metric: String,
    /// Individual participants and their contributions.
    pub participants: Vec<RaidParticipant>,
    /// Rewards for all participants on victory.
    pub rewards: Vec<RewardType>,
    /// XP awarded on victory.
    pub xp_reward: XpAmount,
    /// When the raid opens for participation.
    pub starts_at: DateTime<Utc>,
    /// When the raid ends (win or lose).
    pub ends_at: DateTime<Utc>,
    /// Current raid state.
    pub status: RaidStatus,
}

impl CooperativeRaid {
    /// Create a new raid.
    pub fn new(
        community_id: Uuid,
        name: impl Into<String>,
        description: impl Into<String>,
        target: u64,
        metric: impl Into<String>,
        starts_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            community_id,
            target,
            current: 0,
            metric: metric.into(),
            participants: Vec::new(),
            rewards: Vec::new(),
            xp_reward: 0,
            starts_at,
            ends_at,
            status: RaidStatus::Recruiting,
        }
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

    /// Set the initial status.
    pub fn with_status(mut self, status: RaidStatus) -> Self {
        self.status = status;
        self
    }

    /// Add a participant to the raid. Returns `true` if newly joined.
    pub fn join(&mut self, pubkey: &str) -> bool {
        if self.participants.iter().any(|p| p.pubkey == pubkey) {
            return false;
        }
        self.participants.push(RaidParticipant {
            pubkey: pubkey.to_string(),
            contribution: 0,
            joined_at: Utc::now(),
        });
        true
    }

    /// Record a contribution from a participant.
    ///
    /// Returns `true` if the raid target is now met (victory!).
    pub fn contribute(&mut self, pubkey: &str, amount: u64) -> Result<bool, QuestError> {
        if self.status != RaidStatus::Active {
            return Err(QuestError::InvalidState(format!(
                "raid is {:?}, not Active",
                self.status
            )));
        }

        let participant = self
            .participants
            .iter_mut()
            .find(|p| p.pubkey == pubkey)
            .ok_or_else(|| QuestError::NotFound(format!("participant {pubkey}")))?;

        participant.contribution = participant.contribution.saturating_add(amount);
        self.current = self.current.saturating_add(amount);

        if self.current >= self.target {
            self.status = RaidStatus::Victory;
            return Ok(true);
        }

        Ok(false)
    }

    /// Check the raid status, factoring in the time window.
    ///
    /// If the raid is Active and the end time has passed without reaching
    /// the target, transitions to Defeat.
    pub fn check_status(&mut self, now: DateTime<Utc>) -> RaidStatus {
        if self.status == RaidStatus::Active && now >= self.ends_at && self.current < self.target {
            self.status = RaidStatus::Defeat;
        }
        if self.status == RaidStatus::Recruiting && now >= self.starts_at {
            self.status = RaidStatus::Active;
        }
        self.status
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress_fraction(&self) -> f64 {
        if self.target == 0 {
            return 1.0;
        }
        (self.current as f64 / self.target as f64).min(1.0)
    }

    /// Number of participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}

/// Raid lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RaidStatus {
    /// Accepting participants, not yet started.
    Recruiting,
    /// Raid in progress.
    Active,
    /// Target met -- everyone wins!
    Victory,
    /// Time expired. No punishment. "We'll get 'em next time!"
    Defeat,
    /// Cancelled by the community.
    Cancelled,
}

/// A participant in a cooperative raid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaidParticipant {
    /// The participant's public key.
    pub pubkey: String,
    /// Cumulative contribution.
    pub contribution: u64,
    /// When the participant joined the raid.
    pub joined_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// MentorshipProgram
// ---------------------------------------------------------------------------

/// A mentorship program that rewards teaching.
///
/// Mentors earn bonus XP when their mentees achieve milestones. This makes
/// teaching a first-class progression path in Quest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentorshipProgram {
    /// Unique identifier.
    pub id: Uuid,
    /// The mentor's public key.
    pub mentor_pubkey: String,
    /// Current mentees.
    pub mentees: Vec<MenteeRecord>,
    /// XP multiplier for mentorship activities.
    pub xp_multiplier: f64,
    /// Total mentees helped (lifetime).
    pub total_mentees_helped: u32,
    /// Total bonus XP earned from mentoring.
    pub total_bonus_xp: XpAmount,
}

impl MentorshipProgram {
    /// Create a new mentorship program for a mentor.
    pub fn new(mentor_pubkey: impl Into<String>, xp_multiplier: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            mentor_pubkey: mentor_pubkey.into(),
            mentees: Vec::new(),
            xp_multiplier,
            total_mentees_helped: 0,
            total_bonus_xp: 0,
        }
    }

    /// Add a new mentee.
    pub fn add_mentee(&mut self, pubkey: impl Into<String>) {
        self.mentees.push(MenteeRecord {
            pubkey: pubkey.into(),
            started_at: Utc::now(),
            milestones_achieved: 0,
            xp_earned_by_mentor: 0,
        });
        self.total_mentees_helped += 1;
    }

    /// Record a mentee achieving a milestone.
    ///
    /// Returns the bonus XP earned by the mentor for this milestone.
    pub fn record_mentee_progress(
        &mut self,
        mentee_pubkey: &str,
        base_xp: XpAmount,
    ) -> Result<XpAmount, QuestError> {
        let mentee = self
            .mentees
            .iter_mut()
            .find(|m| m.pubkey == mentee_pubkey)
            .ok_or_else(|| QuestError::NotFound(format!("mentee {mentee_pubkey}")))?;

        mentee.milestones_achieved += 1;
        let bonus = (base_xp as f64 * self.xp_multiplier) as XpAmount;
        mentee.xp_earned_by_mentor = mentee.xp_earned_by_mentor.saturating_add(bonus);
        self.total_bonus_xp = self.total_bonus_xp.saturating_add(bonus);

        Ok(bonus)
    }

    /// Number of current mentees.
    pub fn mentee_count(&self) -> usize {
        self.mentees.len()
    }
}

/// A record of one mentee's progress under a mentor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenteeRecord {
    /// The mentee's public key.
    pub pubkey: String,
    /// When the mentorship started.
    pub started_at: DateTime<Utc>,
    /// Number of milestones achieved while mentored.
    pub milestones_achieved: u32,
    /// Bonus XP the mentor earned from this mentee's progress.
    pub xp_earned_by_mentor: XpAmount,
}

// ---------------------------------------------------------------------------
// CooperativeBoard
// ---------------------------------------------------------------------------

/// Manages all cooperative activities.
///
/// Central coordination point for group achievements, community milestones,
/// cooperative raids, and mentorship programs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CooperativeBoard {
    group_achievements: Vec<GroupAchievement>,
    milestones: Vec<CommunityMilestone>,
    raids: Vec<CooperativeRaid>,
    mentorships: Vec<MentorshipProgram>,
}

impl CooperativeBoard {
    /// Create an empty cooperative board.
    pub fn new() -> Self {
        Self {
            group_achievements: Vec::new(),
            milestones: Vec::new(),
            raids: Vec::new(),
            mentorships: Vec::new(),
        }
    }

    // --- Group Achievements ---

    /// Register a group achievement.
    pub fn add_group_achievement(&mut self, achievement: GroupAchievement) {
        self.group_achievements.push(achievement);
    }

    /// Record a contribution to a group achievement.
    ///
    /// Returns `true` if the achievement is now complete.
    pub fn update_group_progress(
        &mut self,
        id: Uuid,
        contributor_pubkey: &str,
        increment: u64,
    ) -> Result<bool, QuestError> {
        let achievement = self
            .group_achievements
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| QuestError::NotFound(format!("group achievement {id}")))?;

        if achievement.status != GroupAchievementStatus::Active {
            return Err(QuestError::InvalidState(format!(
                "achievement is {:?}, not Active",
                achievement.status
            )));
        }

        Ok(achievement.contribute(contributor_pubkey, increment))
    }

    // --- Milestones ---

    /// Register a community milestone.
    pub fn add_milestone(&mut self, milestone: CommunityMilestone) {
        self.milestones.push(milestone);
    }

    /// Check a milestone against a current value.
    ///
    /// Returns `true` if the milestone was newly reached.
    pub fn check_milestone(&mut self, id: Uuid, current_value: u64) -> Result<bool, QuestError> {
        let milestone = self
            .milestones
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(|| QuestError::NotFound(format!("milestone {id}")))?;

        Ok(milestone.check(current_value))
    }

    // --- Raids ---

    /// Register a cooperative raid.
    pub fn create_raid(&mut self, raid: CooperativeRaid) {
        self.raids.push(raid);
    }

    /// Join a raid.
    pub fn join_raid(&mut self, raid_id: Uuid, pubkey: &str) -> Result<bool, QuestError> {
        let raid = self
            .raids
            .iter_mut()
            .find(|r| r.id == raid_id)
            .ok_or_else(|| QuestError::NotFound(format!("raid {raid_id}")))?;

        if raid.status != RaidStatus::Recruiting && raid.status != RaidStatus::Active {
            return Err(QuestError::InvalidState(format!(
                "raid is {:?}, cannot join",
                raid.status
            )));
        }

        Ok(raid.join(pubkey))
    }

    /// Contribute to a raid.
    pub fn contribute_to_raid(
        &mut self,
        raid_id: Uuid,
        pubkey: &str,
        amount: u64,
    ) -> Result<bool, QuestError> {
        let raid = self
            .raids
            .iter_mut()
            .find(|r| r.id == raid_id)
            .ok_or_else(|| QuestError::NotFound(format!("raid {raid_id}")))?;

        raid.contribute(pubkey, amount)
    }

    /// Check a raid's status (factoring in time).
    pub fn check_raid_status(
        &mut self,
        raid_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<RaidStatus, QuestError> {
        let raid = self
            .raids
            .iter_mut()
            .find(|r| r.id == raid_id)
            .ok_or_else(|| QuestError::NotFound(format!("raid {raid_id}")))?;

        Ok(raid.check_status(now))
    }

    // --- Mentorship ---

    /// Register a mentorship program.
    pub fn register_mentorship(&mut self, program: MentorshipProgram) {
        self.mentorships.push(program);
    }

    /// Add a mentee to a program.
    pub fn add_mentee(
        &mut self,
        program_id: Uuid,
        mentee_pubkey: impl Into<String>,
    ) -> Result<(), QuestError> {
        let program = self
            .mentorships
            .iter_mut()
            .find(|m| m.id == program_id)
            .ok_or_else(|| QuestError::NotFound(format!("mentorship {program_id}")))?;

        program.add_mentee(mentee_pubkey);
        Ok(())
    }

    /// Record a mentee's milestone achievement. Returns bonus XP for the mentor.
    pub fn record_mentee_progress(
        &mut self,
        program_id: Uuid,
        mentee_pubkey: &str,
        base_xp: XpAmount,
    ) -> Result<XpAmount, QuestError> {
        let program = self
            .mentorships
            .iter_mut()
            .find(|m| m.id == program_id)
            .ok_or_else(|| QuestError::NotFound(format!("mentorship {program_id}")))?;

        program.record_mentee_progress(mentee_pubkey, base_xp)
    }

    // --- Queries ---

    /// All currently active raids.
    pub fn active_raids(&self) -> Vec<&CooperativeRaid> {
        self.raids
            .iter()
            .filter(|r| r.status == RaidStatus::Active || r.status == RaidStatus::Recruiting)
            .collect()
    }

    /// All milestones for a specific community.
    pub fn community_milestones(&self, community_id: Uuid) -> Vec<&CommunityMilestone> {
        self.milestones
            .iter()
            .filter(|m| m.community_id == community_id)
            .collect()
    }

    /// Look up a mentorship program by the mentor's public key.
    pub fn mentor_stats(&self, pubkey: &str) -> Option<&MentorshipProgram> {
        self.mentorships.iter().find(|m| m.mentor_pubkey == pubkey)
    }

    /// Counts of all cooperative activities: (achievements, milestones, raids, mentorships).
    pub fn count_all(&self) -> (usize, usize, usize, usize) {
        (
            self.group_achievements.len(),
            self.milestones.len(),
            self.raids.len(),
            self.mentorships.len(),
        )
    }

    // --- Federation-scoped queries ---

    /// All group achievements visible within the federation scope.
    ///
    /// When the scope is unrestricted, all achievements are returned.
    /// When scoped, only achievements whose `community_id` is visible are returned.
    pub fn group_achievements_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&GroupAchievement> {
        if scope.is_unrestricted() {
            return self.group_achievements.iter().collect();
        }
        self.group_achievements
            .iter()
            .filter(|a| scope.is_visible_uuid(&a.community_id))
            .collect()
    }

    /// All active raids visible within the federation scope.
    ///
    /// When the scope is unrestricted, all active/recruiting raids are returned.
    /// When scoped, only raids whose `community_id` is visible are returned.
    pub fn active_raids_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&CooperativeRaid> {
        self.raids
            .iter()
            .filter(|r| {
                (r.status == RaidStatus::Active || r.status == RaidStatus::Recruiting)
                    && scope.is_visible_uuid(&r.community_id)
            })
            .collect()
    }

    /// All milestones visible within the federation scope.
    ///
    /// When the scope is unrestricted, all milestones are returned.
    /// When scoped, only milestones whose `community_id` is visible are returned.
    pub fn milestones_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&CommunityMilestone> {
        if scope.is_unrestricted() {
            return self.milestones.iter().collect();
        }
        self.milestones
            .iter()
            .filter(|m| scope.is_visible_uuid(&m.community_id))
            .collect()
    }

    /// Count cooperative activities visible within the federation scope.
    ///
    /// Returns `(achievements, milestones, raids, mentorships)` filtered to
    /// visible communities. Mentorships are not community-scoped and always
    /// included in full.
    pub fn count_all_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> (usize, usize, usize, usize) {
        if scope.is_unrestricted() {
            return self.count_all();
        }
        (
            self.group_achievements
                .iter()
                .filter(|a| scope.is_visible_uuid(&a.community_id))
                .count(),
            self.milestones
                .iter()
                .filter(|m| scope.is_visible_uuid(&m.community_id))
                .count(),
            self.raids
                .iter()
                .filter(|r| scope.is_visible_uuid(&r.community_id))
                .count(),
            // Mentorships are person-to-person, not community-scoped
            self.mentorships.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use crate::reward::BadgeTier;

    fn community_id() -> Uuid {
        Uuid::new_v4()
    }

    fn sample_group_achievement() -> GroupAchievement {
        GroupAchievement::new(
            community_id(),
            "Community Quilt",
            "Create 100 ideas together",
            100,
            "ideas_created",
        )
        .with_xp_reward(500)
    }

    fn sample_milestone() -> CommunityMilestone {
        CommunityMilestone::new(
            community_id(),
            "Century Club",
            "Reach 100 members",
            100,
            "member_count",
        )
    }

    fn sample_raid() -> CooperativeRaid {
        let now = Utc::now();
        CooperativeRaid::new(
            community_id(),
            "Boss Rush",
            "Reach the target together",
            1000,
            "actions_taken",
            now,
            now + Duration::days(7),
        )
        .with_xp_reward(1000)
        .with_status(RaidStatus::Active)
    }

    fn sample_mentorship() -> MentorshipProgram {
        MentorshipProgram::new("cpub1mentor", 1.5)
    }

    // --- GroupAchievement ---

    #[test]
    fn group_achievement_new() {
        let ga = sample_group_achievement();
        assert_eq!(ga.name, "Community Quilt");
        assert_eq!(ga.target, 100);
        assert_eq!(ga.current, 0);
        assert_eq!(ga.xp_reward, 500);
        assert!(ga.contributors.is_empty());
        assert_eq!(ga.status, GroupAchievementStatus::Active);
    }

    #[test]
    fn group_achievement_contribute() {
        let mut ga = sample_group_achievement();
        let completed = ga.contribute("alice", 30);
        assert!(!completed);
        assert_eq!(ga.current, 30);
        assert_eq!(ga.contributors.len(), 1);
        assert_eq!(ga.contributors[0].contribution, 30);
    }

    #[test]
    fn group_achievement_contribute_multiple() {
        let mut ga = sample_group_achievement();
        ga.contribute("alice", 30);
        ga.contribute("alice", 20);
        assert_eq!(ga.current, 50);
        assert_eq!(ga.contributors.len(), 1);
        assert_eq!(ga.contributors[0].contribution, 50);
    }

    #[test]
    fn group_achievement_contribute_multiple_people() {
        let mut ga = sample_group_achievement();
        ga.contribute("alice", 30);
        ga.contribute("bob", 20);
        assert_eq!(ga.current, 50);
        assert_eq!(ga.contributors.len(), 2);
    }

    #[test]
    fn group_achievement_completion() {
        let mut ga = sample_group_achievement();
        ga.contribute("alice", 60);
        let completed = ga.contribute("bob", 40);
        assert!(completed);
        assert_eq!(ga.status, GroupAchievementStatus::Completed);
        assert!(ga.completed_at.is_some());
    }

    #[test]
    fn group_achievement_completion_overshoot() {
        let mut ga = sample_group_achievement();
        let completed = ga.contribute("alice", 200);
        assert!(completed);
        assert_eq!(ga.current, 200);
    }

    #[test]
    fn group_achievement_no_double_complete() {
        let mut ga = sample_group_achievement();
        ga.contribute("alice", 100);
        // Already completed, further contributions should not re-trigger
        let completed = ga.contribute("bob", 50);
        assert!(!completed);
        assert_eq!(ga.status, GroupAchievementStatus::Completed);
    }

    #[test]
    fn group_achievement_progress_fraction() {
        let mut ga = sample_group_achievement();
        assert_eq!(ga.progress_fraction(), 0.0);
        ga.contribute("alice", 50);
        assert_eq!(ga.progress_fraction(), 0.5);
        ga.contribute("bob", 50);
        assert_eq!(ga.progress_fraction(), 1.0);
        ga.contribute("charlie", 50);
        assert_eq!(ga.progress_fraction(), 1.0); // capped at 1.0
    }

    #[test]
    fn group_achievement_progress_fraction_zero_target() {
        let ga = GroupAchievement::new(community_id(), "Empty", "desc", 0, "m");
        assert_eq!(ga.progress_fraction(), 1.0);
    }

    #[test]
    fn group_achievement_serde_round_trip() {
        let mut ga = sample_group_achievement();
        ga.contribute("alice", 30);
        let json = serde_json::to_string(&ga).unwrap();
        let restored: GroupAchievement = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, ga.id);
        assert_eq!(restored.current, 30);
        assert_eq!(restored.contributors.len(), 1);
    }

    // --- GroupAchievementStatus ---

    #[test]
    fn group_achievement_status_serde_round_trip() {
        let statuses = [
            GroupAchievementStatus::Active,
            GroupAchievementStatus::Completed,
            GroupAchievementStatus::Expired,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let restored: GroupAchievementStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, s);
        }
    }

    // --- CommunityMilestone ---

    #[test]
    fn milestone_new() {
        let m = sample_milestone();
        assert_eq!(m.name, "Century Club");
        assert_eq!(m.threshold, 100);
        assert!(!m.reached);
    }

    #[test]
    fn milestone_check_not_reached() {
        let mut m = sample_milestone();
        assert!(!m.check(50));
        assert!(!m.reached);
        assert_eq!(m.current, 50);
    }

    #[test]
    fn milestone_check_reached() {
        let mut m = sample_milestone();
        assert!(m.check(100));
        assert!(m.reached);
        assert!(m.reached_at.is_some());
    }

    #[test]
    fn milestone_check_no_double_trigger() {
        let mut m = sample_milestone();
        assert!(m.check(100));
        assert!(!m.check(200)); // already reached
    }

    #[test]
    fn milestone_with_celebration() {
        let badge = Badge::new("milestone-100", "Century Badge", "100 members!", "icon-100", BadgeTier::Gold);
        let m = sample_milestone().with_celebration(
            Celebration::new("We hit 100 members!")
                .with_badge(badge)
                .with_cool_reward(100),
        );

        assert!(m.celebration.is_some());
        let c = m.celebration.unwrap();
        assert_eq!(c.message, "We hit 100 members!");
        assert!(c.badge.is_some());
        assert_eq!(c.cool_reward, Some(100));
    }

    #[test]
    fn milestone_serde_round_trip() {
        let m = sample_milestone();
        let json = serde_json::to_string(&m).unwrap();
        let restored: CommunityMilestone = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, m.id);
        assert_eq!(restored.threshold, 100);
    }

    // --- Celebration ---

    #[test]
    fn celebration_new() {
        let c = Celebration::new("Hooray!");
        assert_eq!(c.message, "Hooray!");
        assert!(c.badge.is_none());
        assert!(c.cool_reward.is_none());
    }

    #[test]
    fn celebration_serde_round_trip() {
        let c = Celebration::new("Yay!").with_cool_reward(50);
        let json = serde_json::to_string(&c).unwrap();
        let restored: Celebration = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.message, "Yay!");
        assert_eq!(restored.cool_reward, Some(50));
    }

    // --- CooperativeRaid ---

    #[test]
    fn raid_new() {
        let r = sample_raid();
        assert_eq!(r.name, "Boss Rush");
        assert_eq!(r.target, 1000);
        assert_eq!(r.current, 0);
        assert_eq!(r.xp_reward, 1000);
        assert!(r.participants.is_empty());
    }

    #[test]
    fn raid_join() {
        let mut r = sample_raid();
        assert!(r.join("alice"));
        assert_eq!(r.participant_count(), 1);
        assert!(!r.join("alice")); // duplicate
        assert_eq!(r.participant_count(), 1);
    }

    #[test]
    fn raid_contribute() {
        let mut r = sample_raid();
        r.join("alice");
        let victory = r.contribute("alice", 500).unwrap();
        assert!(!victory);
        assert_eq!(r.current, 500);
    }

    #[test]
    fn raid_contribute_victory() {
        let mut r = sample_raid();
        r.join("alice");
        r.join("bob");
        r.contribute("alice", 600).unwrap();
        let victory = r.contribute("bob", 400).unwrap();
        assert!(victory);
        assert_eq!(r.status, RaidStatus::Victory);
    }

    #[test]
    fn raid_contribute_not_active() {
        let mut r = sample_raid().with_status(RaidStatus::Recruiting);
        r.join("alice");
        let result = r.contribute("alice", 100);
        assert!(result.is_err());
    }

    #[test]
    fn raid_contribute_not_joined() {
        let mut r = sample_raid();
        let result = r.contribute("alice", 100);
        assert!(result.is_err());
    }

    #[test]
    fn raid_check_status_defeat() {
        let now = Utc::now();
        let mut r = CooperativeRaid::new(
            community_id(),
            "Quick Raid",
            "desc",
            100,
            "m",
            now - Duration::days(2),
            now - Duration::days(1), // already ended
        )
        .with_status(RaidStatus::Active);

        let status = r.check_status(now);
        assert_eq!(status, RaidStatus::Defeat);
    }

    #[test]
    fn raid_check_status_auto_start() {
        let now = Utc::now();
        let mut r = CooperativeRaid::new(
            community_id(),
            "Starting Raid",
            "desc",
            100,
            "m",
            now - Duration::hours(1), // started an hour ago
            now + Duration::days(7),
        );
        // Status starts as Recruiting
        assert_eq!(r.status, RaidStatus::Recruiting);
        let status = r.check_status(now);
        assert_eq!(status, RaidStatus::Active);
    }

    #[test]
    fn raid_progress_fraction() {
        let mut r = sample_raid();
        assert_eq!(r.progress_fraction(), 0.0);
        r.join("alice");
        r.contribute("alice", 500).unwrap();
        assert_eq!(r.progress_fraction(), 0.5);
    }

    #[test]
    fn raid_progress_fraction_zero_target() {
        let now = Utc::now();
        let r = CooperativeRaid::new(community_id(), "x", "d", 0, "m", now, now + Duration::days(1));
        assert_eq!(r.progress_fraction(), 1.0);
    }

    #[test]
    fn raid_serde_round_trip() {
        let r = sample_raid();
        let json = serde_json::to_string(&r).unwrap();
        let restored: CooperativeRaid = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, r.id);
        assert_eq!(restored.target, 1000);
    }

    #[test]
    fn raid_defeat_no_penalty() {
        // Defeat should leave progress intact and not remove participants
        let now = Utc::now();
        let mut r = CooperativeRaid::new(
            community_id(),
            "Tough Fight",
            "desc",
            1000,
            "m",
            now - Duration::days(2),
            now - Duration::days(1),
        )
        .with_status(RaidStatus::Active);

        r.join("alice");
        // Can't contribute because status will change on check
        // Manually set progress to simulate
        r.current = 500;
        r.participants[0].contribution = 500;

        r.check_status(now);
        assert_eq!(r.status, RaidStatus::Defeat);
        // Progress preserved
        assert_eq!(r.current, 500);
        assert_eq!(r.participants[0].contribution, 500);
    }

    // --- RaidStatus ---

    #[test]
    fn raid_status_serde_round_trip() {
        let statuses = [
            RaidStatus::Recruiting,
            RaidStatus::Active,
            RaidStatus::Victory,
            RaidStatus::Defeat,
            RaidStatus::Cancelled,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let restored: RaidStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, s);
        }
    }

    // --- MentorshipProgram ---

    #[test]
    fn mentorship_new() {
        let m = sample_mentorship();
        assert_eq!(m.mentor_pubkey, "cpub1mentor");
        assert_eq!(m.xp_multiplier, 1.5);
        assert_eq!(m.total_mentees_helped, 0);
        assert_eq!(m.total_bonus_xp, 0);
    }

    #[test]
    fn mentorship_add_mentee() {
        let mut m = sample_mentorship();
        m.add_mentee("cpub1student");
        assert_eq!(m.mentee_count(), 1);
        assert_eq!(m.total_mentees_helped, 1);
    }

    #[test]
    fn mentorship_record_progress() {
        let mut m = sample_mentorship();
        m.add_mentee("cpub1student");

        let bonus = m.record_mentee_progress("cpub1student", 100).unwrap();
        assert_eq!(bonus, 150); // 100 * 1.5
        assert_eq!(m.total_bonus_xp, 150);
        assert_eq!(m.mentees[0].milestones_achieved, 1);
        assert_eq!(m.mentees[0].xp_earned_by_mentor, 150);
    }

    #[test]
    fn mentorship_record_progress_accumulates() {
        let mut m = sample_mentorship();
        m.add_mentee("cpub1student");

        m.record_mentee_progress("cpub1student", 100).unwrap();
        m.record_mentee_progress("cpub1student", 200).unwrap();

        assert_eq!(m.total_bonus_xp, 450); // 150 + 300
        assert_eq!(m.mentees[0].milestones_achieved, 2);
        assert_eq!(m.mentees[0].xp_earned_by_mentor, 450);
    }

    #[test]
    fn mentorship_record_progress_not_found() {
        let mut m = sample_mentorship();
        let result = m.record_mentee_progress("unknown", 100);
        assert!(result.is_err());
    }

    #[test]
    fn mentorship_multiple_mentees() {
        let mut m = sample_mentorship();
        m.add_mentee("cpub1a");
        m.add_mentee("cpub1b");
        assert_eq!(m.mentee_count(), 2);
        assert_eq!(m.total_mentees_helped, 2);

        m.record_mentee_progress("cpub1a", 100).unwrap();
        m.record_mentee_progress("cpub1b", 200).unwrap();
        assert_eq!(m.total_bonus_xp, 450); // 150 + 300
    }

    #[test]
    fn mentorship_serde_round_trip() {
        let mut m = sample_mentorship();
        m.add_mentee("cpub1student");
        m.record_mentee_progress("cpub1student", 100).unwrap();
        let json = serde_json::to_string(&m).unwrap();
        let restored: MentorshipProgram = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.mentor_pubkey, "cpub1mentor");
        assert_eq!(restored.total_bonus_xp, 150);
    }

    // --- CooperativeBoard ---

    #[test]
    fn board_empty() {
        let board = CooperativeBoard::new();
        assert_eq!(board.count_all(), (0, 0, 0, 0));
    }

    #[test]
    fn board_add_group_achievement() {
        let mut board = CooperativeBoard::new();
        board.add_group_achievement(sample_group_achievement());
        assert_eq!(board.count_all(), (1, 0, 0, 0));
    }

    #[test]
    fn board_update_group_progress() {
        let mut board = CooperativeBoard::new();
        let ga = sample_group_achievement();
        let id = ga.id;
        board.add_group_achievement(ga);

        let completed = board.update_group_progress(id, "alice", 50).unwrap();
        assert!(!completed);

        let completed = board.update_group_progress(id, "bob", 50).unwrap();
        assert!(completed);
    }

    #[test]
    fn board_update_group_progress_not_found() {
        let mut board = CooperativeBoard::new();
        let result = board.update_group_progress(Uuid::new_v4(), "alice", 10);
        assert!(result.is_err());
    }

    #[test]
    fn board_update_group_progress_completed() {
        let mut board = CooperativeBoard::new();
        let ga = sample_group_achievement();
        let id = ga.id;
        board.add_group_achievement(ga);

        board.update_group_progress(id, "alice", 100).unwrap();
        let result = board.update_group_progress(id, "bob", 10);
        assert!(result.is_err()); // already completed
    }

    #[test]
    fn board_add_milestone() {
        let mut board = CooperativeBoard::new();
        board.add_milestone(sample_milestone());
        assert_eq!(board.count_all(), (0, 1, 0, 0));
    }

    #[test]
    fn board_check_milestone() {
        let mut board = CooperativeBoard::new();
        let m = sample_milestone();
        let id = m.id;
        board.add_milestone(m);

        assert!(!board.check_milestone(id, 50).unwrap());
        assert!(board.check_milestone(id, 100).unwrap());
        assert!(!board.check_milestone(id, 200).unwrap()); // already reached
    }

    #[test]
    fn board_check_milestone_not_found() {
        let mut board = CooperativeBoard::new();
        let result = board.check_milestone(Uuid::new_v4(), 100);
        assert!(result.is_err());
    }

    #[test]
    fn board_create_raid() {
        let mut board = CooperativeBoard::new();
        board.create_raid(sample_raid());
        assert_eq!(board.count_all(), (0, 0, 1, 0));
    }

    #[test]
    fn board_join_raid() {
        let mut board = CooperativeBoard::new();
        let r = sample_raid();
        let id = r.id;
        board.create_raid(r);

        let joined = board.join_raid(id, "alice").unwrap();
        assert!(joined);

        let joined = board.join_raid(id, "alice").unwrap();
        assert!(!joined); // already joined
    }

    #[test]
    fn board_join_raid_not_found() {
        let mut board = CooperativeBoard::new();
        let result = board.join_raid(Uuid::new_v4(), "alice");
        assert!(result.is_err());
    }

    #[test]
    fn board_contribute_to_raid() {
        let mut board = CooperativeBoard::new();
        let r = sample_raid();
        let id = r.id;
        board.create_raid(r);
        board.join_raid(id, "alice").unwrap();

        let victory = board.contribute_to_raid(id, "alice", 500).unwrap();
        assert!(!victory);

        let victory = board.contribute_to_raid(id, "alice", 500).unwrap();
        assert!(victory);
    }

    #[test]
    fn board_check_raid_status() {
        let mut board = CooperativeBoard::new();
        let r = sample_raid();
        let id = r.id;
        board.create_raid(r);

        let status = board.check_raid_status(id, Utc::now()).unwrap();
        assert_eq!(status, RaidStatus::Active);
    }

    #[test]
    fn board_register_mentorship() {
        let mut board = CooperativeBoard::new();
        board.register_mentorship(sample_mentorship());
        assert_eq!(board.count_all(), (0, 0, 0, 1));
    }

    #[test]
    fn board_add_mentee() {
        let mut board = CooperativeBoard::new();
        let m = sample_mentorship();
        let id = m.id;
        board.register_mentorship(m);

        board.add_mentee(id, "cpub1student").unwrap();
        let stats = board.mentor_stats("cpub1mentor").unwrap();
        assert_eq!(stats.mentee_count(), 1);
    }

    #[test]
    fn board_add_mentee_not_found() {
        let mut board = CooperativeBoard::new();
        let result = board.add_mentee(Uuid::new_v4(), "x");
        assert!(result.is_err());
    }

    #[test]
    fn board_record_mentee_progress() {
        let mut board = CooperativeBoard::new();
        let m = sample_mentorship();
        let id = m.id;
        board.register_mentorship(m);
        board.add_mentee(id, "cpub1student").unwrap();

        let bonus = board
            .record_mentee_progress(id, "cpub1student", 100)
            .unwrap();
        assert_eq!(bonus, 150);
    }

    #[test]
    fn board_active_raids() {
        let mut board = CooperativeBoard::new();
        board.create_raid(sample_raid()); // Active

        let now = Utc::now();
        let recruiting = CooperativeRaid::new(
            community_id(),
            "Recruiting",
            "desc",
            100,
            "m",
            now + Duration::days(1),
            now + Duration::days(8),
        );
        board.create_raid(recruiting);

        assert_eq!(board.active_raids().len(), 2); // Active + Recruiting
    }

    #[test]
    fn board_community_milestones() {
        let mut board = CooperativeBoard::new();
        let cid = community_id();
        let m = CommunityMilestone::new(cid, "A", "desc", 100, "x");
        board.add_milestone(m);
        board.add_milestone(sample_milestone()); // different community

        assert_eq!(board.community_milestones(cid).len(), 1);
    }

    #[test]
    fn board_mentor_stats() {
        let mut board = CooperativeBoard::new();
        board.register_mentorship(sample_mentorship());

        assert!(board.mentor_stats("cpub1mentor").is_some());
        assert!(board.mentor_stats("cpub1unknown").is_none());
    }

    #[test]
    fn board_serde_round_trip() {
        let mut board = CooperativeBoard::new();
        board.add_group_achievement(sample_group_achievement());
        board.add_milestone(sample_milestone());
        board.create_raid(sample_raid());
        board.register_mentorship(sample_mentorship());

        let json = serde_json::to_string(&board).unwrap();
        let restored: CooperativeBoard = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.count_all(), (1, 1, 1, 1));
    }

    // --- No dark patterns ---

    #[test]
    fn raid_defeat_preserves_progress() {
        let mut board = CooperativeBoard::new();
        let now = Utc::now();
        let r = CooperativeRaid::new(
            community_id(),
            "Time Up",
            "desc",
            1000,
            "m",
            now - Duration::days(2),
            now - Duration::days(1),
        )
        .with_status(RaidStatus::Active);
        let id = r.id;
        board.create_raid(r);

        // Simulate progress that was recorded before time expired
        board.join_raid(id, "alice").unwrap();
        // We can't contribute because check_status will transition to Defeat,
        // but let's check that status transitions cleanly
        let status = board.check_raid_status(id, now).unwrap();
        assert_eq!(status, RaidStatus::Defeat);
    }

    #[test]
    fn group_achievement_saturating_add() {
        let mut ga = GroupAchievement::new(community_id(), "Big", "desc", u64::MAX, "m");
        ga.contribute("alice", u64::MAX);
        ga.contribute("bob", 1);
        assert_eq!(ga.current, u64::MAX); // saturated, not overflowed
    }

    // --- Federation-scoped queries ---

    #[test]
    fn group_achievements_scoped_unrestricted() {
        let mut board = CooperativeBoard::new();
        board.add_group_achievement(sample_group_achievement());
        board.add_group_achievement(sample_group_achievement());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(board.group_achievements_scoped(&scope).len(), 2);
    }

    #[test]
    fn group_achievements_scoped_filters_by_community() {
        let mut board = CooperativeBoard::new();
        let cid1 = Uuid::new_v4();
        let cid2 = Uuid::new_v4();

        board.add_group_achievement(
            GroupAchievement::new(cid1, "Alpha Goal", "desc", 100, "m"),
        );
        board.add_group_achievement(
            GroupAchievement::new(cid2, "Beta Goal", "desc", 100, "m"),
        );
        board.add_group_achievement(
            GroupAchievement::new(Uuid::new_v4(), "Gamma Goal", "desc", 100, "m"),
        );

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid1.to_string(),
            cid2.to_string(),
        ]);
        let visible = board.group_achievements_scoped(&scope);
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().any(|a| a.name == "Alpha Goal"));
        assert!(visible.iter().any(|a| a.name == "Beta Goal"));
    }

    #[test]
    fn active_raids_scoped_unrestricted() {
        let mut board = CooperativeBoard::new();
        board.create_raid(sample_raid());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(board.active_raids_scoped(&scope).len(), 1);
    }

    #[test]
    fn active_raids_scoped_filters_by_community() {
        let mut board = CooperativeBoard::new();
        let cid1 = Uuid::new_v4();
        let cid2 = Uuid::new_v4();
        let now = Utc::now();

        board.create_raid(
            CooperativeRaid::new(cid1, "Raid A", "desc", 100, "m", now, now + Duration::days(7))
                .with_status(RaidStatus::Active),
        );
        board.create_raid(
            CooperativeRaid::new(cid2, "Raid B", "desc", 100, "m", now, now + Duration::days(7))
                .with_status(RaidStatus::Active),
        );

        let scope =
            crate::federation_scope::FederationScope::from_communities([cid1.to_string()]);
        let visible = board.active_raids_scoped(&scope);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "Raid A");
    }

    #[test]
    fn active_raids_scoped_excludes_inactive() {
        let mut board = CooperativeBoard::new();
        let cid = Uuid::new_v4();
        let now = Utc::now();

        board.create_raid(
            CooperativeRaid::new(cid, "Ended Raid", "desc", 100, "m", now, now + Duration::days(7))
                .with_status(RaidStatus::Victory),
        );

        let scope =
            crate::federation_scope::FederationScope::from_communities([cid.to_string()]);
        assert!(board.active_raids_scoped(&scope).is_empty());
    }

    #[test]
    fn milestones_scoped_unrestricted() {
        let mut board = CooperativeBoard::new();
        board.add_milestone(sample_milestone());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(board.milestones_scoped(&scope).len(), 1);
    }

    #[test]
    fn milestones_scoped_filters_by_community() {
        let mut board = CooperativeBoard::new();
        let cid1 = Uuid::new_v4();
        let cid2 = Uuid::new_v4();

        board.add_milestone(CommunityMilestone::new(cid1, "M1", "desc", 100, "m"));
        board.add_milestone(CommunityMilestone::new(cid2, "M2", "desc", 200, "m"));
        board.add_milestone(CommunityMilestone::new(Uuid::new_v4(), "M3", "desc", 300, "m"));

        let scope =
            crate::federation_scope::FederationScope::from_communities([cid1.to_string()]);
        let visible = board.milestones_scoped(&scope);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "M1");
    }

    #[test]
    fn count_all_scoped_unrestricted() {
        let mut board = CooperativeBoard::new();
        board.add_group_achievement(sample_group_achievement());
        board.add_milestone(sample_milestone());
        board.create_raid(sample_raid());
        board.register_mentorship(sample_mentorship());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(board.count_all_scoped(&scope), (1, 1, 1, 1));
    }

    #[test]
    fn count_all_scoped_filters_community_activities() {
        let mut board = CooperativeBoard::new();
        let cid = Uuid::new_v4();
        let now = Utc::now();

        // One from cid, one from another
        board.add_group_achievement(
            GroupAchievement::new(cid, "In scope", "desc", 100, "m"),
        );
        board.add_group_achievement(
            GroupAchievement::new(Uuid::new_v4(), "Out of scope", "desc", 100, "m"),
        );
        board.add_milestone(CommunityMilestone::new(cid, "In scope", "desc", 100, "m"));
        board.create_raid(
            CooperativeRaid::new(cid, "In scope", "desc", 100, "m", now, now + Duration::days(7)),
        );
        board.create_raid(
            CooperativeRaid::new(Uuid::new_v4(), "Out", "desc", 100, "m", now, now + Duration::days(7)),
        );
        board.register_mentorship(sample_mentorship());

        let scope =
            crate::federation_scope::FederationScope::from_communities([cid.to_string()]);
        // 1 achievement (of 2), 1 milestone, 1 raid (of 2), 1 mentorship (always included)
        assert_eq!(board.count_all_scoped(&scope), (1, 1, 1, 1));
    }

    #[test]
    fn count_all_scoped_empty_scope_returns_nothing() {
        let mut board = CooperativeBoard::new();
        board.add_group_achievement(sample_group_achievement());
        board.add_milestone(sample_milestone());
        board.create_raid(sample_raid());
        board.register_mentorship(sample_mentorship());

        // Scope to a community that doesn't match anything
        let scope = crate::federation_scope::FederationScope::from_communities([
            "nonexistent-community",
        ]);
        // Mentorships still included (not community-scoped)
        assert_eq!(board.count_all_scoped(&scope), (0, 0, 0, 1));
    }
}
