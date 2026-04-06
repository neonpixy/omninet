use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Central configuration for the Advisor cognitive system.
///
/// All tunables in one place. Constants from the quarry (LordOnyx + Solas v3)
/// are preserved as defaults. Use presets for common configurations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorConfig {
    // ── Expression Pressure ──────────────────────────────────────
    /// Base pressure increment per tick (default: 0.05)
    pub pressure_base_rate: f64,
    /// Bonus when novel content is encountered (default: 0.3)
    pub pressure_novel_content_bonus: f64,
    /// Bonus when high-salience memory is recalled (default: 0.2)
    pub pressure_high_salience_memory_bonus: f64,
    /// Bonus when user is idle (default: 0.1)
    pub pressure_user_idle_bonus: f64,
    /// Bonus when a connection between thoughts is discovered (default: 0.4)
    pub pressure_connection_discovered_bonus: f64,
    /// Bonus for urgent external events (default: 0.5)
    pub pressure_urgent_external_bonus: f64,
    /// Threshold to trigger expression (default: 0.8)
    pub pressure_expression_threshold: f64,
    /// Threshold for urgent interruption (default: 0.95)
    pub pressure_urgent_threshold: f64,

    // ── Synapse ──────────────────────────────────────────────────
    /// Strength added when a synapse is referenced (default: 0.2)
    pub synapse_strength_increment: f64,
    /// Strength lost per day of inactivity (default: 0.05)
    pub synapse_daily_decay: f64,
    /// Minimum strength before pruning (default: 0.1)
    pub synapse_minimum_strength: f64,
    /// Maximum synapse strength (default: 1.0)
    pub synapse_maximum_strength: f64,
    /// Initial strength for new synapses (default: 0.5)
    pub synapse_initial_strength: f64,

    // ── Clipboard ────────────────────────────────────────────────
    /// Maximum entries in working memory (default: 100)
    pub clipboard_max_entries: usize,
    /// Daily priority decay for clipboard entries (default: 0.02)
    pub clipboard_daily_decay: f64,
    /// Minimum priority before eviction (default: 0.1)
    pub clipboard_minimum_priority: f64,

    // ── Cognitive Loop ───────────────────────────────────────────
    /// Target tick interval for the main consciousness loop (default: 2s)
    pub loop_tick_interval: Duration,
    /// Minimum inner voice interval (default: 2s)
    pub inner_voice_min_interval: Duration,
    /// Maximum inner voice interval (default: 8s)
    pub inner_voice_max_interval: Duration,
    /// Maximum thoughts in inner voice buffer (default: 100)
    pub inner_voice_buffer_size: usize,
    /// Pressure release fraction after expression (default: 0.8, i.e., release 80%)
    pub pressure_release_fraction: f64,
    /// Time after which pressure acceleration kicks in (default: 300s / 5 min)
    pub pressure_acceleration_after: Duration,

    // ── Store ────────────────────────────────────────────────────
    /// Minimum cosine similarity for semantic search results (default: 0.5)
    pub semantic_search_min_score: f64,
    /// Maximum search results to return (default: 20)
    pub search_max_results: usize,
}

impl Default for AdvisorConfig {
    fn default() -> Self {
        Self {
            // Expression Pressure (from LordOnyx ExpressionPressure.swift)
            pressure_base_rate: 0.05,
            pressure_novel_content_bonus: 0.3,
            pressure_high_salience_memory_bonus: 0.2,
            pressure_user_idle_bonus: 0.1,
            pressure_connection_discovered_bonus: 0.4,
            pressure_urgent_external_bonus: 0.5,
            pressure_expression_threshold: 0.8,
            pressure_urgent_threshold: 0.95,

            // Synapse (from LordOnyx Synapse.swift)
            synapse_strength_increment: 0.2,
            synapse_daily_decay: 0.05,
            synapse_minimum_strength: 0.1,
            synapse_maximum_strength: 1.0,
            synapse_initial_strength: 0.5,

            // Clipboard (from LordOnyx GlobalClipboard.swift)
            clipboard_max_entries: 100,
            clipboard_daily_decay: 0.02,
            clipboard_minimum_priority: 0.1,

            // Cognitive Loop (from Solas v3)
            loop_tick_interval: Duration::from_secs(2),
            inner_voice_min_interval: Duration::from_secs(2),
            inner_voice_max_interval: Duration::from_secs(8),
            inner_voice_buffer_size: 100,
            pressure_release_fraction: 0.8,
            pressure_acceleration_after: Duration::from_secs(300),

            // Store
            semantic_search_min_score: 0.5,
            search_max_results: 20,
        }
    }
}

