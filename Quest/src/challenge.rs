//! Challenges -- community and industry-wide events.
//!
//! Challenges are time-scoped activities that bring participants together around a
//! shared goal. They can be creative ("Design with only two colors"), communal
//! ("Our community publishes 100 ideas this month"), or cooperative ("Help N newcomers").
//!
//! # Design Principles
//!
//! - **Cooperative > competitive.** Most challenges use `CriteriaScope::Collective` --
//!   everyone contributes toward a shared target.
//! - **Opt-in everything.** Joining a challenge is a conscious choice.
//! - **No punitive mechanics.** An ended challenge is an ended challenge. No shame,
//!   no lost progress.
//!
//! # Example
//!
//! ```
//! use quest::challenge::{
//!     Challenge, ChallengeType, ChallengeScope, ChallengeCreator,
//!     ChallengeCriteria, CriteriaScope, ChallengeBoard, ChallengeParticipant,
//! };
//! use chrono::Utc;
//!
//! let mut board = ChallengeBoard::new();
//!
//! let challenge = Challenge::new("100 Ideas", "Publish 100 ideas as a community")
//!     .with_type(ChallengeType::Community)
//!     .with_criteria(ChallengeCriteria {
//!         metric: "ideas_published".into(),
//!         target: 100,
//!         scope: CriteriaScope::Collective,
//!     })
//!     .with_xp_reward(200)
//!     .with_starts_at(Utc::now());
//!
//! board.add_challenge(challenge);
//! assert_eq!(board.challenge_count(), 1);
//! ```

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::QuestError;
use crate::progression::XpAmount;
use crate::reward::RewardType;

// ---------------------------------------------------------------------------
// Challenge
// ---------------------------------------------------------------------------

/// A time-scoped activity bringing participants together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// What this challenge is about.
    pub description: String,
    /// Classification of the challenge.
    pub challenge_type: ChallengeType,
    /// What needs to happen and how it's measured.
    pub criteria: ChallengeCriteria,
    /// Rewards for participants who complete the challenge.
    pub rewards: Vec<RewardType>,
    /// XP awarded on completion.
    pub xp_reward: XpAmount,
    /// When the challenge starts.
    pub starts_at: DateTime<Utc>,
    /// When the challenge ends. `None` = perpetual.
    pub ends_at: Option<DateTime<Utc>>,
    /// Who can participate.
    pub scope: ChallengeScope,
    /// Who created this challenge.
    pub created_by: ChallengeCreator,
    /// Maximum number of participants. `None` = unlimited.
    pub max_participants: Option<u32>,
    /// Current challenge state.
    pub status: ChallengeStatus,
}

impl Challenge {
    /// Create a new challenge with sensible defaults.
    ///
    /// Defaults to `ChallengeType::Community`, global scope, system-created,
    /// `ChallengeStatus::Upcoming`, no participant limit, no end date.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            challenge_type: ChallengeType::Community,
            criteria: ChallengeCriteria {
                metric: String::new(),
                target: 0,
                scope: CriteriaScope::Collective,
            },
            rewards: Vec::new(),
            xp_reward: 0,
            starts_at: Utc::now(),
            ends_at: None,
            scope: ChallengeScope::Global,
            created_by: ChallengeCreator::System,
            max_participants: None,
            status: ChallengeStatus::Upcoming,
        }
    }

    /// Set the challenge type.
    pub fn with_type(mut self, challenge_type: ChallengeType) -> Self {
        self.challenge_type = challenge_type;
        self
    }

    /// Set the criteria.
    pub fn with_criteria(mut self, criteria: ChallengeCriteria) -> Self {
        self.criteria = criteria;
        self
    }

    /// Add a reward.
    pub fn with_reward(mut self, reward: RewardType) -> Self {
        self.rewards.push(reward);
        self
    }

    /// Set the XP reward.
    pub fn with_xp_reward(mut self, xp: XpAmount) -> Self {
        self.xp_reward = xp;
        self
    }

    /// Set the start time.
    pub fn with_starts_at(mut self, when: DateTime<Utc>) -> Self {
        self.starts_at = when;
        self
    }

    /// Set the end time.
    pub fn with_ends_at(mut self, when: DateTime<Utc>) -> Self {
        self.ends_at = Some(when);
        self
    }

    /// Set the scope.
    pub fn with_scope(mut self, scope: ChallengeScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set who created this challenge.
    pub fn with_creator(mut self, creator: ChallengeCreator) -> Self {
        self.created_by = creator;
        self
    }

    /// Set a maximum number of participants.
    pub fn with_max_participants(mut self, max: u32) -> Self {
        self.max_participants = Some(max);
        self
    }

    /// Set the initial status.
    pub fn with_status(mut self, status: ChallengeStatus) -> Self {
        self.status = status;
        self
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Classification of what kind of challenge this is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeType {
    /// "Design something with only two colors."
    Creative,
    /// "Our community publishes 100 ideas this month."
    Community,
    /// "Build the most accessible product."
    Innovation,
    /// Time-themed (harvest festival, winter solstice, etc.).
    Seasonal,
    /// Group effort, shared reward.
    Cooperative,
    /// Help N newcomers get started.
    Mentorship,
    /// "Achieve 100% voter turnout."
    Governance,
    /// Anything else.
    Custom(String),
}

/// Who can participate in this challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeScope {
    /// Open to everyone.
    Global,
    /// Limited to a specific community.
    Community {
        /// The community this challenge belongs to.
        community_id: Uuid,
    },
    /// Sponsored by a consortium.
    Consortia {
        /// The sponsoring consortium.
        consortia_id: Uuid,
    },
    /// A challenge between multiple communities.
    InterCommunity {
        /// The participating communities.
        community_ids: Vec<Uuid>,
    },
}

/// Who created this challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChallengeCreator {
    /// System-created challenge.
    System,
    /// Created by a community member.
    Community {
        /// The community.
        community_id: Uuid,
        /// The creator's public key.
        creator_pubkey: String,
    },
    /// Created by a consortium.
    Consortia {
        /// The consortium.
        consortia_id: Uuid,
        /// The creator's public key.
        creator_pubkey: String,
    },
}

