//! Network intelligence queries — BFS traversal collecting verifications and flags.
//!
//! Port of AuthBook's `findVerificationsInNetwork` and `findFlagsInNetwork`.
//! Queries traverse the trust graph from a querier's perspective, collecting
//! all visible information about a target within N degrees of separation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::JailConfig;
use crate::flag::types::{AccountabilityFlag, FlagCategory, FlagSeverity};
use crate::trust_graph::edge::VerificationSentiment;
use crate::trust_graph::graph::TrustGraph;
use crate::trust_graph::pattern::{analyze_pattern, VerificationPattern};
use crate::trust_graph::recommendation::{generate_recommendation, TrustRecommendation};

/// A verification visible from the querier's network perspective.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkVerification {
    /// Who performed the verification.
    pub verifier_pubkey: String,
    /// How the verifier felt about it.
    pub sentiment: VerificationSentiment,
    /// Graph distance from querier to verifier.
    pub degree_from_querier: usize,
    /// When the verification occurred.
    pub verified_at: DateTime<Utc>,
}

/// A flag visible from the querier's network perspective.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkFlag {
    /// The flag's unique ID.
    pub flag_id: Uuid,
    /// Who raised the flag.
    pub flagger_pubkey: String,
    /// Category of concern.
    pub category: FlagCategory,
    /// Severity level.
    pub severity: FlagSeverity,
    /// Graph distance from querier to flagger.
    pub degree_from_querier: usize,
}

/// Full safety profile for a person, as seen from a querier's network position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkIntelligence {
    /// Target person being queried about.
    pub about_pubkey: String,
    /// When this query was computed.
    pub queried_at: DateTime<Utc>,
    /// Shortest path to target in the trust graph (None if not reachable).
    pub closest_degree: Option<usize>,
    /// Number of verifications visible about the target.
    pub verification_count: usize,
    /// Number of flags visible about the target.
    pub flag_count: usize,
    /// Derived verification pattern.
    pub pattern: VerificationPattern,
    /// Safety recommendation.
    pub recommendation: TrustRecommendation,
    /// Visible verifications (detailed).
    pub verifications: Vec<NetworkVerification>,
    /// Visible flags (detailed).
    pub flags: Vec<NetworkFlag>,
}

/// Find all verifications about `target` visible from `querier`'s network within
/// `max_degrees` hops.
///
/// BFS from querier. At each visited node, check if that node has verified
/// the target. Collect matches with their degree from querier.
pub fn query_verifications(
    graph: &TrustGraph,
    querier: &str,
    target: &str,
    max_degrees: usize,
) -> Vec<NetworkVerification> {
    let visited = graph.bfs_traverse(querier, max_degrees);
    let mut results = Vec::new();

    for (pubkey, degree) in &visited {
        for edge in graph.edges_from(pubkey) {
            if edge.verified_pubkey == target {
                results.push(NetworkVerification {
                    verifier_pubkey: pubkey.clone(),
                    sentiment: edge.sentiment,
                    degree_from_querier: *degree,
                    verified_at: edge.verified_at,
                });
            }
        }
    }

    results
}

/// Find all flags about `target` visible from `querier`'s network within
/// `propagation_degrees` hops.
///
/// BFS from querier. At each visited node, check if that node has flagged
/// the target. Collect matches with their degree from querier.
pub fn query_flags(
    graph: &TrustGraph,
    querier: &str,
    target: &str,
    all_flags: &[AccountabilityFlag],
    propagation_degrees: usize,
) -> Vec<NetworkFlag> {
    let visited = graph.bfs_traverse(querier, propagation_degrees);
    let mut results = Vec::new();

    for (pubkey, degree) in &visited {
        for flag in all_flags {
            if flag.flagger_pubkey == *pubkey && flag.flagged_pubkey == target {
                results.push(NetworkFlag {
                    flag_id: flag.id,
                    flagger_pubkey: pubkey.clone(),
                    category: flag.category,
                    severity: flag.severity,
                    degree_from_querier: *degree,
                });
            }
        }
    }

    results
}

/// Build a complete safety profile for `target` as seen from `querier`.
///
/// Combines BFS verification query, flag query, pattern analysis, and
/// recommendation generation into a single `NetworkIntelligence` result.
pub fn query_intelligence(
    graph: &TrustGraph,
    querier: &str,
    target: &str,
    all_flags: &[AccountabilityFlag],
    config: &JailConfig,
) -> NetworkIntelligence {
    let verifications = query_verifications(graph, querier, target, config.max_query_degrees);
    let flags = query_flags(
        graph,
        querier,
        target,
        all_flags,
        config.flag_propagation_degrees,
    );

    let closest_degree = verifications
        .iter()
        .map(|v| v.degree_from_querier)
        .min();

    let pattern = analyze_pattern(&verifications, &flags);
    let recommendation = generate_recommendation(pattern, closest_degree, flags.len());

    NetworkIntelligence {
        about_pubkey: target.to_string(),
        queried_at: Utc::now(),
        closest_degree,
        verification_count: verifications.len(),
        flag_count: flags.len(),
        pattern,
        recommendation,
        verifications,
        flags,
    }
}

