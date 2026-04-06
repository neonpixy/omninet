//! Anti-weaponization detection — protecting the accountability system from abuse.
//!
//! From Constellation Art. 5 §6-12: "All challenges shall be brought in good faith
//! to address genuine breaches, not to harass, exhaust, or silence legitimate actors."
//!
//! Detects: serial filing, coordinated campaigns, repetitive grounds, resource warfare.
//! Responds: warning → require cosigner → suspend filing → public identification.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::types::AccountabilityFlag;
use crate::config::JailConfig;

/// Types of flag abuse patterns (Art. 5 §11).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AbusePattern {
    /// One person filing excessive flags against multiple targets.
    SerialFiling,
    /// Multiple people coordinating flags against one target.
    CoordinatedCampaign,
    /// Repeatedly filing on identical grounds already addressed.
    RepetitiveGrounds,
    /// Using flag processes to drain resources rather than seek remedy.
    ResourceWarfare,
}

impl std::fmt::Display for AbusePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerialFiling => write!(f, "serial_filing"),
            Self::CoordinatedCampaign => write!(f, "coordinated_campaign"),
            Self::RepetitiveGrounds => write!(f, "repetitive_grounds"),
            Self::ResourceWarfare => write!(f, "resource_warfare"),
        }
    }
}

/// An indicator that weaponization may be occurring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbuseIndicator {
    /// The type of abuse pattern detected.
    pub pattern: AbusePattern,
    /// Evidence supporting the detection.
    pub evidence: String,
    /// When the pattern was detected.
    pub detected_at: DateTime<Utc>,
    /// Confidence level (0.0 to 1.0).
    pub confidence: f64,
}

/// Graduated consequences for flag abuse (Art. 5 §9).
///
/// "Initial abuses shall result in warnings... continued abuse may lead to
/// requiring community co-signers, temporary suspension, public identification."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbuseConsequence {
    /// First offense: warning and education about proper procedures.
    Warning,
    /// Continued abuse: future flags require a community co-signer.
    RequireCosigner,
    /// Persistent abuse: temporarily suspend ability to file flags.
    SuspendFiling,
    /// Severe/repeated: public identification as a vexatious filer.
    PublicIdentification,
}

impl AbuseConsequence {
    pub fn description(&self) -> &'static str {
        match self {
            Self::Warning => "Warning and education about proper flag procedures",
            Self::RequireCosigner => "Future flags require a community co-signer",
            Self::SuspendFiling => "Temporarily suspended from filing new flags",
            Self::PublicIdentification => "Publicly identified as a vexatious filer",
        }
    }
}

/// Detect serial filing: one person filing too many flags within a time window.
pub fn detect_serial_filing(
    flags_by_flagger: &[AccountabilityFlag],
    window_days: u64,
    threshold: usize,
) -> Option<AbuseIndicator> {
    let cutoff = Utc::now() - Duration::days(window_days as i64);
    let recent_count = flags_by_flagger
        .iter()
        .filter(|f| f.raised_at > cutoff)
        .count();

    if recent_count >= threshold {
        Some(AbuseIndicator {
            pattern: AbusePattern::SerialFiling,
            evidence: format!(
                "{recent_count} flags filed in {window_days} days (threshold: {threshold})"
            ),
            detected_at: Utc::now(),
            confidence: (recent_count as f64 / threshold as f64).min(1.0),
        })
    } else {
        None
    }
}

/// Detect coordinated campaign: multiple people filing against the same target
/// in a suspiciously short window.
pub fn detect_coordinated_campaign(
    flags_against_target: &[AccountabilityFlag],
    window_days: u64,
) -> Option<AbuseIndicator> {
    let cutoff = Utc::now() - Duration::days(window_days as i64);
    let recent: Vec<&AccountabilityFlag> = flags_against_target
        .iter()
        .filter(|f| f.raised_at > cutoff)
        .collect();

    if recent.len() < 3 {
        return None;
    }

    // Check if multiple distinct flaggers are all filing in the same short window
    let distinct_flaggers: std::collections::HashSet<&str> =
        recent.iter().map(|f| f.flagger_pubkey.as_str()).collect();

    // Coordinated = 3+ distinct flaggers in the window
    if distinct_flaggers.len() >= 3 {
        // Further heuristic: if they all filed within a very tight timeframe
        // (say, 1/4 of the window), that's more suspicious
        let earliest = recent.iter().map(|f| f.raised_at).min()
            .expect("recent is non-empty: checked len >= 3 above");
        let latest = recent.iter().map(|f| f.raised_at).max()
            .expect("recent is non-empty: checked len >= 3 above");
        let span = latest - earliest;
        let window_duration = Duration::days(window_days as i64);

        let tight_window = span < window_duration / 4;
        let confidence = if tight_window { 0.9 } else { 0.6 };

        Some(AbuseIndicator {
            pattern: AbusePattern::CoordinatedCampaign,
            evidence: format!(
                "{} distinct flaggers filed {} flags in {} days",
                distinct_flaggers.len(),
                recent.len(),
                span.num_days()
            ),
            detected_at: Utc::now(),
            confidence,
        })
    } else {
        None
    }
}

