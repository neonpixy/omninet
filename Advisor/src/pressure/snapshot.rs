use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Immutable snapshot of expression pressure state.
///
/// Used for logging, display, and passing pressure state
/// without giving access to mutation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PressureSnapshot {
    pub value: f64,
    pub should_express: bool,
    pub is_urgent: bool,
    pub last_updated_at: DateTime<Utc>,
    pub last_released_at: DateTime<Utc>,
}

impl PressureSnapshot {
    /// Pressure as a percentage (0–100).
    pub fn percentage(&self) -> f64 {
        self.value * 100.0
    }

    /// Human-readable pressure level.
    pub fn level(&self) -> &'static str {
        if self.is_urgent {
            "urgent"
        } else if self.should_express {
            "ready"
        } else if self.value > 0.5 {
            "building"
        } else if self.value > 0.2 {
            "low"
        } else {
            "calm"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(value: f64, should_express: bool, is_urgent: bool) -> PressureSnapshot {
        let now = Utc::now();
        PressureSnapshot {
            value,
            should_express,
            is_urgent,
            last_updated_at: now,
            last_released_at: now,
        }
    }

    #[test]
    fn percentage() {
        let s = snapshot(0.42, false, false);
        assert!((s.percentage() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn levels() {
        assert_eq!(snapshot(0.0, false, false).level(), "calm");
        assert_eq!(snapshot(0.1, false, false).level(), "calm");
        assert_eq!(snapshot(0.3, false, false).level(), "low");
        assert_eq!(snapshot(0.6, false, false).level(), "building");
        assert_eq!(snapshot(0.85, true, false).level(), "ready");
        assert_eq!(snapshot(0.98, true, true).level(), "urgent");
    }

    #[test]
    fn snapshot_serialization_roundtrip() {
        let s = snapshot(0.75, true, false);
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: PressureSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(s.value, deserialized.value);
        assert_eq!(s.should_express, deserialized.should_express);
    }
}