/// Find all flags about `target` visible from `querier`'s network, filtered
/// by federation scope.
///
/// Flags without a `community_id` are always included (personal, not
/// community-scoped). Flags from defederated communities are excluded.
pub fn query_flags_scoped(
    graph: &TrustGraph,
    querier: &str,
    target: &str,
    all_flags: &[AccountabilityFlag],
    propagation_degrees: usize,
    scope: &crate::federation_scope::FederationScope,
) -> Vec<NetworkFlag> {
    if scope.is_unrestricted() {
        return query_flags(graph, querier, target, all_flags, propagation_degrees);
    }

    let visible_flags: Vec<&AccountabilityFlag> = all_flags
        .iter()
        .filter(|f| scope.is_visible_opt(f.community_id.as_deref()))
        .collect();

    let visited = graph.bfs_traverse(querier, propagation_degrees);
    let mut results = Vec::new();

    for (pubkey, degree) in &visited {
        for flag in &visible_flags {
            if flag.flagger_pubkey == *pubkey && flag.flagged_pubkey == target {
                results.push(NetworkFlag {
                    flag_id: flag.id,
                    flagger_pubkey: pubkey.clone(),
                    category: flag.category,
                    severity: flag.severity,
                    degree_from_querier: *degree,
                });
            }
        }
    }

    results
}

