//! Community admission checks — should this person be admitted?
//!
//! Port of AuthBook's `checkCommunityAdmission`. Uses the trust graph
//! and flag data to recommend an admission action.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::JailConfig;
use crate::flag::types::{AccountabilityFlag, FlagSeverity};
use crate::trust_graph::graph::TrustGraph;

/// Result of checking a prospect for community admission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdmissionRecommendation {
    /// The prospect being evaluated.
    pub prospect_pubkey: String,
    /// The community being applied to.
    pub community_id: String,
    /// Recommended action.
    pub action: AdmissionAction,
    /// Verifications from community members about the prospect.
    pub verification_count: usize,
    /// Flags visible from community members.
    pub flags_in_network: usize,
    /// Shortest path to a community member (None if no connection).
    pub closest_member_degree: Option<usize>,
    /// Reasons for the recommendation.
    pub reasons: Vec<String>,
    /// When this check was computed.
    pub computed_at: DateTime<Utc>,
}

/// Recommended admission action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AdmissionAction {
    /// Approved to join.
    Admit,
    /// Need more verifications from community members.
    RequireMoreVerifications,
    /// Manual review / interview needed.
    RequireInterview,
    /// Rejected due to safety concerns.
    Deny,
    /// Has flags — needs community review before decision.
    FlagForReview,
}

impl std::fmt::Display for AdmissionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admit => write!(f, "admit"),
            Self::RequireMoreVerifications => write!(f, "require_more_verifications"),
            Self::RequireInterview => write!(f, "require_interview"),
            Self::Deny => write!(f, "deny"),
            Self::FlagForReview => write!(f, "flag_for_review"),
        }
    }
}

/// Check whether a prospect should be admitted to a community.
///
/// Decision tree (from AuthBook):
/// 1. Any HIGH/CRITICAL flags from community members → Deny
/// 2. Any flags from community members → FlagForReview
/// 3. Insufficient verifications → RequireMoreVerifications
/// 4. No direct verifications → RequireInterview
/// 5. Otherwise → Admit
pub fn check_admission(
    graph: &TrustGraph,
    prospect: &str,
    community_id: &str,
    community_members: &[String],
    all_flags: &[AccountabilityFlag],
    config: &JailConfig,
) -> AdmissionRecommendation {
    let mut reasons = Vec::new();
    let mut verification_count = 0;
    let mut closest_degree: Option<usize> = None;

    // Count verifications from community members about the prospect
    for member in community_members {
        let edges = graph.edges_from(member);
        for edge in edges {
            if edge.verified_pubkey == prospect {
                verification_count += 1;
                let degree = 1; // direct verification = degree 1
                closest_degree = Some(closest_degree.map_or(degree, |d: usize| d.min(degree)));
            }
        }
    }

    // Check flags from community members against the prospect
    let member_flags: Vec<&AccountabilityFlag> = all_flags
        .iter()
        .filter(|f| {
            f.flagged_pubkey == prospect
                && community_members.contains(&f.flagger_pubkey)
        })
        .collect();

    let flags_in_network = member_flags.len();

    // Decision tree
    let has_severe_flags = member_flags
        .iter()
        .any(|f| f.severity >= FlagSeverity::High);

    let action = if has_severe_flags {
        reasons.push("HIGH or CRITICAL flags from community members".into());
        AdmissionAction::Deny
    } else if !member_flags.is_empty() {
        reasons.push(format!(
            "{} flag(s) from community members require review",
            member_flags.len()
        ));
        AdmissionAction::FlagForReview
    } else if verification_count < config.admission_min_verifications {
        reasons.push(format!(
            "Need {} verification(s) from community members, have {}",
            config.admission_min_verifications, verification_count
        ));
        AdmissionAction::RequireMoreVerifications
    } else if closest_degree.is_none() {
        reasons.push("No direct verifications from community members".into());
        AdmissionAction::RequireInterview
    } else {
        reasons.push(format!(
            "{} verification(s) from community members, no flags",
            verification_count
        ));
        AdmissionAction::Admit
    };

    AdmissionRecommendation {
        prospect_pubkey: prospect.to_string(),
        community_id: community_id.to_string(),
        action,
        verification_count,
        flags_in_network,
        closest_member_degree: closest_degree,
        reasons,
        computed_at: Utc::now(),
    }
}

