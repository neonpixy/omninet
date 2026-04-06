//! # AI Consent Escalation (R6D)
//!
//! Granular consent gates for Advisor actions. Defines what the Advisor can
//! do without asking, what requires auto-approval, and what requires explicit
//! human approval.
//!
//! The `CognitiveLoop::tick()` checks `ConsentProfile` before executing any
//! `SkillCall`. If the gate requires approval, the action is queued and Pager
//! notifies the user. The action executes only after approval (or times out
//! and is discarded).
//!
//! # Covenant Alignment
//!
//! **Consent** — the core principle. Every escalation level exists because the
//! human must control what their Advisor does on their behalf.
//! **Sovereignty** — users customize their profile. Power users auto-approve
//! everything. Cautious users gate Create/Modify too.
//! **Dignity** — communities can set minimums via charter policy, ensuring
//! AI-assisted actions respect community norms.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── ConsentEscalation ────────────────────────────────────────────────

/// The type of action an Advisor wants to perform, ordered by escalation level.
///
/// Each variant maps to a gate in `ConsentProfile` that determines whether
/// the action can proceed without human approval.
///
/// # Example
///
/// ```
/// use advisor::consent_escalation::ConsentEscalation;
///
/// let action = ConsentEscalation::Publish;
/// assert!(action.requires_approval_by_default());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsentEscalation {
    /// Suggest anything. No gate. Always allowed.
    Suggest,
    /// Create new Digits in the current document.
    Create,
    /// Modify existing Digits.
    Modify,
    /// Publish to Globe.
    Publish,
    /// Fortune transactions (spending Cool).
    Transact,
    /// Governance votes.
    Govern,
    /// Send messages via Equipment.
    Communicate,
}

impl ConsentEscalation {
    /// Whether this action type requires explicit approval by default.
    pub fn requires_approval_by_default(&self) -> bool {
        matches!(
            self,
            ConsentEscalation::Publish
                | ConsentEscalation::Transact
                | ConsentEscalation::Communicate
        )
    }

    /// Whether this action type is always allowed (no gate).
    pub fn is_always_allowed(&self) -> bool {
        matches!(self, ConsentEscalation::Suggest)
    }

    /// Whether this action type is auto-approved by default.
    pub fn is_auto_approved_by_default(&self) -> bool {
        matches!(
            self,
            ConsentEscalation::Create | ConsentEscalation::Modify
        )
    }

    /// All escalation levels, in order from least to most restricted.
    pub fn all_levels() -> &'static [ConsentEscalation] {
        &[
            ConsentEscalation::Suggest,
            ConsentEscalation::Create,
            ConsentEscalation::Modify,
            ConsentEscalation::Publish,
            ConsentEscalation::Transact,
            ConsentEscalation::Govern,
            ConsentEscalation::Communicate,
        ]
    }
}

impl std::fmt::Display for ConsentEscalation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsentEscalation::Suggest => write!(f, "Suggest"),
            ConsentEscalation::Create => write!(f, "Create"),
            ConsentEscalation::Modify => write!(f, "Modify"),
            ConsentEscalation::Publish => write!(f, "Publish"),
            ConsentEscalation::Transact => write!(f, "Transact"),
            ConsentEscalation::Govern => write!(f, "Govern"),
            ConsentEscalation::Communicate => write!(f, "Communicate"),
        }
    }
}

// ── ConsentApproval ──────────────────────────────────────────────────

/// A record of a human approving or rejecting an Advisor action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsentApproval {
    /// What the Advisor wanted to do.
    pub action_description: String,
    /// Whether the human approved.
    pub approved: bool,
    /// When the decision was made.
    pub timestamp: DateTime<Utc>,
}

impl ConsentApproval {
    /// Record an approval.
    pub fn approve(description: impl Into<String>) -> Self {
        Self {
            action_description: description.into(),
            approved: true,
            timestamp: Utc::now(),
        }
    }

    /// Record a rejection.
    pub fn reject(description: impl Into<String>) -> Self {
        Self {
            action_description: description.into(),
            approved: false,
            timestamp: Utc::now(),
        }
    }
}

