//! # Network Defense — Connection-level safety for Towers
//!
//! Towers are relay nodes with open ports. This module provides the
//! connection-level defense layer: IP allowlisting (from Gospel),
//! rate limiting, and connection policies that compose into a single
//! [`ConnectionGuard`] check.
//!
//! This lives in Globe (not Bulwark) because it's about TCP connections
//! and transport — not identity or trust. Bulwark handles people;
//! Globe handles packets.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — rate limits protect the network without discrimination.
//! **Sovereignty** — Tower operators choose their connection policy.
//! **Consent** — allowlist membership is explicit and revocable.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Instant;

use log::warn;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ConnectionPolicy
// ---------------------------------------------------------------------------

/// How a Tower decides which connections to accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionPolicy {
    /// Accept any connection (rate limits still apply).
    AllowAll,
    /// Only accept connections from known Tower IPs in the allowlist.
    AllowlistOnly,
    /// Accept allowlisted Tower IPs plus authenticated clients.
    AllowlistWithClientAuth,
}

// ---------------------------------------------------------------------------
// ConnectionVerdict
// ---------------------------------------------------------------------------

/// The outcome of a connection check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionVerdict {
    /// Connection is allowed to proceed.
    Accept,
    /// Connection is rejected.
    Reject { reason: String },
    /// Connection is temporarily throttled.
    RateLimit { retry_after_secs: u64 },
}

impl ConnectionVerdict {
    /// Returns `true` if the verdict allows the connection.
    pub fn is_accepted(&self) -> bool {
        matches!(self, ConnectionVerdict::Accept)
    }
}

// ---------------------------------------------------------------------------
// RateLimitConfig
// ---------------------------------------------------------------------------

/// Configuration for rate limiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum simultaneous connections from a single IP.
    pub max_connections_per_ip: u32,
    /// Maximum events per minute from a single IP.
    pub max_events_per_minute_per_ip: u32,
    /// Maximum total simultaneous connections across all IPs.
    pub max_connections_total: u32,
    /// Cooldown period in seconds when limits are exceeded.
    pub cooldown_secs: u64,
}

impl RateLimitConfig {
    /// Create a new config with sensible defaults.
    pub fn new() -> Self {
        Self {
            max_connections_per_ip: 100,
            max_events_per_minute_per_ip: 120,
            max_connections_total: 10_000,
            cooldown_secs: 60,
        }
    }

    /// Set the maximum connections per IP.
    pub fn with_max_connections_per_ip(mut self, max: u32) -> Self {
        self.max_connections_per_ip = max;
        self
    }

    /// Set the maximum events per minute per IP.
    pub fn with_max_events_per_minute_per_ip(mut self, max: u32) -> Self {
        self.max_events_per_minute_per_ip = max;
        self
    }

    /// Set the maximum total connections.
    pub fn with_max_connections_total(mut self, max: u32) -> Self {
        self.max_connections_total = max;
        self
    }

