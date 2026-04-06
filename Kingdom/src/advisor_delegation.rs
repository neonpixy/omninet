//! # Advisor Delegation — AI as Liquid Democracy Delegate
//!
//! Extends Kingdom's LiquidDemocracy to support Advisor as a delegate type.
//! Members can delegate their voting power to their AI Advisor, which votes
//! according to their configured governance values.
//!
//! From MPv6 R3A: "Advisor delegation is a convenience, not a replacement for
//! engagement. The system is designed so that overriding your Advisor is always
//! one tap away."
//!
//! ## Design
//!
//! - **DelegateType** distinguishes person-to-person delegation from Advisor delegation.
//! - **AdvisorDelegation** tracks a member's active Advisor delegation and override history.
//! - **GovernanceAIPolicy** lets communities control Advisor voting within their charter.
//! - **DeliberationWindow** ensures members have time to override before Advisor votes are tallied.
//! - **DelegationStats** provides community-level insight into delegation distribution.
//!
//! ## Covenant Alignment
//!
//! **Sovereignty** — members always retain override authority. Advisor votes on behalf,
//! never instead of.
//! **Consent** — delegation is opt-in, revocable, and transparent.
//! **Dignity** — no tier is better. Advisor delegation is a tool, not a rank.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::KingdomError;

// ---------------------------------------------------------------------------
// DelegateType
// ---------------------------------------------------------------------------

/// Who receives delegated voting power.
///
/// Extends Kingdom's implicit string-based delegation to distinguish
/// person-to-person delegation from Advisor (AI) delegation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DelegateType {
    /// Delegation to another community member by their public key.
    Person(String),
    /// Delegation to the member's AI Advisor.
    Advisor,
}

impl DelegateType {
    /// Returns `true` if this delegation targets an AI Advisor.
    pub fn is_advisor(&self) -> bool {
        matches!(self, DelegateType::Advisor)
    }

    /// Returns `true` if this delegation targets a human person.
    pub fn is_person(&self) -> bool {
        matches!(self, DelegateType::Person(_))
    }

    /// Returns the person's pubkey if this is a person delegation.
    pub fn person_pubkey(&self) -> Option<&str> {
        match self {
            DelegateType::Person(pk) => Some(pk),
            DelegateType::Advisor => None,
        }
    }
}

impl std::fmt::Display for DelegateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelegateType::Person(pk) => write!(f, "Person({pk})"),
            DelegateType::Advisor => write!(f, "Advisor"),
        }
    }
}

// ---------------------------------------------------------------------------
// DelegationOverride
// ---------------------------------------------------------------------------

/// A record of a member overriding their Advisor's position on a proposal.
///
/// Override is always available during the deliberation window and is the
/// primary mechanism ensuring human sovereignty over AI delegation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegationOverride {
    /// The member who overrode their Advisor.
    pub member_pubkey: String,
    /// The proposal this override applies to.
    pub proposal_id: Uuid,
    /// What the Advisor would have voted.
    pub original_position: String,
    /// What the member chose to vote instead.
    pub override_position: String,
    /// When the override was recorded.
    pub overridden_at: DateTime<Utc>,
}

