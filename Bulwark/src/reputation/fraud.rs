use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::federation_scope::FederationScope;

/// A detected suspicious pattern.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuspiciousPattern {
    pub id: Uuid,
    pub pattern_type: SuspiciousPatternType,
    pub subject: String,
    pub confidence: f64,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    /// Which community this pattern was detected in.
    ///
    /// `None` for network-global patterns or legacy data.
    #[serde(default)]
    pub community_id: Option<String>,
}

/// Types of suspicious patterns to detect.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SuspiciousPatternType {
    CircularTrading,
    SelfTrading,
    PriceManipulation,
    DumpAndRun,
    MultipleIdentities,
    InactiveHarvesting,
    VouchingRing,
    NonFulfillment,
    SerialDisputer,
    IsolationPattern,
    ReputationGaming,
}

/// A fraud indicator with severity and confidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FraudIndicator {
    pub indicator_type: SuspiciousPatternType,
    pub severity: FraudSeverity,
    pub confidence: f64,
    pub description: String,
    /// Which community this indicator came from.
    ///
    /// `None` for network-global indicators or legacy data.
    #[serde(default)]
    pub community_id: Option<String>,
}

/// How severe a fraud signal is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FraudSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Overall fraud risk assessment for a person.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskScore {
    pub pubkey: String,
    pub score: f64,
    pub indicators: Vec<FraudIndicator>,
    pub recommendation: RiskRecommendation,
    pub computed_at: DateTime<Utc>,
}

impl RiskScore {
    /// Compute risk score from indicators.
    pub fn compute(pubkey: impl Into<String>, indicators: Vec<FraudIndicator>) -> Self {
        let mut total: f64 = 0.0;
        for indicator in &indicators {
            total += indicator.confidence * 0.3;
        }
        let score = total.min(1.0);
        let recommendation = RiskRecommendation::from_score(score);

        Self {
            pubkey: pubkey.into(),
            score,
            indicators,
            recommendation,
            computed_at: Utc::now(),
        }
    }

    /// Compute risk score from indicators, filtering to visible communities.
    ///
    /// Indicators with no `community_id` are always included (backward compat).
    /// Indicators from communities outside the scope are excluded.
    ///
    /// When the scope is unrestricted, this is identical to [`compute`](Self::compute).
    pub fn compute_scoped(
        pubkey: impl Into<String>,
        indicators: Vec<FraudIndicator>,
        scope: &FederationScope,
    ) -> Self {
        if scope.is_unrestricted() {
            return Self::compute(pubkey, indicators);
        }

        let filtered: Vec<FraudIndicator> = indicators
            .into_iter()
            .filter(|ind| match &ind.community_id {
                None => true,
                Some(id) => scope.is_visible(id),
            })
            .collect();

        Self::compute(pubkey, filtered)
    }
}

/// Risk recommendation based on score.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RiskRecommendation {
    Safe,
    Caution,
    AvoidLargeTransactions,
    DoNotTrade,
}

impl RiskRecommendation {
    pub fn from_score(score: f64) -> Self {
        if score < 0.25 {
            RiskRecommendation::Safe
        } else if score < 0.5 {
            RiskRecommendation::Caution
        } else if score < 0.75 {
            RiskRecommendation::AvoidLargeTransactions
        } else {
            RiskRecommendation::DoNotTrade
        }
    }
}

