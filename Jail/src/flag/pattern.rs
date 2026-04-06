//! Cross-community flag pattern detection.
//!
//! A pattern is established when 2+ distinct communities have flagged the same
//! person. This is the threshold for inter-community warnings (duty to warn).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::types::AccountabilityFlag;
use crate::config::JailConfig;

/// A detected pattern of flags across communities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlagPattern {
    /// The person who has been flagged.
    pub flagged_pubkey: String,
    /// All flags contributing to this pattern.
    pub flag_ids: Vec<uuid::Uuid>,
    /// Number of distinct communities that have flagged this person.
    pub distinct_communities: usize,
    /// Whether the pattern threshold has been met.
    pub pattern_established: bool,
    /// When the pattern was first established (if ever).
    pub established_at: Option<DateTime<Utc>>,
    /// Total flags across all communities.
    pub total_flags: usize,
}

/// Detect cross-community flag patterns for a specific person.
///
/// Counts distinct community IDs across all flags against the target.
/// A pattern is established when `distinct_communities >= config.pattern_threshold_communities`.
pub fn detect_pattern(
    flags: &[AccountabilityFlag],
    flagged_pubkey: &str,
    config: &JailConfig,
) -> FlagPattern {
    let relevant: Vec<&AccountabilityFlag> = flags
        .iter()
        .filter(|f| f.flagged_pubkey == flagged_pubkey)
        .collect();

    let communities: HashSet<&str> = relevant
        .iter()
        .filter_map(|f| f.community_id.as_deref())
        .collect();

    let distinct = communities.len();
    let established = distinct >= config.pattern_threshold_communities;

    FlagPattern {
        flagged_pubkey: flagged_pubkey.to_string(),
        flag_ids: relevant.iter().map(|f| f.id).collect(),
        distinct_communities: distinct,
        pattern_established: established,
        established_at: if established { Some(Utc::now()) } else { None },
        total_flags: relevant.len(),
    }
}

/// Scan all flags and detect patterns for every flagged person.
pub fn detect_all_patterns(
    flags: &[AccountabilityFlag],
    config: &JailConfig,
) -> Vec<FlagPattern> {
    let flagged_pubkeys: HashSet<&str> = flags.iter().map(|f| f.flagged_pubkey.as_str()).collect();

    flagged_pubkeys
        .into_iter()
        .map(|pubkey| detect_pattern(flags, pubkey, config))
        .filter(|p| p.pattern_established)
        .collect()
}

/// Detect cross-community flag patterns for a specific person, filtered
/// by federation scope.
///
/// Flags from defederated communities are excluded from pattern detection.
/// Flags without a `community_id` are always included.
pub fn detect_pattern_scoped(
    flags: &[AccountabilityFlag],
    flagged_pubkey: &str,
    config: &JailConfig,
    scope: &crate::federation_scope::FederationScope,
) -> FlagPattern {
    if scope.is_unrestricted() {
        return detect_pattern(flags, flagged_pubkey, config);
    }

    let visible: Vec<AccountabilityFlag> = flags
        .iter()
        .filter(|f| scope.is_visible_opt(f.community_id.as_deref()))
        .cloned()
        .collect();

    detect_pattern(&visible, flagged_pubkey, config)
}