impl DelegationOverride {
    /// Create a new override record.
    pub fn new(
        member_pubkey: impl Into<String>,
        proposal_id: Uuid,
        original_position: impl Into<String>,
        override_position: impl Into<String>,
    ) -> Self {
        Self {
            member_pubkey: member_pubkey.into(),
            proposal_id,
            original_position: original_position.into(),
            override_position: override_position.into(),
            overridden_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// AdvisorDelegation
// ---------------------------------------------------------------------------

/// A member's active delegation to their AI Advisor.
///
/// Tracks when the delegation was activated, the member's governance
/// configuration, and history of overrides for accountability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorDelegation {
    /// The member who delegated to their Advisor.
    pub member_pubkey: String,
    /// The community this delegation is active in.
    pub community_id: String,
    /// When the delegation was activated.
    pub activated_at: DateTime<Utc>,
    /// Serializable governance mode configuration (from R1D).
    /// Opaque to Kingdom — interpreted by Advisor.
    pub config: AdvisorDelegationConfig,
    /// History of overrides this member has made.
    pub override_history: Vec<DelegationOverride>,
}

impl AdvisorDelegation {
    /// Create a new Advisor delegation.
    pub fn new(
        member_pubkey: impl Into<String>,
        community_id: impl Into<String>,
        config: AdvisorDelegationConfig,
    ) -> Self {
        Self {
            member_pubkey: member_pubkey.into(),
            community_id: community_id.into(),
            activated_at: Utc::now(),
            config,
            override_history: Vec::new(),
        }
    }

    /// Record an override of the Advisor's position.
    pub fn record_override(&mut self, delegation_override: DelegationOverride) {
        self.override_history.push(delegation_override);
    }

    /// Count overrides for a specific proposal.
    pub fn overrides_for_proposal(&self, proposal_id: Uuid) -> Vec<&DelegationOverride> {
        self.override_history
            .iter()
            .filter(|o| o.proposal_id == proposal_id)
            .collect()
    }

    /// Check if the member has overridden for a given proposal.
    pub fn has_overridden(&self, proposal_id: Uuid) -> bool {
        self.override_history
            .iter()
            .any(|o| o.proposal_id == proposal_id)
    }

    /// Total number of overrides recorded.
    pub fn total_overrides(&self) -> usize {
        self.override_history.len()
    }
}

// ---------------------------------------------------------------------------
// AdvisorDelegationConfig
// ---------------------------------------------------------------------------

/// Configuration for how an Advisor should vote on behalf of a member.
///
/// This is opaque to Kingdom — the Advisor crate interprets these values.
/// Kingdom only stores and passes them through.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorDelegationConfig {
    /// The member's governance values profile identifier.
    pub values_profile_id: Option<String>,
    /// Default deliberation window duration in seconds.
    /// Gives the member this long to override before the Advisor's vote is tallied.
    pub deliberation_window_secs: u64,
    /// Categories the member always wants to vote on directly.
    pub always_direct_categories: Vec<String>,
    /// Whether to receive detailed reasoning from the Advisor.
    pub reasoning_enabled: bool,
    /// Freeform configuration extensions (future-proof).
    pub extensions: std::collections::HashMap<String, String>,
}

impl Default for AdvisorDelegationConfig {
    fn default() -> Self {
        Self {
            values_profile_id: None,
            deliberation_window_secs: 86400, // 24 hours
            always_direct_categories: Vec::new(),
            reasoning_enabled: true,
            extensions: std::collections::HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// DelegationStats
// ---------------------------------------------------------------------------

/// Community-level statistics on delegation distribution.
///
/// Used for transparency and to enforce governance AI policy caps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegationStats {
    /// The community these stats describe.
    pub community_id: String,
    /// Total community members eligible to vote.
    pub total_members: usize,
    /// Members who have delegated to an AI Advisor.
    pub advisor_delegated: usize,
    /// Members who have delegated to another person.
    pub person_delegated: usize,
    /// Members who vote directly (no delegation).
    pub direct_voters: usize,
    /// Percentage of members using Advisor delegation (0.0 to 100.0).
    pub advisor_percentage: f64,
}

impl DelegationStats {
    /// Compute delegation stats from membership data.
    ///
    /// `delegations` maps each member to their delegate type (if any).
    /// Members not present in the map are counted as direct voters.
    pub fn compute(
        community_id: impl Into<String>,
        total_members: usize,
        delegations: &[(String, DelegateType)],
    ) -> Self {
        let mut advisor_delegated = 0usize;
        let mut person_delegated = 0usize;

        for (_member, delegate_type) in delegations {
            match delegate_type {
                DelegateType::Advisor => advisor_delegated += 1,
                DelegateType::Person(_) => person_delegated += 1,
            }
        }

        let direct_voters = total_members.saturating_sub(advisor_delegated + person_delegated);
        let advisor_percentage = if total_members == 0 {
            0.0
        } else {
            (advisor_delegated as f64 / total_members as f64) * 100.0
        };

        Self {
            community_id: community_id.into(),
            total_members,
            advisor_delegated,
            person_delegated,
            direct_voters,
            advisor_percentage,
        }
    }

    /// Check if the advisor percentage exceeds a given cap.
    pub fn exceeds_cap(&self, cap_percentage: f64) -> bool {
        self.advisor_percentage > cap_percentage
    }
}

// ---------------------------------------------------------------------------
// DeliberationWindow
// ---------------------------------------------------------------------------

/// Tracks the deliberation window for a proposal, during which Advisor-delegated
/// members can override their Advisor's position.
///
/// After the window closes, Advisor votes are tallied for non-overriding members.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliberationWindow {
    /// The proposal this window applies to.
    pub proposal_id: Uuid,
    /// The community holding the vote.
    pub community_id: String,
    /// When the deliberation window opened.
    pub window_start: DateTime<Utc>,
    /// Duration of the window in seconds.
    pub window_duration_secs: u64,
    /// Deadline for member overrides (computed from start + duration).
    pub override_deadline: DateTime<Utc>,
    /// Whether the window has been extended (e.g., due to cap exceedance).
    pub extended: bool,
}

/// Duration of the automatic extension when advisor cap is exceeded (48 hours).
const CAP_EXTENSION_SECS: u64 = 48 * 60 * 60;

impl DeliberationWindow {
    /// Create a new deliberation window.
    pub fn new(
        proposal_id: Uuid,
        community_id: impl Into<String>,
        window_duration_secs: u64,
    ) -> Self {
        let start = Utc::now();
        let deadline = start + chrono::Duration::seconds(window_duration_secs as i64);
        Self {
            proposal_id,
            community_id: community_id.into(),
            window_start: start,
            window_duration_secs,
            override_deadline: deadline,
            extended: false,
        }
    }

