//! Duty to warn — inter-community notification when patterns emerge.
//!
//! From Constellation Art. 7 §4: "Communities responding to breaches shall
//! coordinate their actions through transparent assemblies and shared
//! communication."
//!
//! When a cross-community pattern is established (2+ communities flag the same
//! person), affected communities have a duty to warn others.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::pattern::FlagPattern;

/// A duty-to-warn notification issued to other communities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DutyToWarn {
    /// Unique warning identifier.
    pub id: Uuid,
    /// The community issuing the warning.
    pub source_community: String,
    /// The person the warning is about.
    pub flagged_pubkey: String,
    /// The detected pattern that triggered the warning.
    pub pattern_summary: PatternSummary,
    /// Communities that have been warned.
    pub warning_records: Vec<WarningRecord>,
    /// When the warning was issued.
    pub issued_at: DateTime<Utc>,
}

/// Summary of the pattern for inclusion in warnings.
/// (Avoids exposing full flag details to non-involved communities.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternSummary {
    /// Number of distinct communities that flagged this person.
    pub distinct_communities: usize,
    /// Total number of flags.
    pub total_flags: usize,
    /// Whether the pattern is established.
    pub pattern_established: bool,
}

impl From<&FlagPattern> for PatternSummary {
    fn from(pattern: &FlagPattern) -> Self {
        Self {
            distinct_communities: pattern.distinct_communities,
            total_flags: pattern.total_flags,
            pattern_established: pattern.pattern_established,
        }
    }
}

/// Record of a single community being warned.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WarningRecord {
    /// The community that was warned.
    pub target_community: String,
    /// When the warning was received.
    pub received_at: DateTime<Utc>,
    /// Whether the community acknowledged the warning.
    pub acknowledged: bool,
    /// When acknowledgment occurred.
    pub acknowledged_at: Option<DateTime<Utc>>,
}

impl DutyToWarn {
    /// Issue a warning based on a detected pattern.
    pub fn issue(
        source_community: impl Into<String>,
        flagged_pubkey: impl Into<String>,
        pattern: &FlagPattern,
        target_communities: &[String],
    ) -> Self {
        let now = Utc::now();
        let records = target_communities
            .iter()
            .map(|community| WarningRecord {
                target_community: community.clone(),
                received_at: now,
                acknowledged: false,
                acknowledged_at: None,
            })
            .collect();

        Self {
            id: Uuid::new_v4(),
            source_community: source_community.into(),
            flagged_pubkey: flagged_pubkey.into(),
            pattern_summary: PatternSummary::from(pattern),
            warning_records: records,
            issued_at: now,
        }
    }

    /// Mark a community's warning as acknowledged.
    pub fn acknowledge(&mut self, community: &str) -> bool {
        if let Some(record) = self
            .warning_records
            .iter_mut()
            .find(|r| r.target_community == community)
        {
            record.acknowledged = true;
            record.acknowledged_at = Some(Utc::now());
            true
        } else {
            false
        }
    }

    /// How many communities have acknowledged the warning.
    pub fn acknowledgment_count(&self) -> usize {
        self.warning_records.iter().filter(|r| r.acknowledged).count()
    }

    /// Whether all warned communities have acknowledged.
    pub fn fully_acknowledged(&self) -> bool {
        self.warning_records.iter().all(|r| r.acknowledged)
    }