impl AdvisorConfig {
    /// Contemplative preset: slower tick rate, higher expression threshold.
    /// For quiet reflection modes where the advisor speaks less.
    pub fn contemplative() -> Self {
        Self {
            pressure_base_rate: 0.02,
            pressure_expression_threshold: 0.9,
            pressure_urgent_threshold: 0.98,
            loop_tick_interval: Duration::from_secs(5),
            inner_voice_min_interval: Duration::from_secs(4),
            inner_voice_max_interval: Duration::from_secs(15),
            ..Default::default()
        }
    }

    /// Responsive preset: faster tick rate, lower expression threshold.
    /// For active conversation where the advisor is more engaged.
    pub fn responsive() -> Self {
        Self {
            pressure_base_rate: 0.08,
            pressure_expression_threshold: 0.6,
            pressure_urgent_threshold: 0.85,
            loop_tick_interval: Duration::from_secs(1),
            inner_voice_min_interval: Duration::from_secs(1),
            inner_voice_max_interval: Duration::from_secs(4),
            ..Default::default()
        }
    }

    /// Validate all configuration values are within acceptable ranges.
    pub fn validate(&self) -> Result<(), crate::error::AdvisorError> {
        use crate::error::AdvisorError;

        if !(0.0..=1.0).contains(&self.pressure_expression_threshold) {
            return Err(AdvisorError::InvalidPressureThreshold {
                value: self.pressure_expression_threshold,
            });
        }
        if !(0.0..=1.0).contains(&self.pressure_urgent_threshold) {
            return Err(AdvisorError::InvalidPressureThreshold {
                value: self.pressure_urgent_threshold,
            });
        }
        if self.pressure_expression_threshold >= self.pressure_urgent_threshold {
            return Err(AdvisorError::InvalidConfiguration(
                "expression threshold must be less than urgent threshold".into(),
            ));
        }
        if self.synapse_minimum_strength >= self.synapse_maximum_strength {
            return Err(AdvisorError::InvalidConfiguration(
                "synapse minimum strength must be less than maximum".into(),
            ));
        }
        if self.clipboard_max_entries == 0 {
            return Err(AdvisorError::InvalidConfiguration(
                "clipboard must allow at least one entry".into(),
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
        let config = AdvisorConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn contemplative_preset_is_valid() {
        let config = AdvisorConfig::contemplative();
        assert!(config.validate().is_ok());
        assert!(config.pressure_expression_threshold > AdvisorConfig::default().pressure_expression_threshold);
    }

    #[test]
    fn responsive_preset_is_valid() {
        let config = AdvisorConfig::responsive();
        assert!(config.validate().is_ok());
        assert!(config.pressure_expression_threshold < AdvisorConfig::default().pressure_expression_threshold);
    }

    #[test]
    fn invalid_threshold_rejected() {
        let config = AdvisorConfig {
            pressure_expression_threshold: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn threshold_ordering_enforced() {
        let config = AdvisorConfig {
            pressure_expression_threshold: 0.95,
            pressure_urgent_threshold: 0.8,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn synapse_range_enforced() {
        let config = AdvisorConfig {
            synapse_minimum_strength: 1.0,
            synapse_maximum_strength: 0.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn zero_clipboard_rejected() {
        let config = AdvisorConfig {
            clipboard_max_entries: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = AdvisorConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AdvisorConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
