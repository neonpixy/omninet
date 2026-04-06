//! Trust recommendations — safety guidance based on network intelligence.
//!
//! Port of AuthBook's `generateRecommendation`. Takes a verification pattern,
//! closest degree of separation, and flag count to produce actionable guidance.

use serde::{Deserialize, Serialize};

use super::pattern::VerificationPattern;

/// Safety recommendation for interacting with a person.
///
/// Decision tree (from AuthBook):
/// - Flagged → Avoid
/// - Suspicious → GroupOnly
/// - Isolated → PublicOnly
/// - Limited → if closest degree ≤ 2: Caution, else: PublicOnly
/// - Healthy → if closest degree == 1 && no flags: Safe, else: Caution
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TrustRecommendation {
    /// Close trusted connection, no flags. Safe to interact freely.
    Safe,
    /// Verify identity in person. Generally trustworthy but verify.
    Caution,
    /// Only meet in public places with witnesses.
    PublicOnly,
    /// Meet only in group settings, not one-on-one.
    GroupOnly,
    /// Serious safety concern. Avoid interaction.
    Avoid,
}

impl TrustRecommendation {
    /// Human-readable description of what this recommendation means.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Safe => "Trusted connection with no concerns",
            Self::Caution => "Generally trustworthy — verify identity when meeting",
            Self::PublicOnly => "Meet only in public places with witnesses present",
            Self::GroupOnly => "Interact only in group settings, not one-on-one",
            Self::Avoid => "Serious safety concern — avoid interaction",
        }
    }

    /// Severity level (0 = safe, 4 = avoid).
    pub fn severity(&self) -> u8 {
        match self {
            Self::Safe => 0,
            Self::Caution => 1,
            Self::PublicOnly => 2,
            Self::GroupOnly => 3,
            Self::Avoid => 4,
        }
    }
}

impl std::fmt::Display for TrustRecommendation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Safe => write!(f, "safe"),
            Self::Caution => write!(f, "caution"),
            Self::PublicOnly => write!(f, "public_only"),
            Self::GroupOnly => write!(f, "group_only"),
            Self::Avoid => write!(f, "avoid"),
        }
    }
}

/// Generate a safety recommendation from network intelligence.
pub fn generate_recommendation(
    pattern: VerificationPattern,
    closest_degree: Option<usize>,
    flag_count: usize,
) -> TrustRecommendation {
    match pattern {
        VerificationPattern::Flagged => TrustRecommendation::Avoid,
        VerificationPattern::Suspicious => TrustRecommendation::GroupOnly,
        VerificationPattern::Isolated => TrustRecommendation::PublicOnly,
        VerificationPattern::Limited => {
            if closest_degree.is_some_and(|d| d <= 2) {
                TrustRecommendation::Caution
            } else {
                TrustRecommendation::PublicOnly
            }
        }
        VerificationPattern::Healthy => {
            if closest_degree == Some(1) && flag_count == 0 {
                TrustRecommendation::Safe
            } else {
                TrustRecommendation::Caution
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flagged_always_avoid() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Flagged, Some(1), 0),
            TrustRecommendation::Avoid
        );
    }

    #[test]
    fn suspicious_always_group_only() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Suspicious, Some(2), 2),
            TrustRecommendation::GroupOnly
        );
    }

    #[test]
    fn isolated_always_public_only() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Isolated, None, 0),
            TrustRecommendation::PublicOnly
        );
    }

    #[test]
    fn limited_close_degree_caution() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Limited, Some(2), 0),
            TrustRecommendation::Caution
        );
    }

    #[test]
    fn limited_far_degree_public_only() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Limited, Some(3), 0),
            TrustRecommendation::PublicOnly
        );
    }

    #[test]
    fn limited_no_degree_public_only() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Limited, None, 0),
            TrustRecommendation::PublicOnly
        );
    }

    #[test]
    fn healthy_close_no_flags_safe() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Healthy, Some(1), 0),
            TrustRecommendation::Safe
        );
    }

    #[test]
    fn healthy_close_with_flags_caution() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Healthy, Some(1), 1),
            TrustRecommendation::Caution
        );
    }

    #[test]
    fn healthy_far_degree_caution() {
        assert_eq!(
            generate_recommendation(VerificationPattern::Healthy, Some(3), 0),
            TrustRecommendation::Caution
        );
    }

    #[test]
    fn recommendation_severity_ordering() {
        assert!(TrustRecommendation::Safe.severity() < TrustRecommendation::Caution.severity());
        assert!(TrustRecommendation::Caution.severity() < TrustRecommendation::PublicOnly.severity());
        assert!(TrustRecommendation::PublicOnly.severity() < TrustRecommendation::GroupOnly.severity());
        assert!(TrustRecommendation::GroupOnly.severity() < TrustRecommendation::Avoid.severity());
    }

    #[test]
    fn recommendation_display() {
        assert_eq!(TrustRecommendation::Safe.to_string(), "safe");
        assert_eq!(TrustRecommendation::Avoid.to_string(), "avoid");
    }

    #[test]
    fn recommendation_description_non_empty() {
        for rec in [
            TrustRecommendation::Safe,
            TrustRecommendation::Caution,
            TrustRecommendation::PublicOnly,
            TrustRecommendation::GroupOnly,
            TrustRecommendation::Avoid,
        ] {
            assert!(!rec.description().is_empty());
        }
    }
}
