//! # Community AI Pools (R6B)
//!
//! Communities can pool compute resources for shared AI inference.
//! When a member's local Advisor lacks a capability (R6A assessment),
//! it falls back to the community AI pool. The pool routes to an
//! available `PooledProvider`. If no pool is available, DeferToHuman.
//!
//! # Fortune Integration
//!
//! Tower operators and pool contributors can earn Cool for providing
//! AI inference. The `AIPoolReward` tracks contributor compensation.
//!
//! # Covenant Alignment
//!
//! **Dignity** — AI equity means communities can share compute so everyone
//! gets meaningful AI assistance, regardless of individual hardware.
//! **Sovereignty** — pools are community-controlled. Access policies,
//! fair use limits, and priority ordering are charter decisions.
//! **Consent** — contributing to a pool is voluntary. Fair use limits
//! prevent exploitation.

use std::collections::HashMap;

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::KingdomError;

// ── MinimumCapabilities (mirrors Advisor's R6A) ──────────────────────

bitflags::bitflags! {
    /// AI capabilities that a provider may possess.
    ///
    /// Mirrors `advisor::capability_floor::MinimumCapabilities` with
    /// identical bit values for cross-crate serialization compatibility.
    /// Kingdom doesn't depend on Advisor, so the type is defined here
    /// with the same layout.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct MinimumCapabilities: u32 {
        const TEXT_EDITING          = 0b0000_0001;
        const DESIGN_SUGGESTION     = 0b0000_0010;
        const ACCESSIBILITY_CHECK   = 0b0000_0100;
        const DATA_ANALYSIS         = 0b0000_1000;
        const TRANSLATION           = 0b0001_0000;
        const GOVERNANCE_REASONING  = 0b0010_0000;
        const SEARCH_ASSISTANCE     = 0b0100_0000;
    }
}

impl MinimumCapabilities {
    /// Check if all required capabilities are met.
    pub fn satisfies(&self, required: MinimumCapabilities) -> bool {
        self.contains(required)
    }
}

// ── ProviderCapacity ─────────────────────────────────────────────────

/// The compute capacity a pooled provider offers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderCapacity {
    /// How many concurrent requests this provider can handle.
    pub concurrent_requests: usize,
    /// Maximum tokens per day this provider will serve.
    pub tokens_per_day: usize,
    /// UTC hour ranges when this provider is available.
    /// Each tuple is (start_hour, end_hour) in 24h format.
    pub available_hours: Vec<(u8, u8)>,
}

impl ProviderCapacity {
    /// Create a new capacity with given limits.
    pub fn new(concurrent_requests: usize, tokens_per_day: usize) -> Self {
        Self {
            concurrent_requests,
            tokens_per_day,
            available_hours: Vec::new(),
        }
    }

    /// Add an availability window (UTC hours).
    pub fn with_hours(mut self, start: u8, end: u8) -> Self {
        self.available_hours.push((start, end));
        self
    }

    /// Always available (24/7).
    pub fn always_available(concurrent_requests: usize, tokens_per_day: usize) -> Self {
        Self {
            concurrent_requests,
            tokens_per_day,
            available_hours: vec![(0, 24)],
        }
    }

    /// Check if the provider is available at the given UTC hour.
    pub fn is_available_at(&self, hour: u8) -> bool {
        if self.available_hours.is_empty() {
            return true; // No restrictions = always available.
        }
        self.available_hours
            .iter()
            .any(|&(start, end)| hour >= start && hour < end)
    }
}

// ── PooledProvider ───────────────────────────────────────────────────

/// A provider contributed to a community AI pool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PooledProvider {
    /// Unique identifier for this provider instance.
    pub provider_id: String,
    /// The public key of the community member who contributed this provider.
    pub contributor_pubkey: String,
    /// What this provider can do (from R6A assessment).
    pub capabilities: MinimumCapabilities,
    /// How much compute this provider offers.
    pub capacity: ProviderCapacity,
    /// When this provider was contributed to the pool.
    pub contributed_at: DateTime<Utc>,
}

impl PooledProvider {
    /// Create a new pooled provider.
    pub fn new(
        provider_id: impl Into<String>,
        contributor_pubkey: impl Into<String>,
        capabilities: MinimumCapabilities,
        capacity: ProviderCapacity,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            contributor_pubkey: contributor_pubkey.into(),
            capabilities,
            capacity,
            contributed_at: Utc::now(),
        }
    }

    /// Whether this provider can serve a request needing the given capabilities.
    pub fn can_serve(&self, needed: MinimumCapabilities) -> bool {
        self.capabilities.satisfies(needed)
    }
}