    /// Issue a warning filtered by federation scope.
    ///
    /// Only target communities visible in the scope receive the warning.
    /// Communities that have defederated won't see warnings from this source.
    pub fn issue_scoped(
        source_community: impl Into<String>,
        flagged_pubkey: impl Into<String>,
        pattern: &FlagPattern,
        target_communities: &[String],
        scope: &crate::federation_scope::FederationScope,
    ) -> Self {
        let visible_targets: Vec<String> = target_communities
            .iter()
            .filter(|c| scope.is_visible(c))
            .cloned()
            .collect();

        Self::issue(source_community, flagged_pubkey, pattern, &visible_targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pattern(distinct: usize, total: usize) -> FlagPattern {
        FlagPattern {
            flagged_pubkey: "bob".into(),
            flag_ids: vec![],
            distinct_communities: distinct,
            pattern_established: distinct >= 2,
            established_at: Some(Utc::now()),
            total_flags: total,
        }
    }

    #[test]
    fn issue_warning() {
        let pattern = make_pattern(3, 5);
        let targets = vec!["comm_a".to_string(), "comm_b".to_string()];
        let warning = DutyToWarn::issue("comm_source", "bob", &pattern, &targets);

        assert_eq!(warning.source_community, "comm_source");
        assert_eq!(warning.flagged_pubkey, "bob");
        assert_eq!(warning.warning_records.len(), 2);
        assert_eq!(warning.pattern_summary.distinct_communities, 3);
        assert_eq!(warning.pattern_summary.total_flags, 5);
    }

    #[test]
    fn acknowledge_warning() {
        let pattern = make_pattern(2, 3);
        let targets = vec!["comm_a".to_string(), "comm_b".to_string()];
        let mut warning = DutyToWarn::issue("comm_source", "bob", &pattern, &targets);

        assert_eq!(warning.acknowledgment_count(), 0);
        assert!(!warning.fully_acknowledged());

        assert!(warning.acknowledge("comm_a"));
        assert_eq!(warning.acknowledgment_count(), 1);
        assert!(!warning.fully_acknowledged());

        assert!(warning.acknowledge("comm_b"));
        assert_eq!(warning.acknowledgment_count(), 2);
        assert!(warning.fully_acknowledged());
    }

    #[test]
    fn acknowledge_unknown_community() {
        let pattern = make_pattern(2, 2);
        let targets = vec!["comm_a".to_string()];
        let mut warning = DutyToWarn::issue("source", "bob", &pattern, &targets);

        assert!(!warning.acknowledge("unknown_community"));
    }

    #[test]
    fn pattern_summary_from_pattern() {
        let pattern = make_pattern(4, 10);
        let summary = PatternSummary::from(&pattern);
        assert_eq!(summary.distinct_communities, 4);
        assert_eq!(summary.total_flags, 10);
        assert!(summary.pattern_established);
    }

    #[test]
    fn warning_serialization_roundtrip() {
        let pattern = make_pattern(2, 3);
        let warning = DutyToWarn::issue("source", "bob", &pattern, &["comm_a".to_string()]);
        let json = serde_json::to_string(&warning).unwrap();
        let deserialized: DutyToWarn = serde_json::from_str(&json).unwrap();
        assert_eq!(warning, deserialized);
    }

    // -----------------------------------------------------------------------
    // Federation-scoped warning tests
    // -----------------------------------------------------------------------

    #[test]
    fn issue_scoped_filters_defederated_targets() {
        use crate::federation_scope::FederationScope;

        let pattern = make_pattern(3, 5);
        let targets = vec![
            "comm_a".to_string(),
            "comm_b".to_string(),
            "comm_defederated".to_string(),
        ];

        let scope = FederationScope::from_communities(["comm_a", "comm_b"]);
        let warning = DutyToWarn::issue_scoped("source", "bob", &pattern, &targets, &scope);

        assert_eq!(warning.warning_records.len(), 2);
        let warned: Vec<&str> = warning
            .warning_records
            .iter()
            .map(|r| r.target_community.as_str())
            .collect();
        assert!(warned.contains(&"comm_a"));
        assert!(warned.contains(&"comm_b"));
        assert!(!warned.contains(&"comm_defederated"));
    }

    #[test]
    fn issue_scoped_unrestricted_warns_all() {
        use crate::federation_scope::FederationScope;

        let pattern = make_pattern(2, 3);
        let targets = vec!["comm_a".to_string(), "comm_b".to_string()];

        let scope = FederationScope::new(); // unrestricted
        let warning = DutyToWarn::issue_scoped("source", "bob", &pattern, &targets, &scope);
        assert_eq!(warning.warning_records.len(), 2);
    }
}