    /// Set the cooldown period in seconds.
    pub fn with_cooldown_secs(mut self, secs: u64) -> Self {
        self.cooldown_secs = secs;
        self
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConnectionTracker (internal)
// ---------------------------------------------------------------------------

/// Per-IP connection state. Not public — internal to [`RateLimiter`].
#[derive(Debug, Clone)]
struct ConnectionTracker {
    /// Current number of active connections from this IP.
    count: u32,
    /// Number of events recorded in the current window.
    events: u32,
    /// Start of the current rate-limit window.
    window_start: Instant,
    /// If set, this IP is blocked until the given instant.
    blocked_until: Option<Instant>,
}

impl ConnectionTracker {
    fn new() -> Self {
        Self {
            count: 0,
            events: 0,
            window_start: Instant::now(),
            blocked_until: None,
        }
    }
}

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

/// Tracks connection counts and event rates per IP.
///
/// Not serializable — contains [`Instant`] timing state that only makes
/// sense for the lifetime of the process.
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    trackers: HashMap<IpAddr, ConnectionTracker>,
    total_connections: u32,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            trackers: HashMap::new(),
            total_connections: 0,
        }
    }

    /// Check whether a new connection from `ip` should be allowed.
    ///
    /// If accepted, the connection count is incremented. Call
    /// [`release_connection`](Self::release_connection) when the connection
    /// closes.
    ///
    /// Loopback addresses (127.0.0.0/8 and ::1) are exempt from per-IP
    /// limits — they are internal connections (daemon self-connect,
    /// tunnel proxies). They still count toward the global total.
    pub fn check_connection(&mut self, ip: IpAddr) -> ConnectionVerdict {
        // Global connection cap.
        if self.total_connections >= self.config.max_connections_total {
            warn!(
                "total connection limit reached ({}/{})",
                self.total_connections, self.config.max_connections_total
            );
            return ConnectionVerdict::Reject {
                reason: "total connection limit reached".into(),
            };
        }

        // Loopback is always accepted (no per-IP limits).
        if ip.is_loopback() {
            self.total_connections += 1;
            return ConnectionVerdict::Accept;
        }

        let cooldown_secs = self.config.cooldown_secs;
        let max_per_ip = self.config.max_connections_per_ip;

        let tracker = self
            .trackers
            .entry(ip)
            .or_insert_with(ConnectionTracker::new);

        // Check if this IP is currently blocked.
        if let Some(blocked_until) = tracker.blocked_until {
            if Instant::now() < blocked_until {
                let remaining = blocked_until.duration_since(Instant::now()).as_secs();
                return ConnectionVerdict::RateLimit {
                    retry_after_secs: remaining.max(1),
                };
            }
            // Block expired — reset.
            tracker.blocked_until = None;
            tracker.events = 0;
            tracker.window_start = Instant::now();
        }

        // Per-IP connection limit.
        if tracker.count >= max_per_ip {
            warn!(
                "per-IP connection limit reached for {} ({}/{})",
                ip, tracker.count, max_per_ip
            );
            tracker.blocked_until =
                Some(Instant::now() + std::time::Duration::from_secs(cooldown_secs));
            return ConnectionVerdict::RateLimit {
                retry_after_secs: cooldown_secs,
            };
        }

        // Accept — increment counts.
        tracker.count += 1;
        self.total_connections += 1;
        ConnectionVerdict::Accept
    }

    /// Record an event from `ip` (e.g. a message, a request).
    ///
    /// Returns a rate-limit verdict if the event rate has been exceeded.
    pub fn record_event(&mut self, ip: IpAddr) -> ConnectionVerdict {
        let cooldown_secs = self.config.cooldown_secs;
        let max_events = self.config.max_events_per_minute_per_ip;

        let tracker = self
            .trackers
            .entry(ip)
            .or_insert_with(ConnectionTracker::new);

        // Check if blocked.
        if let Some(blocked_until) = tracker.blocked_until {
            if Instant::now() < blocked_until {
                let remaining = blocked_until.duration_since(Instant::now()).as_secs();
                return ConnectionVerdict::RateLimit {
                    retry_after_secs: remaining.max(1),
                };
            }
            // Block expired — reset.
            tracker.blocked_until = None;
            tracker.events = 0;
            tracker.window_start = Instant::now();
        }

        // Reset window if cooldown has elapsed.
        let elapsed = tracker.window_start.elapsed();
        if elapsed.as_secs() >= cooldown_secs {
            tracker.events = 0;
            tracker.window_start = Instant::now();
        }

        tracker.events += 1;

        if tracker.events > max_events {
            warn!(
                "event rate limit exceeded for {} ({}/{})",
                ip, tracker.events, max_events
            );
            tracker.blocked_until =
                Some(Instant::now() + std::time::Duration::from_secs(cooldown_secs));
            return ConnectionVerdict::RateLimit {
                retry_after_secs: cooldown_secs,
            };
        }

        ConnectionVerdict::Accept
    }

    /// Release a connection from `ip` (call when the connection closes).
    pub fn release_connection(&mut self, ip: IpAddr) {
        if let Some(tracker) = self.trackers.get_mut(&ip) {
            tracker.count = tracker.count.saturating_sub(1);
            self.total_connections = self.total_connections.saturating_sub(1);
        }
    }

    /// Remove expired entries. Call periodically to reclaim memory.
    pub fn cleanup(&mut self) {
        let cooldown = std::time::Duration::from_secs(self.config.cooldown_secs);
        self.trackers.retain(|_ip, tracker| {
            // Keep if there are active connections.
            if tracker.count > 0 {
                return true;
            }
            // Keep if currently blocked.
            if let Some(blocked_until) = tracker.blocked_until {
                if Instant::now() < blocked_until {
                    return true;
                }
            }
            // Keep if the window is still active.
            tracker.window_start.elapsed() < cooldown
        });
    }

    /// Number of IPs currently in a blocked state.
    pub fn blocked_count(&self) -> usize {
        let now = Instant::now();
        self.trackers
            .values()
            .filter(|t| t.blocked_until.is_some_and(|b| now < b))
            .count()
    }
}

