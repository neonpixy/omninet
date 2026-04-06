//! Quest configuration -- all tunable parameters.
//!
//! `QuestConfig` uses a builder pattern with `with_*` methods and named presets
//! for common use cases.

use serde::{Deserialize, Serialize};

/// All tunable parameters for the Quest system.
///
/// # Defaults
///
/// The default configuration is designed for a casual-to-standard experience
/// with generous forgiveness and Covenant-mandated consent.
///
/// # Presets
///
/// - [`QuestConfig::casual()`] -- relaxed, high forgiveness, low scaling
/// - [`QuestConfig::standard()`] -- balanced (same as default)
/// - [`QuestConfig::ambitious()`] -- harder scaling, tighter windows
///
/// # Example
///
/// ```
/// use quest::QuestConfig;
///
/// let config = QuestConfig::default()
///     .with_max_active_missions(10)
///     .with_streak_forgiveness_days(3);
///
/// assert_eq!(config.max_active_missions, 10);
/// assert_eq!(config.streak_forgiveness_days, 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestConfig {
    /// Maximum number of missions a participant can have active at once.
    pub max_active_missions: usize,

    /// Base XP required for level 1.
    pub xp_per_level_base: u64,

    /// Scaling factor for XP per level. Each level requires `base * scaling^(level-1)` XP.
    pub xp_level_scaling: f64,

    /// Maximum achievable level.
    pub max_level: u32,

    /// Number of days to look back when comparing personal bests.
    pub personal_best_window_days: u32,

    /// How quickly difficulty adapts to performance (0.0 = never, 1.0 = instant).
    pub difficulty_adaptation_rate: f64,

    /// XP multiplier when helping others (mentorship activities).
    pub mentorship_multiplier: f64,

    /// Whether Covenant consent is required to participate. Default: `true`.
    /// The Covenant mandates opt-in participation.
    pub consent_required: bool,

    /// Minimum seconds between reward claims. Default: `0` (no cooldown).
    pub cooldown_between_claims_secs: u64,

    /// Number of grace days before a streak is affected by inactivity.
    /// Default: `2`. No dark patterns: missing a day or two is human, not failure.
    pub streak_forgiveness_days: u32,
}

impl Default for QuestConfig {
    fn default() -> Self {
        Self {
            max_active_missions: 5,
            xp_per_level_base: 100,
            xp_level_scaling: 1.5,
            max_level: 100,
            personal_best_window_days: 30,
            difficulty_adaptation_rate: 0.1,
            mentorship_multiplier: 1.5,
            consent_required: true,
            cooldown_between_claims_secs: 0,
            streak_forgiveness_days: 2,
        }
    }
}

impl QuestConfig {
    /// Casual preset -- relaxed progression, generous forgiveness.
    ///
    /// Good for people who check in occasionally and want to enjoy, not grind.
    pub fn casual() -> Self {
        Self {
            max_active_missions: 3,
            xp_per_level_base: 50,
            xp_level_scaling: 1.3,
            max_level: 50,
            personal_best_window_days: 60,
            difficulty_adaptation_rate: 0.05,
            mentorship_multiplier: 2.0,
            consent_required: true,
            cooldown_between_claims_secs: 0,
            streak_forgiveness_days: 5,
        }
    }

    /// Standard preset -- balanced progression (same as default).
    pub fn standard() -> Self {
        Self::default()
    }

    /// Ambitious preset -- faster scaling, tighter personal-best windows.
    ///
    /// For people who want a challenge. Still no dark patterns.
    pub fn ambitious() -> Self {
        Self {
            max_active_missions: 10,
            xp_per_level_base: 150,
            xp_level_scaling: 1.8,
            max_level: 200,
            personal_best_window_days: 14,
            difficulty_adaptation_rate: 0.2,
            mentorship_multiplier: 1.25,
            consent_required: true,
            cooldown_between_claims_secs: 0,
            streak_forgiveness_days: 1,
        }
    }

    /// Set the maximum number of concurrent active missions.
    pub fn with_max_active_missions(mut self, n: usize) -> Self {
        self.max_active_missions = n;
        self
    }

    /// Set the base XP required for level 1.
    pub fn with_xp_per_level_base(mut self, xp: u64) -> Self {
        self.xp_per_level_base = xp;
        self
    }

    /// Set the XP scaling factor per level.
    pub fn with_xp_level_scaling(mut self, factor: f64) -> Self {
        self.xp_level_scaling = factor;
        self
    }

    /// Set the maximum achievable level.
    pub fn with_max_level(mut self, level: u32) -> Self {
        self.max_level = level;
        self
    }

    /// Set the personal best comparison window in days.
    pub fn with_personal_best_window_days(mut self, days: u32) -> Self {
        self.personal_best_window_days = days;
        self
    }

    /// Set the difficulty adaptation rate.
    pub fn with_difficulty_adaptation_rate(mut self, rate: f64) -> Self {
        self.difficulty_adaptation_rate = rate;
        self
    }

    /// Set the XP multiplier for mentorship activities.
    pub fn with_mentorship_multiplier(mut self, multiplier: f64) -> Self {
        self.mentorship_multiplier = multiplier;
        self
    }

    /// Set whether Covenant consent is required.
    ///
    /// The Covenant mandates `true`. Setting to `false` is only valid
    /// for testing or explicitly non-Covenant contexts.
    pub fn with_consent_required(mut self, required: bool) -> Self {
        self.consent_required = required;
        self
    }