// ── PoolAccess / PoolPriority ────────────────────────────────────────

/// Who can access the community AI pool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PoolAccess {
    /// All community members.
    AllMembers,
    /// Steward role and above only.
    StewardAndAbove,
    /// Specific approved list of public keys.
    ApprovedList(Vec<String>),
}

impl PoolAccess {
    /// Check if a member pubkey is allowed access.
    ///
    /// For `AllMembers` and `StewardAndAbove`, returns true for any
    /// non-empty pubkey (role check happens at a higher layer).
    pub fn allows(&self, pubkey: &str) -> bool {
        match self {
            PoolAccess::AllMembers => !pubkey.is_empty(),
            PoolAccess::StewardAndAbove => !pubkey.is_empty(), // Role check at higher layer.
            PoolAccess::ApprovedList(list) => list.iter().any(|pk| pk == pubkey),
        }
    }
}

/// How pool requests are prioritized.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PoolPriority {
    /// First in, first out.
    FIFO,
    /// Governance requests get highest priority.
    GovernanceFirst,
    /// Round-robin across members.
    RoundRobin,
}

// ── AIPoolPolicy ─────────────────────────────────────────────────────

/// Policy governing how the community AI pool operates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIPoolPolicy {
    /// Who can access the pool.
    pub access: PoolAccess,
    /// Maximum requests per member per day. `None` = unlimited.
    pub fair_use_limit: Option<usize>,
    /// How requests are prioritized.
    pub priority: PoolPriority,
    /// Cool reward rate per 1000 requests served (0 = no rewards).
    pub reward_rate: i64,
}

impl Default for AIPoolPolicy {
    fn default() -> Self {
        Self {
            access: PoolAccess::AllMembers,
            fair_use_limit: Some(100),
            priority: PoolPriority::GovernanceFirst,
            reward_rate: 0,
        }
    }
}

// ── AIPoolUsage ──────────────────────────────────────────────────────

/// Usage tracking for a community AI pool within a period.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIPoolUsage {
    /// Total requests served this period.
    pub total_requests: usize,
    /// Requests per member (pubkey -> count).
    pub requests_by_member: HashMap<String, usize>,
    /// Requests per type (capability name -> count).
    pub requests_by_type: HashMap<String, usize>,
    /// When this tracking period started.
    pub period_start: DateTime<Utc>,
}

impl AIPoolUsage {
    /// Start a new tracking period.
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            requests_by_member: HashMap::new(),
            requests_by_type: HashMap::new(),
            period_start: Utc::now(),
        }
    }

    /// Record a request from a member for a capability type.
    pub fn record_request(&mut self, member_pubkey: &str, capability_type: &str) {
        self.total_requests += 1;
        *self
            .requests_by_member
            .entry(member_pubkey.to_string())
            .or_default() += 1;
        *self
            .requests_by_type
            .entry(capability_type.to_string())
            .or_default() += 1;
    }

    /// How many requests a member has made this period.
    pub fn member_request_count(&self, member_pubkey: &str) -> usize {
        self.requests_by_member
            .get(member_pubkey)
            .copied()
            .unwrap_or(0)
    }

    /// Reset usage for a new period.
    pub fn reset(&mut self) {
        self.total_requests = 0;
        self.requests_by_member.clear();
        self.requests_by_type.clear();
        self.period_start = Utc::now();
    }
}

impl Default for AIPoolUsage {
    fn default() -> Self {
        Self::new()
    }
}

// ── RequestPriority ──────────────────────────────────────────────────

/// Priority level for pool requests.
///
/// Governance is highest — governance votes should not wait behind
/// background tasks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequestPriority {
    /// Low priority background tasks.
    Background = 0,
    /// MagicalIndex search assistance.
    Search = 1,
    /// Normal Throne creative work.
    Creation = 2,
    /// Governance votes and proposal analysis — highest priority.
    Governance = 3,
}

// ── PoolRequest / PoolResponse ───────────────────────────────────────

/// A request routed to the community AI pool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolRequest {
    /// Unique request ID.
    pub id: Uuid,
    /// The requesting member's public key.
    pub requester_pubkey: String,
    /// Which capability is needed.
    pub capability_needed: MinimumCapabilities,
    /// Request priority.
    pub priority: RequestPriority,
    /// The generation context (serialized from Advisor's GenerationContext).
    pub context_json: String,
    /// When the request was submitted.
    pub submitted_at: DateTime<Utc>,
}

