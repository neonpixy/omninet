use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::GlobeError;

use super::tier::GospelTier;

/// Policy for anti-squatting protections on the naming system.
///
/// Controls payment requirements, timestamp validation, and name
/// expiration for Tower-level enforcement. Defaults are permissive
/// so existing tests and lightweight nodes work without configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamePolicy {
    /// Require payment proof tag on name claims.
    pub require_payment: bool,
    /// Minimum Cool amount for name claims.
    pub min_payment_amount: u64,
    /// Max allowed clock drift for name events (seconds).
    pub timestamp_window_secs: i64,
    /// Name time-to-live (seconds). Default: 1 year (31,536,000).
    pub name_ttl_secs: i64,
}

impl Default for NamePolicy {
    fn default() -> Self {
        Self {
            require_payment: false,
            min_payment_amount: 100,
            timestamp_window_secs: 600,
            name_ttl_secs: 31_536_000,
        }
    }
}

/// Configuration for the gospel evangelization system.
///
/// Gospel propagates registry records (names, relay hints) across
/// the relay network. These settings control sync frequency, capacity,
/// and peer connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GospelConfig {
    /// How often to evangelize with peers, in milliseconds (default: 60000).
    pub evangelize_interval_ms: u64,

    /// Maximum number of peer relay connections (default: 8).
    pub max_peers: usize,

    /// Seed peer relay URLs to connect to for gospel sync.
    pub peer_urls: Vec<Url>,

    /// Maximum name records in the registry (default: 100,000).
    pub max_name_records: usize,

    /// Maximum relay hint records in the registry (default: 100,000).
    pub max_hint_records: usize,

    /// Whether to verify event signatures on insert (default: true).
    pub verify_signatures: bool,

    /// Which gospel tiers to eagerly propagate (default: all).
    ///
    /// Universal: names, relay hints, lighthouse (everyone needs it).
    /// Community: beacons, asset announcements (community peers).
    /// Extended: pull-on-demand only (never eagerly pushed).
    #[serde(default = "GospelTier::all")]
    pub propagation_tiers: Vec<GospelTier>,

    /// Anti-squatting policy for the naming system.
    #[serde(default)]
    pub name_policy: NamePolicy,
}

impl Default for GospelConfig {
    fn default() -> Self {
        Self {
            evangelize_interval_ms: 60_000,
            max_peers: 8,
            peer_urls: Vec::new(),
            max_name_records: 100_000,
            max_hint_records: 100_000,
            verify_signatures: true,
            propagation_tiers: GospelTier::all(),
            name_policy: NamePolicy::default(),
        }
    }
}

impl GospelConfig {
    /// The evangelize interval as a `Duration`.
    pub fn evangelize_interval(&self) -> Duration {
        Duration::from_millis(self.evangelize_interval_ms)
    }

    /// Validate configuration.
    pub fn validate(&self) -> Result<(), GlobeError> {
        if self.evangelize_interval_ms == 0 {
            return Err(GlobeError::InvalidConfig(
                "gospel evangelize_interval_ms must be > 0".into(),
            ));
        }
        if self.max_peers == 0 {
            return Err(GlobeError::InvalidConfig(
                "gospel max_peers must be > 0".into(),
            ));
        }
        if self.max_name_records == 0 {
            return Err(GlobeError::InvalidConfig(
                "gospel max_name_records must be > 0".into(),
            ));
        }
        if self.max_hint_records == 0 {
            return Err(GlobeError::InvalidConfig(
                "gospel max_hint_records must be > 0".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = GospelConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.evangelize_interval_ms, 60_000);
        assert_eq!(config.max_peers, 8);
        assert!(config.verify_signatures);
    }

    #[test]
    fn evangelize_interval_duration() {
        let config = GospelConfig::default();
        assert_eq!(config.evangelize_interval(), Duration::from_secs(60));
    }

    #[test]
    fn zero_interval_fails() {
        let config = GospelConfig {
            evangelize_interval_ms: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_max_peers_fails() {
        let config = GospelConfig {
            max_peers: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn serde_round_trip() {
        let config = GospelConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let loaded: GospelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.evangelize_interval_ms, config.evangelize_interval_ms);
        assert_eq!(loaded.max_peers, config.max_peers);
    }

    #[test]
    fn name_policy_defaults_are_permissive() {
        let policy = NamePolicy::default();
        assert!(!policy.require_payment);
        assert_eq!(policy.min_payment_amount, 100);
        assert_eq!(policy.timestamp_window_secs, 600);
        assert_eq!(policy.name_ttl_secs, 31_536_000);
    }

    #[test]
    fn name_policy_in_gospel_config() {
        let config = GospelConfig::default();
        assert!(!config.name_policy.require_payment);
        assert_eq!(config.name_policy.name_ttl_secs, 31_536_000);
    }

    #[test]
    fn name_policy_serde_round_trip() {
        let policy = NamePolicy {
            require_payment: true,
            min_payment_amount: 500,
            timestamp_window_secs: 300,
            name_ttl_secs: 86_400,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let loaded: NamePolicy = serde_json::from_str(&json).unwrap();
        assert!(loaded.require_payment);
        assert_eq!(loaded.min_payment_amount, 500);
        assert_eq!(loaded.timestamp_window_secs, 300);
        assert_eq!(loaded.name_ttl_secs, 86_400);
    }

    #[test]
    fn legacy_config_without_name_policy_deserializes() {
        // Simulates a JSON from before name_policy was added.
        let json = r#"{
            "evangelize_interval_ms": 60000,
            "max_peers": 8,
            "peer_urls": [],
            "max_name_records": 100000,
            "max_hint_records": 100000,
            "verify_signatures": true
        }"#;
        let loaded: GospelConfig = serde_json::from_str(json).unwrap();
        // name_policy should use defaults via serde(default).
        assert!(!loaded.name_policy.require_payment);
        assert_eq!(loaded.name_policy.name_ttl_secs, 31_536_000);
    }
}