// ---------------------------------------------------------------------------
// IpAllowlist
// ---------------------------------------------------------------------------

/// A set of allowed IP addresses, dynamically updatable.
///
/// Populated from Gospel (the registry of known Tower IPs).
#[derive(Debug, Clone)]
pub struct IpAllowlist {
    ips: HashSet<IpAddr>,
}

impl IpAllowlist {
    /// Create an empty allowlist.
    pub fn new() -> Self {
        Self {
            ips: HashSet::new(),
        }
    }

    /// Create an allowlist pre-populated with the given IPs.
    pub fn with_ips(ips: impl IntoIterator<Item = IpAddr>) -> Self {
        Self {
            ips: ips.into_iter().collect(),
        }
    }

    /// Add an IP to the allowlist.
    pub fn allow(&mut self, ip: IpAddr) {
        self.ips.insert(ip);
    }

    /// Remove an IP from the allowlist.
    pub fn remove(&mut self, ip: IpAddr) {
        self.ips.remove(&ip);
    }

    /// Check if an IP is in the allowlist.
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        self.ips.contains(&ip)
    }

    /// Replace the entire allowlist (e.g. when Gospel propagates new IPs).
    pub fn update(&mut self, ips: HashSet<IpAddr>) {
        self.ips = ips;
    }

    /// Number of IPs in the allowlist.
    pub fn len(&self) -> usize {
        self.ips.len()
    }

    /// Whether the allowlist is empty.
    pub fn is_empty(&self) -> bool {
        self.ips.is_empty()
    }
}

impl Default for IpAllowlist {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConnectionStats
// ---------------------------------------------------------------------------

/// Simple stats snapshot for a [`ConnectionGuard`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// Total connections accepted since creation.
    pub total_allowed: u64,
    /// Total connections rejected since creation.
    pub total_rejected: u64,
    /// Total connections rate-limited since creation.
    pub total_rate_limited: u64,
    /// Number of IPs currently in a blocked state.
    pub currently_blocked_ips: usize,
    /// Number of IPs in the allowlist.
    pub allowlist_size: usize,
}

// ---------------------------------------------------------------------------
// ConnectionGuard
// ---------------------------------------------------------------------------

/// Composes allowlist + rate limiter + policy into a single connection check.
///
/// This is the main entry point for Tower network defense. When a new
/// connection arrives, call [`check`](Self::check) with the remote IP.
///
/// ```text
///   incoming connection
///         │
///         ▼
///   ┌─ policy check ─┐
///   │  AllowAll?      │── yes ──► rate limiter ──► verdict
///   │  AllowlistOnly? │── check allowlist ──► rate limiter ──► verdict
///   │  WithClientAuth?│── check allowlist (tower) or defer auth ──► rate limiter ──► verdict
///   └─────────────────┘
/// ```
#[derive(Debug)]
pub struct ConnectionGuard {
    policy: ConnectionPolicy,
    allowlist: IpAllowlist,
    rate_limiter: RateLimiter,
    stats: ConnectionStats,
}

impl ConnectionGuard {
    /// Create a new connection guard.
    pub fn new(
        policy: ConnectionPolicy,
        allowlist: IpAllowlist,
        rate_limiter: RateLimiter,
    ) -> Self {
        let allowlist_size = allowlist.len();
        Self {
            policy,
            allowlist,
            rate_limiter,
            stats: ConnectionStats {
                total_allowed: 0,
                total_rejected: 0,
                total_rate_limited: 0,
                currently_blocked_ips: 0,
                allowlist_size,
            },
        }
    }

