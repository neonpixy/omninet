//! # Governance Budget + Role Rotation (R3D)
//!
//! Two mechanisms to prevent governance fatigue and power concentration.
//!
//! **Governance Budget** — limits the number of active proposals a community
//! processes at once. Prevents proposal spam and decision fatigue. When the
//! budget is full, new proposals queue.
//!
//! **Role Rotation** — term limits and cooling-off periods for governance roles.
//! No coordinating position becomes a pathway to enduring power.
//!
//! From Constellation Art. 8 SS6: "Strict single-term limits for all coordinating
//! positions, prohibition on consecutive service in similar roles."

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::community::CommunityRole;
use crate::KingdomError;

// ---------------------------------------------------------------------------
// Governance Budget
// ---------------------------------------------------------------------------

/// Limits the number of active proposals to prevent governance fatigue.
///
/// When `current_count >= max_active_proposals`, new proposals enter a queue.
/// Proposers are notified when a slot opens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceBudget {
    pub id: Uuid,
    pub community_id: String,
    /// Maximum number of proposals in active voting at once.
    pub max_active_proposals: usize,
    /// Default proposal period in seconds (e.g. 30 days = 2_592_000).
    pub proposal_period_secs: u64,
    /// How many proposals are currently active.
    pub current_count: usize,
    /// Whether cooldown between proposals is enabled.
    pub cooldown_enabled: bool,
}

impl GovernanceBudget {
    /// Create a new governance budget with sensible defaults.
    ///
    /// Default: 5 active proposals, 30-day proposal period, cooldown enabled.
    pub fn new(community_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            community_id: community_id.into(),
            max_active_proposals: 5,
            proposal_period_secs: 30 * 24 * 3600, // 30 days
            current_count: 0,
            cooldown_enabled: true,
        }
    }

    pub fn with_max_proposals(mut self, max: usize) -> Self {
        self.max_active_proposals = max;
        self
    }

    pub fn with_proposal_period_secs(mut self, secs: u64) -> Self {
        self.proposal_period_secs = secs;
        self
    }

    pub fn with_cooldown(mut self, enabled: bool) -> Self {
        self.cooldown_enabled = enabled;
        self
    }

    /// Whether the governance budget has room for another active proposal.
    pub fn has_capacity(&self) -> bool {
        self.current_count < self.max_active_proposals
    }

    /// Try to activate a new proposal. Returns `Ok` if there's room,
    /// or `Err` if the budget is full (proposal should be queued).
    pub fn try_activate(&mut self) -> Result<(), KingdomError> {
        if !self.has_capacity() {
            return Err(KingdomError::GovernanceBudgetFull {
                community_id: self.community_id.clone(),
                max: self.max_active_proposals,
            });
        }
        self.current_count += 1;
        Ok(())
    }

    /// Mark a proposal as resolved, freeing a budget slot.
    pub fn release(&mut self) {
        self.current_count = self.current_count.saturating_sub(1);
    }

    /// How many slots are available.
    pub fn available_slots(&self) -> usize {
        self.max_active_proposals.saturating_sub(self.current_count)
    }
}

// ---------------------------------------------------------------------------
// Proposal Queue
// ---------------------------------------------------------------------------

/// An entry in the proposal queue, waiting for a governance budget slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedProposal {
    pub proposal_id: Uuid,
    pub author: String,
    pub title: String,
    pub queued_at: DateTime<Utc>,
}

impl QueuedProposal {
    pub fn new(
        proposal_id: Uuid,
        author: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            proposal_id,
            author: author.into(),
            title: title.into(),
            queued_at: Utc::now(),
        }
    }
}

/// Manages the queue of proposals waiting for governance budget slots.
///
/// FIFO ordering — proposals enter voting in the order they were queued.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposalQueue {
    pub community_id: String,
    pub entries: Vec<QueuedProposal>,
}

impl ProposalQueue {
    pub fn new(community_id: impl Into<String>) -> Self {
        Self {
            community_id: community_id.into(),
            entries: Vec::new(),
        }
    }

    /// Add a proposal to the back of the queue.
    pub fn enqueue(&mut self, entry: QueuedProposal) {
        self.entries.push(entry);
    }

    /// Remove and return the next proposal from the front of the queue.
    pub fn dequeue(&mut self) -> Option<QueuedProposal> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0))
        }
    }

    /// Peek at the next proposal without removing it.
    pub fn peek(&self) -> Option<&QueuedProposal> {
        self.entries.first()
    }

    /// Number of proposals waiting.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove a specific proposal from the queue (e.g. if withdrawn).
    pub fn remove(&mut self, proposal_id: Uuid) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.proposal_id != proposal_id);
        self.entries.len() < before
    }
}

// ---------------------------------------------------------------------------
// Role Rotation Policy
// ---------------------------------------------------------------------------