    /// Create a deliberation window with an explicit start time (for testing).
    pub fn with_start(
        proposal_id: Uuid,
        community_id: impl Into<String>,
        window_start: DateTime<Utc>,
        window_duration_secs: u64,
    ) -> Self {
        let deadline = window_start + chrono::Duration::seconds(window_duration_secs as i64);
        Self {
            proposal_id,
            community_id: community_id.into(),
            window_start,
            window_duration_secs,
            override_deadline: deadline,
            extended: false,
        }
    }

    /// Check whether the override deadline has passed.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.override_deadline
    }

    /// Check whether the override deadline has passed, given a specific timestamp.
    pub fn is_expired_at(&self, at: DateTime<Utc>) -> bool {
        at > self.override_deadline
    }

    /// Remaining seconds until the override deadline. Returns 0 if expired.
    pub fn remaining_secs(&self) -> u64 {
        let remaining = self.override_deadline - Utc::now();
        if remaining.num_seconds() <= 0 {
            0
        } else {
            remaining.num_seconds() as u64
        }
    }

    /// Remaining seconds until the override deadline at a given time.
    pub fn remaining_secs_at(&self, at: DateTime<Utc>) -> u64 {
        let remaining = self.override_deadline - at;
        if remaining.num_seconds() <= 0 {
            0
        } else {
            remaining.num_seconds() as u64
        }
    }

    /// Extend the deliberation window by 48 hours (cap exceedance extension).
    ///
    /// This is triggered when the advisor delegation percentage exceeds the
    /// community's configured cap. Can only be extended once.
    pub fn extend_for_cap_exceedance(&mut self) -> Result<(), KingdomError> {
        if self.extended {
            // Already extended — idempotent, not an error, just no-op.
            return Ok(());
        }
        self.override_deadline += chrono::Duration::seconds(CAP_EXTENSION_SECS as i64);
        self.window_duration_secs += CAP_EXTENSION_SECS;
        self.extended = true;
        Ok(())
    }

    /// Total duration including any extensions.
    pub fn total_duration_secs(&self) -> u64 {
        self.window_duration_secs
    }
}

// ---------------------------------------------------------------------------
// GovernanceAIPolicy
// ---------------------------------------------------------------------------

/// Community-level policy governing Advisor delegation.
///
/// Part of a community's charter configuration. Controls whether Advisor
/// delegation is permitted, which categories require human votes, and the
/// maximum percentage of votes that can be cast by Advisors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GovernanceAIPolicy {
    /// Whether Advisor delegation is allowed in this community.
    pub advisor_delegation_allowed: bool,
    /// Proposal categories that always require a direct human vote.
    /// Advisor delegation is ignored for these categories.
    pub human_required_categories: Vec<String>,
    /// Maximum percentage of total votes that can be Advisor-cast (0.0 to 100.0).
    /// If exceeded, the deliberation window extends by 48 hours.
    /// `None` means no cap.
    pub max_auto_vote_percentage: Option<f64>,
    /// Level of reasoning transparency required from Advisors.
    /// Communities can set "full", "summary", "none", or custom values.
    pub reasoning_transparency: String,
}

impl Default for GovernanceAIPolicy {
    fn default() -> Self {
        Self {
            advisor_delegation_allowed: true,
            human_required_categories: Vec::new(),
            max_auto_vote_percentage: None,
            reasoning_transparency: "full".to_string(),
        }
    }
}

impl GovernanceAIPolicy {
    /// Create a permissive policy (all delegation allowed, no caps).
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Create a restrictive policy (no Advisor delegation).
    pub fn no_advisor() -> Self {
        Self {
            advisor_delegation_allowed: false,
            ..Self::default()
        }
    }

    /// Create a policy with a specific auto-vote cap.
    pub fn with_cap(cap_percentage: f64) -> Self {
        Self {
            max_auto_vote_percentage: Some(cap_percentage.clamp(0.0, 100.0)),
            ..Self::default()
        }
    }

    /// Check if Advisor delegation is allowed.
    pub fn is_allowed(&self) -> bool {
        self.advisor_delegation_allowed
    }

    /// Check if a proposal category requires human votes.
    pub fn requires_human_vote(&self, category: &str) -> bool {
        self.human_required_categories
            .iter()
            .any(|c| c == category)
    }

    /// Return the list of categories requiring human votes.
    pub fn human_required_list(&self) -> &[String] {
        &self.human_required_categories
    }

    /// Check delegation stats against policy. Returns errors if policy is violated.
    ///
    /// Returns `Ok(true)` if the cap was exceeded (window should be extended),
    /// `Ok(false)` if everything is within limits.
    pub fn check_stats(&self, stats: &DelegationStats) -> Result<bool, KingdomError> {
        if !self.advisor_delegation_allowed && stats.advisor_delegated > 0 {
            return Err(KingdomError::AdvisorDelegationNotAllowed);
        }

        if let Some(cap) = self.max_auto_vote_percentage {
            if stats.advisor_percentage > cap {
                return Ok(true); // cap exceeded — extend window
            }
        }

        Ok(false) // within limits
    }