    /// Check whether a connection from `ip` should be accepted.
    ///
    /// Evaluates in order: policy (allowlist check) then rate limits.
    pub fn check(&mut self, ip: IpAddr) -> ConnectionVerdict {
        // Step 1: Policy check.
        match self.policy {
            ConnectionPolicy::AllowAll => {
                // No allowlist check — proceed to rate limiter.
            }
            ConnectionPolicy::AllowlistOnly => {
                if !self.allowlist.is_allowed(ip) {
                    self.stats.total_rejected += 1;
                    return ConnectionVerdict::Reject {
                        reason: format!("IP {} not in allowlist", ip),
                    };
                }
            }
            ConnectionPolicy::AllowlistWithClientAuth => {
                // Allowlisted Tower IPs pass through. Non-allowlisted IPs
                // are not rejected outright — they must authenticate as
                // clients. We accept here and let the application layer
                // handle client auth. The rate limiter still applies.
                if !self.allowlist.is_allowed(ip) {
                    // Not a known Tower — allowed but will need auth.
                    // Still subject to rate limiting below.
                }
            }
        }

        // Step 2: Rate limit check.
        let verdict = self.rate_limiter.check_connection(ip);
        match &verdict {
            ConnectionVerdict::Accept => self.stats.total_allowed += 1,
            ConnectionVerdict::Reject { .. } => self.stats.total_rejected += 1,
            ConnectionVerdict::RateLimit { .. } => self.stats.total_rate_limited += 1,
        }
        self.stats.currently_blocked_ips = self.rate_limiter.blocked_count();
        self.stats.allowlist_size = self.allowlist.len();

        verdict
    }

    /// Release a connection (call when it closes).
    pub fn release(&mut self, ip: IpAddr) {
        self.rate_limiter.release_connection(ip);
    }

    /// Replace the allowlist (called when Gospel propagates new Tower IPs).
    pub fn update_allowlist(&mut self, ips: HashSet<IpAddr>) {
        self.allowlist.update(ips);
        self.stats.allowlist_size = self.allowlist.len();
    }

    /// Change the connection policy at runtime.
    pub fn set_policy(&mut self, policy: ConnectionPolicy) {
        self.policy = policy;
    }

    /// Snapshot of current stats.
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            currently_blocked_ips: self.rate_limiter.blocked_count(),
            allowlist_size: self.allowlist.len(),
            ..self.stats.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// FederationFilter
// ---------------------------------------------------------------------------

/// A set of community IDs that this node considers federated.
///
/// Used by Tower and other apps to check whether incoming events,
/// connections, or peers belong to federated communities.
///
/// From Constellation Art. 3 §3: "Communities may enter into federated
/// agreements to share governance, coordinate resource stewardship, or
/// pursue common purpose."
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FederationFilter {
    federated_communities: HashSet<String>,
}

impl FederationFilter {
    /// Create an empty federation filter (no communities federated).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a filter from a list of federated community IDs.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            federated_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Add a community to the federated set.
    pub fn add_community(&mut self, community_id: impl Into<String>) {
        self.federated_communities.insert(community_id.into());
    }

    /// Remove a community from the federated set.
    pub fn remove_community(&mut self, community_id: &str) {
        self.federated_communities.remove(community_id);
    }

    /// Replace the entire federated set.
    pub fn update(&mut self, communities: HashSet<String>) {
        self.federated_communities = communities;
    }

    /// Check if a specific community is federated.
    pub fn is_federated(&self, community_id: &str) -> bool {
        self.federated_communities.contains(community_id)
    }

    /// Check if ANY of the given communities are federated.
    ///
    /// Used to check Tower announcements: "does this Tower serve
    /// any community that we're federated with?"
    pub fn has_overlap(&self, communities: &[String]) -> bool {
        communities.iter().any(|c| self.federated_communities.contains(c))
    }

    /// Number of federated communities.
    pub fn len(&self) -> usize {
        self.federated_communities.len()
    }

    /// Whether the filter is empty (no communities federated).
    pub fn is_empty(&self) -> bool {
        self.federated_communities.is_empty()
    }

    /// Get all federated community IDs.
    pub fn communities(&self) -> &HashSet<String> {
        &self.federated_communities
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    // -----------------------------------------------------------------------
    // ConnectionPolicy
    // -----------------------------------------------------------------------

    #[test]
    fn connection_policy_serialization() {
        let policy = ConnectionPolicy::AllowlistOnly;
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: ConnectionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, deserialized);
    }