// ── ConsentGate ──────────────────────────────────────────────────────

/// A gate controlling whether Advisor can perform a specific action type.
///
/// Each gate tracks whether the action type is auto-approved and maintains
/// an approval history for auditability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsentGate {
    /// Which action type this gate controls.
    pub action_type: ConsentEscalation,
    /// If true, the Advisor can proceed without asking.
    pub auto_approve: bool,
    /// History of approvals/rejections for this gate.
    pub approval_history: Vec<ConsentApproval>,
}

impl ConsentGate {
    /// Create a new gate.
    pub fn new(action_type: ConsentEscalation, auto_approve: bool) -> Self {
        Self {
            action_type,
            auto_approve,
            approval_history: Vec::new(),
        }
    }

    /// Check if an action can proceed. Returns `true` if auto-approved
    /// or if the action type is always allowed.
    pub fn can_proceed(&self) -> bool {
        self.action_type.is_always_allowed() || self.auto_approve
    }

    /// Record an approval decision.
    pub fn record_approval(&mut self, approval: ConsentApproval) {
        self.approval_history.push(approval);
    }

    /// Number of times this gate was approved.
    pub fn approval_count(&self) -> usize {
        self.approval_history.iter().filter(|a| a.approved).count()
    }

    /// Number of times this gate was rejected.
    pub fn rejection_count(&self) -> usize {
        self.approval_history
            .iter()
            .filter(|a| !a.approved)
            .count()
    }
}

// ── ConsentProfile ───────────────────────────────────────────────────

/// A user's complete consent configuration for Advisor actions.
///
/// Each `ConsentEscalation` level maps to a `ConsentGate`. Users can
/// customize their profile — power users auto-approve everything,
/// cautious users require approval for Create/Modify too.
///
/// # Default Profile
///
/// - **Suggest**: no gate (always allowed)
/// - **Create**: auto-approve (Advisor can create Digits freely during active editing)
/// - **Modify**: auto-approve (same)
/// - **Publish**: requires approval
/// - **Transact**: requires approval
/// - **Govern**: requires approval (per GovernanceModeConfig)
/// - **Communicate**: requires approval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsentProfile {
    /// Map from action type to its gate.
    pub gates: HashMap<ConsentEscalation, ConsentGate>,
}

impl Default for ConsentProfile {
    fn default() -> Self {
        let mut gates = HashMap::new();

        // Suggest: always allowed, auto-approve is moot but set for consistency.
        gates.insert(
            ConsentEscalation::Suggest,
            ConsentGate::new(ConsentEscalation::Suggest, true),
        );

        // Create/Modify: auto-approve during active editing.
        gates.insert(
            ConsentEscalation::Create,
            ConsentGate::new(ConsentEscalation::Create, true),
        );
        gates.insert(
            ConsentEscalation::Modify,
            ConsentGate::new(ConsentEscalation::Modify, true),
        );

        // Publish/Transact/Communicate: requires explicit approval.
        gates.insert(
            ConsentEscalation::Publish,
            ConsentGate::new(ConsentEscalation::Publish, false),
        );
        gates.insert(
            ConsentEscalation::Transact,
            ConsentGate::new(ConsentEscalation::Transact, false),
        );
        gates.insert(
            ConsentEscalation::Govern,
            ConsentGate::new(ConsentEscalation::Govern, false),
        );
        gates.insert(
            ConsentEscalation::Communicate,
            ConsentGate::new(ConsentEscalation::Communicate, false),
        );

        Self { gates }
    }
}

impl ConsentProfile {
    /// Check if an action can proceed without human approval.
    ///
    /// Returns `true` if the action type is always allowed (Suggest)
    /// or if the gate is set to auto-approve.
    pub fn can_proceed(&self, action: ConsentEscalation) -> bool {
        if action.is_always_allowed() {
            return true;
        }
        self.gates
            .get(&action)
            .map(|gate| gate.can_proceed())
            .unwrap_or(false)
    }