    /// Set the cooldown between reward claims in seconds.
    pub fn with_cooldown_between_claims_secs(mut self, secs: u64) -> Self {
        self.cooldown_between_claims_secs = secs;
        self
    }

    /// Set the number of grace days before streaks are affected.
    ///
    /// The Covenant forbids punishing rest. A minimum of 1 is recommended.
    pub fn with_streak_forgiveness_days(mut self, days: u32) -> Self {
        self.streak_forgiveness_days = days;
        self
    }
}

impl PartialEq for QuestConfig {
    fn eq(&self, other: &Self) -> bool {
        self.max_active_missions == other.max_active_missions
            && self.xp_per_level_base == other.xp_per_level_base
            && self.xp_level_scaling == other.xp_level_scaling
            && self.max_level == other.max_level
            && self.personal_best_window_days == other.personal_best_window_days
            && self.difficulty_adaptation_rate == other.difficulty_adaptation_rate
            && self.mentorship_multiplier == other.mentorship_multiplier
            && self.consent_required == other.consent_required
            && self.cooldown_between_claims_secs == other.cooldown_between_claims_secs
            && self.streak_forgiveness_days == other.streak_forgiveness_days
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let config = QuestConfig::default();
        assert_eq!(config.max_active_missions, 5);
        assert_eq!(config.xp_per_level_base, 100);
        assert_eq!(config.xp_level_scaling, 1.5);
        assert_eq!(config.max_level, 100);
        assert_eq!(config.personal_best_window_days, 30);
        assert_eq!(config.difficulty_adaptation_rate, 0.1);
        assert_eq!(config.mentorship_multiplier, 1.5);
        assert!(config.consent_required);
        assert_eq!(config.cooldown_between_claims_secs, 0);
        assert_eq!(config.streak_forgiveness_days, 2);
    }

    #[test]
    fn casual_preset() {
        let config = QuestConfig::casual();
        assert_eq!(config.max_active_missions, 3);
        assert_eq!(config.xp_per_level_base, 50);
        assert_eq!(config.xp_level_scaling, 1.3);
        assert_eq!(config.max_level, 50);
        assert_eq!(config.streak_forgiveness_days, 5);
        assert!(config.consent_required);
    }

    #[test]
    fn standard_is_default() {
        let standard = QuestConfig::standard();
        let default = QuestConfig::default();
        assert_eq!(standard, default);
    }

    #[test]
    fn ambitious_preset() {
        let config = QuestConfig::ambitious();
        assert_eq!(config.max_active_missions, 10);
        assert_eq!(config.xp_per_level_base, 150);
        assert_eq!(config.xp_level_scaling, 1.8);
        assert_eq!(config.max_level, 200);
        assert_eq!(config.personal_best_window_days, 14);
        assert_eq!(config.streak_forgiveness_days, 1);
        assert!(config.consent_required);
    }

    #[test]
    fn builder_pattern() {
        let config = QuestConfig::default()
            .with_max_active_missions(10)
            .with_xp_per_level_base(200)
            .with_xp_level_scaling(2.0)
            .with_max_level(50)
            .with_personal_best_window_days(7)
            .with_difficulty_adaptation_rate(0.5)
            .with_mentorship_multiplier(3.0)
            .with_consent_required(false)
            .with_cooldown_between_claims_secs(60)
            .with_streak_forgiveness_days(7);

        assert_eq!(config.max_active_missions, 10);
        assert_eq!(config.xp_per_level_base, 200);
        assert_eq!(config.xp_level_scaling, 2.0);
        assert_eq!(config.max_level, 50);
        assert_eq!(config.personal_best_window_days, 7);
        assert_eq!(config.difficulty_adaptation_rate, 0.5);
        assert_eq!(config.mentorship_multiplier, 3.0);
        assert!(!config.consent_required);
        assert_eq!(config.cooldown_between_claims_secs, 60);
        assert_eq!(config.streak_forgiveness_days, 7);
    }

    #[test]
    fn consent_always_true_in_presets() {
        assert!(QuestConfig::casual().consent_required);
        assert!(QuestConfig::standard().consent_required);
        assert!(QuestConfig::ambitious().consent_required);
    }

    #[test]
    fn serde_round_trip() {
        let config = QuestConfig::default().with_max_level(42);
        let json = serde_json::to_string(&config).unwrap();
        let restored: QuestConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn serde_round_trip_casual() {
        let config = QuestConfig::casual();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let restored: QuestConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn serde_round_trip_ambitious() {
        let config = QuestConfig::ambitious();
        let json = serde_json::to_string(&config).unwrap();
        let restored: QuestConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn streak_forgiveness_default_is_humane() {
        // The Covenant forbids punishing rest. Default must be >= 1.
        let config = QuestConfig::default();
        assert!(
            config.streak_forgiveness_days >= 1,
            "streak forgiveness must be at least 1 day"
        );
    }

    #[test]
    fn debug_format() {
        let config = QuestConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("QuestConfig"));
        assert!(debug.contains("max_active_missions"));
    }

    #[test]
    fn partial_builder_chain() {
        // Ensure builder works with partial overrides
        let config = QuestConfig::casual().with_max_level(999);
        assert_eq!(config.max_level, 999);
        // Other casual values preserved
        assert_eq!(config.xp_per_level_base, 50);
        assert_eq!(config.streak_forgiveness_days, 5);
    }
}