/// Scan all flags and detect patterns for every flagged person, filtered
/// by federation scope.
pub fn detect_all_patterns_scoped(
    flags: &[AccountabilityFlag],
    config: &JailConfig,
    scope: &crate::federation_scope::FederationScope,
) -> Vec<FlagPattern> {
    if scope.is_unrestricted() {
        return detect_all_patterns(flags, config);
    }

    let visible: Vec<AccountabilityFlag> = flags
        .iter()
        .filter(|f| scope.is_visible_opt(f.community_id.as_deref()))
        .cloned()
        .collect();

    detect_all_patterns(&visible, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flag::types::{FlagCategory, FlagSeverity};

    fn make_flag(flagger: &str, flagged: &str, community: Option<&str>) -> AccountabilityFlag {
        let mut flag = AccountabilityFlag::raise(
            flagger,
            flagged,
            FlagCategory::SuspiciousActivity,
            FlagSeverity::Medium,
            "test",
        );
        flag.community_id = community.map(String::from);
        flag
    }

    #[test]
    fn no_flags_no_pattern() {
        let config = JailConfig::default();
        let pattern = detect_pattern(&[], "bob", &config);
        assert!(!pattern.pattern_established);
        assert_eq!(pattern.distinct_communities, 0);
        assert_eq!(pattern.total_flags, 0);
    }

    #[test]
    fn single_community_no_pattern() {
        let config = JailConfig::default();
        let flags = vec![
            make_flag("alice", "bob", Some("comm_1")),
            make_flag("carol", "bob", Some("comm_1")),
        ];
        let pattern = detect_pattern(&flags, "bob", &config);
        assert!(!pattern.pattern_established);
        assert_eq!(pattern.distinct_communities, 1);
        assert_eq!(pattern.total_flags, 2);
    }

    #[test]
    fn two_communities_establishes_pattern() {
        let config = JailConfig::default();
        let flags = vec![
            make_flag("alice", "bob", Some("comm_1")),
            make_flag("carol", "bob", Some("comm_2")),
        ];
        let pattern = detect_pattern(&flags, "bob", &config);
        assert!(pattern.pattern_established);
        assert_eq!(pattern.distinct_communities, 2);
        assert_eq!(pattern.total_flags, 2);
        assert!(pattern.established_at.is_some());
    }

    #[test]
    fn flags_without_community_not_counted() {
        let config = JailConfig::default();
        let flags = vec![
            make_flag("alice", "bob", None), // no community
            make_flag("carol", "bob", Some("comm_1")),
        ];
        let pattern = detect_pattern(&flags, "bob", &config);
        assert!(!pattern.pattern_established);
        assert_eq!(pattern.distinct_communities, 1);
        assert_eq!(pattern.total_flags, 2); // both flags counted, but only 1 community
    }

    #[test]
    fn three_communities() {
        let config = JailConfig::default();
        let flags = vec![
            make_flag("a", "bob", Some("comm_1")),
            make_flag("b", "bob", Some("comm_2")),
            make_flag("c", "bob", Some("comm_3")),
        ];
        let pattern = detect_pattern(&flags, "bob", &config);
        assert!(pattern.pattern_established);
        assert_eq!(pattern.distinct_communities, 3);
    }

    #[test]
    fn only_counts_target_flags() {
        let config = JailConfig::default();
        let flags = vec![
            make_flag("alice", "bob", Some("comm_1")),
            make_flag("carol", "bob", Some("comm_2")),
            make_flag("alice", "dave", Some("comm_3")), // different target
        ];
        let pattern = detect_pattern(&flags, "bob", &config);
        assert!(pattern.pattern_established);
        assert_eq!(pattern.total_flags, 2); // only bob's flags
    }

    #[test]
    fn detect_all_patterns_filters_unestablished() {
        let config = JailConfig::default();
        let flags = vec![
            make_flag("a", "bob", Some("comm_1")),
            make_flag("b", "bob", Some("comm_2")),
            make_flag("c", "dave", Some("comm_1")), // dave only 1 community
        ];
        let patterns = detect_all_patterns(&flags, &config);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].flagged_pubkey, "bob");
    }

    // -----------------------------------------------------------------------
    // Federation-scoped pattern tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_pattern_scoped_hides_defederated() {
        use crate::federation_scope::FederationScope;

        let config = JailConfig::default();
        let flags = vec![
            make_flag("alice", "bob", Some("comm_1")),
            make_flag("carol", "bob", Some("comm_2")),
            make_flag("dave", "bob", Some("comm_3")), // defederated
        ];

        // All three visible — pattern with 3 communities.
        let pattern_all = detect_pattern(&flags, "bob", &config);
        assert!(pattern_all.pattern_established);
        assert_eq!(pattern_all.distinct_communities, 3);

        // Only comm_1 and comm_2 federated — pattern with 2 communities.
        let scope = FederationScope::from_communities(["comm_1", "comm_2"]);
        let pattern_scoped = detect_pattern_scoped(&flags, "bob", &config, &scope);
        assert!(pattern_scoped.pattern_established);
        assert_eq!(pattern_scoped.distinct_communities, 2);
        assert_eq!(pattern_scoped.total_flags, 2);
    }

    #[test]
    fn detect_pattern_scoped_can_break_pattern() {
        use crate::federation_scope::FederationScope;

        let config = JailConfig::default();
        let flags = vec![
            make_flag("alice", "bob", Some("comm_1")),
            make_flag("carol", "bob", Some("comm_2")), // defederated
        ];

        // Unrestricted: 2 communities → established.
        let pattern_all = detect_pattern(&flags, "bob", &config);
        assert!(pattern_all.pattern_established);

        // Only comm_1 federated: 1 community → NOT established.
        let scope = FederationScope::from_communities(["comm_1"]);
        let pattern_scoped = detect_pattern_scoped(&flags, "bob", &config, &scope);
        assert!(!pattern_scoped.pattern_established);
        assert_eq!(pattern_scoped.distinct_communities, 1);
    }

    #[test]
    fn detect_all_patterns_scoped_filters() {
        use crate::federation_scope::FederationScope;

        let config = JailConfig::default();
        let flags = vec![
            make_flag("a", "bob", Some("comm_1")),
            make_flag("b", "bob", Some("comm_2")),
            make_flag("c", "dave", Some("comm_1")),
            make_flag("d", "dave", Some("comm_3")), // defederated for dave
        ];

        // Unrestricted: both bob and dave have patterns.
        let all = detect_all_patterns(&flags, &config);
        assert_eq!(all.len(), 2);

        // Only comm_1 and comm_2: bob still has pattern (2 communities).
        // Dave only has comm_1 — pattern broken.
        let scope = FederationScope::from_communities(["comm_1", "comm_2"]);
        let scoped = detect_all_patterns_scoped(&flags, &config, &scope);
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].flagged_pubkey, "bob");
    }
}
