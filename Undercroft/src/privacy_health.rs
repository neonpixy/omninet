//! Relay privacy health metrics.
//!
//! Privacy health aggregation for the Omny/Home dashboard. Measures how
//! well the relay network protects user privacy through intermediaries,
//! route diversity, traffic padding/shaping, and pubkey blinding.
//!
//! All data is deidentified: aggregate counts and booleans only.
//! NO relay URLs, NO pubkeys, NO individual routing decisions.

use serde::{Deserialize, Serialize};

/// Aggregated privacy health of the relay network.
///
/// Measures four dimensions of relay privacy:
/// - **Intermediary availability**: how many relays serve as intermediaries
/// - **Route diversity**: what fraction of traffic uses privacy routes
/// - **Traffic protection**: whether padding and shaping are active
/// - **Pubkey blinding**: whether blinding is in use
///
/// All data is deidentified. No relay URLs, no pubkeys, no individual
/// routing decisions are stored or serialized.
///
/// # Examples
///
/// ```
/// use undercroft::RelayPrivacyHealth;
///
/// let health = RelayPrivacyHealth::default();
/// assert_eq!(health.health_score(), 0.0);
///
/// let healthy = RelayPrivacyHealth {
///     intermediary_count: 5,
///     privacy_route_fraction: 0.8,
///     padding_enabled: true,
///     shaping_enabled: true,
///     blinding_active: true,
/// };
/// assert!(healthy.health_score() > 0.9);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelayPrivacyHealth {
    /// How many relays are in Intermediary mode.
    pub intermediary_count: usize,
    /// What fraction of traffic uses intermediaries (0.0-1.0).
    pub privacy_route_fraction: f64,
    /// Whether traffic padding is active.
    pub padding_enabled: bool,
    /// Whether traffic shaping is active.
    pub shaping_enabled: bool,
    /// Whether pubkey blinding is in use.
    pub blinding_active: bool,
}

impl RelayPrivacyHealth {
    /// Composite privacy health score (0.0-1.0).
    ///
    /// Weighted average of four dimensions:
    /// - Intermediary availability (30%): 3+ intermediaries = full score
    /// - Route diversity (30%): fraction of traffic using privacy routes
    /// - Traffic protection (20%): padding + shaping both active = full score
    /// - Pubkey blinding (20%): blinding active = full score
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::RelayPrivacyHealth;
    ///
    /// let health = RelayPrivacyHealth::default();
    /// assert_eq!(health.health_score(), 0.0);
    /// ```
    #[must_use]
    pub fn health_score(&self) -> f64 {
        // 3+ intermediaries = full score
        let intermediary_score = (self.intermediary_count as f64 / 3.0).min(1.0);

        // Already 0.0-1.0
        let route_score = self.privacy_route_fraction.clamp(0.0, 1.0);

        // Both padding and shaping = full score
        let protection_score =
            (self.padding_enabled as u8 as f64 + self.shaping_enabled as u8 as f64) / 2.0;

        // Blinding active = full score
        let blinding_score = self.blinding_active as u8 as f64;

        let score = 0.3 * intermediary_score
            + 0.3 * route_score
            + 0.2 * protection_score
            + 0.2 * blinding_score;

        score.clamp(0.0, 1.0)
    }
}

