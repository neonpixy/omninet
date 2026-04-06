//! Reward types -- what you earn through Quest.
//!
//! Rewards are granted by achievements, missions, challenges, mentorship, and
//! community activities. The `RewardLedger` tracks all rewards for a participant.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What kind of reward is being granted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RewardType {
    /// Cool currency amount.
    Cool(u64),

    /// A visual badge for recognition.
    Badge(Badge),

    /// Unlock a feature or capability.
    Unlock(String),

    /// A display title the participant can use.
    Title(String),

    /// Skill tree progression points.
    SkillPoint {
        /// Which skill tree receives the points.
        tree_id: String,
        /// How many points to award.
        points: u32,
    },
}

/// A visual badge earned through participation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Badge {
    /// Unique identifier for this badge definition.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// What this badge represents.
    pub description: String,
    /// Icon reference (asset ID or name).
    pub icon: String,
    /// Rarity/prestige tier.
    pub tier: BadgeTier,
    /// When this badge was earned, if it has been.
    pub earned_at: Option<DateTime<Utc>>,
}

impl Badge {
    /// Create a new badge definition.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        icon: impl Into<String>,
        tier: BadgeTier,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            icon: icon.into(),
            tier,
            earned_at: None,
        }
    }

    /// Mark this badge as earned now.
    pub fn earned(mut self) -> Self {
        self.earned_at = Some(Utc::now());
        self
    }

    /// Mark this badge as earned at a specific time.
    pub fn earned_at(mut self, when: DateTime<Utc>) -> Self {
        self.earned_at = Some(when);
        self
    }
}

/// Badge prestige tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BadgeTier {
    /// Accessible -- everyone can earn these with basic participation.
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

/// Where a reward came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RewardSource {
    /// Earned through an achievement.
    Achievement,
    /// Earned by completing a mission.
    Mission,
    /// Earned through a challenge.
    Challenge,
    /// Earned by helping others.
    Mentorship,
    /// Earned through community participation.
    Community,
    /// Earned through consortia activities.
    Consortia,
}

/// A single reward instance granted to a participant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reward {
    /// Unique ID for this reward instance.
    pub id: Uuid,
    /// What was rewarded.
    pub reward_type: RewardType,
    /// The achievement, mission, or challenge that granted this reward.
    pub source_id: String,
    /// Category of the source.
    pub source_type: RewardSource,
    /// Recipient's public key.
    pub recipient: String,
    /// When the reward was granted.
    pub granted_at: DateTime<Utc>,
    /// Whether the reward has been claimed by the recipient.
    pub claimed: bool,
    /// When the reward was claimed, if it has been.
    pub claimed_at: Option<DateTime<Utc>>,
}

impl Reward {
    /// Create a new unclaimed reward.
    pub fn new(
        reward_type: RewardType,
        source_id: impl Into<String>,
        source_type: RewardSource,
        recipient: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            reward_type,
            source_id: source_id.into(),
            source_type,
            recipient: recipient.into(),
            granted_at: Utc::now(),
            claimed: false,
            claimed_at: None,
        }
    }

    /// Mark this reward as claimed.
    pub fn claim(&mut self) {
        self.claimed = true;
        self.claimed_at = Some(Utc::now());
    }
}

/// Tracks all rewards for a participant.
///
/// The ledger is append-only for grants. Rewards can be claimed but never removed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RewardLedger {
    rewards: Vec<Reward>,
}

impl RewardLedger {
    /// Create an empty reward ledger.
    pub fn new() -> Self {
        Self {
            rewards: Vec::new(),
        }
    }

    /// Grant a new reward to the ledger.
    pub fn grant(&mut self, reward: Reward) {
        self.rewards.push(reward);
    }

    /// Claim a reward by its ID. Returns `true` if found and newly claimed.
    pub fn claim(&mut self, reward_id: Uuid) -> bool {
        if let Some(reward) = self.rewards.iter_mut().find(|r| r.id == reward_id) {
            if !reward.claimed {
                reward.claim();
                return true;
            }
        }
        false
    }

    /// All unclaimed rewards.
    pub fn unclaimed(&self) -> Vec<&Reward> {
        self.rewards.iter().filter(|r| !r.claimed).collect()
    }

    /// Rewards from a specific source type.
    pub fn by_source(&self, source: RewardSource) -> Vec<&Reward> {
        self.rewards
            .iter()
            .filter(|r| r.source_type == source)
            .collect()
    }

    /// Rewards matching a specific reward type category.
    ///
    /// Matches on the variant, not the inner value (e.g., all `Cool` rewards
    /// regardless of amount).
    pub fn by_type(&self, reward_type: &RewardType) -> Vec<&Reward> {
        self.rewards
            .iter()
            .filter(|r| std::mem::discriminant(&r.reward_type) == std::mem::discriminant(reward_type))
            .collect()
    }