/// Build a complete safety profile for `target` as seen from `querier`,
/// filtered by federation scope.
///
/// Identical to `query_intelligence` but uses `query_flags_scoped` so
/// flags from defederated communities are excluded from the intelligence
/// report.
pub fn query_intelligence_scoped(
    graph: &TrustGraph,
    querier: &str,
    target: &str,
    all_flags: &[AccountabilityFlag],
    config: &JailConfig,
    scope: &crate::federation_scope::FederationScope,
) -> NetworkIntelligence {
    let verifications = query_verifications(graph, querier, target, config.max_query_degrees);
    let flags = query_flags_scoped(
        graph,
        querier,
        target,
        all_flags,
        config.flag_propagation_degrees,
        scope,
    );

    let closest_degree = verifications
        .iter()
        .map(|v| v.degree_from_querier)
        .min();

    let pattern = analyze_pattern(&verifications, &flags);
    let recommendation = generate_recommendation(pattern, closest_degree, flags.len());

    NetworkIntelligence {
        about_pubkey: target.to_string(),
        queried_at: Utc::now(),
        closest_degree,
        verification_count: verifications.len(),
        flag_count: flags.len(),
        pattern,
        recommendation,
        verifications,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust_graph::edge::VerificationEdge;

    fn make_edge(verifier: &str, verified: &str) -> VerificationEdge {
        VerificationEdge::new(
            verifier,
            verified,
            "mutual_vouch",
            VerificationSentiment::Positive,
            0.9,
        )
    }

    fn make_flag(flagger: &str, flagged: &str, severity: FlagSeverity) -> AccountabilityFlag {
        AccountabilityFlag::raise(
            flagger,
            flagged,
            FlagCategory::SuspiciousActivity,
            severity,
            "test flag",
        )
    }

    #[test]
    fn query_verifications_linear_chain() {
        // alice → bob → carol → dave
        // Query: alice asks about dave (3 degrees away)
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();
        graph.add_edge(make_edge("carol", "dave")).unwrap();

        let results = query_verifications(&graph, "alice", "dave", 3);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].verifier_pubkey, "carol");
        assert_eq!(results[0].degree_from_querier, 2);
    }

    #[test]
    fn query_verifications_beyond_depth() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();
        graph.add_edge(make_edge("carol", "dave")).unwrap();

        // Only 1 degree — can't see carol verifying dave
        let results = query_verifications(&graph, "alice", "dave", 1);
        assert!(results.is_empty());
    }

    #[test]
    fn query_flags_in_network() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();

        let flags = vec![
            make_flag("bob", "dave", FlagSeverity::Medium),
            make_flag("carol", "dave", FlagSeverity::High),
            make_flag("eve", "dave", FlagSeverity::Low), // eve not in network
        ];

        let results = query_flags(&graph, "alice", "dave", &flags, 3);
        assert_eq!(results.len(), 2); // bob and carol visible, eve is not
    }

    #[test]
    fn query_intelligence_full() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();
        graph.add_edge(make_edge("carol", "dave")).unwrap();

        let flags: Vec<AccountabilityFlag> = vec![];
        let config = JailConfig::default();

        let intel = query_intelligence(&graph, "alice", "dave", &flags, &config);
        assert_eq!(intel.about_pubkey, "dave");
        assert_eq!(intel.verification_count, 1);
        assert_eq!(intel.flag_count, 0);
        assert_eq!(intel.closest_degree, Some(2));
        assert_eq!(intel.pattern, VerificationPattern::Limited);
        assert_eq!(intel.recommendation, TrustRecommendation::Caution);
    }

    #[test]
    fn intelligence_with_flags() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();

        let flags = vec![make_flag("bob", "dave", FlagSeverity::Critical)];
        let config = JailConfig::default();

        let intel = query_intelligence(&graph, "alice", "dave", &flags, &config);
        assert_eq!(intel.flag_count, 1);
        assert_eq!(intel.pattern, VerificationPattern::Flagged);
        assert_eq!(intel.recommendation, TrustRecommendation::Avoid);
    }

    #[test]
    fn intelligence_no_connection() {
        let graph = TrustGraph::new();
        let flags: Vec<AccountabilityFlag> = vec![];
        let config = JailConfig::default();

        let intel = query_intelligence(&graph, "alice", "dave", &flags, &config);
        assert_eq!(intel.verification_count, 0);
        assert_eq!(intel.closest_degree, None);
        assert_eq!(intel.pattern, VerificationPattern::Isolated);
        assert_eq!(intel.recommendation, TrustRecommendation::PublicOnly);
    }

    // -----------------------------------------------------------------------
    // Federation-scoped query tests
    // -----------------------------------------------------------------------

    fn make_flag_with_community(
        flagger: &str,
        flagged: &str,
        severity: FlagSeverity,
        community: &str,
    ) -> AccountabilityFlag {
        AccountabilityFlag::raise(
            flagger,
            flagged,
            FlagCategory::SuspiciousActivity,
            severity,
            "test flag",
        )
        .with_community(community)
    }

    #[test]
    fn query_flags_scoped_filters_defederated_community() {
        use crate::federation_scope::FederationScope;

        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("alice", "carol")).unwrap();

        let flags = vec![
            make_flag_with_community("bob", "dave", FlagSeverity::High, "comm_a"),
            make_flag_with_community("carol", "dave", FlagSeverity::Critical, "comm_b"),
        ];

        // Only comm_a is federated — comm_b's critical flag should be invisible.
        let scope = FederationScope::from_communities(["comm_a"]);
        let results = query_flags_scoped(&graph, "alice", "dave", &flags, 3, &scope);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, FlagSeverity::High);
    }

    #[test]
    fn query_flags_scoped_unrestricted_returns_all() {
        use crate::federation_scope::FederationScope;

        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("alice", "carol")).unwrap();

        let flags = vec![
            make_flag_with_community("bob", "dave", FlagSeverity::High, "comm_a"),
            make_flag_with_community("carol", "dave", FlagSeverity::Medium, "comm_b"),
        ];

        let scope = FederationScope::new(); // unrestricted
        let results = query_flags_scoped(&graph, "alice", "dave", &flags, 3, &scope);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_flags_scoped_includes_flags_without_community() {
        use crate::federation_scope::FederationScope;

        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();

        let flags = vec![
            // No community — personal flag, always visible.
            make_flag("bob", "dave", FlagSeverity::Medium),
            make_flag_with_community("bob", "dave", FlagSeverity::Low, "comm_defederated"),
        ];

        let scope = FederationScope::from_communities(["comm_a"]);
        let results = query_flags_scoped(&graph, "alice", "dave", &flags, 3, &scope);
        // Personal flag visible, defederated community flag hidden.
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, FlagSeverity::Medium);
    }

    #[test]
    fn query_intelligence_scoped_affects_recommendation() {
        use crate::federation_scope::FederationScope;

        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();

        let flags = vec![
            make_flag_with_community("bob", "dave", FlagSeverity::Critical, "comm_defederated"),
        ];
        let config = JailConfig::default();

        // Without scope: critical flag visible → Flagged → Avoid.
        let intel_all = query_intelligence(&graph, "alice", "dave", &flags, &config);
        assert_eq!(intel_all.pattern, VerificationPattern::Flagged);
        assert_eq!(intel_all.recommendation, TrustRecommendation::Avoid);

        // With scope excluding that community: no flags → different pattern.
        let scope = FederationScope::from_communities(["comm_federated"]);
        let intel_scoped =
            query_intelligence_scoped(&graph, "alice", "dave", &flags, &config, &scope);
        assert_eq!(intel_scoped.flag_count, 0);
        assert_ne!(intel_scoped.recommendation, TrustRecommendation::Avoid);
    }
}