impl Default for RelayPrivacyHealth {
    fn default() -> Self {
        Self {
            intermediary_count: 0,
            privacy_route_fraction: 0.0,
            padding_enabled: false,
            shaping_enabled: false,
            blinding_active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_score_is_zero() {
        let health = RelayPrivacyHealth::default();
        assert_eq!(health.health_score(), 0.0);
    }

    #[test]
    fn all_maximums_approaches_one() {
        let health = RelayPrivacyHealth {
            intermediary_count: 10,
            privacy_route_fraction: 1.0,
            padding_enabled: true,
            shaping_enabled: true,
            blinding_active: true,
        };
        assert!((health.health_score() - 1.0).abs() < 0.001);
    }

    #[test]
    fn intermediary_weight_is_thirty_percent() {
        // Only intermediary_count set, everything else zero/false.
        let health = RelayPrivacyHealth {
            intermediary_count: 3,
            ..Default::default()
        };
        // intermediary_score = 1.0, weight = 0.3
        assert!((health.health_score() - 0.3).abs() < 0.001);
    }

    #[test]
    fn route_weight_is_thirty_percent() {
        let health = RelayPrivacyHealth {
            privacy_route_fraction: 1.0,
            ..Default::default()
        };
        // route_score = 1.0, weight = 0.3
        assert!((health.health_score() - 0.3).abs() < 0.001);
    }

    #[test]
    fn protection_weight_is_twenty_percent() {
        let health = RelayPrivacyHealth {
            padding_enabled: true,
            shaping_enabled: true,
            ..Default::default()
        };
        // protection_score = 1.0, weight = 0.2
        assert!((health.health_score() - 0.2).abs() < 0.001);
    }

    #[test]
    fn blinding_weight_is_twenty_percent() {
        let health = RelayPrivacyHealth {
            blinding_active: true,
            ..Default::default()
        };
        // blinding_score = 1.0, weight = 0.2
        assert!((health.health_score() - 0.2).abs() < 0.001);
    }

    #[test]
    fn three_plus_intermediaries_full_score() {
        let h3 = RelayPrivacyHealth {
            intermediary_count: 3,
            ..Default::default()
        };
        let h5 = RelayPrivacyHealth {
            intermediary_count: 5,
            ..Default::default()
        };
        // Both should give the same intermediary contribution (clamped at 1.0)
        assert!((h3.health_score() - h5.health_score()).abs() < 0.001);
    }

    #[test]
    fn one_intermediary_partial_score() {
        let health = RelayPrivacyHealth {
            intermediary_count: 1,
            ..Default::default()
        };
        // intermediary_score = 1/3 ≈ 0.333, * 0.3 weight ≈ 0.1
        let expected = 0.3 * (1.0 / 3.0);
        assert!((health.health_score() - expected).abs() < 0.001);
    }

    #[test]
    fn padding_only_half_protection() {
        let health = RelayPrivacyHealth {
            padding_enabled: true,
            shaping_enabled: false,
            ..Default::default()
        };
        // protection_score = 0.5, * 0.2 weight = 0.1
        assert!((health.health_score() - 0.1).abs() < 0.001);
    }

    #[test]
    fn serde_round_trip() {
        let health = RelayPrivacyHealth {
            intermediary_count: 4,
            privacy_route_fraction: 0.75,
            padding_enabled: true,
            shaping_enabled: false,
            blinding_active: true,
        };
        let json = serde_json::to_string(&health).unwrap();
        let restored: RelayPrivacyHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, health);
    }

    #[test]
    fn serde_default_round_trip() {
        let health = RelayPrivacyHealth::default();
        let json = serde_json::to_string(&health).unwrap();
        let restored: RelayPrivacyHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, health);
        assert_eq!(restored.health_score(), 0.0);
    }

    #[test]
    fn no_relay_urls_or_pubkeys_in_output() {
        let health = RelayPrivacyHealth {
            intermediary_count: 5,
            privacy_route_fraction: 0.9,
            padding_enabled: true,
            shaping_enabled: true,
            blinding_active: true,
        };
        let json = serde_json::to_string(&health).unwrap();

        // Verify no relay URLs or pubkeys leaked
        assert!(!json.contains("wss://"));
        assert!(!json.contains("relay"));
        assert!(!json.contains("cpub"));
        assert!(!json.contains("pubkey"));
    }

    #[test]
    fn route_fraction_clamped() {
        // Even if privacy_route_fraction is out of range, score clamps properly.
        let health = RelayPrivacyHealth {
            privacy_route_fraction: 1.5,
            ..Default::default()
        };
        // route_score clamped to 1.0, * 0.3 = 0.3
        assert!((health.health_score() - 0.3).abs() < 0.001);

        let negative = RelayPrivacyHealth {
            privacy_route_fraction: -0.5,
            ..Default::default()
        };
        assert_eq!(negative.health_score(), 0.0);
    }
}