    /// Total Cool currency earned (claimed and unclaimed).
    pub fn total_cool_earned(&self) -> u64 {
        self.rewards
            .iter()
            .filter_map(|r| match &r.reward_type {
                RewardType::Cool(amount) => Some(*amount),
                _ => None,
            })
            .sum()
    }

    /// All badge rewards.
    pub fn badges(&self) -> Vec<&Badge> {
        self.rewards
            .iter()
            .filter_map(|r| match &r.reward_type {
                RewardType::Badge(badge) => Some(badge),
                _ => None,
            })
            .collect()
    }

    /// Most recent `n` rewards, newest first.
    pub fn recent(&self, n: usize) -> Vec<&Reward> {
        let mut sorted: Vec<&Reward> = self.rewards.iter().collect();
        sorted.sort_by(|a, b| b.granted_at.cmp(&a.granted_at));
        sorted.truncate(n);
        sorted
    }

    /// Total number of rewards in the ledger.
    pub fn count(&self) -> usize {
        self.rewards.len()
    }

    /// All rewards in the ledger.
    pub fn all(&self) -> &[Reward] {
        &self.rewards
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_badge(id: &str, tier: BadgeTier) -> Badge {
        Badge::new(id, format!("Badge {id}"), "A test badge", "icon-test", tier)
    }

    fn make_cool_reward(amount: u64, source: RewardSource) -> Reward {
        Reward::new(RewardType::Cool(amount), "test-source", source, "cpub1alice")
    }

    fn make_badge_reward(badge: Badge) -> Reward {
        Reward::new(
            RewardType::Badge(badge),
            "achievement-1",
            RewardSource::Achievement,
            "cpub1alice",
        )
    }

    // --- Badge tests ---

    #[test]
    fn badge_creation() {
        let badge = make_badge("explorer", BadgeTier::Bronze);
        assert_eq!(badge.id, "explorer");
        assert_eq!(badge.name, "Badge explorer");
        assert_eq!(badge.tier, BadgeTier::Bronze);
        assert!(badge.earned_at.is_none());
    }

    #[test]
    fn badge_earned() {
        let badge = make_badge("creator", BadgeTier::Silver).earned();
        assert!(badge.earned_at.is_some());
    }

    #[test]
    fn badge_earned_at_specific_time() {
        let when = Utc::now();
        let badge = make_badge("mentor", BadgeTier::Gold).earned_at(when);
        assert_eq!(badge.earned_at, Some(when));
    }

    #[test]
    fn badge_serde_round_trip() {
        let badge = make_badge("legendary", BadgeTier::Legendary).earned();
        let json = serde_json::to_string(&badge).unwrap();
        let restored: Badge = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, badge.id);
        assert_eq!(restored.tier, badge.tier);
        assert!(restored.earned_at.is_some());
    }

    #[test]
    fn badge_tier_ordering() {
        // Tiers are distinct values
        let tiers = [
            BadgeTier::Bronze,
            BadgeTier::Silver,
            BadgeTier::Gold,
            BadgeTier::Platinum,
            BadgeTier::Legendary,
        ];
        for (i, a) in tiers.iter().enumerate() {
            for (j, b) in tiers.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // --- RewardType tests ---

    #[test]
    fn reward_type_cool() {
        let rt = RewardType::Cool(100);
        let json = serde_json::to_string(&rt).unwrap();
        let restored: RewardType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, rt);
    }

    #[test]
    fn reward_type_unlock() {
        let rt = RewardType::Unlock("dark-theme".into());
        let json = serde_json::to_string(&rt).unwrap();
        let restored: RewardType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, rt);
    }

