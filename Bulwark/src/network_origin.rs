use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Network origin and bootstrap phase tracking.
///
/// During bootstrap (< threshold members), the origin account has
/// relaxed requirements for vouching and sponsorship to seed the network.
/// Once the threshold is crossed, normal rules apply to everyone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkOrigin {
    pub origin_pubkey: String,
    pub bootstrap_threshold: u64,
    pub bootstrap_capabilities: BootstrapCapabilities,
    pub member_count: u64,
    pub initialized_at: DateTime<Utc>,
    pub bootstrap_ended_at: Option<DateTime<Utc>>,
}

impl NetworkOrigin {
    /// Create a new network origin with the given founder and bootstrap threshold.
    pub fn new(origin_pubkey: impl Into<String>, bootstrap_threshold: u64) -> Self {
        Self {
            origin_pubkey: origin_pubkey.into(),
            bootstrap_threshold,
            bootstrap_capabilities: BootstrapCapabilities::default(),
            member_count: 1, // origin counts
            initialized_at: Utc::now(),
            bootstrap_ended_at: None,
        }
    }

    /// Whether the network is still in bootstrap phase (below member threshold).
    pub fn is_bootstrap(&self) -> bool {
        self.bootstrap_ended_at.is_none() && self.member_count < self.bootstrap_threshold
    }

    /// Check if the given pubkey is the network origin (founder).
    pub fn is_origin(&self, pubkey: &str) -> bool {
        self.origin_pubkey == pubkey
    }

    /// Register a new member. Ends bootstrap once the threshold is reached.
    pub fn add_member(&mut self) {
        self.member_count += 1;
        if self.member_count >= self.bootstrap_threshold && self.bootstrap_ended_at.is_none() {
            self.bootstrap_ended_at = Some(Utc::now());
        }
    }

    /// Current network phase — Bootstrap or Established.
    pub fn phase(&self) -> BootstrapPhase {
        if self.is_bootstrap() {
            BootstrapPhase::Bootstrap
        } else {
            BootstrapPhase::Established
        }
    }
}

/// What the origin account can do during bootstrap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapCapabilities {
    /// Origin can vouch without network age requirement.
    pub immediate_vouching: bool,
    /// Origin has no sponsorship limit.
    pub unlimited_sponsorship: bool,
    /// Bond requirements still apply (no shortcuts on physical proof).
    pub relaxed_bond_requirements: bool,
}

impl Default for BootstrapCapabilities {
    fn default() -> Self {
        Self {
            immediate_vouching: true,
            unlimited_sponsorship: true,
            relaxed_bond_requirements: false, // bonds always real
        }
    }
}

/// Current phase of the network.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BootstrapPhase {
    /// Network is seeding (< threshold members).
    Bootstrap,
    /// Network is self-sustaining.
    Established,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_phase() {
        let mut origin = NetworkOrigin::new("sam", 100);
        assert!(origin.is_bootstrap());
        assert_eq!(origin.phase(), BootstrapPhase::Bootstrap);
        assert!(origin.is_origin("sam"));
        assert!(!origin.is_origin("alice"));

        // Add members until threshold
        for _ in 0..99 {
            origin.add_member();
        }
        assert!(!origin.is_bootstrap());
        assert_eq!(origin.phase(), BootstrapPhase::Established);
        assert!(origin.bootstrap_ended_at.is_some());
    }

    #[test]
    fn default_capabilities() {
        let caps = BootstrapCapabilities::default();
        assert!(caps.immediate_vouching);
        assert!(caps.unlimited_sponsorship);
        assert!(!caps.relaxed_bond_requirements); // bonds always real
    }
}