/// The lifecycle state of a challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChallengeStatus {
    /// Not yet started.
    Upcoming,
    /// Currently running.
    Active,
    /// Target met by participants.
    Completed,
    /// Time expired without completion.
    Ended,
    /// Cancelled by the creator or system.
    Cancelled,
}

// ---------------------------------------------------------------------------
// ChallengeCriteria
// ---------------------------------------------------------------------------

/// What needs to happen for the challenge to be completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeCriteria {
    /// What to measure (e.g., "ideas_published", "votes_cast").
    pub metric: String,
    /// The numeric target.
    pub target: u64,
    /// Whether each participant must hit the target individually or the group collectively.
    pub scope: CriteriaScope,
}

/// Whether the challenge target applies to individuals or the group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CriteriaScope {
    /// Each participant must individually reach the target.
    Individual,
    /// The group total must reach the target.
    Collective,
}

// ---------------------------------------------------------------------------
// ChallengeEntry
// ---------------------------------------------------------------------------

/// A participant's entry in a challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeEntry {
    /// Which challenge this entry belongs to.
    pub challenge_id: Uuid,
    /// Who or what is participating.
    pub participant: ChallengeParticipant,
    /// Current progress toward the target.
    pub progress: u64,
    /// When the participant joined.
    pub joined_at: DateTime<Utc>,
    /// When the participant completed the challenge (if individual scope).
    pub completed_at: Option<DateTime<Utc>>,
}

impl ChallengeEntry {
    /// Create a new entry for a participant joining a challenge.
    pub fn new(challenge_id: Uuid, participant: ChallengeParticipant) -> Self {
        Self {
            challenge_id,
            participant,
            progress: 0,
            joined_at: Utc::now(),
            completed_at: None,
        }
    }
}

/// Who or what is participating in a challenge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChallengeParticipant {
    /// A single person.
    Individual {
        /// The participant's public key.
        pubkey: String,
    },
    /// A whole community.
    Community {
        /// The community.
        community_id: Uuid,
    },
    /// A consortium.
    Consortia {
        /// The consortium.
        consortia_id: Uuid,
    },
}

// ---------------------------------------------------------------------------
// ChallengeBoard
// ---------------------------------------------------------------------------

/// Manages all challenges and their entries.
///
/// The board is the coordination point for challenge lifecycle: creation,
/// participation, progress tracking, and completion detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChallengeBoard {
    challenges: Vec<Challenge>,
    entries: Vec<ChallengeEntry>,
}