impl PoolRequest {
    /// Create a new pool request.
    pub fn new(
        requester_pubkey: impl Into<String>,
        capability_needed: MinimumCapabilities,
        priority: RequestPriority,
        context_json: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            requester_pubkey: requester_pubkey.into(),
            capability_needed,
            priority,
            context_json: context_json.into(),
            submitted_at: Utc::now(),
        }
    }
}

/// A response from the community AI pool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolResponse {
    /// Which request this responds to.
    pub request_id: Uuid,
    /// Which provider served the request.
    pub provider_used: String,
    /// The generation result (serialized from Advisor's GenerationResult).
    pub result_json: String,
    /// Compute cost in Cool, if the pool charges.
    pub compute_cost: Option<i64>,
}

// ── AIPoolReward ─────────────────────────────────────────────────────

/// Reward tracking for a pool contributor.
///
/// Contributors earn Cool for providing AI inference to the community.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIPoolReward {
    /// The contributor's public key.
    pub contributor_pubkey: String,
    /// Number of requests served in this period.
    pub requests_served: usize,
    /// Cool earned this period.
    pub cool_earned: i64,
    /// Duration of the reward period in seconds.
    pub period_seconds: u64,
}

impl AIPoolReward {
    /// Calculate reward for a contributor based on policy rate.
    pub fn calculate(
        contributor_pubkey: impl Into<String>,
        requests_served: usize,
        reward_rate: i64,
        period_seconds: u64,
    ) -> Self {
        let cool_earned = (requests_served as i64 * reward_rate) / 1000;
        Self {
            contributor_pubkey: contributor_pubkey.into(),
            requests_served,
            cool_earned,
            period_seconds,
        }
    }
}

// ── AIPool ───────────────────────────────────────────────────────────

/// A community AI pool — shared compute for community members.
///
/// # Example
///
/// ```
/// use kingdom::ai_pool::*;
///
/// let mut pool = AIPool::new("pool-1", "design-guild");
/// pool.add_provider(PooledProvider::new(
///     "llama-node-1",
///     "cpub1alice",
///     MinimumCapabilities::all(),
///     ProviderCapacity::always_available(4, 100_000),
/// ));
/// assert_eq!(pool.provider_count(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AIPool {
    /// Unique pool identifier.
    pub id: Uuid,
    /// Human-readable pool name.
    pub name: String,
    /// The community this pool belongs to.
    pub community_id: String,
    /// Providers contributed to this pool.
    pub providers: Vec<PooledProvider>,
    /// Pool access and priority policy.
    pub policy: AIPoolPolicy,
    /// Usage tracking for the current period.
    pub usage: AIPoolUsage,
}

