use serde::{Deserialize, Serialize};

/// Individual health status (5 levels).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserHealthStatus {
    /// Lowest score — person is well-connected and engaged.
    Thriving,
    /// Healthy engagement.
    Good,
    /// Some signals worth watching, nothing urgent.
    Okay,
    /// Multiple factors suggest this person needs attention.
    Struggling,
    /// Highest score — multiple concerning signals, may need community support.
    NeedSupport,
}

impl UserHealthStatus {
    /// Derive status from total score (0-12).
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=2 => UserHealthStatus::Thriving,
            3..=4 => UserHealthStatus::Good,
            5..=6 => UserHealthStatus::Okay,
            7..=9 => UserHealthStatus::Struggling,
            _ => UserHealthStatus::NeedSupport,
        }
    }

    /// Map this status to a severity level for alerting.
    pub fn severity(&self) -> HealthSeverity {
        match self {
            UserHealthStatus::Thriving | UserHealthStatus::Good => HealthSeverity::None,
            UserHealthStatus::Okay => HealthSeverity::Low,
            UserHealthStatus::Struggling => HealthSeverity::Medium,
            UserHealthStatus::NeedSupport => HealthSeverity::High,
        }
    }
}

/// Collective health status (5 levels).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CollectiveHealthStatus {
    /// Community is vibrant, well-connected, and inclusive.
    Thriving,
    /// Community is functioning well.
    Healthy,
    /// Community is low-activity but not concerning.
    Quiet,
    /// Community shows signs of tension or unhealthy dynamics.
    Tense,
    /// Community shows multiple warning signs (isolation, power concentration, etc.).
    Toxic,
}

impl CollectiveHealthStatus {
    /// Derive status from total score (0-19, weighted).
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=3 => CollectiveHealthStatus::Thriving,
            4..=6 => CollectiveHealthStatus::Healthy,
            7..=9 => CollectiveHealthStatus::Quiet,
            10..=13 => CollectiveHealthStatus::Tense,
            _ => CollectiveHealthStatus::Toxic,
        }
    }

    /// Map this status to a severity level for alerting.
    pub fn severity(&self) -> HealthSeverity {
        match self {
            CollectiveHealthStatus::Thriving | CollectiveHealthStatus::Healthy => {
                HealthSeverity::None
            }
            CollectiveHealthStatus::Quiet => HealthSeverity::Low,
            CollectiveHealthStatus::Tense => HealthSeverity::Medium,
            CollectiveHealthStatus::Toxic => HealthSeverity::High,
        }
    }
}

/// How severe a health concern is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HealthSeverity {
    /// No concern.
    None,
    /// Worth monitoring.
    Low,
    /// Needs attention.
    Medium,
    /// Requires immediate response.
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_status_thresholds() {
        assert_eq!(UserHealthStatus::from_score(0), UserHealthStatus::Thriving);
        assert_eq!(UserHealthStatus::from_score(2), UserHealthStatus::Thriving);
        assert_eq!(UserHealthStatus::from_score(3), UserHealthStatus::Good);
        assert_eq!(UserHealthStatus::from_score(5), UserHealthStatus::Okay);
        assert_eq!(UserHealthStatus::from_score(7), UserHealthStatus::Struggling);
        assert_eq!(UserHealthStatus::from_score(10), UserHealthStatus::NeedSupport);
        assert_eq!(UserHealthStatus::from_score(12), UserHealthStatus::NeedSupport);
    }

    #[test]
    fn collective_status_thresholds() {
        assert_eq!(CollectiveHealthStatus::from_score(0), CollectiveHealthStatus::Thriving);
        assert_eq!(CollectiveHealthStatus::from_score(4), CollectiveHealthStatus::Healthy);
        assert_eq!(CollectiveHealthStatus::from_score(7), CollectiveHealthStatus::Quiet);
        assert_eq!(CollectiveHealthStatus::from_score(10), CollectiveHealthStatus::Tense);
        assert_eq!(CollectiveHealthStatus::from_score(14), CollectiveHealthStatus::Toxic);
    }

    #[test]
    fn severity_ordering() {
        assert!(HealthSeverity::None < HealthSeverity::Low);
        assert!(HealthSeverity::Low < HealthSeverity::Medium);
        assert!(HealthSeverity::Medium < HealthSeverity::High);
    }
}