    // -----------------------------------------------------------------------
    // ConnectionVerdict
    // -----------------------------------------------------------------------

    #[test]
    fn verdict_is_accepted() {
        assert!(ConnectionVerdict::Accept.is_accepted());
        assert!(!ConnectionVerdict::Reject {
            reason: "nope".into()
        }
        .is_accepted());
        assert!(!ConnectionVerdict::RateLimit {
            retry_after_secs: 30
        }
        .is_accepted());
    }

    // -----------------------------------------------------------------------
    // RateLimitConfig
    // -----------------------------------------------------------------------

    #[test]
    fn rate_limit_config_defaults() {
        let config = RateLimitConfig::new();
        assert_eq!(config.max_connections_per_ip, 100);
        assert_eq!(config.max_events_per_minute_per_ip, 120);
        assert_eq!(config.max_connections_total, 10_000);
        assert_eq!(config.cooldown_secs, 60);
    }

    #[test]
    fn rate_limit_config_builder() {
        let config = RateLimitConfig::new()
            .with_max_connections_per_ip(5)
            .with_max_events_per_minute_per_ip(60)
            .with_max_connections_total(500)
            .with_cooldown_secs(30);
        assert_eq!(config.max_connections_per_ip, 5);
        assert_eq!(config.max_events_per_minute_per_ip, 60);
        assert_eq!(config.max_connections_total, 500);
        assert_eq!(config.cooldown_secs, 30);
    }

    #[test]
    fn rate_limit_config_serialization() {
        let config = RateLimitConfig::new().with_max_connections_per_ip(5);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RateLimitConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    // -----------------------------------------------------------------------
    // IpAllowlist
    // -----------------------------------------------------------------------

    #[test]
    fn allowlist_empty() {
        let allowlist = IpAllowlist::new();
        assert!(allowlist.is_empty());
        assert_eq!(allowlist.len(), 0);
        assert!(!allowlist.is_allowed(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    }

    #[test]
    fn allowlist_add_remove() {
        let mut allowlist = IpAllowlist::new();
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        allowlist.allow(ip);
        assert!(allowlist.is_allowed(ip));
        assert_eq!(allowlist.len(), 1);

        allowlist.remove(ip);
        assert!(!allowlist.is_allowed(ip));
        assert!(allowlist.is_empty());
    }

    #[test]
    fn allowlist_with_ips() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let ip3 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

        let allowlist = IpAllowlist::with_ips(vec![ip1, ip2]);
        assert!(allowlist.is_allowed(ip1));
        assert!(allowlist.is_allowed(ip2));
        assert!(!allowlist.is_allowed(ip3));
        assert_eq!(allowlist.len(), 2);
    }

    #[test]
    fn allowlist_update_replaces_all() {
        let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let ip3 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

        let mut allowlist = IpAllowlist::with_ips(vec![ip1, ip2]);
        assert!(allowlist.is_allowed(ip1));

        let new_set: HashSet<IpAddr> = vec![ip3].into_iter().collect();
        allowlist.update(new_set);

        assert!(!allowlist.is_allowed(ip1));
        assert!(!allowlist.is_allowed(ip2));
        assert!(allowlist.is_allowed(ip3));
        assert_eq!(allowlist.len(), 1);
    }

    #[test]
    fn allowlist_supports_ipv6() {
        let ipv6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let mut allowlist = IpAllowlist::new();
        allowlist.allow(ipv6);
        assert!(allowlist.is_allowed(ipv6));
    }

    // -----------------------------------------------------------------------
    // RateLimiter
    // -----------------------------------------------------------------------

    #[test]
    fn rate_limiter_loopback_exempt_from_per_ip() {
        let config = RateLimitConfig::new().with_max_connections_per_ip(1);
        let mut limiter = RateLimiter::new(config);
        let lo4 = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let lo6 = IpAddr::V6(Ipv6Addr::LOCALHOST);

        // Loopback should never hit per-IP limits.
        for _ in 0..50 {
            assert!(limiter.check_connection(lo4).is_accepted());
            assert!(limiter.check_connection(lo6).is_accepted());
        }
    }

    #[test]
    fn rate_limiter_loopback_counts_toward_global() {
        let config = RateLimitConfig::new()
            .with_max_connections_total(3);
        let mut limiter = RateLimiter::new(config);
        let lo = IpAddr::V4(Ipv4Addr::LOCALHOST);

        assert!(limiter.check_connection(lo).is_accepted());
        assert!(limiter.check_connection(lo).is_accepted());
        assert!(limiter.check_connection(lo).is_accepted());

        // Global limit reached.
        let verdict = limiter.check_connection(lo);
        assert!(matches!(verdict, ConnectionVerdict::Reject { .. }));
    }

    #[test]
    fn rate_limiter_accepts_within_limits() {
        let config = RateLimitConfig::new().with_max_connections_per_ip(3);
        let mut limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        assert!(limiter.check_connection(ip).is_accepted());
        assert!(limiter.check_connection(ip).is_accepted());
        assert!(limiter.check_connection(ip).is_accepted());
    }

    #[test]
    fn rate_limiter_blocks_on_per_ip_limit() {
        let config = RateLimitConfig::new()
            .with_max_connections_per_ip(2)
            .with_cooldown_secs(30);
        let mut limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        assert!(limiter.check_connection(ip).is_accepted());
        assert!(limiter.check_connection(ip).is_accepted());

        // Third connection should be rate-limited.
        let verdict = limiter.check_connection(ip);
        assert!(
            matches!(verdict, ConnectionVerdict::RateLimit { retry_after_secs } if retry_after_secs > 0),
            "expected rate limit, got {:?}",
            verdict
        );
    }

    #[test]
    fn rate_limiter_per_ip_isolation() {
        let config = RateLimitConfig::new().with_max_connections_per_ip(1);
        let mut limiter = RateLimiter::new(config);

        let ip_a = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip_b = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));

        assert!(limiter.check_connection(ip_a).is_accepted());
        assert!(limiter.check_connection(ip_b).is_accepted());

        // Both should be blocked on a second connection.
        assert!(!limiter.check_connection(ip_a).is_accepted());
        assert!(!limiter.check_connection(ip_b).is_accepted());
    }

