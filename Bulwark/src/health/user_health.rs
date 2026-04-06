use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::status::UserHealthStatus;

/// A snapshot of a person's structural health — never content.
///
/// 4 factors, each 0-3 points. Total 0-12 maps to 5 statuses.
/// Measures structural signals (connections, activity patterns),
/// NOT what someone says or creates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserHealthPulse {
    pub pubkey: String,
    pub status: UserHealthStatus,
    pub factors: UserHealthFactors,
    pub computed_at: DateTime<Utc>,
    pub ttl_hours: u32,
}

impl UserHealthPulse {
    /// Compute a health pulse from the given factors and derive the status.
    pub fn compute(pubkey: impl Into<String>, factors: UserHealthFactors) -> Self {
        let status = UserHealthStatus::from_score(factors.total_score());
        Self {
            pubkey: pubkey.into(),
            status,
            factors,
            computed_at: Utc::now(),
            ttl_hours: 24,
        }
    }

    /// Whether this pulse has expired and should be recomputed.
    pub fn is_expired(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.computed_at);
        age.num_hours() > i64::from(self.ttl_hours)
    }
}

/// The 4 health factors — each contributes 0-3 to the total score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserHealthFactors {
    pub connection_level: ConnectionLevel,
    pub communication_balance: CommunicationBalance,
    pub activity_pattern: ActivityPattern,
    pub content_sentiment: ContentSentiment,
}

impl UserHealthFactors {
    /// Sum of all 4 factor scores (0-12).
    pub fn total_score(&self) -> u32 {
        self.connection_level.score()
            + self.communication_balance.score()
            + self.activity_pattern.score()
            + self.content_sentiment.score()
    }
}

/// Factor 1: How connected is the person? (0-3)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConnectionLevel {
    WellConnected,
    Connected,
    FewConnections,
    Isolated,
}

impl ConnectionLevel {
    pub fn score(&self) -> u32 {
        match self {
            ConnectionLevel::WellConnected => 0,
            ConnectionLevel::Connected => 1,
            ConnectionLevel::FewConnections => 2,
            ConnectionLevel::Isolated => 3,
        }
    }
}

/// Factor 2: Communication balance (0-2).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommunicationBalance {
    Balanced,
    MostlyGiving,
    MostlyReceiving,
    Minimal,
}

impl CommunicationBalance {
    pub fn score(&self) -> u32 {
        match self {
            CommunicationBalance::Balanced => 0,
            CommunicationBalance::MostlyGiving => 1,
            CommunicationBalance::MostlyReceiving => 1,
            CommunicationBalance::Minimal => 2,
        }
    }
}

/// Factor 3: Activity pattern (0-3).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ActivityPattern {
    Consistent,
    Variable,
    Declining,
    Inactive,
}

impl ActivityPattern {
    pub fn score(&self) -> u32 {
        match self {
            ActivityPattern::Consistent => 0,
            ActivityPattern::Variable => 1,
            ActivityPattern::Declining => 2,
            ActivityPattern::Inactive => 3,
        }
    }
}

/// Factor 4: Content sentiment (0-3). Structural only — never reads content.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContentSentiment {
    Positive,
    Neutral,
    Mixed,
    Concerning,
}

impl ContentSentiment {
    pub fn score(&self) -> u32 {
        match self {
            ContentSentiment::Positive => 0,
            ContentSentiment::Neutral => 1,
            ContentSentiment::Mixed => 2,
            ContentSentiment::Concerning => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thriving_user() {
        let factors = UserHealthFactors {
            connection_level: ConnectionLevel::WellConnected,
            communication_balance: CommunicationBalance::Balanced,
            activity_pattern: ActivityPattern::Consistent,
            content_sentiment: ContentSentiment::Positive,
        };
        assert_eq!(factors.total_score(), 0);
        let pulse = UserHealthPulse::compute("alice", factors);
        assert_eq!(pulse.status, UserHealthStatus::Thriving);
    }

    #[test]
    fn struggling_user() {
        let factors = UserHealthFactors {
            connection_level: ConnectionLevel::FewConnections,
            communication_balance: CommunicationBalance::Minimal,
            activity_pattern: ActivityPattern::Declining,
            content_sentiment: ContentSentiment::Mixed,
        };
        // 2 + 2 + 2 + 2 = 8
        assert_eq!(factors.total_score(), 8);
        let pulse = UserHealthPulse::compute("bob", factors);
        assert_eq!(pulse.status, UserHealthStatus::Struggling);
    }

    #[test]
    fn need_support_user() {
        let factors = UserHealthFactors {
            connection_level: ConnectionLevel::Isolated,
            communication_balance: CommunicationBalance::Minimal,
            activity_pattern: ActivityPattern::Inactive,
            content_sentiment: ContentSentiment::Concerning,
        };
        // 3 + 2 + 3 + 3 = 11
        assert_eq!(factors.total_score(), 11);
        let pulse = UserHealthPulse::compute("charlie", factors);
        assert_eq!(pulse.status, UserHealthStatus::NeedSupport);
    }

    #[test]
    fn score_range_covers_all_statuses() {
        // Min score = 0 (thriving), max = 11 (need support)
        assert_eq!(UserHealthStatus::from_score(0), UserHealthStatus::Thriving);
        assert_eq!(UserHealthStatus::from_score(4), UserHealthStatus::Good);
        assert_eq!(UserHealthStatus::from_score(6), UserHealthStatus::Okay);
        assert_eq!(UserHealthStatus::from_score(9), UserHealthStatus::Struggling);
        assert_eq!(UserHealthStatus::from_score(11), UserHealthStatus::NeedSupport);
    }
}
