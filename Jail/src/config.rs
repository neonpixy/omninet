use serde::{Deserialize, Serialize};

/// All tunable parameters for Jail's accountability system.
///
/// Communities configure these within Covenant bounds. Three presets provided:
/// `default()`, `testing()` (permissive for dev), `strict()` (shorter windows).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JailConfig {
    /// Maximum BFS traversal depth for trust graph queries.
    pub max_query_degrees: usize,

    /// How many degrees flags propagate through the trust network.
    pub flag_propagation_degrees: usize,

    /// Maximum flags a single pubkey can file per day (rate limiting).
    pub max_flags_per_pubkey_per_day: usize,

    /// Minimum distinct communities flagging before a pattern is established.
    pub pattern_threshold_communities: usize,

    /// Minimum attestations required to complete re-verification.
    pub reverification_attestations_required: usize,

    /// Hours before a re-verification session expires.
    pub reverification_expiry_hours: u64,

    /// Days between mandatory reviews of protective exclusions.
    pub exclusion_review_days: u64,

    /// Window (in days) for detecting weaponization patterns.
    pub weaponization_window_days: u64,

    /// Number of flags within the weaponization window before abuse detection triggers.
    pub weaponization_threshold: usize,

    /// Minimum verifications from community members for admission.
    pub admission_min_verifications: usize,
}

impl Default for JailConfig {
    fn default() -> Self {
        Self {
            max_query_degrees: 3,
            flag_propagation_degrees: 3,
            max_flags_per_pubkey_per_day: 5,
            pattern_threshold_communities: 2,
            reverification_attestations_required: 2,
            reverification_expiry_hours: 168, // 7 days
            exclusion_review_days: 90,
            weaponization_window_days: 30,
            weaponization_threshold: 5,
            admission_min_verifications: 1,
        }
    }
}

impl JailConfig {
    /// Permissive configuration for testing and development.
    pub fn testing() -> Self {
        Self {
            max_query_degrees: 5,
            flag_propagation_degrees: 5,
            max_flags_per_pubkey_per_day: 100,
            pattern_threshold_communities: 2,
            reverification_attestations_required: 1,
            reverification_expiry_hours: 24,
            exclusion_review_days: 7,
            weaponization_window_days: 7,
            weaponization_threshold: 20,
            admission_min_verifications: 1,
        }
    }

    /// Strict configuration — shorter windows, tighter limits.
    pub fn strict() -> Self {
        Self {
            max_query_degrees: 2,
            flag_propagation_degrees: 2,
            max_flags_per_pubkey_per_day: 3,
            pattern_threshold_communities: 2,
            reverification_attestations_required: 3,
            reverification_expiry_hours: 72, // 3 days
            exclusion_review_days: 30,
            weaponization_window_days: 14,
            weaponization_threshold: 3,
            admission_min_verifications: 2,
        }
    }

    /// Validate configuration parameters.
    pub fn validate(&self) -> Result<(), crate::error::JailError> {
        if self.max_query_degrees == 0 {
            return Err(crate::error::JailError::ConfigError(
                "max_query_degrees must be > 0".into(),
            ));
        }
        if self.pattern_threshold_communities < 2 {
            return Err(crate::error::JailError::ConfigError(
                "pattern_threshold_communities must be >= 2".into(),
            ));
        }
        if self.reverification_attestations_required == 0 {
            return Err(crate::error::JailError::ConfigError(
                "reverification_attestations_required must be > 0".into(),
            ));
        }
        if self.exclusion_review_days == 0 {
            return Err(crate::error::JailError::ConfigError(
                "exclusion_review_days must be > 0 (no permanent castes)".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = JailConfig::default();
        assert_eq!(config.max_query_degrees, 3);
        assert_eq!(config.flag_propagation_degrees, 3);
        assert_eq!(config.max_flags_per_pubkey_per_day, 5);
        assert_eq!(config.pattern_threshold_communities, 2);
        assert_eq!(config.reverification_attestations_required, 2);
        assert_eq!(config.reverification_expiry_hours, 168);
        assert_eq!(config.exclusion_review_days, 90);
        assert_eq!(config.weaponization_window_days, 30);
        assert_eq!(config.weaponization_threshold, 5);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn testing_config() {
        let config = JailConfig::testing();
        assert_eq!(config.max_query_degrees, 5);
        assert_eq!(config.max_flags_per_pubkey_per_day, 100);
        assert_eq!(config.reverification_attestations_required, 1);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn strict_config() {
        let config = JailConfig::strict();
        assert_eq!(config.max_query_degrees, 2);
        assert_eq!(config.max_flags_per_pubkey_per_day, 3);
        assert_eq!(config.reverification_attestations_required, 3);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validation_rejects_zero_query_depth() {
        let config = JailConfig {
            max_query_degrees: 0,
            ..JailConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_single_community_threshold() {
        let config = JailConfig {
            pattern_threshold_communities: 1,
            ..JailConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_zero_attestations() {
        let config = JailConfig {
            reverification_attestations_required: 0,
            ..JailConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_zero_exclusion_review_days() {
        let config = JailConfig {
            exclusion_review_days: 0,
            ..JailConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("permanent castes"));
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = JailConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: JailConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
