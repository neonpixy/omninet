//! Verification pattern analysis — rule-based safety scoring.
//!
//! Port of AuthBook's `analyzePattern`. Derives a person's verification
//! status from the verifications and flags visible in the querier's network.

use serde::{Deserialize, Serialize};

use super::query::{NetworkFlag, NetworkVerification};
use crate::flag::types::FlagSeverity;

/// Derived verification pattern for a person in the network.
///
/// Decision tree (from AuthBook):
/// 1. Any Critical flag → Flagged
/// 2. 2+ High flags → Suspicious
/// 3. No verifications → Isolated
/// 4. < 3 verifications → Limited
/// 5. Otherwise → Healthy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerificationPattern {
    /// 3+ verifications, no concerning flags.
    Healthy,
    /// 1-2 verifications.
    Limited,
    /// No verifications visible in network.
    Isolated,
    /// 2+ high-severity flags.
    Suspicious,
    /// 1+ critical flag.
    Flagged,
}

impl std::fmt::Display for VerificationPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Limited => write!(f, "limited"),
            Self::Isolated => write!(f, "isolated"),
            Self::Suspicious => write!(f, "suspicious"),
            Self::Flagged => write!(f, "flagged"),
        }
    }
}

/// Analyze the verification pattern from visible network data.
pub fn analyze_pattern(
    verifications: &[NetworkVerification],
    flags: &[NetworkFlag],
) -> VerificationPattern {
    // 1. Any critical flag → Flagged
    if flags.iter().any(|f| f.severity == FlagSeverity::Critical) {
        return VerificationPattern::Flagged;
    }

    // 2. 2+ high flags → Suspicious
    let high_count = flags
        .iter()
        .filter(|f| f.severity == FlagSeverity::High)
        .count();
    if high_count >= 2 {
        return VerificationPattern::Suspicious;
    }

    // 3. No verifications → Isolated
    if verifications.is_empty() {
        return VerificationPattern::Isolated;
    }

    // 4. < 3 verifications → Limited
    if verifications.len() < 3 {
        return VerificationPattern::Limited;
    }

    // 5. Otherwise → Healthy
    VerificationPattern::Healthy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flag::types::FlagCategory;
    use crate::trust_graph::edge::VerificationSentiment;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_verification(degree: usize) -> NetworkVerification {
        NetworkVerification {
            verifier_pubkey: format!("verifier_{degree}"),
            sentiment: VerificationSentiment::Positive,
            degree_from_querier: degree,
            verified_at: Utc::now(),
        }
    }

    fn make_network_flag(severity: FlagSeverity) -> NetworkFlag {
        NetworkFlag {
            flag_id: Uuid::new_v4(),
            flagger_pubkey: "flagger".into(),
            category: FlagCategory::SuspiciousActivity,
            severity,
            degree_from_querier: 1,
        }
    }

    #[test]
    fn healthy_pattern() {
        let verifications: Vec<_> = (0..3).map(make_verification).collect();
        let flags: Vec<NetworkFlag> = vec![];
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Healthy
        );
    }

    #[test]
    fn limited_pattern() {
        let verifications = vec![make_verification(1), make_verification(2)];
        let flags: Vec<NetworkFlag> = vec![];
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Limited
        );
    }

    #[test]
    fn isolated_pattern() {
        let verifications: Vec<NetworkVerification> = vec![];
        let flags: Vec<NetworkFlag> = vec![];
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Isolated
        );
    }

    #[test]
    fn suspicious_pattern() {
        let verifications = vec![make_verification(1)];
        let flags = vec![
            make_network_flag(FlagSeverity::High),
            make_network_flag(FlagSeverity::High),
        ];
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Suspicious
        );
    }

    #[test]
    fn flagged_pattern() {
        let verifications = vec![make_verification(1), make_verification(2), make_verification(3)];
        let flags = vec![make_network_flag(FlagSeverity::Critical)];
        // Critical flag overrides even healthy verification count
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Flagged
        );
    }

    #[test]
    fn critical_overrides_everything() {
        let verifications: Vec<_> = (0..10).map(make_verification).collect();
        let flags = vec![make_network_flag(FlagSeverity::Critical)];
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Flagged
        );
    }

    #[test]
    fn single_high_flag_not_suspicious() {
        let verifications = vec![make_verification(1)];
        let flags = vec![make_network_flag(FlagSeverity::High)];
        // Only 1 high flag + 1 verification → Limited, not Suspicious
        assert_eq!(
            analyze_pattern(&verifications, &flags),
            VerificationPattern::Limited
        );
    }

    #[test]
    fn pattern_display() {
        assert_eq!(VerificationPattern::Healthy.to_string(), "healthy");
        assert_eq!(VerificationPattern::Flagged.to_string(), "flagged");
    }
}