impl ChallengeBoard {
    /// Create an empty challenge board.
    pub fn new() -> Self {
        Self {
            challenges: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Register a new challenge.
    pub fn add_challenge(&mut self, challenge: Challenge) {
        self.challenges.push(challenge);
    }

    /// Look up a challenge by ID.
    pub fn get_challenge(&self, challenge_id: Uuid) -> Option<&Challenge> {
        self.challenges.iter().find(|c| c.id == challenge_id)
    }

    /// Mutably look up a challenge by ID.
    fn get_challenge_mut(&mut self, challenge_id: Uuid) -> Option<&mut Challenge> {
        self.challenges.iter_mut().find(|c| c.id == challenge_id)
    }

    /// All currently active challenges.
    pub fn list_active(&self) -> Vec<&Challenge> {
        self.challenges
            .iter()
            .filter(|c| c.status == ChallengeStatus::Active)
            .collect()
    }

    /// All upcoming challenges.
    pub fn list_upcoming(&self) -> Vec<&Challenge> {
        self.challenges
            .iter()
            .filter(|c| c.status == ChallengeStatus::Upcoming)
            .collect()
    }

    /// Join a challenge. Returns the new entry.
    pub fn join(
        &mut self,
        challenge_id: Uuid,
        participant: ChallengeParticipant,
    ) -> Result<ChallengeEntry, QuestError> {
        let challenge = self
            .get_challenge(challenge_id)
            .ok_or_else(|| QuestError::NotFound(format!("challenge {challenge_id}")))?;

        // Must be active
        if challenge.status != ChallengeStatus::Active {
            return Err(QuestError::InvalidState(format!(
                "challenge is {:?}, not Active",
                challenge.status
            )));
        }

        // Check participant limit
        if let Some(max) = challenge.max_participants {
            let current_count = self
                .entries
                .iter()
                .filter(|e| e.challenge_id == challenge_id)
                .count() as u32;
            if current_count >= max {
                return Err(QuestError::NotEligible(
                    "challenge is full".into(),
                ));
            }
        }

        // Check for duplicate entry
        let already_joined = self.entries.iter().any(|e| {
            e.challenge_id == challenge_id && e.participant == participant
        });
        if already_joined {
            return Err(QuestError::AlreadyExists(
                "already joined this challenge".into(),
            ));
        }

        let entry = ChallengeEntry::new(challenge_id, participant);
        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Update a participant's progress in a challenge.
    pub fn update_progress(
        &mut self,
        challenge_id: Uuid,
        participant: &ChallengeParticipant,
        increment: u64,
    ) -> Result<(), QuestError> {
        let challenge = self
            .get_challenge(challenge_id)
            .ok_or_else(|| QuestError::NotFound(format!("challenge {challenge_id}")))?;

        if challenge.status != ChallengeStatus::Active {
            return Err(QuestError::InvalidState(
                "challenge is not active".into(),
            ));
        }

        let target = challenge.criteria.target;
        let scope = challenge.criteria.scope;

        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.challenge_id == challenge_id && &e.participant == participant)
            .ok_or_else(|| {
                QuestError::NotFound("participant not found in challenge".into())
            })?;

        entry.progress = entry.progress.saturating_add(increment);

        // For individual scope, mark individual completion
        if scope == CriteriaScope::Individual && entry.progress >= target && entry.completed_at.is_none() {
            entry.completed_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Check whether a challenge should be marked as completed.
    ///
    /// For `CriteriaScope::Collective`, checks if the sum of all participants'
    /// progress meets the target. For `CriteriaScope::Individual`, checks if all
    /// participants have individually met the target.
    pub fn check_completion(&mut self, challenge_id: Uuid) -> Result<ChallengeStatus, QuestError> {
        let challenge = self
            .get_challenge(challenge_id)
            .ok_or_else(|| QuestError::NotFound(format!("challenge {challenge_id}")))?;

        if challenge.status != ChallengeStatus::Active {
            return Ok(challenge.status);
        }

        let target = challenge.criteria.target;
        let scope = challenge.criteria.scope;

        let entries: Vec<&ChallengeEntry> = self
            .entries
            .iter()
            .filter(|e| e.challenge_id == challenge_id)
            .collect();

        let completed = match scope {
            CriteriaScope::Collective => {
                let total: u64 = entries.iter().map(|e| e.progress).sum();
                total >= target
            }
            CriteriaScope::Individual => {
                !entries.is_empty() && entries.iter().all(|e| e.progress >= target)
            }
        };

        if completed {
            if let Some(c) = self.get_challenge_mut(challenge_id) {
                c.status = ChallengeStatus::Completed;
            }
            Ok(ChallengeStatus::Completed)
        } else {
            Ok(ChallengeStatus::Active)
        }
    }

    /// All entries (participants) for a given challenge.
    pub fn participants(&self, challenge_id: Uuid) -> Vec<&ChallengeEntry> {
        self.entries
            .iter()
            .filter(|e| e.challenge_id == challenge_id)
            .collect()
    }

    /// Sum of all participants' progress for a collective challenge.
    pub fn collective_progress(&self, challenge_id: Uuid) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.challenge_id == challenge_id)
            .map(|e| e.progress)
            .sum()
    }

    /// All challenges of a specific type.
    pub fn by_type(&self, challenge_type: &ChallengeType) -> Vec<&Challenge> {
        self.challenges
            .iter()
            .filter(|c| &c.challenge_type == challenge_type)
            .collect()
    }

    /// All challenges in a specific scope.
    pub fn by_scope(&self, scope: &ChallengeScope) -> Vec<&Challenge> {
        self.challenges
            .iter()
            .filter(|c| &c.scope == scope)
            .collect()
    }

    /// Total number of registered challenges.
    pub fn challenge_count(&self) -> usize {
        self.challenges.len()
    }

    /// Index of all challenge entries keyed by participant, for efficient lookup.
    pub fn entries_by_participant(&self) -> HashMap<&ChallengeParticipant, Vec<&ChallengeEntry>> {
        let mut map: HashMap<&ChallengeParticipant, Vec<&ChallengeEntry>> = HashMap::new();
        for entry in &self.entries {
            map.entry(&entry.participant).or_default().push(entry);
        }
        map
    }

    // --- Federation-scoped queries ---

    /// All active challenges visible within the federation scope.
    ///
    /// A challenge is visible if its scope matches a visible community:
    /// - `ChallengeScope::Global` -- always visible.
    /// - `ChallengeScope::Community { community_id }` -- visible if community_id is in scope.
    /// - `ChallengeScope::Consortia { consortia_id }` -- visible if consortia_id is in scope.
    /// - `ChallengeScope::InterCommunity { community_ids }` -- visible if any ID is in scope.
    ///
    /// When the scope is unrestricted, all active challenges are returned.
    pub fn list_active_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&Challenge> {
        if scope.is_unrestricted() {
            return self.list_active();
        }
        self.challenges
            .iter()
            .filter(|c| c.status == ChallengeStatus::Active && Self::challenge_is_visible(c, scope))
            .collect()
    }

    /// All challenges (any status) visible within the federation scope.
    ///
    /// Uses the same visibility rules as [`list_active_scoped`](Self::list_active_scoped).
    pub fn challenges_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> Vec<&Challenge> {
        if scope.is_unrestricted() {
            return self.challenges.iter().collect();
        }
        self.challenges
            .iter()
            .filter(|c| Self::challenge_is_visible(c, scope))
            .collect()
    }

    /// Count of challenges visible within the federation scope.
    pub fn challenge_count_scoped(
        &self,
        scope: &crate::federation_scope::FederationScope,
    ) -> usize {
        if scope.is_unrestricted() {
            return self.challenge_count();
        }
        self.challenges
            .iter()
            .filter(|c| Self::challenge_is_visible(c, scope))
            .count()
    }

    /// Check whether a challenge is visible within a federation scope.
    fn challenge_is_visible(
        challenge: &Challenge,
        scope: &crate::federation_scope::FederationScope,
    ) -> bool {
        match &challenge.scope {
            ChallengeScope::Global => true,
            ChallengeScope::Community { community_id } => {
                scope.is_visible_uuid(community_id)
            }
            ChallengeScope::Consortia { consortia_id } => {
                scope.is_visible_uuid(consortia_id)
            }
            ChallengeScope::InterCommunity { community_ids } => {
                community_ids.iter().any(|id| scope.is_visible_uuid(id))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample_challenge() -> Challenge {
        Challenge::new("Community Sprint", "Publish 50 ideas together")
            .with_type(ChallengeType::Community)
            .with_criteria(ChallengeCriteria {
                metric: "ideas_published".into(),
                target: 50,
                scope: CriteriaScope::Collective,
            })
            .with_xp_reward(200)
            .with_status(ChallengeStatus::Active)
    }

    fn alice() -> ChallengeParticipant {
        ChallengeParticipant::Individual {
            pubkey: "cpub1alice".into(),
        }
    }

    fn bob() -> ChallengeParticipant {
        ChallengeParticipant::Individual {
            pubkey: "cpub1bob".into(),
        }
    }

    // --- Challenge construction ---

    #[test]
    fn challenge_new() {
        let c = Challenge::new("Test", "A test challenge");
        assert_eq!(c.name, "Test");
        assert_eq!(c.description, "A test challenge");
        assert_eq!(c.challenge_type, ChallengeType::Community);
        assert_eq!(c.status, ChallengeStatus::Upcoming);
        assert!(c.max_participants.is_none());
        assert!(c.ends_at.is_none());
    }

    #[test]
    fn challenge_builder() {
        let now = Utc::now();
        let end = now + Duration::days(30);
        let c = Challenge::new("Builder", "Built with builder")
            .with_type(ChallengeType::Creative)
            .with_criteria(ChallengeCriteria {
                metric: "designs".into(),
                target: 10,
                scope: CriteriaScope::Individual,
            })
            .with_reward(RewardType::Cool(100))
            .with_xp_reward(500)
            .with_starts_at(now)
            .with_ends_at(end)
            .with_scope(ChallengeScope::Global)
            .with_creator(ChallengeCreator::System)
            .with_max_participants(50)
            .with_status(ChallengeStatus::Active);

        assert_eq!(c.challenge_type, ChallengeType::Creative);
        assert_eq!(c.criteria.target, 10);
        assert_eq!(c.rewards.len(), 1);
        assert_eq!(c.xp_reward, 500);
        assert_eq!(c.max_participants, Some(50));
        assert_eq!(c.status, ChallengeStatus::Active);
    }

    #[test]
    fn challenge_serde_round_trip() {
        let c = sample_challenge();
        let json = serde_json::to_string(&c).unwrap();
        let restored: Challenge = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, c.id);
        assert_eq!(restored.name, "Community Sprint");
        assert_eq!(restored.criteria.target, 50);
    }

    // --- ChallengeType ---

    #[test]
    fn challenge_type_serde_round_trip() {
        let types = vec![
            ChallengeType::Creative,
            ChallengeType::Community,
            ChallengeType::Innovation,
            ChallengeType::Seasonal,
            ChallengeType::Cooperative,
            ChallengeType::Mentorship,
            ChallengeType::Governance,
            ChallengeType::Custom("hackathon".into()),
        ];
        for ct in &types {
            let json = serde_json::to_string(ct).unwrap();
            let restored: ChallengeType = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, ct);
        }
    }

    // --- ChallengeScope ---

    #[test]
    fn challenge_scope_serde_round_trip() {
        let scopes = vec![
            ChallengeScope::Global,
            ChallengeScope::Community {
                community_id: Uuid::new_v4(),
            },
            ChallengeScope::Consortia {
                consortia_id: Uuid::new_v4(),
            },
            ChallengeScope::InterCommunity {
                community_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
            },
        ];
        for scope in &scopes {
            let json = serde_json::to_string(scope).unwrap();
            let restored: ChallengeScope = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, scope);
        }
    }

    // --- ChallengeCreator ---

    #[test]
    fn challenge_creator_serde_round_trip() {
        let creators = vec![
            ChallengeCreator::System,
            ChallengeCreator::Community {
                community_id: Uuid::new_v4(),
                creator_pubkey: "cpub1creator".into(),
            },
            ChallengeCreator::Consortia {
                consortia_id: Uuid::new_v4(),
                creator_pubkey: "cpub1sponsor".into(),
            },
        ];
        for creator in &creators {
            let json = serde_json::to_string(creator).unwrap();
            let restored: ChallengeCreator = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, creator);
        }
    }

    // --- ChallengeStatus ---

    #[test]
    fn challenge_status_serde_round_trip() {
        let statuses = [
            ChallengeStatus::Upcoming,
            ChallengeStatus::Active,
            ChallengeStatus::Completed,
            ChallengeStatus::Ended,
            ChallengeStatus::Cancelled,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let restored: ChallengeStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, s);
        }
    }