    /// Set whether an action type is auto-approved.
    pub fn set_auto_approve(&mut self, action: ConsentEscalation, auto: bool) {
        let gate = self
            .gates
            .entry(action)
            .or_insert_with(|| ConsentGate::new(action, false));
        gate.auto_approve = auto;
    }

    /// Record an approval decision for an action type.
    pub fn record_approval(&mut self, action: ConsentEscalation, approval: ConsentApproval) {
        let gate = self
            .gates
            .entry(action)
            .or_insert_with(|| ConsentGate::new(action, false));
        gate.record_approval(approval);
    }

    /// Create a profile where everything is auto-approved (power user).
    pub fn permissive() -> Self {
        let mut profile = Self::default();
        for level in ConsentEscalation::all_levels() {
            profile.set_auto_approve(*level, true);
        }
        profile
    }

    /// Create a profile where everything requires approval (cautious user).
    pub fn restrictive() -> Self {
        let mut profile = Self::default();
        for level in ConsentEscalation::all_levels() {
            if !level.is_always_allowed() {
                profile.set_auto_approve(*level, false);
            }
        }
        profile
    }

    /// Apply a community override: ensure a specific action type requires approval.
    ///
    /// Community charters can mandate consent floors. This enforces them
    /// regardless of the user's personal preference.
    pub fn apply_community_override(&mut self, action: ConsentEscalation) {
        if !action.is_always_allowed() {
            self.set_auto_approve(action, false);
        }
    }

    /// Apply multiple community overrides from a `CommunityConsentPolicy`.
    pub fn apply_community_policy(&mut self, policy: &CommunityConsentPolicy) {
        for &action in &policy.required_approval {
            self.apply_community_override(action);
        }
    }
}

// ── CommunityConsentPolicy ───────────────────────────────────────────

/// A community's minimum consent requirements for AI actions.
///
/// Stored in the charter's `GovernanceAIPolicy`. Overrides individual
/// `ConsentProfile` settings for actions within that community.
///
/// For example, a community can say "AI-assisted publications must have
/// explicit approval" — this forces `Publish` to require approval even
/// for users who normally auto-approve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommunityConsentPolicy {
    /// The community this policy applies to.
    pub community_id: String,
    /// Action types that must require human approval in this community.
    pub required_approval: Vec<ConsentEscalation>,
}

impl CommunityConsentPolicy {
    /// Create a new community consent policy.
    pub fn new(community_id: impl Into<String>) -> Self {
        Self {
            community_id: community_id.into(),
            required_approval: Vec::new(),
        }
    }

    /// Require approval for a specific action type.
    pub fn require_approval(mut self, action: ConsentEscalation) -> Self {
        if !self.required_approval.contains(&action) {
            self.required_approval.push(action);
        }
        self
    }

    /// A strict policy: all non-Suggest actions require approval.
    pub fn strict(community_id: impl Into<String>) -> Self {
        Self {
            community_id: community_id.into(),
            required_approval: vec![
                ConsentEscalation::Create,
                ConsentEscalation::Modify,
                ConsentEscalation::Publish,
                ConsentEscalation::Transact,
                ConsentEscalation::Govern,
                ConsentEscalation::Communicate,
            ],
        }
    }
}

// ── PendingAction ────────────────────────────────────────────────────

/// An action queued pending human approval.
///
/// Created when `ConsentProfile::can_proceed` returns false. The platform
/// layer shows this to the user via Pager and waits for approval or timeout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingAction {
    /// What kind of action.
    pub action_type: ConsentEscalation,
    /// Human-readable description of what the Advisor wants to do.
    pub description: String,
    /// When this action was queued.
    pub queued_at: DateTime<Utc>,
    /// When this action expires if not approved.
    pub expires_at: DateTime<Utc>,
}