    #[test]
    fn rate_limiter_global_limit() {
        let config = RateLimitConfig::new()
            .with_max_connections_per_ip(100)
            .with_max_connections_total(2);
        let mut limiter = RateLimiter::new(config);

        let ip_a = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip_b = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let ip_c = IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3));

        assert!(limiter.check_connection(ip_a).is_accepted());
        assert!(limiter.check_connection(ip_b).is_accepted());

        // Global limit hit.
        let verdict = limiter.check_connection(ip_c);
        assert!(
            matches!(verdict, ConnectionVerdict::Reject { .. }),
            "expected reject, got {:?}",
            verdict
        );
    }

    #[test]
    fn rate_limiter_release_frees_slot() {
        let config = RateLimitConfig::new()
            .with_max_connections_per_ip(1)
            .with_max_connections_total(1);
        let mut limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        assert!(limiter.check_connection(ip).is_accepted());
        assert!(!limiter.check_connection(ip).is_accepted());

        limiter.release_connection(ip);
        let ip2 = IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8));
        assert!(
            limiter.check_connection(ip2).is_accepted(),
            "global slot should have freed up after release"
        );
    }

    #[test]
    fn rate_limiter_event_rate_limiting() {
        let config = RateLimitConfig::new()
            .with_max_events_per_minute_per_ip(3)
            .with_cooldown_secs(60);
        let mut limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        assert!(limiter.record_event(ip).is_accepted());
        assert!(limiter.record_event(ip).is_accepted());
        assert!(limiter.record_event(ip).is_accepted());

        // Fourth event exceeds the limit.
        let verdict = limiter.record_event(ip);
        assert!(
            matches!(verdict, ConnectionVerdict::RateLimit { .. }),
            "expected rate limit, got {:?}",
            verdict
        );
    }

    #[test]
    fn rate_limiter_blocked_count() {
        let config = RateLimitConfig::new()
            .with_max_connections_per_ip(1)
            .with_cooldown_secs(300);
        let mut limiter = RateLimiter::new(config);

        let ip_a = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip_b = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));

        assert_eq!(limiter.blocked_count(), 0);

        limiter.check_connection(ip_a);
        limiter.check_connection(ip_a); // triggers block
        assert_eq!(limiter.blocked_count(), 1);

        limiter.check_connection(ip_b);
        limiter.check_connection(ip_b); // triggers block
        assert_eq!(limiter.blocked_count(), 2);
    }

    #[test]
    fn rate_limiter_cleanup_removes_idle() {
        let config = RateLimitConfig::new()
            .with_max_connections_per_ip(100)
            .with_cooldown_secs(0); // instant expiry for testing
        let mut limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        limiter.check_connection(ip);
        limiter.release_connection(ip);

        limiter.cleanup();
        assert_eq!(limiter.trackers.len(), 0);
    }

    // -----------------------------------------------------------------------
    // ConnectionGuard
    // -----------------------------------------------------------------------

    fn tower_ips() -> HashSet<IpAddr> {
        vec![
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
        ]
        .into_iter()
        .collect()
    }

    fn make_guard(policy: ConnectionPolicy) -> ConnectionGuard {
        let allowlist = IpAllowlist::with_ips(tower_ips());
        let limiter = RateLimiter::new(
            RateLimitConfig::new()
                .with_max_connections_per_ip(5)
                .with_max_connections_total(100),
        );
        ConnectionGuard::new(policy, allowlist, limiter)
    }

    #[test]
    fn guard_allow_all_accepts_unknown_ip() {
        let mut guard = make_guard(ConnectionPolicy::AllowAll);
        let unknown = IpAddr::V4(Ipv4Addr::new(99, 99, 99, 99));
        assert!(guard.check(unknown).is_accepted());
    }

    #[test]
    fn guard_allowlist_only_rejects_unknown_ip() {
        let mut guard = make_guard(ConnectionPolicy::AllowlistOnly);
        let unknown = IpAddr::V4(Ipv4Addr::new(99, 99, 99, 99));
        let verdict = guard.check(unknown);
        assert!(
            matches!(verdict, ConnectionVerdict::Reject { .. }),
            "expected reject, got {:?}",
            verdict
        );
    }

    #[test]
    fn guard_allowlist_only_accepts_known_ip() {
        let mut guard = make_guard(ConnectionPolicy::AllowlistOnly);
        let known = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(guard.check(known).is_accepted());
    }

    #[test]
    fn guard_allowlist_with_client_auth_accepts_unknown_ip() {
        let mut guard = make_guard(ConnectionPolicy::AllowlistWithClientAuth);
        let unknown = IpAddr::V4(Ipv4Addr::new(99, 99, 99, 99));
        assert!(guard.check(unknown).is_accepted());
    }

    #[test]
    fn guard_rate_limits_even_known_ips() {
        let allowlist = IpAllowlist::with_ips(tower_ips());
        let limiter = RateLimiter::new(
            RateLimitConfig::new()
                .with_max_connections_per_ip(2)
                .with_cooldown_secs(60),
        );
        let mut guard =
            ConnectionGuard::new(ConnectionPolicy::AllowlistOnly, allowlist, limiter);

        let known = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(guard.check(known).is_accepted());
        assert!(guard.check(known).is_accepted());

        let verdict = guard.check(known);
        assert!(
            matches!(verdict, ConnectionVerdict::RateLimit { .. }),
            "expected rate limit for known IP, got {:?}",
            verdict
        );
    }

    #[test]
    fn guard_update_allowlist() {
        let mut guard = make_guard(ConnectionPolicy::AllowlistOnly);
        let new_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 99));

        assert!(!guard.check(new_ip).is_accepted());

        let mut new_set = tower_ips();
        new_set.insert(new_ip);
        guard.update_allowlist(new_set);

        assert!(guard.check(new_ip).is_accepted());
    }

    #[test]
    fn guard_set_policy() {
        let mut guard = make_guard(ConnectionPolicy::AllowlistOnly);
        let unknown = IpAddr::V4(Ipv4Addr::new(99, 99, 99, 99));

        assert!(!guard.check(unknown).is_accepted());

        guard.set_policy(ConnectionPolicy::AllowAll);
        assert!(guard.check(unknown).is_accepted());
    }

    #[test]
    fn guard_stats_tracking() {
        let mut guard = make_guard(ConnectionPolicy::AllowlistOnly);
        let known = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let unknown = IpAddr::V4(Ipv4Addr::new(99, 99, 99, 99));

        guard.check(known);
        guard.check(unknown);
        guard.check(known);

        let stats = guard.stats();
        assert_eq!(stats.total_allowed, 2);
        assert_eq!(stats.total_rejected, 1);
        assert_eq!(stats.allowlist_size, 2);
    }

    #[test]
    fn guard_release_connection() {
        let allowlist = IpAllowlist::with_ips(tower_ips());
        let limiter = RateLimiter::new(
            RateLimitConfig::new()
                .with_max_connections_per_ip(1)
                .with_max_connections_total(100),
        );
        let mut guard =
            ConnectionGuard::new(ConnectionPolicy::AllowlistOnly, allowlist, limiter);

        let known = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(guard.check(known).is_accepted());
        assert!(!guard.check(known).is_accepted());

        guard.release(known);
        let known2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        assert!(guard.check(known2).is_accepted());
    }

    // -----------------------------------------------------------------------
    // ConnectionStats
    // -----------------------------------------------------------------------

    #[test]
    fn connection_stats_serialization() {
        let stats = ConnectionStats {
            total_allowed: 42,
            total_rejected: 3,
            total_rate_limited: 1,
            currently_blocked_ips: 2,
            allowlist_size: 10,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ConnectionStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn release_nonexistent_ip_is_noop() {
        let mut limiter = RateLimiter::new(RateLimitConfig::new());
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        limiter.release_connection(ip);
    }

    #[test]
    fn cleanup_with_no_entries() {
        let mut limiter = RateLimiter::new(RateLimitConfig::new());
        limiter.cleanup();
    }

    #[test]
    fn allowlist_duplicate_add() {
        let mut allowlist = IpAllowlist::new();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        allowlist.allow(ip);
        allowlist.allow(ip);
        assert_eq!(allowlist.len(), 1);
    }

    #[test]
    fn allowlist_remove_nonexistent() {
        let mut allowlist = IpAllowlist::new();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        allowlist.remove(ip);
        assert!(allowlist.is_empty());
    }

    // -----------------------------------------------------------------------
    // FederationFilter
    // -----------------------------------------------------------------------

    #[test]
    fn federation_filter_empty() {
        let filter = FederationFilter::new();
        assert!(filter.is_empty());
        assert_eq!(filter.len(), 0);
        assert!(!filter.is_federated("any-community"));
    }

    #[test]
    fn federation_filter_from_communities() {
        let filter = FederationFilter::from_communities(vec!["alpha", "beta", "gamma"]);
        assert_eq!(filter.len(), 3);
        assert!(filter.is_federated("alpha"));
        assert!(filter.is_federated("beta"));
        assert!(filter.is_federated("gamma"));
        assert!(!filter.is_federated("delta"));
    }

    #[test]
    fn federation_filter_add_remove() {
        let mut filter = FederationFilter::new();
        filter.add_community("alpha");
        assert!(filter.is_federated("alpha"));

        filter.remove_community("alpha");
        assert!(!filter.is_federated("alpha"));
        assert!(filter.is_empty());
    }

    #[test]
    fn federation_filter_has_overlap() {
        let filter = FederationFilter::from_communities(vec!["alpha", "beta"]);

        assert!(filter.has_overlap(&["alpha".into(), "gamma".into()]));
        assert!(filter.has_overlap(&["beta".into()]));
        assert!(!filter.has_overlap(&["gamma".into(), "delta".into()]));
        assert!(!filter.has_overlap(&[]));
    }

    #[test]
    fn federation_filter_update() {
        let mut filter = FederationFilter::from_communities(vec!["alpha"]);
        assert!(filter.is_federated("alpha"));

        let mut new_set = HashSet::new();
        new_set.insert("beta".to_string());
        new_set.insert("gamma".to_string());
        filter.update(new_set);

        assert!(!filter.is_federated("alpha"));
        assert!(filter.is_federated("beta"));
        assert!(filter.is_federated("gamma"));
    }

    #[test]
    fn federation_filter_serde_round_trip() {
        let filter = FederationFilter::from_communities(vec!["alpha", "beta"]);
        let json = serde_json::to_string(&filter).unwrap();
        let restored: FederationFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.len(), 2);
        assert!(restored.is_federated("alpha"));
        assert!(restored.is_federated("beta"));
    }
}