/// Detect repetitive grounds: same person filing multiple flags against the same
/// target with the same category.
pub fn detect_repetitive_grounds(
    flags_by_flagger: &[AccountabilityFlag],
    target: &str,
) -> Option<AbuseIndicator> {
    let targeting: Vec<&AccountabilityFlag> = flags_by_flagger
        .iter()
        .filter(|f| f.flagged_pubkey == target)
        .collect();

    if targeting.len() < 2 {
        return None;
    }

    // Check for repeated categories
    let mut category_counts = std::collections::HashMap::new();
    for flag in &targeting {
        *category_counts.entry(flag.category).or_insert(0usize) += 1;
    }

    let max_repeat = category_counts.values().max().copied().unwrap_or(0);
    if max_repeat >= 2 {
        Some(AbuseIndicator {
            pattern: AbusePattern::RepetitiveGrounds,
            evidence: format!(
                "{max_repeat} flags with same category against {target}"
            ),
            detected_at: Utc::now(),
            confidence: (max_repeat as f64 / 4.0).min(1.0),
        })
    } else {
        None
    }
}

/// Recommend a graduated consequence based on detected abuse indicators.
///
/// Follows Art. 5 §9: "Initial abuses → warnings... continued abuse →
/// cosigners → suspension → public identification."
pub fn recommend_consequence(indicators: &[AbuseIndicator]) -> Option<AbuseConsequence> {
    if indicators.is_empty() {
        return None;
    }

    let max_confidence = indicators
        .iter()
        .map(|i| i.confidence)
        .fold(0.0f64, f64::max);

    let count = indicators.len();

    if count >= 3 || max_confidence >= 0.95 {
        Some(AbuseConsequence::PublicIdentification)
    } else if count >= 2 || max_confidence >= 0.8 {
        Some(AbuseConsequence::SuspendFiling)
    } else if max_confidence >= 0.6 {
        Some(AbuseConsequence::RequireCosigner)
    } else {
        Some(AbuseConsequence::Warning)
    }
}