impl PendingAction {
    /// Create a pending action with a timeout.
    pub fn new(
        action_type: ConsentEscalation,
        description: impl Into<String>,
        timeout_seconds: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            action_type,
            description: description.into(),
            queued_at: now,
            expires_at: now + chrono::Duration::seconds(timeout_seconds),
        }
    }

    /// Whether this action has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- ConsentEscalation ---

    #[test]
    fn suggest_always_allowed() {
        assert!(ConsentEscalation::Suggest.is_always_allowed());
        assert!(!ConsentEscalation::Suggest.requires_approval_by_default());
    }

    #[test]
    fn publish_requires_approval() {
        assert!(ConsentEscalation::Publish.requires_approval_by_default());
        assert!(!ConsentEscalation::Publish.is_always_allowed());
    }

    #[test]
    fn create_auto_approved_by_default() {
        assert!(ConsentEscalation::Create.is_auto_approved_by_default());
        assert!(ConsentEscalation::Modify.is_auto_approved_by_default());
        assert!(!ConsentEscalation::Publish.is_auto_approved_by_default());
    }

    #[test]
    fn all_levels_count() {
        assert_eq!(ConsentEscalation::all_levels().len(), 7);
    }

    #[test]
    fn display_format() {
        assert_eq!(format!("{}", ConsentEscalation::Govern), "Govern");
        assert_eq!(format!("{}", ConsentEscalation::Communicate), "Communicate");
    }

    #[test]
    fn escalation_serde() {
        let action = ConsentEscalation::Transact;
        let json = serde_json::to_string(&action).unwrap();
        let restored: ConsentEscalation = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ConsentEscalation::Transact);
    }

    // --- ConsentApproval ---

    #[test]
    fn approval_approve() {
        let a = ConsentApproval::approve("publish logo to Globe");
        assert!(a.approved);
        assert!(a.action_description.contains("publish"));
    }

    #[test]
    fn approval_reject() {
        let a = ConsentApproval::reject("send message to strangers");
        assert!(!a.approved);
    }

    #[test]
    fn approval_serde() {
        let a = ConsentApproval::approve("test action");
        let json = serde_json::to_string(&a).unwrap();
        let restored: ConsentApproval = serde_json::from_str(&json).unwrap();
        assert!(restored.approved);
    }

    // --- ConsentGate ---

    #[test]
    fn gate_auto_approve_proceeds() {
        let gate = ConsentGate::new(ConsentEscalation::Create, true);
        assert!(gate.can_proceed());
    }

    #[test]
    fn gate_manual_blocks() {
        let gate = ConsentGate::new(ConsentEscalation::Publish, false);
        assert!(!gate.can_proceed());
    }

    #[test]
    fn gate_suggest_always_proceeds() {
        let gate = ConsentGate::new(ConsentEscalation::Suggest, false);
        assert!(gate.can_proceed()); // Suggest is always allowed.
    }

    #[test]
    fn gate_records_approvals() {
        let mut gate = ConsentGate::new(ConsentEscalation::Publish, false);
        gate.record_approval(ConsentApproval::approve("publish logo"));
        gate.record_approval(ConsentApproval::reject("publish draft"));
        gate.record_approval(ConsentApproval::approve("publish final"));

        assert_eq!(gate.approval_count(), 2);
        assert_eq!(gate.rejection_count(), 1);
    }

    // --- ConsentProfile ---

    #[test]
    fn default_profile_suggest_allowed() {
        let profile = ConsentProfile::default();
        assert!(profile.can_proceed(ConsentEscalation::Suggest));
    }

    #[test]
    fn default_profile_create_auto() {
        let profile = ConsentProfile::default();
        assert!(profile.can_proceed(ConsentEscalation::Create));
        assert!(profile.can_proceed(ConsentEscalation::Modify));
    }

    #[test]
    fn default_profile_publish_blocked() {
        let profile = ConsentProfile::default();
        assert!(!profile.can_proceed(ConsentEscalation::Publish));
        assert!(!profile.can_proceed(ConsentEscalation::Transact));
        assert!(!profile.can_proceed(ConsentEscalation::Govern));
        assert!(!profile.can_proceed(ConsentEscalation::Communicate));
    }

    #[test]
    fn permissive_profile() {
        let profile = ConsentProfile::permissive();
        for level in ConsentEscalation::all_levels() {
            assert!(profile.can_proceed(*level));
        }
    }

    #[test]
    fn restrictive_profile() {
        let profile = ConsentProfile::restrictive();
        assert!(profile.can_proceed(ConsentEscalation::Suggest));
        assert!(!profile.can_proceed(ConsentEscalation::Create));
        assert!(!profile.can_proceed(ConsentEscalation::Modify));
        assert!(!profile.can_proceed(ConsentEscalation::Publish));
    }

    #[test]
    fn set_auto_approve() {
        let mut profile = ConsentProfile::default();
        assert!(!profile.can_proceed(ConsentEscalation::Publish));

        profile.set_auto_approve(ConsentEscalation::Publish, true);
        assert!(profile.can_proceed(ConsentEscalation::Publish));
    }

    #[test]
    fn record_approval_in_profile() {
        let mut profile = ConsentProfile::default();
        profile.record_approval(
            ConsentEscalation::Publish,
            ConsentApproval::approve("publish logo"),
        );
        let gate = profile.gates.get(&ConsentEscalation::Publish).unwrap();
        assert_eq!(gate.approval_count(), 1);
    }

    #[test]
    fn community_override() {
        let mut profile = ConsentProfile::permissive();
        assert!(profile.can_proceed(ConsentEscalation::Publish));

        profile.apply_community_override(ConsentEscalation::Publish);
        assert!(!profile.can_proceed(ConsentEscalation::Publish));
    }

    #[test]
    fn community_override_cannot_block_suggest() {
        let mut profile = ConsentProfile::default();
        profile.apply_community_override(ConsentEscalation::Suggest);
        // Suggest remains always allowed.
        assert!(profile.can_proceed(ConsentEscalation::Suggest));
    }

    #[test]
    fn community_policy_applied() {
        let policy = CommunityConsentPolicy::new("design-guild")
            .require_approval(ConsentEscalation::Create)
            .require_approval(ConsentEscalation::Publish);

        let mut profile = ConsentProfile::permissive();
        profile.apply_community_policy(&policy);

        assert!(!profile.can_proceed(ConsentEscalation::Create));
        assert!(!profile.can_proceed(ConsentEscalation::Publish));
        // Unaffected actions remain permissive.
        assert!(profile.can_proceed(ConsentEscalation::Modify));
    }

    #[test]
    fn strict_community_policy() {
        let policy = CommunityConsentPolicy::strict("secure-guild");
        assert_eq!(policy.required_approval.len(), 6);
        assert!(!policy.required_approval.contains(&ConsentEscalation::Suggest));
    }

    #[test]
    fn profile_serde() {
        let profile = ConsentProfile::default();
        let json = serde_json::to_string(&profile).unwrap();
        let restored: ConsentProfile = serde_json::from_str(&json).unwrap();
        assert!(restored.can_proceed(ConsentEscalation::Suggest));
        assert!(restored.can_proceed(ConsentEscalation::Create));
        assert!(!restored.can_proceed(ConsentEscalation::Publish));
    }

    // --- PendingAction ---

    #[test]
    fn pending_action_not_expired() {
        let action = PendingAction::new(
            ConsentEscalation::Publish,
            "publish design to Globe",
            300, // 5 minutes
        );
        assert!(!action.is_expired());
    }

    #[test]
    fn pending_action_expired() {
        let mut action = PendingAction::new(
            ConsentEscalation::Transact,
            "buy 10 Cool worth of compute",
            60,
        );
        // Force expiry.
        action.expires_at = Utc::now() - chrono::Duration::seconds(1);
        assert!(action.is_expired());
    }

    #[test]
    fn pending_action_serde() {
        let action = PendingAction::new(ConsentEscalation::Communicate, "send mail", 120);
        let json = serde_json::to_string(&action).unwrap();
        let restored: PendingAction = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.action_type, ConsentEscalation::Communicate);
    }
}