/// Policy for rotating governance roles to prevent power concentration.
///
/// Stored in a community's Charter. When a member reaches `max_consecutive_terms`,
/// their role transitions to the next eligible member (by Liquid Democracy
/// delegation or community vote).
///
/// From Constellation Art. 8 SS6: "No coordinating role may become a pathway
/// to enduring power or privilege."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoleRotationPolicy {
    /// Maximum number of consecutive terms before mandatory rotation.
    pub max_consecutive_terms: usize,
    /// Duration of each term in seconds.
    pub term_duration_secs: u64,
    /// Which roles this rotation policy applies to.
    /// Default: Elder, Steward. NOT Founder during initial formation.
    pub rotation_applies_to: Vec<CommunityRole>,
    /// How long a member must sit out before re-serving, in seconds.
    pub cooling_off_period_secs: u64,
}

impl RoleRotationPolicy {
    /// Create a policy with sensible defaults.
    ///
    /// Default: 3 consecutive terms, 180-day terms, applies to Elder and Steward,
    /// cooling-off period equals one term.
    pub fn new() -> Self {
        let term_secs = 180 * 24 * 3600; // 180 days
        Self {
            max_consecutive_terms: 3,
            term_duration_secs: term_secs,
            rotation_applies_to: vec![CommunityRole::Elder, CommunityRole::Steward],
            cooling_off_period_secs: term_secs, // 1 term
        }
    }

    pub fn with_max_terms(mut self, terms: usize) -> Self {
        self.max_consecutive_terms = terms;
        self
    }

    pub fn with_term_duration_secs(mut self, secs: u64) -> Self {
        self.term_duration_secs = secs;
        self
    }

    pub fn with_applies_to(mut self, roles: Vec<CommunityRole>) -> Self {
        self.rotation_applies_to = roles;
        self
    }

    pub fn with_cooling_off_secs(mut self, secs: u64) -> Self {
        self.cooling_off_period_secs = secs;
        self
    }

    /// Whether this policy applies to a given role.
    pub fn applies_to(&self, role: &CommunityRole) -> bool {
        self.rotation_applies_to.contains(role)
    }
}

impl Default for RoleRotationPolicy {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Role Term Tracker
// ---------------------------------------------------------------------------

/// Tracks how many consecutive terms a member has served in a specific role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoleTermTracker {
    pub id: Uuid,
    pub member_pubkey: String,
    pub role: CommunityRole,
    /// How many consecutive terms the member has served.
    pub terms_served: usize,
    /// When the current term started, if currently serving.
    pub current_term_start: Option<DateTime<Utc>>,
    /// If set, the member is in a cooling-off period until this time.
    pub cooling_off_until: Option<DateTime<Utc>>,
}

impl RoleTermTracker {
    pub fn new(member_pubkey: impl Into<String>, role: CommunityRole) -> Self {
        Self {
            id: Uuid::new_v4(),
            member_pubkey: member_pubkey.into(),
            role,
            terms_served: 0,
            current_term_start: None,
            cooling_off_until: None,
        }
    }

    /// Start a new term. Fails if the member is in a cooling-off period or
    /// has reached the maximum consecutive terms.
    pub fn start_term(
        &mut self,
        policy: &RoleRotationPolicy,
    ) -> Result<(), KingdomError> {
        if self.is_cooling_off() {
            return Err(KingdomError::RoleCoolingOff {
                member: self.member_pubkey.clone(),
                role: format!("{:?}", self.role),
                until: self
                    .cooling_off_until
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            });
        }
        if self.terms_served >= policy.max_consecutive_terms {
            return Err(KingdomError::MaxTermsReached {
                member: self.member_pubkey.clone(),
                role: format!("{:?}", self.role),
                max: policy.max_consecutive_terms,
            });
        }
        self.terms_served += 1;
        self.current_term_start = Some(Utc::now());
        Ok(())
    }

    /// End the current term. If max terms reached, begin cooling-off.
    pub fn end_term(&mut self, policy: &RoleRotationPolicy) {
        self.current_term_start = None;

        if self.terms_served >= policy.max_consecutive_terms {
            let cooling_off_duration =
                chrono::Duration::seconds(policy.cooling_off_period_secs as i64);
            self.cooling_off_until = Some(Utc::now() + cooling_off_duration);
        }
    }

    /// Whether the member is currently in a cooling-off period.
    pub fn is_cooling_off(&self) -> bool {
        self.cooling_off_until
            .is_some_and(|until| Utc::now() < until)
    }

    /// Whether the member is currently serving a term.
    pub fn is_serving(&self) -> bool {
        self.current_term_start.is_some()
    }

    /// Whether the member has reached the maximum consecutive terms.
    pub fn at_term_limit(&self, policy: &RoleRotationPolicy) -> bool {
        self.terms_served >= policy.max_consecutive_terms
    }