/// Check if a flagger has exceeded the rate limit for flag filing.
pub fn check_rate_limit(
    flagger_pubkey: &str,
    all_flags: &[AccountabilityFlag],
    config: &JailConfig,
) -> Result<(), crate::error::JailError> {
    let today_start = Utc::now() - Duration::days(1);
    let recent_count = all_flags
        .iter()
        .filter(|f| f.flagger_pubkey == flagger_pubkey && f.raised_at > today_start)
        .count();

    if recent_count >= config.max_flags_per_pubkey_per_day {
        Err(crate::error::JailError::FlagRateLimited {
            pubkey: flagger_pubkey.to_string(),
            count: recent_count,
            max: config.max_flags_per_pubkey_per_day,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flag::types::{FlagCategory, FlagSeverity};

    fn make_flag_at(flagger: &str, flagged: &str, when: DateTime<Utc>) -> AccountabilityFlag {
        let mut flag = AccountabilityFlag::raise(
            flagger,
            flagged,
            FlagCategory::SuspiciousActivity,
            FlagSeverity::Medium,
            "test",
        );
        flag.raised_at = when;
        flag
    }

    fn make_recent_flag(flagger: &str, flagged: &str) -> AccountabilityFlag {
        make_flag_at(flagger, flagged, Utc::now())
    }

    #[test]
    fn no_serial_filing_below_threshold() {
        let flags: Vec<_> = (0..3)
            .map(|i| make_recent_flag("alice", &format!("target_{i}")))
            .collect();
        assert!(detect_serial_filing(&flags, 30, 5).is_none());
    }

    #[test]
    fn serial_filing_at_threshold() {
        let flags: Vec<_> = (0..5)
            .map(|i| make_recent_flag("alice", &format!("target_{i}")))
            .collect();
        let indicator = detect_serial_filing(&flags, 30, 5).unwrap();
        assert_eq!(indicator.pattern, AbusePattern::SerialFiling);
        assert!(indicator.confidence >= 1.0);
    }

    #[test]
    fn serial_filing_ignores_old_flags() {
        let old = Utc::now() - Duration::days(60);
        let flags: Vec<_> = (0..5)
            .map(|i| make_flag_at("alice", &format!("target_{i}"), old))
            .collect();
        assert!(detect_serial_filing(&flags, 30, 5).is_none());
    }

    #[test]
    fn coordinated_campaign_detected() {
        let flags = vec![
            make_recent_flag("alice", "victim"),
            make_recent_flag("bob", "victim"),
            make_recent_flag("carol", "victim"),
        ];
        let indicator = detect_coordinated_campaign(&flags, 30).unwrap();
        assert_eq!(indicator.pattern, AbusePattern::CoordinatedCampaign);
    }

    #[test]
    fn no_coordination_with_few_flaggers() {
        let flags = vec![
            make_recent_flag("alice", "victim"),
            make_recent_flag("alice", "victim"), // same flagger
            make_recent_flag("bob", "victim"),
        ];
        assert!(detect_coordinated_campaign(&flags, 30).is_none());
    }

    #[test]
    fn repetitive_grounds_detected() {
        let flags = vec![
            make_recent_flag("alice", "bob"),
            make_recent_flag("alice", "bob"), // same category (SuspiciousActivity)
        ];
        let indicator = detect_repetitive_grounds(&flags, "bob").unwrap();
        assert_eq!(indicator.pattern, AbusePattern::RepetitiveGrounds);
    }

    #[test]
    fn no_repetitive_with_different_targets() {
        let flags = vec![
            make_recent_flag("alice", "bob"),
            make_recent_flag("alice", "carol"), // different target
        ];
        assert!(detect_repetitive_grounds(&flags, "bob").is_none());
    }

    #[test]
    fn consequence_graduated() {
        // No indicators → no consequence
        assert!(recommend_consequence(&[]).is_none());

        // Low confidence → warning
        let low = vec![AbuseIndicator {
            pattern: AbusePattern::SerialFiling,
            evidence: "test".into(),
            detected_at: Utc::now(),
            confidence: 0.3,
        }];
        assert_eq!(recommend_consequence(&low), Some(AbuseConsequence::Warning));

        // Medium confidence → require cosigner
        let medium = vec![AbuseIndicator {
            pattern: AbusePattern::SerialFiling,
            evidence: "test".into(),
            detected_at: Utc::now(),
            confidence: 0.7,
        }];
        assert_eq!(
            recommend_consequence(&medium),
            Some(AbuseConsequence::RequireCosigner)
        );

        // High confidence → suspend filing
        let high = vec![AbuseIndicator {
            pattern: AbusePattern::SerialFiling,
            evidence: "test".into(),
            detected_at: Utc::now(),
            confidence: 0.85,
        }];
        assert_eq!(
            recommend_consequence(&high),
            Some(AbuseConsequence::SuspendFiling)
        );

        // Many indicators → public identification
        let many: Vec<_> = (0..3)
            .map(|_| AbuseIndicator {
                pattern: AbusePattern::SerialFiling,
                evidence: "test".into(),
                detected_at: Utc::now(),
                confidence: 0.5,
            })
            .collect();
        assert_eq!(
            recommend_consequence(&many),
            Some(AbuseConsequence::PublicIdentification)
        );
    }

    #[test]
    fn rate_limit_passes_under_threshold() {
        let config = JailConfig::default();
        let flags = vec![make_recent_flag("alice", "bob")];
        assert!(check_rate_limit("alice", &flags, &config).is_ok());
    }

    #[test]
    fn rate_limit_fails_at_threshold() {
        let config = JailConfig {
            max_flags_per_pubkey_per_day: 2,
            ..JailConfig::default()
        };
        let flags = vec![
            make_recent_flag("alice", "bob"),
            make_recent_flag("alice", "carol"),
        ];
        assert!(check_rate_limit("alice", &flags, &config).is_err());
    }

    #[test]
    fn abuse_consequence_ordering() {
        assert!(AbuseConsequence::Warning < AbuseConsequence::RequireCosigner);
        assert!(AbuseConsequence::RequireCosigner < AbuseConsequence::SuspendFiling);
        assert!(AbuseConsequence::SuspendFiling < AbuseConsequence::PublicIdentification);
    }
}