    /// Validate that a proposal can accept Advisor votes given its category.
    pub fn validate_proposal_category(&self, category: &str) -> Result<(), KingdomError> {
        if !self.advisor_delegation_allowed {
            return Err(KingdomError::AdvisorDelegationNotAllowed);
        }
        if self.requires_human_vote(category) {
            return Err(KingdomError::HumanVoteRequired(category.to_string()));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AdvisorDelegationRegistry
// ---------------------------------------------------------------------------

/// Registry of advisor delegations within a community.
///
/// Provides the core operations: register, deactivate, query, and compute stats.
/// This is a pure data structure — no async, no storage, no network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorDelegationRegistry {
    /// Active advisor delegations keyed by member pubkey.
    delegations: std::collections::HashMap<String, AdvisorDelegation>,
    /// The community this registry belongs to.
    community_id: String,
    /// The governance AI policy for this community.
    policy: GovernanceAIPolicy,
}

impl AdvisorDelegationRegistry {
    /// Create a new registry for a community.
    pub fn new(community_id: impl Into<String>, policy: GovernanceAIPolicy) -> Self {
        Self {
            delegations: std::collections::HashMap::new(),
            community_id: community_id.into(),
            policy,
        }
    }

    /// Register a new Advisor delegation for a member.
    pub fn register(
        &mut self,
        member_pubkey: impl Into<String>,
        config: AdvisorDelegationConfig,
    ) -> Result<&AdvisorDelegation, KingdomError> {
        if !self.policy.is_allowed() {
            return Err(KingdomError::AdvisorDelegationNotAllowed);
        }

        let pubkey = member_pubkey.into();
        if self.delegations.contains_key(&pubkey) {
            return Err(KingdomError::AdvisorDelegationExists(pubkey));
        }

        let delegation = AdvisorDelegation::new(&pubkey, &self.community_id, config);
        self.delegations.insert(pubkey.clone(), delegation);
        Ok(self.delegations.get(&pubkey).expect("just inserted"))
    }

    /// Deactivate (remove) a member's Advisor delegation.
    pub fn deactivate(
        &mut self,
        member_pubkey: &str,
    ) -> Result<AdvisorDelegation, KingdomError> {
        self.delegations
            .remove(member_pubkey)
            .ok_or_else(|| KingdomError::AdvisorDelegationNotFound(member_pubkey.to_string()))
    }

    /// Get a member's active Advisor delegation.
    pub fn get(&self, member_pubkey: &str) -> Option<&AdvisorDelegation> {
        self.delegations.get(member_pubkey)
    }

    /// Get a mutable reference to a member's Advisor delegation.
    pub fn get_mut(&mut self, member_pubkey: &str) -> Option<&mut AdvisorDelegation> {
        self.delegations.get_mut(member_pubkey)
    }

    /// Check if a member has an active Advisor delegation.
    pub fn is_advisor_delegated(&self, member_pubkey: &str) -> bool {
        self.delegations.contains_key(member_pubkey)
    }

    /// Number of active Advisor delegations.
    pub fn count(&self) -> usize {
        self.delegations.len()
    }

    /// All active Advisor delegations.
    pub fn all(&self) -> impl Iterator<Item = &AdvisorDelegation> {
        self.delegations.values()
    }

    /// The community's governance AI policy.
    pub fn policy(&self) -> &GovernanceAIPolicy {
        &self.policy
    }

    /// Update the community's governance AI policy.
    pub fn set_policy(&mut self, policy: GovernanceAIPolicy) {
        self.policy = policy;
    }

    /// Compute delegation stats for this community.
    ///
    /// `total_members` is the total number of eligible voting members.
    /// `person_delegations` is the count of members who have delegated to another person.
    pub fn stats(&self, total_members: usize, person_delegations: usize) -> DelegationStats {
        let advisor_delegated = self.delegations.len();
        let direct_voters =
            total_members.saturating_sub(advisor_delegated + person_delegations);
        let advisor_percentage = if total_members == 0 {
            0.0
        } else {
            (advisor_delegated as f64 / total_members as f64) * 100.0
        };

        DelegationStats {
            community_id: self.community_id.clone(),
            total_members,
            advisor_delegated,
            person_delegated: person_delegations,
            direct_voters,
            advisor_percentage,
        }
    }

    /// Record an override for a member and proposal.
    pub fn record_override(
        &mut self,
        member_pubkey: &str,
        delegation_override: DelegationOverride,
    ) -> Result<(), KingdomError> {
        let delegation = self
            .delegations
            .get_mut(member_pubkey)
            .ok_or_else(|| KingdomError::AdvisorDelegationNotFound(member_pubkey.to_string()))?;
        delegation.record_override(delegation_override);
        Ok(())
    }

    /// Check whether a proposal category is eligible for Advisor voting.
    pub fn can_advisor_vote_on(&self, category: &str) -> Result<(), KingdomError> {
        self.policy.validate_proposal_category(category)
    }

    /// Check stats against policy and determine if a deliberation window
    /// should be extended.
    pub fn should_extend_window(
        &self,
        total_members: usize,
        person_delegations: usize,
    ) -> Result<bool, KingdomError> {
        let stats = self.stats(total_members, person_delegations);
        self.policy.check_stats(&stats)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // DelegateType
    // -----------------------------------------------------------------------

    #[test]
    fn delegate_type_advisor_variant() {
        let dt = DelegateType::Advisor;
        assert!(dt.is_advisor());
        assert!(!dt.is_person());
        assert_eq!(dt.person_pubkey(), None);
        assert_eq!(dt.to_string(), "Advisor");
    }

    #[test]
    fn delegate_type_person_variant() {
        let dt = DelegateType::Person("alice_pk".into());
        assert!(!dt.is_advisor());
        assert!(dt.is_person());
        assert_eq!(dt.person_pubkey(), Some("alice_pk"));
        assert!(dt.to_string().contains("alice_pk"));
    }

    #[test]
    fn delegate_type_equality() {
        let a = DelegateType::Advisor;
        let b = DelegateType::Advisor;
        assert_eq!(a, b);

        let c = DelegateType::Person("x".into());
        let d = DelegateType::Person("x".into());
        assert_eq!(c, d);

        assert_ne!(a, c);

        let e = DelegateType::Person("y".into());
        assert_ne!(c, e);
    }

    #[test]
    fn delegate_type_serialization_roundtrip() {
        let types = vec![
            DelegateType::Advisor,
            DelegateType::Person("bob_pk".into()),
        ];
        for dt in types {
            let json = serde_json::to_string(&dt).unwrap();
            let restored: DelegateType = serde_json::from_str(&json).unwrap();
            assert_eq!(dt, restored);
        }
    }

    // -----------------------------------------------------------------------
    // DelegationOverride
    // -----------------------------------------------------------------------

    #[test]
    fn delegation_override_creation() {
        let pid = Uuid::new_v4();
        let ov = DelegationOverride::new("alice", pid, "Support", "Oppose");
        assert_eq!(ov.member_pubkey, "alice");
        assert_eq!(ov.proposal_id, pid);
        assert_eq!(ov.original_position, "Support");
        assert_eq!(ov.override_position, "Oppose");
    }

    #[test]
    fn delegation_override_serialization() {
        let ov = DelegationOverride::new("alice", Uuid::new_v4(), "Support", "Oppose");
        let json = serde_json::to_string(&ov).unwrap();
        let restored: DelegationOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(ov, restored);
    }

    // -----------------------------------------------------------------------
    // AdvisorDelegationConfig
    // -----------------------------------------------------------------------

    #[test]
    fn config_default_values() {
        let cfg = AdvisorDelegationConfig::default();
        assert_eq!(cfg.deliberation_window_secs, 86400);
        assert!(cfg.reasoning_enabled);
        assert!(cfg.always_direct_categories.is_empty());
        assert!(cfg.values_profile_id.is_none());
    }

    #[test]
    fn config_serialization() {
        let cfg = AdvisorDelegationConfig {
            values_profile_id: Some("profile_123".into()),
            always_direct_categories: vec!["Emergency".into(), "Amendment".into()],
            ..Default::default()
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let restored: AdvisorDelegationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    // -----------------------------------------------------------------------
    // AdvisorDelegation
    // -----------------------------------------------------------------------

    #[test]
    fn advisor_delegation_creation() {
        let del = AdvisorDelegation::new("alice", "community_1", AdvisorDelegationConfig::default());
        assert_eq!(del.member_pubkey, "alice");
        assert_eq!(del.community_id, "community_1");
        assert!(del.override_history.is_empty());
        assert_eq!(del.total_overrides(), 0);
    }

    #[test]
    fn advisor_delegation_record_override() {
        let mut del =
            AdvisorDelegation::new("alice", "community_1", AdvisorDelegationConfig::default());
        let pid = Uuid::new_v4();

        assert!(!del.has_overridden(pid));

        del.record_override(DelegationOverride::new("alice", pid, "Support", "Oppose"));
        assert!(del.has_overridden(pid));
        assert_eq!(del.total_overrides(), 1);
        assert_eq!(del.overrides_for_proposal(pid).len(), 1);
    }

    #[test]
    fn advisor_delegation_multiple_overrides() {
        let mut del =
            AdvisorDelegation::new("alice", "community_1", AdvisorDelegationConfig::default());
        let pid1 = Uuid::new_v4();
        let pid2 = Uuid::new_v4();

        del.record_override(DelegationOverride::new("alice", pid1, "Support", "Oppose"));
        del.record_override(DelegationOverride::new("alice", pid2, "Oppose", "Support"));
        del.record_override(DelegationOverride::new("alice", pid1, "Oppose", "Block"));

        assert_eq!(del.total_overrides(), 3);
        assert_eq!(del.overrides_for_proposal(pid1).len(), 2);
        assert_eq!(del.overrides_for_proposal(pid2).len(), 1);
    }

    #[test]
    fn advisor_delegation_serialization() {
        let del = AdvisorDelegation::new("alice", "community_1", AdvisorDelegationConfig::default());
        let json = serde_json::to_string(&del).unwrap();
        let restored: AdvisorDelegation = serde_json::from_str(&json).unwrap();
        assert_eq!(del, restored);
    }

    // -----------------------------------------------------------------------
    // DelegationStats
    // -----------------------------------------------------------------------

    #[test]
    fn stats_compute_basic() {
        let delegations = vec![
            ("alice".into(), DelegateType::Advisor),
            ("bob".into(), DelegateType::Advisor),
            ("charlie".into(), DelegateType::Person("dave".into())),
        ];

        let stats = DelegationStats::compute("community_1", 10, &delegations);
        assert_eq!(stats.total_members, 10);
        assert_eq!(stats.advisor_delegated, 2);
        assert_eq!(stats.person_delegated, 1);
        assert_eq!(stats.direct_voters, 7);
        assert!((stats.advisor_percentage - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_compute_empty() {
        let stats = DelegationStats::compute("community_1", 5, &[]);
        assert_eq!(stats.advisor_delegated, 0);
        assert_eq!(stats.person_delegated, 0);
        assert_eq!(stats.direct_voters, 5);
        assert!((stats.advisor_percentage - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_compute_zero_members() {
        let stats = DelegationStats::compute("community_1", 0, &[]);
        assert_eq!(stats.advisor_percentage, 0.0);
    }

    #[test]
    fn stats_exceeds_cap() {
        let delegations = vec![
            ("a".into(), DelegateType::Advisor),
            ("b".into(), DelegateType::Advisor),
            ("c".into(), DelegateType::Advisor),
        ];
        let stats = DelegationStats::compute("c1", 5, &delegations);
        // 60% advisor
        assert!(stats.exceeds_cap(50.0));
        assert!(!stats.exceeds_cap(60.0));
        assert!(!stats.exceeds_cap(70.0));
    }

    #[test]
    fn stats_serialization() {
        let stats = DelegationStats::compute("c1", 10, &[("a".into(), DelegateType::Advisor)]);
        let json = serde_json::to_string(&stats).unwrap();
        let restored: DelegationStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }

    // -----------------------------------------------------------------------
    // DeliberationWindow
    // -----------------------------------------------------------------------

    #[test]
    fn deliberation_window_creation() {
        let pid = Uuid::new_v4();
        let window = DeliberationWindow::new(pid, "community_1", 86400);
        assert_eq!(window.proposal_id, pid);
        assert_eq!(window.community_id, "community_1");
        assert_eq!(window.window_duration_secs, 86400);
        assert!(!window.extended);
    }

    #[test]
    fn deliberation_window_expiry() {
        let pid = Uuid::new_v4();
        let past = Utc::now() - chrono::Duration::hours(2);
        let window = DeliberationWindow::with_start(pid, "c1", past, 3600); // 1 hour window, started 2 hours ago

        assert!(window.is_expired());
        assert_eq!(window.remaining_secs(), 0);
    }

    #[test]
    fn deliberation_window_active() {
        let pid = Uuid::new_v4();
        let window = DeliberationWindow::new(pid, "c1", 86400); // 24 hours from now

        assert!(!window.is_expired());
        assert!(window.remaining_secs() > 0);
    }

    #[test]
    fn deliberation_window_is_expired_at() {
        let pid = Uuid::new_v4();
        let start = Utc::now();
        let window = DeliberationWindow::with_start(pid, "c1", start, 3600);

        // 30 minutes in: not expired
        let at_30m = start + chrono::Duration::minutes(30);
        assert!(!window.is_expired_at(at_30m));
        assert!(window.remaining_secs_at(at_30m) > 0);

        // 2 hours in: expired
        let at_2h = start + chrono::Duration::hours(2);
        assert!(window.is_expired_at(at_2h));
        assert_eq!(window.remaining_secs_at(at_2h), 0);
    }

    #[test]
    fn deliberation_window_extend_for_cap() {
        let pid = Uuid::new_v4();
        let window_secs = 86400u64; // 24 hours
        let mut window = DeliberationWindow::new(pid, "c1", window_secs);
        let original_deadline = window.override_deadline;

        assert!(!window.extended);
        window.extend_for_cap_exceedance().unwrap();

        assert!(window.extended);
        assert_eq!(window.total_duration_secs(), window_secs + CAP_EXTENSION_SECS);
        assert!(window.override_deadline > original_deadline);

        // Second extension is idempotent (no-op)
        let deadline_after_first = window.override_deadline;
        window.extend_for_cap_exceedance().unwrap();
        assert_eq!(window.override_deadline, deadline_after_first);
    }

    #[test]
    fn deliberation_window_serialization() {
        let window = DeliberationWindow::new(Uuid::new_v4(), "c1", 86400);
        let json = serde_json::to_string(&window).unwrap();
        let restored: DeliberationWindow = serde_json::from_str(&json).unwrap();
        assert_eq!(window, restored);
    }

    // -----------------------------------------------------------------------
    // GovernanceAIPolicy
    // -----------------------------------------------------------------------

    #[test]
    fn policy_default_is_permissive() {
        let policy = GovernanceAIPolicy::default();
        assert!(policy.is_allowed());
        assert!(policy.human_required_categories.is_empty());
        assert!(policy.max_auto_vote_percentage.is_none());
        assert_eq!(policy.reasoning_transparency, "full");
    }

    #[test]
    fn policy_no_advisor() {
        let policy = GovernanceAIPolicy::no_advisor();
        assert!(!policy.is_allowed());
    }

    #[test]
    fn policy_with_cap() {
        let policy = GovernanceAIPolicy::with_cap(40.0);
        assert!(policy.is_allowed());
        assert_eq!(policy.max_auto_vote_percentage, Some(40.0));
    }

    #[test]
    fn policy_cap_clamps() {
        let policy = GovernanceAIPolicy::with_cap(150.0);
        assert_eq!(policy.max_auto_vote_percentage, Some(100.0));

        let policy = GovernanceAIPolicy::with_cap(-10.0);
        assert_eq!(policy.max_auto_vote_percentage, Some(0.0));
    }

    #[test]
    fn policy_human_required_categories() {
        let policy = GovernanceAIPolicy {
            human_required_categories: vec!["Emergency".into(), "Amendment".into()],
            ..GovernanceAIPolicy::default()
        };

        assert!(policy.requires_human_vote("Emergency"));
        assert!(policy.requires_human_vote("Amendment"));
        assert!(!policy.requires_human_vote("Standard"));
        assert_eq!(policy.human_required_list().len(), 2);
    }

    #[test]
    fn policy_validate_proposal_category_allowed() {
        let policy = GovernanceAIPolicy::default();
        assert!(policy.validate_proposal_category("Standard").is_ok());
    }

    #[test]
    fn policy_validate_proposal_category_not_allowed() {
        let policy = GovernanceAIPolicy::no_advisor();
        let result = policy.validate_proposal_category("Standard");
        assert!(result.is_err());
        assert!(matches!(result, Err(KingdomError::AdvisorDelegationNotAllowed)));
    }

    #[test]
    fn policy_validate_proposal_category_human_required() {
        let policy = GovernanceAIPolicy {
            human_required_categories: vec!["Emergency".into()],
            ..GovernanceAIPolicy::default()
        };
        let result = policy.validate_proposal_category("Emergency");
        assert!(result.is_err());
        assert!(matches!(result, Err(KingdomError::HumanVoteRequired(_))));
    }

    #[test]
    fn policy_check_stats_within_limits() {
        let policy = GovernanceAIPolicy::with_cap(50.0);
        let stats = DelegationStats {
            community_id: "c1".into(),
            total_members: 10,
            advisor_delegated: 3,
            person_delegated: 2,
            direct_voters: 5,
            advisor_percentage: 30.0,
        };
        assert!(!policy.check_stats(&stats).unwrap());
    }

    #[test]
    fn policy_check_stats_cap_exceeded() {
        let policy = GovernanceAIPolicy::with_cap(40.0);
        let stats = DelegationStats {
            community_id: "c1".into(),
            total_members: 10,
            advisor_delegated: 5,
            person_delegated: 0,
            direct_voters: 5,
            advisor_percentage: 50.0,
        };
        assert!(policy.check_stats(&stats).unwrap());
    }

    #[test]
    fn policy_check_stats_advisor_not_allowed() {
        let policy = GovernanceAIPolicy::no_advisor();
        let stats = DelegationStats {
            community_id: "c1".into(),
            total_members: 10,
            advisor_delegated: 1,
            person_delegated: 0,
            direct_voters: 9,
            advisor_percentage: 10.0,
        };
        assert!(policy.check_stats(&stats).is_err());
    }

    #[test]
    fn policy_serialization() {
        let policy = GovernanceAIPolicy {
            advisor_delegation_allowed: true,
            human_required_categories: vec!["Emergency".into()],
            max_auto_vote_percentage: Some(60.0),
            reasoning_transparency: "summary".into(),
        };
        let json = serde_json::to_string(&policy).unwrap();
        let restored: GovernanceAIPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    // -----------------------------------------------------------------------
    // AdvisorDelegationRegistry
    // -----------------------------------------------------------------------

    #[test]
    fn registry_register_and_get() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        let del = reg
            .register("alice", AdvisorDelegationConfig::default())
            .unwrap();
        assert_eq!(del.member_pubkey, "alice");
        assert!(reg.is_advisor_delegated("alice"));
        assert!(!reg.is_advisor_delegated("bob"));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn registry_register_duplicate_fails() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();
        let result = reg.register("alice", AdvisorDelegationConfig::default());
        assert!(result.is_err());
        assert!(matches!(result, Err(KingdomError::AdvisorDelegationExists(_))));
    }

    #[test]
    fn registry_register_when_not_allowed() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::no_advisor());
        let result = reg.register("alice", AdvisorDelegationConfig::default());
        assert!(result.is_err());
        assert!(matches!(result, Err(KingdomError::AdvisorDelegationNotAllowed)));
    }

    #[test]
    fn registry_deactivate() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();
        assert_eq!(reg.count(), 1);

        let removed = reg.deactivate("alice").unwrap();
        assert_eq!(removed.member_pubkey, "alice");
        assert_eq!(reg.count(), 0);
        assert!(!reg.is_advisor_delegated("alice"));
    }

    #[test]
    fn registry_deactivate_not_found() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        let result = reg.deactivate("alice");
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(KingdomError::AdvisorDelegationNotFound(_))
        ));
    }

    #[test]
    fn registry_stats() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();
        reg.register("bob", AdvisorDelegationConfig::default())
            .unwrap();

        let stats = reg.stats(10, 3);
        assert_eq!(stats.total_members, 10);
        assert_eq!(stats.advisor_delegated, 2);
        assert_eq!(stats.person_delegated, 3);
        assert_eq!(stats.direct_voters, 5);
        assert!((stats.advisor_percentage - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_record_override() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();

        let pid = Uuid::new_v4();
        let ov = DelegationOverride::new("alice", pid, "Support", "Oppose");
        reg.record_override("alice", ov).unwrap();

        let del = reg.get("alice").unwrap();
        assert!(del.has_overridden(pid));
    }

    #[test]
    fn registry_record_override_not_found() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        let ov = DelegationOverride::new("alice", Uuid::new_v4(), "Support", "Oppose");
        let result = reg.record_override("alice", ov);
        assert!(result.is_err());
    }

    #[test]
    fn registry_can_advisor_vote_on() {
        let policy = GovernanceAIPolicy {
            human_required_categories: vec!["Emergency".into()],
            ..GovernanceAIPolicy::default()
        };
        let reg = AdvisorDelegationRegistry::new("community_1", policy);

        assert!(reg.can_advisor_vote_on("Standard").is_ok());
        assert!(reg.can_advisor_vote_on("Emergency").is_err());
    }

    #[test]
    fn registry_should_extend_window() {
        let policy = GovernanceAIPolicy::with_cap(30.0);
        let mut reg = AdvisorDelegationRegistry::new("community_1", policy);
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();
        reg.register("bob", AdvisorDelegationConfig::default())
            .unwrap();
        reg.register("charlie", AdvisorDelegationConfig::default())
            .unwrap();

        // 3 out of 5 = 60% > 30% cap
        let should_extend = reg.should_extend_window(5, 0).unwrap();
        assert!(should_extend);

        // 3 out of 20 = 15% < 30% cap
        let should_extend = reg.should_extend_window(20, 0).unwrap();
        assert!(!should_extend);
    }

    #[test]
    fn registry_set_policy() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        assert!(reg.policy().is_allowed());

        reg.set_policy(GovernanceAIPolicy::no_advisor());
        assert!(!reg.policy().is_allowed());
    }

    #[test]
    fn registry_all_delegations() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();
        reg.register("bob", AdvisorDelegationConfig::default())
            .unwrap();

        let all: Vec<_> = reg.all().collect();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn registry_serialization() {
        let mut reg =
            AdvisorDelegationRegistry::new("community_1", GovernanceAIPolicy::default());
        reg.register("alice", AdvisorDelegationConfig::default())
            .unwrap();

        let json = serde_json::to_string(&reg).unwrap();
        let restored: AdvisorDelegationRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, restored);
    }

    // -----------------------------------------------------------------------
    // Integration: cap exceedance triggers window extension
    // -----------------------------------------------------------------------

    #[test]
    fn integration_cap_exceedance_extends_window() {
        let policy = GovernanceAIPolicy::with_cap(40.0);
        let mut reg = AdvisorDelegationRegistry::new("community_1", policy);

        // Register enough advisor delegations to exceed 40%
        for name in &["alice", "bob", "charlie", "dave", "eve"] {
            reg.register(*name, AdvisorDelegationConfig::default())
                .unwrap();
        }

        // 5 out of 8 = 62.5% > 40% cap
        let should_extend = reg.should_extend_window(8, 0).unwrap();
        assert!(should_extend);

        // Create a deliberation window and extend it
        let pid = Uuid::new_v4();
        let mut window = DeliberationWindow::new(pid, "community_1", 86400);
        assert!(!window.extended);

        if should_extend {
            window.extend_for_cap_exceedance().unwrap();
        }

        assert!(window.extended);
        assert_eq!(window.total_duration_secs(), 86400 + CAP_EXTENSION_SECS);
    }

    #[test]
    fn integration_human_required_blocks_advisor_vote() {
        let policy = GovernanceAIPolicy {
            human_required_categories: vec![
                "Emergency".into(),
                "Amendment".into(),
            ],
            ..GovernanceAIPolicy::default()
        };
        let reg = AdvisorDelegationRegistry::new("community_1", policy);

        // Standard proposals: advisor can vote
        assert!(reg.can_advisor_vote_on("Standard").is_ok());
        assert!(reg.can_advisor_vote_on("Policy").is_ok());

        // Human-required categories: advisor cannot vote
        let result = reg.can_advisor_vote_on("Emergency");
        assert!(matches!(result, Err(KingdomError::HumanVoteRequired(cat)) if cat == "Emergency"));

        let result = reg.can_advisor_vote_on("Amendment");
        assert!(matches!(result, Err(KingdomError::HumanVoteRequired(cat)) if cat == "Amendment"));
    }
}