    #[test]
    fn reward_type_title() {
        let rt = RewardType::Title("Sovereign Artisan".into());
        let json = serde_json::to_string(&rt).unwrap();
        let restored: RewardType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, rt);
    }

    #[test]
    fn reward_type_skill_point() {
        let rt = RewardType::SkillPoint {
            tree_id: "studio-design".into(),
            points: 5,
        };
        let json = serde_json::to_string(&rt).unwrap();
        let restored: RewardType = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, rt);
    }

    // --- Reward tests ---

    #[test]
    fn reward_creation() {
        let reward = make_cool_reward(50, RewardSource::Mission);
        assert!(!reward.claimed);
        assert!(reward.claimed_at.is_none());
        assert_eq!(reward.recipient, "cpub1alice");
    }

    #[test]
    fn reward_claim() {
        let mut reward = make_cool_reward(50, RewardSource::Mission);
        reward.claim();
        assert!(reward.claimed);
        assert!(reward.claimed_at.is_some());
    }

    #[test]
    fn reward_serde_round_trip() {
        let mut reward = make_cool_reward(100, RewardSource::Achievement);
        reward.claim();
        let json = serde_json::to_string(&reward).unwrap();
        let restored: Reward = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, reward.id);
        assert!(restored.claimed);
        assert!(restored.claimed_at.is_some());
    }

    // --- RewardLedger tests ---

    #[test]
    fn ledger_empty() {
        let ledger = RewardLedger::new();
        assert_eq!(ledger.count(), 0);
        assert!(ledger.unclaimed().is_empty());
        assert_eq!(ledger.total_cool_earned(), 0);
        assert!(ledger.badges().is_empty());
    }

    #[test]
    fn ledger_grant_and_count() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_cool_reward(50, RewardSource::Mission));
        assert_eq!(ledger.count(), 2);
    }

    #[test]
    fn ledger_claim() {
        let mut ledger = RewardLedger::new();
        let reward = make_cool_reward(100, RewardSource::Achievement);
        let id = reward.id;
        ledger.grant(reward);

        assert_eq!(ledger.unclaimed().len(), 1);
        assert!(ledger.claim(id));
        assert!(ledger.unclaimed().is_empty());
    }

    #[test]
    fn ledger_double_claim_returns_false() {
        let mut ledger = RewardLedger::new();
        let reward = make_cool_reward(100, RewardSource::Achievement);
        let id = reward.id;
        ledger.grant(reward);

        assert!(ledger.claim(id));
        assert!(!ledger.claim(id)); // already claimed
    }

    #[test]
    fn ledger_claim_nonexistent_returns_false() {
        let mut ledger = RewardLedger::new();
        assert!(!ledger.claim(Uuid::new_v4()));
    }

    #[test]
    fn ledger_by_source() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_cool_reward(50, RewardSource::Mission));
        ledger.grant(make_cool_reward(25, RewardSource::Achievement));

        assert_eq!(ledger.by_source(RewardSource::Achievement).len(), 2);
        assert_eq!(ledger.by_source(RewardSource::Mission).len(), 1);
        assert_eq!(ledger.by_source(RewardSource::Challenge).len(), 0);
    }

    #[test]
    fn ledger_by_type() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_badge_reward(make_badge("test", BadgeTier::Bronze)));
        ledger.grant(make_cool_reward(50, RewardSource::Mission));

        let cool_rewards = ledger.by_type(&RewardType::Cool(0));
        assert_eq!(cool_rewards.len(), 2);

        let badge_rewards = ledger.by_type(&RewardType::Badge(make_badge("x", BadgeTier::Bronze)));
        assert_eq!(badge_rewards.len(), 1);
    }

    #[test]
    fn ledger_total_cool_earned() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_cool_reward(50, RewardSource::Mission));
        ledger.grant(make_badge_reward(make_badge("test", BadgeTier::Bronze)));

        assert_eq!(ledger.total_cool_earned(), 150);
    }

    #[test]
    fn ledger_badges() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_badge_reward(make_badge("first", BadgeTier::Bronze)));
        ledger.grant(make_badge_reward(make_badge("second", BadgeTier::Silver)));

        let badges = ledger.badges();
        assert_eq!(badges.len(), 2);
        assert_eq!(badges[0].id, "first");
        assert_eq!(badges[1].id, "second");
    }

    #[test]
    fn ledger_recent() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_cool_reward(200, RewardSource::Mission));
        ledger.grant(make_cool_reward(300, RewardSource::Challenge));

        let recent = ledger.recent(2);
        assert_eq!(recent.len(), 2);
        // Most recent should be first (300, then 200)
        if let RewardType::Cool(amount) = &recent[0].reward_type {
            assert_eq!(*amount, 300);
        } else {
            panic!("expected Cool reward");
        }
    }

    #[test]
    fn ledger_recent_more_than_available() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));

        let recent = ledger.recent(10);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn ledger_serde_round_trip() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(100, RewardSource::Achievement));
        ledger.grant(make_badge_reward(make_badge("test", BadgeTier::Gold)));
        let id = ledger.all()[0].id;
        ledger.claim(id);

        let json = serde_json::to_string(&ledger).unwrap();
        let restored: RewardLedger = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.count(), 2);
        assert_eq!(restored.total_cool_earned(), 100);
        assert_eq!(restored.unclaimed().len(), 1);
    }

    #[test]
    fn reward_source_serde_round_trip() {
        let sources = [
            RewardSource::Achievement,
            RewardSource::Mission,
            RewardSource::Challenge,
            RewardSource::Mentorship,
            RewardSource::Community,
            RewardSource::Consortia,
        ];
        for source in &sources {
            let json = serde_json::to_string(source).unwrap();
            let restored: RewardSource = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, source);
        }
    }

    #[test]
    fn ledger_all_returns_slice() {
        let mut ledger = RewardLedger::new();
        ledger.grant(make_cool_reward(10, RewardSource::Community));
        assert_eq!(ledger.all().len(), 1);
    }

    #[test]
    fn reward_type_debug_format() {
        let rt = RewardType::Cool(42);
        let debug = format!("{rt:?}");
        assert!(debug.contains("Cool"));
        assert!(debug.contains("42"));
    }
}