    // --- ChallengeCriteria ---

    #[test]
    fn challenge_criteria_serde_round_trip() {
        let criteria = ChallengeCriteria {
            metric: "posts".into(),
            target: 100,
            scope: CriteriaScope::Collective,
        };
        let json = serde_json::to_string(&criteria).unwrap();
        let restored: ChallengeCriteria = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, criteria);
    }

    // --- CriteriaScope ---

    #[test]
    fn criteria_scope_serde_round_trip() {
        for scope in [CriteriaScope::Individual, CriteriaScope::Collective] {
            let json = serde_json::to_string(&scope).unwrap();
            let restored: CriteriaScope = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, scope);
        }
    }

    // --- ChallengeEntry ---

    #[test]
    fn challenge_entry_new() {
        let id = Uuid::new_v4();
        let entry = ChallengeEntry::new(id, alice());
        assert_eq!(entry.challenge_id, id);
        assert_eq!(entry.progress, 0);
        assert!(entry.completed_at.is_none());
    }

    #[test]
    fn challenge_entry_serde_round_trip() {
        let entry = ChallengeEntry::new(Uuid::new_v4(), alice());
        let json = serde_json::to_string(&entry).unwrap();
        let restored: ChallengeEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.challenge_id, entry.challenge_id);
        assert_eq!(restored.participant, alice());
    }

    // --- ChallengeParticipant ---

    #[test]
    fn challenge_participant_serde_round_trip() {
        let participants = vec![
            ChallengeParticipant::Individual {
                pubkey: "cpub1x".into(),
            },
            ChallengeParticipant::Community {
                community_id: Uuid::new_v4(),
            },
            ChallengeParticipant::Consortia {
                consortia_id: Uuid::new_v4(),
            },
        ];
        for p in &participants {
            let json = serde_json::to_string(p).unwrap();
            let restored: ChallengeParticipant = serde_json::from_str(&json).unwrap();
            assert_eq!(&restored, p);
        }
    }

    // --- ChallengeBoard ---

    #[test]
    fn board_empty() {
        let board = ChallengeBoard::new();
        assert_eq!(board.challenge_count(), 0);
        assert!(board.list_active().is_empty());
        assert!(board.list_upcoming().is_empty());
    }

    #[test]
    fn board_add_and_get() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);
        assert_eq!(board.challenge_count(), 1);
        assert!(board.get_challenge(id).is_some());
        assert!(board.get_challenge(Uuid::new_v4()).is_none());
    }

    #[test]
    fn board_list_active() {
        let mut board = ChallengeBoard::new();
        board.add_challenge(sample_challenge()); // Active
        board.add_challenge(Challenge::new("Upcoming", "desc")); // Upcoming by default
        assert_eq!(board.list_active().len(), 1);
    }

    #[test]
    fn board_list_upcoming() {
        let mut board = ChallengeBoard::new();
        board.add_challenge(Challenge::new("Soon", "desc")); // Upcoming by default
        board.add_challenge(sample_challenge()); // Active
        assert_eq!(board.list_upcoming().len(), 1);
    }

    #[test]
    fn board_join() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);

        let entry = board.join(id, alice()).unwrap();
        assert_eq!(entry.challenge_id, id);
        assert_eq!(entry.progress, 0);
    }

    #[test]
    fn board_join_not_found() {
        let mut board = ChallengeBoard::new();
        let result = board.join(Uuid::new_v4(), alice());
        assert!(result.is_err());
    }

    #[test]
    fn board_join_not_active() {
        let mut board = ChallengeBoard::new();
        let c = Challenge::new("Upcoming", "desc"); // Upcoming, not Active
        let id = c.id;
        board.add_challenge(c);

        let result = board.join(id, alice());
        assert!(result.is_err());
    }

    #[test]
    fn board_join_full() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge().with_max_participants(1);
        let id = c.id;
        board.add_challenge(c);

        board.join(id, alice()).unwrap();
        let result = board.join(id, bob());
        assert!(result.is_err());
    }

    #[test]
    fn board_join_duplicate() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);

        board.join(id, alice()).unwrap();
        let result = board.join(id, alice());
        assert!(result.is_err());
    }

    #[test]
    fn board_update_progress() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);
        board.join(id, alice()).unwrap();

        board.update_progress(id, &alice(), 10).unwrap();
        let entries = board.participants(id);
        assert_eq!(entries[0].progress, 10);
    }

    #[test]
    fn board_update_progress_not_active() {
        let mut board = ChallengeBoard::new();
        let c = Challenge::new("Ended", "desc").with_status(ChallengeStatus::Ended);
        let id = c.id;
        board.add_challenge(c);

        let result = board.update_progress(id, &alice(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn board_update_progress_not_joined() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);

        let result = board.update_progress(id, &alice(), 10);
        assert!(result.is_err());
    }

    #[test]
    fn board_check_completion_collective() {
        let mut board = ChallengeBoard::new();
        let mut c = sample_challenge();
        c.criteria.target = 20;
        let id = c.id;
        board.add_challenge(c);

        board.join(id, alice()).unwrap();
        board.join(id, bob()).unwrap();

        board.update_progress(id, &alice(), 12).unwrap();
        board.update_progress(id, &bob(), 5).unwrap();

        // 17 < 20, not done yet
        let status = board.check_completion(id).unwrap();
        assert_eq!(status, ChallengeStatus::Active);

        board.update_progress(id, &bob(), 5).unwrap();

        // 12 + 10 = 22 >= 20
        let status = board.check_completion(id).unwrap();
        assert_eq!(status, ChallengeStatus::Completed);
    }

    #[test]
    fn board_check_completion_individual() {
        let mut board = ChallengeBoard::new();
        let c = Challenge::new("Individual", "desc")
            .with_criteria(ChallengeCriteria {
                metric: "posts".into(),
                target: 5,
                scope: CriteriaScope::Individual,
            })
            .with_status(ChallengeStatus::Active);
        let id = c.id;
        board.add_challenge(c);

        board.join(id, alice()).unwrap();
        board.join(id, bob()).unwrap();

        board.update_progress(id, &alice(), 5).unwrap();

        // Alice done, but Bob not
        let status = board.check_completion(id).unwrap();
        assert_eq!(status, ChallengeStatus::Active);

        board.update_progress(id, &bob(), 5).unwrap();

        // Both done
        let status = board.check_completion(id).unwrap();
        assert_eq!(status, ChallengeStatus::Completed);
    }

    #[test]
    fn board_check_completion_individual_marks_entry() {
        let mut board = ChallengeBoard::new();
        let c = Challenge::new("IndividualMark", "desc")
            .with_criteria(ChallengeCriteria {
                metric: "posts".into(),
                target: 3,
                scope: CriteriaScope::Individual,
            })
            .with_status(ChallengeStatus::Active);
        let id = c.id;
        board.add_challenge(c);
        board.join(id, alice()).unwrap();

        board.update_progress(id, &alice(), 3).unwrap();
        let entries = board.participants(id);
        assert!(entries[0].completed_at.is_some());
    }

    #[test]
    fn board_collective_progress() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);

        board.join(id, alice()).unwrap();
        board.join(id, bob()).unwrap();

        board.update_progress(id, &alice(), 15).unwrap();
        board.update_progress(id, &bob(), 10).unwrap();

        assert_eq!(board.collective_progress(id), 25);
    }

    #[test]
    fn board_collective_progress_empty() {
        let board = ChallengeBoard::new();
        assert_eq!(board.collective_progress(Uuid::new_v4()), 0);
    }

    #[test]
    fn board_participants() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);

        board.join(id, alice()).unwrap();
        board.join(id, bob()).unwrap();

        assert_eq!(board.participants(id).len(), 2);
        assert!(board.participants(Uuid::new_v4()).is_empty());
    }

    #[test]
    fn board_by_type() {
        let mut board = ChallengeBoard::new();
        board.add_challenge(sample_challenge()); // Community
        board.add_challenge(
            Challenge::new("Creative", "desc").with_type(ChallengeType::Creative),
        );

        assert_eq!(board.by_type(&ChallengeType::Community).len(), 1);
        assert_eq!(board.by_type(&ChallengeType::Creative).len(), 1);
        assert!(board.by_type(&ChallengeType::Seasonal).is_empty());
    }

    #[test]
    fn board_by_scope() {
        let mut board = ChallengeBoard::new();
        board.add_challenge(sample_challenge()); // Global

        assert_eq!(board.by_scope(&ChallengeScope::Global).len(), 1);
        assert!(board
            .by_scope(&ChallengeScope::Community {
                community_id: Uuid::new_v4()
            })
            .is_empty());
    }

    #[test]
    fn board_serde_round_trip() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);
        board.join(id, alice()).unwrap();
        board.update_progress(id, &alice(), 5).unwrap();

        let json = serde_json::to_string(&board).unwrap();
        let restored: ChallengeBoard = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.challenge_count(), 1);
        assert_eq!(restored.collective_progress(id), 5);
    }

    #[test]
    fn board_entries_by_participant() {
        let mut board = ChallengeBoard::new();
        let c1 = sample_challenge();
        let c2 = Challenge::new("Second", "desc").with_status(ChallengeStatus::Active);
        let id1 = c1.id;
        let id2 = c2.id;
        board.add_challenge(c1);
        board.add_challenge(c2);

        board.join(id1, alice()).unwrap();
        board.join(id2, alice()).unwrap();
        board.join(id1, bob()).unwrap();

        let by_participant = board.entries_by_participant();
        assert_eq!(by_participant[&alice()].len(), 2);
        assert_eq!(by_participant[&bob()].len(), 1);
    }

    // --- No dark patterns ---

    #[test]
    fn ended_challenge_no_penalty() {
        // An ended challenge should not punish participants
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);
        board.join(id, alice()).unwrap();
        board.update_progress(id, &alice(), 10).unwrap();

        // Manually end the challenge
        board.get_challenge_mut(id).unwrap().status = ChallengeStatus::Ended;

        // Progress earned is still there
        assert_eq!(board.collective_progress(id), 10);
    }

    #[test]
    fn progress_saturating_add() {
        let mut board = ChallengeBoard::new();
        let c = sample_challenge();
        let id = c.id;
        board.add_challenge(c);
        board.join(id, alice()).unwrap();

        // Huge increment should not overflow
        board.update_progress(id, &alice(), u64::MAX).unwrap();
        board.update_progress(id, &alice(), 1).unwrap();
        assert_eq!(board.collective_progress(id), u64::MAX);
    }

    #[test]
    fn check_completion_not_active_returns_current() {
        let mut board = ChallengeBoard::new();
        let c = Challenge::new("Done", "desc").with_status(ChallengeStatus::Completed);
        let id = c.id;
        board.add_challenge(c);
        let status = board.check_completion(id).unwrap();
        assert_eq!(status, ChallengeStatus::Completed);
    }

    #[test]
    fn individual_completion_not_double_marked() {
        let mut board = ChallengeBoard::new();
        let c = Challenge::new("Individual", "desc")
            .with_criteria(ChallengeCriteria {
                metric: "m".into(),
                target: 5,
                scope: CriteriaScope::Individual,
            })
            .with_status(ChallengeStatus::Active);
        let id = c.id;
        board.add_challenge(c);
        board.join(id, alice()).unwrap();

        board.update_progress(id, &alice(), 5).unwrap();
        let first_completion = board.participants(id)[0].completed_at;

        // Update again -- completed_at should not change
        board.update_progress(id, &alice(), 5).unwrap();
        let second_completion = board.participants(id)[0].completed_at;
        assert_eq!(first_completion, second_completion);
    }

    // --- Federation-scoped queries ---

    #[test]
    fn list_active_scoped_unrestricted() {
        let mut board = ChallengeBoard::new();
        board.add_challenge(sample_challenge());

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(board.list_active_scoped(&scope).len(), 1);
    }

    #[test]
    fn list_active_scoped_global_always_visible() {
        let mut board = ChallengeBoard::new();
        // Global challenges are always visible, even with scoped federation
        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Global),
        );

        let scope = crate::federation_scope::FederationScope::from_communities([
            "some-community",
        ]);
        assert_eq!(board.list_active_scoped(&scope).len(), 1);
    }

    #[test]
    fn list_active_scoped_filters_community_challenges() {
        let mut board = ChallengeBoard::new();
        let cid_visible = Uuid::new_v4();
        let cid_hidden = Uuid::new_v4();

        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Community {
                community_id: cid_visible,
            }),
        );
        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Community {
                community_id: cid_hidden,
            }),
        );

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid_visible.to_string(),
        ]);
        let visible = board.list_active_scoped(&scope);
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn list_active_scoped_filters_consortia_challenges() {
        let mut board = ChallengeBoard::new();
        let cid = Uuid::new_v4();

        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Consortia {
                consortia_id: cid,
            }),
        );
        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Consortia {
                consortia_id: Uuid::new_v4(),
            }),
        );

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid.to_string(),
        ]);
        assert_eq!(board.list_active_scoped(&scope).len(), 1);
    }

    #[test]
    fn list_active_scoped_inter_community_partial_match() {
        let mut board = ChallengeBoard::new();
        let cid1 = Uuid::new_v4();
        let cid2 = Uuid::new_v4();
        let cid3 = Uuid::new_v4();

        // InterCommunity challenge with cid1 and cid2
        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::InterCommunity {
                community_ids: vec![cid1, cid2],
            }),
        );

        // Scope includes cid2 but not cid1 -- should still be visible
        let scope = crate::federation_scope::FederationScope::from_communities([
            cid2.to_string(),
        ]);
        assert_eq!(board.list_active_scoped(&scope).len(), 1);

        // Scope includes only cid3 -- should not be visible
        let scope2 = crate::federation_scope::FederationScope::from_communities([
            cid3.to_string(),
        ]);
        assert!(board.list_active_scoped(&scope2).is_empty());
    }

    #[test]
    fn list_active_scoped_excludes_inactive() {
        let mut board = ChallengeBoard::new();
        // Upcoming, not Active
        board.add_challenge(
            Challenge::new("Upcoming", "desc").with_scope(ChallengeScope::Global),
        );

        let scope = crate::federation_scope::FederationScope::new();
        assert!(board.list_active_scoped(&scope).is_empty());
    }

    #[test]
    fn challenges_scoped_returns_all_statuses() {
        let mut board = ChallengeBoard::new();
        let cid = Uuid::new_v4();

        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Community {
                community_id: cid,
            }),
        );
        board.add_challenge(
            Challenge::new("Upcoming", "desc")
                .with_scope(ChallengeScope::Community {
                    community_id: cid,
                }),
        );

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid.to_string(),
        ]);
        // Both Active and Upcoming challenges are returned
        assert_eq!(board.challenges_scoped(&scope).len(), 2);
    }

    #[test]
    fn challenge_count_scoped_unrestricted() {
        let mut board = ChallengeBoard::new();
        board.add_challenge(sample_challenge());
        board.add_challenge(Challenge::new("Another", "desc"));

        let scope = crate::federation_scope::FederationScope::new();
        assert_eq!(board.challenge_count_scoped(&scope), 2);
    }

    #[test]
    fn challenge_count_scoped_filters() {
        let mut board = ChallengeBoard::new();
        let cid = Uuid::new_v4();

        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Community {
                community_id: cid,
            }),
        );
        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Community {
                community_id: Uuid::new_v4(),
            }),
        );
        // Global always visible
        board.add_challenge(
            sample_challenge().with_scope(ChallengeScope::Global),
        );

        let scope = crate::federation_scope::FederationScope::from_communities([
            cid.to_string(),
        ]);
        // 1 community match + 1 global = 2
        assert_eq!(board.challenge_count_scoped(&scope), 2);
    }
}