/// A pluggable fraud detection algorithm.
pub trait FraudDetection: Send + Sync {
    fn detect(&self, pubkey: &str) -> Vec<SuspiciousPattern>;
    fn detector_id(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_score_from_indicators() {
        let indicators = vec![
            FraudIndicator {
                indicator_type: SuspiciousPatternType::CircularTrading,
                severity: FraudSeverity::High,
                confidence: 0.8,
                description: "Wash trading detected".into(),
                community_id: None,
            },
            FraudIndicator {
                indicator_type: SuspiciousPatternType::ReputationGaming,
                severity: FraudSeverity::Medium,
                confidence: 0.6,
                description: "Endorsement farming".into(),
                community_id: None,
            },
        ];
        let risk = RiskScore::compute("alice", indicators);
        // (0.8 * 0.3) + (0.6 * 0.3) = 0.24 + 0.18 = 0.42
        assert!((risk.score - 0.42).abs() < 0.01);
        assert_eq!(risk.recommendation, RiskRecommendation::Caution);
    }

    #[test]
    fn safe_with_no_indicators() {
        let risk = RiskScore::compute("alice", vec![]);
        assert_eq!(risk.score, 0.0);
        assert_eq!(risk.recommendation, RiskRecommendation::Safe);
    }

    #[test]
    fn do_not_trade_high_risk() {
        let indicators = vec![
            FraudIndicator {
                indicator_type: SuspiciousPatternType::MultipleIdentities,
                severity: FraudSeverity::Critical,
                confidence: 0.95,
                description: "Sybil attack".into(),
                community_id: None,
            },
            FraudIndicator {
                indicator_type: SuspiciousPatternType::SelfTrading,
                severity: FraudSeverity::Critical,
                confidence: 0.9,
                description: "Trading with own alts".into(),
                community_id: None,
            },
            FraudIndicator {
                indicator_type: SuspiciousPatternType::InactiveHarvesting,
                severity: FraudSeverity::High,
                confidence: 0.85,
                description: "Only claims UBI".into(),
                community_id: None,
            },
        ];
        let risk = RiskScore::compute("attacker", indicators);
        // (0.95 + 0.9 + 0.85) * 0.3 = 0.81, clamped to 0.81
        assert!(risk.score > 0.75);
        assert_eq!(risk.recommendation, RiskRecommendation::DoNotTrade);
    }

    #[test]
    fn recommendation_thresholds() {
        assert_eq!(RiskRecommendation::from_score(0.1), RiskRecommendation::Safe);
        assert_eq!(RiskRecommendation::from_score(0.3), RiskRecommendation::Caution);
        assert_eq!(RiskRecommendation::from_score(0.6), RiskRecommendation::AvoidLargeTransactions);
        assert_eq!(RiskRecommendation::from_score(0.9), RiskRecommendation::DoNotTrade);
    }

    #[test]
    fn severity_ordering() {
        assert!(FraudSeverity::Low < FraudSeverity::Medium);
        assert!(FraudSeverity::Medium < FraudSeverity::High);
        assert!(FraudSeverity::High < FraudSeverity::Critical);
    }

    // ── Federation Scope ──────────────────────────────────────────────────

    #[test]
    fn compute_scoped_unrestricted_matches_compute() {
        let indicators = vec![
            FraudIndicator {
                indicator_type: SuspiciousPatternType::CircularTrading,
                severity: FraudSeverity::High,
                confidence: 0.8,
                description: "Wash trading".into(),
                community_id: Some("alpha".into()),
            },
        ];
        let unscoped = RiskScore::compute("alice", indicators.clone());
        let scoped = RiskScore::compute_scoped("alice", indicators, &FederationScope::new());
        assert_eq!(unscoped.score, scoped.score);
    }

    #[test]
    fn compute_scoped_filters_indicators() {
        let indicators = vec![
            FraudIndicator {
                indicator_type: SuspiciousPatternType::CircularTrading,
                severity: FraudSeverity::High,
                confidence: 0.8,
                description: "Wash trading in alpha".into(),
                community_id: Some("alpha".into()),
            },
            FraudIndicator {
                indicator_type: SuspiciousPatternType::ReputationGaming,
                severity: FraudSeverity::Medium,
                confidence: 0.6,
                description: "Gaming in beta".into(),
                community_id: Some("beta".into()),
            },
        ];

        // Full: (0.8 + 0.6) * 0.3 = 0.42
        let full = RiskScore::compute("alice", indicators.clone());
        assert!((full.score - 0.42).abs() < 0.01);

        // Scoped to alpha only: 0.8 * 0.3 = 0.24
        let scoped = RiskScore::compute_scoped(
            "alice",
            indicators,
            &FederationScope::from_communities(["alpha"]),
        );
        assert!((scoped.score - 0.24).abs() < 0.01);
        assert_eq!(scoped.indicators.len(), 1);
    }

    #[test]
    fn compute_scoped_includes_untagged_indicators() {
        let indicators = vec![
            FraudIndicator {
                indicator_type: SuspiciousPatternType::SelfTrading,
                severity: FraudSeverity::Critical,
                confidence: 0.9,
                description: "Global indicator".into(),
                community_id: None, // untagged — always visible
            },
            FraudIndicator {
                indicator_type: SuspiciousPatternType::VouchingRing,
                severity: FraudSeverity::High,
                confidence: 0.7,
                description: "Ring in beta".into(),
                community_id: Some("beta".into()),
            },
        ];

        // Scoped to alpha: only the untagged indicator passes.
        let scoped = RiskScore::compute_scoped(
            "alice",
            indicators,
            &FederationScope::from_communities(["alpha"]),
        );
        assert_eq!(scoped.indicators.len(), 1);
        assert!((scoped.score - 0.27).abs() < 0.01); // 0.9 * 0.3 = 0.27
    }
}