/// Check admission filtered by federation scope.
///
/// Flags from defederated communities are excluded from the admission
/// decision. Verifications in the trust graph are NOT filtered — graph
/// edges don't carry community_id (they are person-to-person).
pub fn check_admission_scoped(
    graph: &TrustGraph,
    prospect: &str,
    community_id: &str,
    community_members: &[String],
    all_flags: &[AccountabilityFlag],
    config: &JailConfig,
    scope: &crate::federation_scope::FederationScope,
) -> AdmissionRecommendation {
    if scope.is_unrestricted() {
        return check_admission(graph, prospect, community_id, community_members, all_flags, config);
    }

    let visible_flags: Vec<AccountabilityFlag> = all_flags
        .iter()
        .filter(|f| scope.is_visible_opt(f.community_id.as_deref()))
        .cloned()
        .collect();

    check_admission(graph, prospect, community_id, community_members, &visible_flags, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flag::types::FlagCategory;
    use crate::trust_graph::edge::{VerificationEdge, VerificationSentiment};

    fn make_edge(verifier: &str, verified: &str) -> VerificationEdge {
        VerificationEdge::new(verifier, verified, "vouch", VerificationSentiment::Positive, 0.9)
    }

    fn make_flag(flagger: &str, flagged: &str, severity: FlagSeverity) -> AccountabilityFlag {
        AccountabilityFlag::raise(flagger, flagged, FlagCategory::SuspiciousActivity, severity, "test")
    }

    #[test]
    fn admit_with_verifications() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("member_alice", "prospect")).unwrap();

        let members = vec!["member_alice".to_string()];
        let flags: Vec<AccountabilityFlag> = vec![];
        let config = JailConfig::default();

        let result = check_admission(&graph, "prospect", "comm_1", &members, &flags, &config);
        assert_eq!(result.action, AdmissionAction::Admit);
        assert_eq!(result.verification_count, 1);
    }

    #[test]
    fn deny_with_severe_flags() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("member_alice", "prospect")).unwrap();

        let members = vec!["member_alice".to_string()];
        let flags = vec![make_flag("member_alice", "prospect", FlagSeverity::Critical)];
        let config = JailConfig::default();

        let result = check_admission(&graph, "prospect", "comm_1", &members, &flags, &config);
        assert_eq!(result.action, AdmissionAction::Deny);
    }

    #[test]
    fn flag_for_review_with_low_flags() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("member_alice", "prospect")).unwrap();

        let members = vec!["member_alice".to_string()];
        let flags = vec![make_flag("member_alice", "prospect", FlagSeverity::Low)];
        let config = JailConfig::default();

        let result = check_admission(&graph, "prospect", "comm_1", &members, &flags, &config);
        assert_eq!(result.action, AdmissionAction::FlagForReview);
    }

    #[test]
    fn require_more_verifications() {
        let graph = TrustGraph::new(); // no edges
        let members = vec!["member_alice".to_string()];
        let flags: Vec<AccountabilityFlag> = vec![];
        let config = JailConfig {
            admission_min_verifications: 2,
            ..JailConfig::default()
        };

        let result = check_admission(&graph, "prospect", "comm_1", &members, &flags, &config);
        assert_eq!(result.action, AdmissionAction::RequireMoreVerifications);
    }

    #[test]
    fn non_member_flags_ignored() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("member_alice", "prospect")).unwrap();

        let members = vec!["member_alice".to_string()];
        // Flag from non-member — should be ignored
        let flags = vec![make_flag("outsider", "prospect", FlagSeverity::Critical)];
        let config = JailConfig::default();

        let result = check_admission(&graph, "prospect", "comm_1", &members, &flags, &config);
        assert_eq!(result.action, AdmissionAction::Admit);
        assert_eq!(result.flags_in_network, 0);
    }

    #[test]
    fn admission_serialization_roundtrip() {
        let graph = TrustGraph::new();
        let config = JailConfig::default();
        let result = check_admission(&graph, "prospect", "comm", &[], &[], &config);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: AdmissionRecommendation = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    // -----------------------------------------------------------------------
    // Federation-scoped admission tests
    // -----------------------------------------------------------------------

    #[test]
    fn admission_scoped_hides_defederated_flags() {
        use crate::federation_scope::FederationScope;

        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("member_alice", "prospect")).unwrap();

        let members = vec!["member_alice".to_string()];
        // Critical flag from a defederated community.
        let mut flag = make_flag("member_alice", "prospect", FlagSeverity::Critical);
        flag.community_id = Some("defederated_comm".to_string());
        let flags = vec![flag];
        let config = JailConfig::default();

        // Without scope: denied due to critical flag.
        let result = check_admission(
            &graph, "prospect", "comm_1", &members, &flags, &config,
        );
        assert_eq!(result.action, AdmissionAction::Deny);

        // With scope excluding that community: flag invisible, admitted.
        let scope = FederationScope::from_communities(["comm_1"]);
        let result_scoped = check_admission_scoped(
            &graph, "prospect", "comm_1", &members, &flags, &config, &scope,
        );
        assert_eq!(result_scoped.action, AdmissionAction::Admit);
        assert_eq!(result_scoped.flags_in_network, 0);
    }

    #[test]
    fn admission_scoped_unrestricted_matches_unscoped() {
        use crate::federation_scope::FederationScope;

        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("member_alice", "prospect")).unwrap();

        let members = vec!["member_alice".to_string()];
        let mut flag = make_flag("member_alice", "prospect", FlagSeverity::Critical);
        flag.community_id = Some("some_comm".to_string());
        let flags = vec![flag];
        let config = JailConfig::default();

        let result = check_admission(
            &graph, "prospect", "comm_1", &members, &flags, &config,
        );
        let scope = FederationScope::new();
        let result_scoped = check_admission_scoped(
            &graph, "prospect", "comm_1", &members, &flags, &config, &scope,
        );
        assert_eq!(result.action, result_scoped.action);
    }
}