    /// Reset the tracker after the cooling-off period expires.
    /// Call this when the cooling-off period has passed and the member
    /// wants to be eligible again.
    pub fn reset_after_cooloff(&mut self) -> Result<(), KingdomError> {
        if self.is_cooling_off() {
            return Err(KingdomError::RoleCoolingOff {
                member: self.member_pubkey.clone(),
                role: format!("{:?}", self.role),
                until: self
                    .cooling_off_until
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            });
        }
        self.terms_served = 0;
        self.cooling_off_until = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- GovernanceBudget ---

    #[test]
    fn budget_defaults() {
        let budget = GovernanceBudget::new("comm-1");
        assert_eq!(budget.max_active_proposals, 5);
        assert_eq!(budget.current_count, 0);
        assert!(budget.cooldown_enabled);
        assert!(budget.has_capacity());
        assert_eq!(budget.available_slots(), 5);
    }

    #[test]
    fn budget_builder() {
        let budget = GovernanceBudget::new("comm-1")
            .with_max_proposals(3)
            .with_proposal_period_secs(7 * 24 * 3600)
            .with_cooldown(false);
        assert_eq!(budget.max_active_proposals, 3);
        assert!(!budget.cooldown_enabled);
    }

    #[test]
    fn budget_activate_and_release() {
        let mut budget = GovernanceBudget::new("comm-1").with_max_proposals(2);

        budget.try_activate().unwrap();
        assert_eq!(budget.current_count, 1);
        assert_eq!(budget.available_slots(), 1);

        budget.try_activate().unwrap();
        assert_eq!(budget.current_count, 2);
        assert!(!budget.has_capacity());
        assert_eq!(budget.available_slots(), 0);

        // Budget full
        assert!(budget.try_activate().is_err());

        // Release one
        budget.release();
        assert!(budget.has_capacity());
        assert_eq!(budget.available_slots(), 1);
        budget.try_activate().unwrap();
    }

    #[test]
    fn budget_release_does_not_underflow() {
        let mut budget = GovernanceBudget::new("comm-1");
        budget.release();
        assert_eq!(budget.current_count, 0);
    }

    #[test]
    fn budget_serialization_roundtrip() {
        let budget = GovernanceBudget::new("comm-1").with_max_proposals(10);
        let json = serde_json::to_string(&budget).unwrap();
        let restored: GovernanceBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(budget.max_active_proposals, restored.max_active_proposals);
    }

    // --- ProposalQueue ---

    #[test]
    fn queue_fifo_ordering() {
        let mut queue = ProposalQueue::new("comm-1");
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        queue.enqueue(QueuedProposal::new(id1, "alice", "First"));
        queue.enqueue(QueuedProposal::new(id2, "bob", "Second"));
        queue.enqueue(QueuedProposal::new(id3, "charlie", "Third"));

        assert_eq!(queue.len(), 3);
        assert!(!queue.is_empty());
        assert_eq!(queue.peek().unwrap().proposal_id, id1);

        let first = queue.dequeue().unwrap();
        assert_eq!(first.proposal_id, id1);
        assert_eq!(queue.len(), 2);

        let second = queue.dequeue().unwrap();
        assert_eq!(second.proposal_id, id2);
    }

    #[test]
    fn queue_empty_dequeue() {
        let mut queue = ProposalQueue::new("comm-1");
        assert!(queue.dequeue().is_none());
        assert!(queue.peek().is_none());
    }

    #[test]
    fn queue_remove_specific() {
        let mut queue = ProposalQueue::new("comm-1");
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        queue.enqueue(QueuedProposal::new(id1, "alice", "One"));
        queue.enqueue(QueuedProposal::new(id2, "bob", "Two"));

        assert!(queue.remove(id1));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.peek().unwrap().proposal_id, id2);

        // Remove non-existent
        assert!(!queue.remove(Uuid::new_v4()));
    }

    // --- RoleRotationPolicy ---

    #[test]
    fn rotation_policy_defaults() {
        let policy = RoleRotationPolicy::new();
        assert_eq!(policy.max_consecutive_terms, 3);
        assert_eq!(policy.term_duration_secs, 180 * 24 * 3600);
        assert!(policy.applies_to(&CommunityRole::Elder));
        assert!(policy.applies_to(&CommunityRole::Steward));
        assert!(!policy.applies_to(&CommunityRole::Founder));
        assert!(!policy.applies_to(&CommunityRole::Member));
    }

    #[test]
    fn rotation_policy_builder() {
        let policy = RoleRotationPolicy::new()
            .with_max_terms(2)
            .with_term_duration_secs(90 * 24 * 3600)
            .with_applies_to(vec![CommunityRole::Elder])
            .with_cooling_off_secs(60 * 24 * 3600);

        assert_eq!(policy.max_consecutive_terms, 2);
        assert!(policy.applies_to(&CommunityRole::Elder));
        assert!(!policy.applies_to(&CommunityRole::Steward));
    }