impl AIPool {
    /// Create a new empty pool for a community.
    pub fn new(name: impl Into<String>, community_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            community_id: community_id.into(),
            providers: Vec::new(),
            policy: AIPoolPolicy::default(),
            usage: AIPoolUsage::new(),
        }
    }

    /// Create a pool with a specific policy.
    pub fn with_policy(mut self, policy: AIPoolPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Add a provider to the pool.
    pub fn add_provider(&mut self, provider: PooledProvider) {
        self.providers.push(provider);
    }

    /// Remove a provider by ID.
    pub fn remove_provider(&mut self, provider_id: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|p| p.provider_id != provider_id);
        self.providers.len() < before
    }

    /// Number of providers in the pool.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Combined capabilities across all providers.
    pub fn combined_capabilities(&self) -> MinimumCapabilities {
        self.providers
            .iter()
            .fold(MinimumCapabilities::empty(), |acc, p| acc | p.capabilities)
    }

    /// Check if a member can access this pool.
    pub fn can_access(&self, pubkey: &str) -> bool {
        self.policy.access.allows(pubkey)
    }

    /// Check if a member has exceeded fair use limits.
    pub fn exceeds_fair_use(&self, pubkey: &str) -> bool {
        if let Some(limit) = self.policy.fair_use_limit {
            self.usage.member_request_count(pubkey) >= limit
        } else {
            false
        }
    }

    /// Route a request to the best available provider.
    ///
    /// Returns the provider ID that should handle this request, or an error
    /// if no suitable provider is found.
    pub fn route_request(&self, request: &PoolRequest) -> Result<String, KingdomError> {
        // Check access.
        if !self.can_access(&request.requester_pubkey) {
            return Err(KingdomError::MemberNotFound(
                request.requester_pubkey.clone(),
            ));
        }

        // Check fair use.
        if self.exceeds_fair_use(&request.requester_pubkey) {
            return Err(KingdomError::AdvisorCapExceeded {
                percentage: 100.0,
                cap: self.policy.fair_use_limit.unwrap_or(0) as f64,
            });
        }

        // Find a provider with the needed capability.
        let current_hour = Utc::now().hour() as u8;
        let provider = self
            .providers
            .iter()
            .find(|p| {
                p.can_serve(request.capability_needed)
                    && p.capacity.is_available_at(current_hour)
            })
            .ok_or_else(|| {
                KingdomError::DelegateNotFound(format!(
                    "no provider with capability {:?}",
                    request.capability_needed
                ))
            })?;

        Ok(provider.provider_id.clone())
    }

    /// Record a completed request in usage tracking.
    pub fn record_request(&mut self, member_pubkey: &str, capability_type: &str) {
        self.usage.record_request(member_pubkey, capability_type);
    }

    /// Calculate rewards for all contributors in the current period.
    pub fn calculate_rewards(&self, period_seconds: u64) -> Vec<AIPoolReward> {
        let mut provider_requests: HashMap<&str, usize> = HashMap::new();

        // Count requests served per contributor (simplified: proportional to capacity).
        let total_capacity: usize = self
            .providers
            .iter()
            .map(|p| p.capacity.concurrent_requests)
            .sum();

        if total_capacity == 0 || self.usage.total_requests == 0 {
            return Vec::new();
        }

        for provider in &self.providers {
            let share =
                provider.capacity.concurrent_requests as f64 / total_capacity as f64;
            let attributed =
                (self.usage.total_requests as f64 * share).round() as usize;
            *provider_requests
                .entry(provider.contributor_pubkey.as_str())
                .or_default() += attributed;
        }

        provider_requests
            .into_iter()
            .map(|(pubkey, requests)| {
                AIPoolReward::calculate(pubkey, requests, self.policy.reward_rate, period_seconds)
            })
            .collect()
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider(id: &str, pubkey: &str, caps: MinimumCapabilities) -> PooledProvider {
        PooledProvider::new(
            id,
            pubkey,
            caps,
            ProviderCapacity::always_available(4, 100_000),
        )
    }

    // --- MinimumCapabilities ---

    #[test]
    fn capability_flags() {
        let caps = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::GOVERNANCE_REASONING;
        assert!(caps.satisfies(MinimumCapabilities::TEXT_EDITING));
        assert!(!caps.satisfies(MinimumCapabilities::TRANSLATION));
    }

    #[test]
    fn capability_serde() {
        let caps = MinimumCapabilities::all();
        let json = serde_json::to_string(&caps).unwrap();
        let restored: MinimumCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, restored);
    }

    // --- ProviderCapacity ---

    #[test]
    fn capacity_always_available() {
        let cap = ProviderCapacity::always_available(4, 100_000);
        assert!(cap.is_available_at(0));
        assert!(cap.is_available_at(12));
        assert!(cap.is_available_at(23));
    }

    #[test]
    fn capacity_limited_hours() {
        let cap = ProviderCapacity::new(2, 50_000)
            .with_hours(8, 18); // 8am-6pm UTC
        assert!(!cap.is_available_at(6));
        assert!(cap.is_available_at(12));
        assert!(!cap.is_available_at(20));
    }

    #[test]
    fn capacity_no_restrictions() {
        let cap = ProviderCapacity::new(1, 10_000); // No available_hours set.
        assert!(cap.is_available_at(3)); // Empty = always available.
    }

    // --- PooledProvider ---

    #[test]
    fn pooled_provider_can_serve() {
        let provider = make_provider("p1", "cpub1alice", MinimumCapabilities::all());
        assert!(provider.can_serve(MinimumCapabilities::GOVERNANCE_REASONING));
    }

    #[test]
    fn pooled_provider_cannot_serve_missing() {
        let provider = make_provider("p1", "cpub1alice", MinimumCapabilities::TEXT_EDITING);
        assert!(!provider.can_serve(MinimumCapabilities::GOVERNANCE_REASONING));
    }

    // --- PoolAccess ---

    #[test]
    fn access_all_members() {
        let access = PoolAccess::AllMembers;
        assert!(access.allows("cpub1alice"));
        assert!(!access.allows("")); // Empty pubkey rejected.
    }

    #[test]
    fn access_approved_list() {
        let access = PoolAccess::ApprovedList(vec![
            "cpub1alice".to_string(),
            "cpub1bob".to_string(),
        ]);
        assert!(access.allows("cpub1alice"));
        assert!(!access.allows("cpub1carol"));
    }

    // --- AIPoolPolicy ---

    #[test]
    fn default_policy() {
        let policy = AIPoolPolicy::default();
        assert_eq!(policy.access, PoolAccess::AllMembers);
        assert_eq!(policy.fair_use_limit, Some(100));
        assert_eq!(policy.priority, PoolPriority::GovernanceFirst);
    }

    // --- AIPoolUsage ---

    #[test]
    fn usage_tracking() {
        let mut usage = AIPoolUsage::new();
        usage.record_request("cpub1alice", "governance_reasoning");
        usage.record_request("cpub1alice", "text_editing");
        usage.record_request("cpub1bob", "governance_reasoning");

        assert_eq!(usage.total_requests, 3);
        assert_eq!(usage.member_request_count("cpub1alice"), 2);
        assert_eq!(usage.member_request_count("cpub1bob"), 1);
        assert_eq!(usage.member_request_count("cpub1carol"), 0);
    }

    #[test]
    fn usage_reset() {
        let mut usage = AIPoolUsage::new();
        usage.record_request("cpub1alice", "text_editing");
        assert_eq!(usage.total_requests, 1);

        usage.reset();
        assert_eq!(usage.total_requests, 0);
        assert_eq!(usage.member_request_count("cpub1alice"), 0);
    }

    // --- RequestPriority ---

    #[test]
    fn priority_ordering() {
        assert!(RequestPriority::Governance > RequestPriority::Creation);
        assert!(RequestPriority::Creation > RequestPriority::Search);
        assert!(RequestPriority::Search > RequestPriority::Background);
    }

    // --- PoolRequest ---

    #[test]
    fn pool_request_creation() {
        let request = PoolRequest::new(
            "cpub1alice",
            MinimumCapabilities::GOVERNANCE_REASONING,
            RequestPriority::Governance,
            r#"{"prompt":"analyze proposal"}"#,
        );
        assert_eq!(request.requester_pubkey, "cpub1alice");
        assert_eq!(request.priority, RequestPriority::Governance);
    }

    #[test]
    fn pool_request_serde() {
        let request = PoolRequest::new(
            "cpub1bob",
            MinimumCapabilities::TEXT_EDITING,
            RequestPriority::Creation,
            "{}",
        );
        let json = serde_json::to_string(&request).unwrap();
        let restored: PoolRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.requester_pubkey, "cpub1bob");
    }

    // --- AIPoolReward ---

    #[test]
    fn reward_calculation() {
        let reward = AIPoolReward::calculate(
            "cpub1alice",
            5000,
            10, // 10 Cool per 1000 requests
            86400,
        );
        assert_eq!(reward.cool_earned, 50); // 5000 * 10 / 1000
        assert_eq!(reward.requests_served, 5000);
    }

    #[test]
    fn reward_zero_rate() {
        let reward = AIPoolReward::calculate("cpub1bob", 1000, 0, 3600);
        assert_eq!(reward.cool_earned, 0);
    }

    // --- AIPool ---

    #[test]
    fn pool_creation() {
        let pool = AIPool::new("pool-1", "design-guild");
        assert_eq!(pool.community_id, "design-guild");
        assert_eq!(pool.provider_count(), 0);
    }

    #[test]
    fn pool_add_remove_provider() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.add_provider(make_provider("p1", "cpub1alice", MinimumCapabilities::all()));
        pool.add_provider(make_provider("p2", "cpub1bob", MinimumCapabilities::TEXT_EDITING));
        assert_eq!(pool.provider_count(), 2);

        assert!(pool.remove_provider("p1"));
        assert_eq!(pool.provider_count(), 1);
        assert!(!pool.remove_provider("nonexistent"));
    }

    #[test]
    fn pool_combined_capabilities() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.add_provider(make_provider(
            "p1",
            "cpub1alice",
            MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::TRANSLATION,
        ));
        pool.add_provider(make_provider(
            "p2",
            "cpub1bob",
            MinimumCapabilities::GOVERNANCE_REASONING | MinimumCapabilities::DATA_ANALYSIS,
        ));

        let combined = pool.combined_capabilities();
        assert!(combined.contains(MinimumCapabilities::TEXT_EDITING));
        assert!(combined.contains(MinimumCapabilities::GOVERNANCE_REASONING));
        assert!(!combined.contains(MinimumCapabilities::DESIGN_SUGGESTION));
    }

    #[test]
    fn pool_access_check() {
        let pool = AIPool::new("pool-1", "guild");
        assert!(pool.can_access("cpub1alice"));
        assert!(!pool.can_access(""));
    }

    #[test]
    fn pool_fair_use_limit() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.policy.fair_use_limit = Some(2);

        assert!(!pool.exceeds_fair_use("cpub1alice"));
        pool.record_request("cpub1alice", "text_editing");
        assert!(!pool.exceeds_fair_use("cpub1alice"));
        pool.record_request("cpub1alice", "text_editing");
        assert!(pool.exceeds_fair_use("cpub1alice"));
    }

    #[test]
    fn pool_route_request_success() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.add_provider(make_provider("p1", "cpub1alice", MinimumCapabilities::all()));

        let request = PoolRequest::new(
            "cpub1bob",
            MinimumCapabilities::TEXT_EDITING,
            RequestPriority::Creation,
            "{}",
        );

        let result = pool.route_request(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "p1");
    }

    #[test]
    fn pool_route_request_no_capability() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.add_provider(make_provider(
            "p1",
            "cpub1alice",
            MinimumCapabilities::TEXT_EDITING,
        ));

        let request = PoolRequest::new(
            "cpub1bob",
            MinimumCapabilities::GOVERNANCE_REASONING,
            RequestPriority::Governance,
            "{}",
        );

        let result = pool.route_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn pool_route_request_fair_use_exceeded() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.policy.fair_use_limit = Some(1);
        pool.add_provider(make_provider("p1", "cpub1alice", MinimumCapabilities::all()));
        pool.record_request("cpub1bob", "text_editing");

        let request = PoolRequest::new(
            "cpub1bob",
            MinimumCapabilities::TEXT_EDITING,
            RequestPriority::Creation,
            "{}",
        );

        let result = pool.route_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn pool_route_request_access_denied() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.policy.access = PoolAccess::ApprovedList(vec!["cpub1alice".to_string()]);
        pool.add_provider(make_provider("p1", "cpub1alice", MinimumCapabilities::all()));

        let request = PoolRequest::new(
            "cpub1bob",
            MinimumCapabilities::TEXT_EDITING,
            RequestPriority::Creation,
            "{}",
        );

        let result = pool.route_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn pool_with_policy() {
        let policy = AIPoolPolicy {
            access: PoolAccess::StewardAndAbove,
            fair_use_limit: Some(50),
            priority: PoolPriority::RoundRobin,
            reward_rate: 5,
        };
        let pool = AIPool::new("pool-1", "guild").with_policy(policy);
        assert_eq!(pool.policy.priority, PoolPriority::RoundRobin);
        assert_eq!(pool.policy.reward_rate, 5);
    }

    #[test]
    fn pool_calculate_rewards() {
        let mut pool = AIPool::new("pool-1", "guild");
        pool.policy.reward_rate = 10;
        pool.add_provider(make_provider("p1", "cpub1alice", MinimumCapabilities::all()));

        // Record some usage.
        for _ in 0..100 {
            pool.record_request("cpub1bob", "text_editing");
        }

        let rewards = pool.calculate_rewards(86400);
        assert_eq!(rewards.len(), 1);
        assert_eq!(rewards[0].contributor_pubkey, "cpub1alice");
        assert_eq!(rewards[0].cool_earned, 1); // 100 * 10 / 1000
    }

    #[test]
    fn pool_calculate_rewards_empty() {
        let pool = AIPool::new("pool-1", "guild");
        let rewards = pool.calculate_rewards(86400);
        assert!(rewards.is_empty());
    }

    #[test]
    fn pool_serde() {
        let mut pool = AIPool::new("pool-1", "design-guild");
        pool.add_provider(make_provider("p1", "cpub1alice", MinimumCapabilities::all()));

        let json = serde_json::to_string(&pool).unwrap();
        let restored: AIPool = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.community_id, "design-guild");
        assert_eq!(restored.provider_count(), 1);
    }

    #[test]
    fn pool_response_serde() {
        let response = PoolResponse {
            request_id: Uuid::new_v4(),
            provider_used: "p1".to_string(),
            result_json: r#"{"content":"analysis complete"}"#.to_string(),
            compute_cost: Some(5),
        };
        let json = serde_json::to_string(&response).unwrap();
        let restored: PoolResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.provider_used, "p1");
        assert_eq!(restored.compute_cost, Some(5));
    }
}
