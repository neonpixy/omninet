use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::snapshot::PressureSnapshot;

/// The urge to speak. Accumulates over time and through events,
/// triggers expression when thresholds are crossed.
///
/// From LordOnyx (thresholds, events) + Solas v3 (time acceleration, release fractions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpressionPressure {
    /// Current pressure value (0.0..=1.0)
    value: f64,
    /// When pressure was last updated
    pub last_updated_at: DateTime<Utc>,
    /// When pressure was last released (expressed)
    pub last_released_at: DateTime<Utc>,
}

impl Default for ExpressionPressure {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpressionPressure {
    /// Create a new expression pressure accumulator starting at zero.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            value: 0.0,
            last_updated_at: now,
            last_released_at: now,
        }
    }

    /// Current pressure value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Increment by the base rate.
    pub fn increment(&mut self, base_rate: f64) {
        self.value = (self.value + base_rate).min(1.0);
        self.last_updated_at = Utc::now();
    }

    /// Apply a pressure event with the configured bonuses.
    pub fn apply(&mut self, event: &PressureEvent, config: &PressureConfig) {
        let bonus = match event {
            PressureEvent::NovelContent => config.novel_content_bonus,
            PressureEvent::HighSalienceMemory => config.high_salience_memory_bonus,
            PressureEvent::UserIdle => config.user_idle_bonus,
            PressureEvent::ConnectionDiscovered => config.connection_discovered_bonus,
            PressureEvent::UrgentExternal => config.urgent_external_bonus,
            PressureEvent::Custom(amount) => *amount,
        };
        self.value = (self.value + bonus).min(1.0);
        self.last_updated_at = Utc::now();
    }

    /// Full release — resets to 0.
    pub fn release(&mut self) {
        self.value = 0.0;
        self.last_released_at = Utc::now();
        self.last_updated_at = self.last_released_at;
    }

    /// Partial release — release a fraction (e.g., 0.8 releases 80%).
    pub fn partial_release(&mut self, fraction: f64) {
        let release_amount = self.value * fraction.clamp(0.0, 1.0);
        self.value = (self.value - release_amount).max(0.0);
        self.last_released_at = Utc::now();
        self.last_updated_at = self.last_released_at;
    }

    /// Whether the pressure is above the expression threshold.
    pub fn should_express(&self, threshold: f64) -> bool {
        self.value >= threshold
    }

    /// Whether the pressure is at urgent level.
    pub fn is_urgent(&self, urgent_threshold: f64) -> bool {
        self.value >= urgent_threshold
    }

    /// Seconds since last release.
    pub fn seconds_since_release(&self) -> f64 {
        let duration = Utc::now() - self.last_released_at;
        duration.num_milliseconds() as f64 / 1000.0
    }

    /// Take an immutable snapshot of the current state.
    pub fn snapshot(&self, expression_threshold: f64, urgent_threshold: f64) -> PressureSnapshot {
        PressureSnapshot {
            value: self.value,
            should_express: self.should_express(expression_threshold),
            is_urgent: self.is_urgent(urgent_threshold),
            last_updated_at: self.last_updated_at,
            last_released_at: self.last_released_at,
        }
    }
}

/// Events that increase expression pressure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PressureEvent {
    /// Encountered novel, interesting content
    NovelContent,
    /// Recalled a high-relevance memory
    HighSalienceMemory,
    /// User has been idle (opportunity to speak)
    UserIdle,
    /// Discovered a connection between thoughts/ideas
    ConnectionDiscovered,
    /// Urgent external event (calendar, notification)
    UrgentExternal,
    /// Custom pressure amount
    Custom(f64),
}

/// Bonus values for pressure events (extracted from AdvisorConfig).
#[derive(Debug, Clone)]
pub struct PressureConfig {
    pub novel_content_bonus: f64,
    pub high_salience_memory_bonus: f64,
    pub user_idle_bonus: f64,
    pub connection_discovered_bonus: f64,
    pub urgent_external_bonus: f64,
}

impl Default for PressureConfig {
    fn default() -> Self {
        Self {
            novel_content_bonus: 0.3,
            high_salience_memory_bonus: 0.2,
            user_idle_bonus: 0.1,
            connection_discovered_bonus: 0.4,
            urgent_external_bonus: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pressure_is_zero() {
        let p = ExpressionPressure::new();
        assert!((p.value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn increment_adds_base_rate() {
        let mut p = ExpressionPressure::new();
        p.increment(0.05);
        assert!((p.value() - 0.05).abs() < f64::EPSILON);
        p.increment(0.05);
        assert!((p.value() - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn increment_clamps_at_one() {
        let mut p = ExpressionPressure::new();
        p.increment(1.5);
        assert!((p.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_event_bonuses() {
        let mut p = ExpressionPressure::new();
        let config = PressureConfig::default();

        p.apply(&PressureEvent::NovelContent, &config);
        assert!((p.value() - 0.3).abs() < f64::EPSILON);

        p.apply(&PressureEvent::ConnectionDiscovered, &config);
        assert!((p.value() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_custom_event() {
        let mut p = ExpressionPressure::new();
        let config = PressureConfig::default();
        p.apply(&PressureEvent::Custom(0.15), &config);
        assert!((p.value() - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn full_release() {
        let mut p = ExpressionPressure::new();
        p.increment(0.8);
        p.release();
        assert!((p.value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_release() {
        let mut p = ExpressionPressure::new();
        p.increment(1.0);
        p.partial_release(0.8);
        assert!((p.value() - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_release_clamps_fraction() {
        let mut p = ExpressionPressure::new();
        p.increment(0.5);
        p.partial_release(1.5); // clamped to 1.0 → full release
        assert!((p.value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn should_express_thresholds() {
        let mut p = ExpressionPressure::new();
        assert!(!p.should_express(0.8));

        p.increment(0.85);
        assert!(p.should_express(0.8));
        assert!(!p.is_urgent(0.95));

        p.increment(0.15);
        assert!(p.is_urgent(0.95));
    }

    #[test]
    fn snapshot() {
        let mut p = ExpressionPressure::new();
        p.increment(0.9);
        let snap = p.snapshot(0.8, 0.95);
        assert!(snap.should_express);
        assert!(!snap.is_urgent);
        assert!((snap.value - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn pressure_serialization_roundtrip() {
        let mut p = ExpressionPressure::new();
        p.increment(0.42);
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: ExpressionPressure = serde_json::from_str(&json).unwrap();
        assert!((p.value() - deserialized.value()).abs() < f64::EPSILON);
    }
}