    #[test]
    fn rotation_policy_serialization() {
        let policy = RoleRotationPolicy::new();
        let json = serde_json::to_string(&policy).unwrap();
        let restored: RoleRotationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy.max_consecutive_terms, restored.max_consecutive_terms);
    }

    // --- RoleTermTracker ---

    #[test]
    fn tracker_start_and_end_term() {
        let policy = RoleRotationPolicy::new().with_max_terms(2);
        let mut tracker = RoleTermTracker::new("alice", CommunityRole::Elder);

        assert_eq!(tracker.terms_served, 0);
        assert!(!tracker.is_serving());

        tracker.start_term(&policy).unwrap();
        assert_eq!(tracker.terms_served, 1);
        assert!(tracker.is_serving());

        tracker.end_term(&policy);
        assert!(!tracker.is_serving());
        assert!(!tracker.is_cooling_off());

        tracker.start_term(&policy).unwrap();
        assert_eq!(tracker.terms_served, 2);

        tracker.end_term(&policy);
        // After max terms, cooling off begins
        assert!(tracker.is_cooling_off());
    }

    #[test]
    fn tracker_cannot_start_when_at_limit() {
        let policy = RoleRotationPolicy::new().with_max_terms(1);
        let mut tracker = RoleTermTracker::new("bob", CommunityRole::Steward);

        tracker.start_term(&policy).unwrap();
        tracker.end_term(&policy);

        // At term limit + cooling off
        let result = tracker.start_term(&policy);
        assert!(result.is_err());
    }

    #[test]
    fn tracker_cannot_start_when_cooling_off() {
        let policy = RoleRotationPolicy::new().with_max_terms(1);
        let mut tracker = RoleTermTracker::new("charlie", CommunityRole::Elder);

        tracker.start_term(&policy).unwrap();
        tracker.end_term(&policy);
        assert!(tracker.is_cooling_off());

        let result = tracker.start_term(&policy);
        assert!(result.is_err());
    }

    #[test]
    fn tracker_reset_after_cooloff_expired() {
        let policy = RoleRotationPolicy::new().with_max_terms(1);
        let mut tracker = RoleTermTracker::new("diana", CommunityRole::Elder);

        tracker.start_term(&policy).unwrap();
        tracker.end_term(&policy);

        // Simulate cooling-off period expiring
        tracker.cooling_off_until = Some(Utc::now() - chrono::Duration::days(1));
        assert!(!tracker.is_cooling_off());

        tracker.reset_after_cooloff().unwrap();
        assert_eq!(tracker.terms_served, 0);
        assert!(tracker.cooling_off_until.is_none());

        // Can start again
        tracker.start_term(&policy).unwrap();
        assert_eq!(tracker.terms_served, 1);
    }

    #[test]
    fn tracker_reset_fails_during_cooloff() {
        let policy = RoleRotationPolicy::new().with_max_terms(1);
        let mut tracker = RoleTermTracker::new("eve", CommunityRole::Steward);

        tracker.start_term(&policy).unwrap();
        tracker.end_term(&policy);
        assert!(tracker.is_cooling_off());

        assert!(tracker.reset_after_cooloff().is_err());
    }

    #[test]
    fn tracker_at_term_limit() {
        let policy = RoleRotationPolicy::new().with_max_terms(3);
        let mut tracker = RoleTermTracker::new("frank", CommunityRole::Elder);

        for _ in 0..3 {
            tracker.start_term(&policy).unwrap();
            tracker.end_term(&policy);
        }
        assert!(tracker.at_term_limit(&policy));
    }

    #[test]
    fn tracker_serialization_roundtrip() {
        let tracker = RoleTermTracker::new("alice", CommunityRole::Elder);
        let json = serde_json::to_string(&tracker).unwrap();
        let restored: RoleTermTracker = serde_json::from_str(&json).unwrap();
        assert_eq!(tracker.member_pubkey, restored.member_pubkey);
        assert_eq!(tracker.role, restored.role);
    }

    // --- Integration: Budget + Queue ---

    #[test]
    fn budget_full_triggers_queue() {
        let mut budget = GovernanceBudget::new("comm-1").with_max_proposals(1);
        let mut queue = ProposalQueue::new("comm-1");

        // First proposal activates
        budget.try_activate().unwrap();

        // Second proposal should queue
        let p2_id = Uuid::new_v4();
        assert!(budget.try_activate().is_err());
        queue.enqueue(QueuedProposal::new(p2_id, "bob", "Queued proposal"));

        // First resolves, dequeue second
        budget.release();
        assert!(budget.has_capacity());
        let next = queue.dequeue().unwrap();
        assert_eq!(next.proposal_id, p2_id);
        budget.try_activate().unwrap();
    }
}
