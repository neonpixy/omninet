//! # Federation Scope -- Data Boundary for Observatory Operations
//!
//! From Constellation Art. 3 SS3 -- federation is a data boundary.
//!
//! When communities federate, they form a trust cluster. Undercroft's
//! observability respects these boundaries. Health snapshots, community
//! metrics, and network topology are scoped to the visible federation.
//!
//! `FederationScope` is an optional filter applied to Undercroft queries.
//! When empty (unrestricted), all communities and nodes are visible --
//! this is the default and preserves full backward compatibility.
//!
//! All scoped methods maintain Undercroft's core constraint: deidentified
//! aggregate data only. No pubkeys, no individual activity, no URLs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::community::CommunityHealth;
use crate::snapshot::HealthSnapshot;

/// Optional federation scope for observatory operations.
///
/// When set, health metrics and community data are scoped to federated
/// communities. When empty, all communities are visible.
///
/// From Constellation Art. 3 SS3 -- federation is a data boundary.
///
/// # Examples
///
/// ```
/// use undercroft::FederationScope;
///
/// // Unrestricted -- sees everything.
/// let open = FederationScope::new();
/// assert!(open.is_unrestricted());
/// assert!(open.is_visible("any_community"));
///
/// // Scoped to specific communities.
/// let scoped = FederationScope::from_communities(["alpha", "beta"]);
/// assert!(!scoped.is_unrestricted());
/// assert!(scoped.is_visible("alpha"));
/// assert!(!scoped.is_visible("gamma"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationScope {
    visible_communities: HashSet<String>,
}

impl Default for FederationScope {
    fn default() -> Self {
        Self::new()
    }
}

impl FederationScope {
    /// Create an unrestricted scope -- all communities are visible.
    pub fn new() -> Self {
        Self {
            visible_communities: HashSet::new(),
        }
    }

    /// Create a scope limited to the given communities.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Whether a community is visible within this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty) or if
    /// the community is in the visible set.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Whether this scope has no restrictions (all communities visible).
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the visible set.
    ///
    /// Returns 0 for unrestricted scopes -- this does NOT mean "no communities",
    /// it means "all communities". Use [`is_unrestricted`](Self::is_unrestricted)
    /// to distinguish.
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty (i.e., unrestricted).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Add a community to the visible set.
    ///
    /// Returns `true` if the community was newly inserted.
    pub fn add_community(&mut self, community_id: impl Into<String>) -> bool {
        self.visible_communities.insert(community_id.into())
    }

    /// Remove a community from the visible set.
    ///
    /// Returns `true` if the community was present and removed.
    pub fn remove_community(&mut self, community_id: &str) -> bool {
        self.visible_communities.remove(community_id)
    }

    // ── Community health scoped methods ─────────────────────────────

    /// Filter community health records to only those from visible communities.
    ///
    /// Uses the community_id on each `CommunityHealth` to determine visibility.
    /// When the scope is unrestricted, all records pass through.
    pub fn filter_communities_scoped<'a>(
        &self,
        communities: &'a [CommunityHealth],
    ) -> Vec<&'a CommunityHealth> {
        if self.is_unrestricted() {
            return communities.iter().collect();
        }
        communities
            .iter()
            .filter(|ch| self.visible_communities.contains(ch.community_id.to_string().as_str()))
            .collect()
    }

    /// Count the total members across visible communities.
    ///
    /// Returns the aggregate member count within the federation scope.
    pub fn total_members_scoped(&self, communities: &[CommunityHealth]) -> usize {
        if self.is_unrestricted() {
            return communities.iter().map(|ch| ch.member_count).sum();
        }
        communities
            .iter()
            .filter(|ch| self.visible_communities.contains(ch.community_id.to_string().as_str()))
            .map(|ch| ch.member_count)
            .sum()
    }

    /// Count communities with governance activity in the visible scope.
    ///
    /// A community has governance activity if it has any active or
    /// resolved proposals.
    pub fn communities_with_governance_scoped(
        &self,
        communities: &[CommunityHealth],
    ) -> usize {
        let filtered = self.filter_communities_scoped(communities);
        filtered
            .iter()
            .filter(|ch| {
                ch.governance.active_proposals > 0 || ch.governance.resolved_proposals > 0
            })
            .count()
    }

    /// Compute average governance participation across visible communities.
    ///
    /// Returns `None` if no communities with participation data exist in scope.
    pub fn average_participation_scoped(
        &self,
        communities: &[CommunityHealth],
    ) -> Option<f64> {
        let filtered = self.filter_communities_scoped(communities);
        let rates: Vec<f64> = filtered
            .iter()
            .filter(|ch| ch.governance.resolved_proposals > 0)
            .map(|ch| ch.governance.average_participation)
            .collect();

        if rates.is_empty() {
            return None;
        }
        let sum: f64 = rates.iter().sum();
        Some(sum / rates.len() as f64)
    }

    /// Count communities with health concerns in the visible scope.
    ///
    /// A community has health concerns if its collective health score
    /// is present and above the given threshold.
    pub fn communities_with_health_concerns_scoped(
        &self,
        communities: &[CommunityHealth],
        score_threshold: u32,
    ) -> usize {
        let filtered = self.filter_communities_scoped(communities);
        filtered
            .iter()
            .filter(|ch| {
                ch.collective_health_score
                    .is_some_and(|score| score > score_threshold)
            })
            .count()
    }

    // ── Snapshot scoped methods ─────────────────────────────────────

    /// Create a scoped view of a health snapshot.
    ///
    /// Returns a new snapshot with communities filtered to the visible
    /// set. Network health and economic health pass through unchanged
    /// (they are already deidentified aggregate data without community
    /// granularity).
    pub fn scope_snapshot(&self, snapshot: &HealthSnapshot) -> HealthSnapshot {
        if self.is_unrestricted() {
            return snapshot.clone();
        }
        let communities: Vec<CommunityHealth> = snapshot
            .communities
            .iter()
            .filter(|ch| {
                self.visible_communities
                    .contains(ch.community_id.to_string().as_str())
            })
            .cloned()
            .collect();

        HealthSnapshot {
            network: snapshot.network.clone(),
            communities,
            economic: snapshot.economic.clone(),
            quest: snapshot.quest.clone(),
            relay_privacy: snapshot.relay_privacy.clone(),
            timestamp: snapshot.timestamp,
        }
    }

    /// Count visible communities in a snapshot.
    pub fn community_count_scoped(&self, snapshot: &HealthSnapshot) -> usize {
        if self.is_unrestricted() {
            return snapshot.communities.len();
        }
        snapshot
            .communities
            .iter()
            .filter(|ch| {
                self.visible_communities
                    .contains(ch.community_id.to_string().as_str())
            })
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::GovernanceActivity;
    use crate::economic::EconomicHealth;
    use crate::network::NetworkHealth;
    use chrono::Utc;
    use uuid::Uuid;

    // ── Core FederationScope ─────────────────────────────────────────

    #[test]
    fn unrestricted_scope_sees_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
        assert!(scope.is_visible("any_community"));
        assert!(scope.is_visible("another_community"));
    }

    #[test]
    fn scoped_scope_filters_communities() {
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        assert!(!scope.is_unrestricted());
        assert!(!scope.is_empty());
        assert_eq!(scope.len(), 2);
        assert!(scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert!(!scope.is_visible("gamma"));
        assert!(!scope.is_visible("delta"));
    }

    #[test]
    fn default_is_unrestricted() {
        let scope = FederationScope::default();
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn add_and_remove_communities() {
        let mut scope = FederationScope::new();
        assert!(scope.is_unrestricted());

        assert!(scope.add_community("alpha"));
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("alpha"));
        assert!(!scope.is_visible("beta"));

        assert!(!scope.add_community("alpha"));

        assert!(scope.remove_community("alpha"));
        assert!(scope.is_unrestricted());

        assert!(!scope.remove_community("alpha"));
    }

    #[test]
    fn empty_communities_iterator() {
        let scope = FederationScope::from_communities(Vec::<String>::new());
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn serialization_roundtrip() {
        let scope = FederationScope::from_communities(["alpha", "beta", "gamma"]);
        let json = serde_json::to_string(&scope).expect("serialize");
        let deserialized: FederationScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn unrestricted_serialization_roundtrip() {
        let scope = FederationScope::new();
        let json = serde_json::to_string(&scope).expect("serialize");
        let deserialized: FederationScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, deserialized);
        assert!(deserialized.is_unrestricted());
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FederationScope>();
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn make_community_health(name: &str, id: Uuid, members: usize) -> CommunityHealth {
        CommunityHealth {
            community_id: id,
            community_name: name.to_string(),
            member_count: members,
            active_status: "Active".to_string(),
            role_distribution: std::collections::HashMap::new(),
            governance: GovernanceActivity::default(),
            collective_health_score: None,
            collective_health_status: None,
            computed_at: Utc::now(),
        }
    }

    fn make_community_health_with_governance(
        name: &str,
        id: Uuid,
        members: usize,
        active_proposals: usize,
        resolved_proposals: usize,
        participation: f64,
    ) -> CommunityHealth {
        CommunityHealth {
            community_id: id,
            community_name: name.to_string(),
            member_count: members,
            active_status: "Active".to_string(),
            role_distribution: std::collections::HashMap::new(),
            governance: GovernanceActivity {
                active_proposals,
                resolved_proposals,
                total_votes_cast: 0,
                average_participation: participation,
            },
            collective_health_score: None,
            collective_health_status: None,
            computed_at: Utc::now(),
        }
    }

    fn make_community_health_with_health_score(
        name: &str,
        id: Uuid,
        score: u32,
    ) -> CommunityHealth {
        CommunityHealth {
            community_id: id,
            community_name: name.to_string(),
            member_count: 10,
            active_status: "Active".to_string(),
            role_distribution: std::collections::HashMap::new(),
            governance: GovernanceActivity::default(),
            collective_health_score: Some(score),
            collective_health_status: Some("Healthy".to_string()),
            computed_at: Utc::now(),
        }
    }

    /// Fixed UUIDs for test determinism.
    const ALPHA_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);
    const BETA_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0002);
    const GAMMA_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0003);
    const ID_A: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_000a);
    const ID_B: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_000b);
    const ID_C: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_000c);

    fn test_communities() -> Vec<CommunityHealth> {
        vec![
            make_community_health("Alpha Village", ALPHA_ID, 50),
            make_community_health("Beta Town", BETA_ID, 100),
            make_community_health("Gamma City", GAMMA_ID, 200),
        ]
    }

    fn scope_for(ids: &[Uuid]) -> FederationScope {
        let id_strings: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        FederationScope::from_communities(id_strings)
    }

    // ── Community health scoped methods ─────────────────────────────

    #[test]
    fn filter_communities_unrestricted_returns_all() {
        let communities = test_communities();
        let scope = FederationScope::new();
        let filtered = scope.filter_communities_scoped(&communities);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn filter_communities_scoped_filters_by_id() {
        let communities = test_communities();
        let scope = scope_for(&[ALPHA_ID, GAMMA_ID]);
        let filtered = scope.filter_communities_scoped(&communities);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].community_name, "Alpha Village");
        assert_eq!(filtered[1].community_name, "Gamma City");
    }

    #[test]
    fn filter_communities_scoped_excludes_invisible() {
        let communities = test_communities();
        let scope = scope_for(&[BETA_ID]);
        let filtered = scope.filter_communities_scoped(&communities);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].community_name, "Beta Town");
    }

    #[test]
    fn total_members_unrestricted() {
        let communities = test_communities();
        let scope = FederationScope::new();
        assert_eq!(scope.total_members_scoped(&communities), 350);
    }

    #[test]
    fn total_members_scoped() {
        let communities = test_communities();
        let scope = scope_for(&[ALPHA_ID, BETA_ID]);
        assert_eq!(scope.total_members_scoped(&communities), 150);
    }

    #[test]
    fn total_members_scoped_empty_result() {
        let communities = test_communities();
        let scope = FederationScope::from_communities(["nonexistent"]);
        assert_eq!(scope.total_members_scoped(&communities), 0);
    }

    #[test]
    fn communities_with_governance_unrestricted() {
        let communities = vec![
            make_community_health_with_governance("A", ID_A, 10, 2, 1, 0.5),
            make_community_health_with_governance("B", ID_B, 20, 0, 0, 0.0),
            make_community_health_with_governance("C", ID_C, 30, 0, 3, 0.8),
        ];
        let scope = FederationScope::new();
        assert_eq!(scope.communities_with_governance_scoped(&communities), 2);
    }

    #[test]
    fn communities_with_governance_scoped() {
        let communities = vec![
            make_community_health_with_governance("A", ID_A, 10, 2, 1, 0.5),
            make_community_health_with_governance("B", ID_B, 20, 0, 0, 0.0),
            make_community_health_with_governance("C", ID_C, 30, 0, 3, 0.8),
        ];
        let scope = scope_for(&[ID_A]);
        assert_eq!(scope.communities_with_governance_scoped(&communities), 1);
    }

    #[test]
    fn average_participation_unrestricted() {
        let communities = vec![
            make_community_health_with_governance("A", ID_A, 10, 0, 5, 0.6),
            make_community_health_with_governance("B", ID_B, 20, 0, 3, 0.8),
        ];
        let scope = FederationScope::new();
        let avg = scope.average_participation_scoped(&communities).unwrap();
        assert!((avg - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn average_participation_scoped() {
        let communities = vec![
            make_community_health_with_governance("A", ID_A, 10, 0, 5, 0.6),
            make_community_health_with_governance("B", ID_B, 20, 0, 3, 0.8),
        ];
        let scope = scope_for(&[ID_A]);
        let avg = scope.average_participation_scoped(&communities).unwrap();
        assert!((avg - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn average_participation_no_data() {
        let communities = vec![
            make_community_health("A", ID_A, 10),
        ];
        let scope = FederationScope::new();
        assert!(scope.average_participation_scoped(&communities).is_none());
    }

    #[test]
    fn health_concerns_unrestricted() {
        let communities = vec![
            make_community_health_with_health_score("A", ID_A, 5),
            make_community_health_with_health_score("B", ID_B, 10),
            make_community_health_with_health_score("C", ID_C, 3),
        ];
        let scope = FederationScope::new();
        // Threshold 4 -- communities with score > 4
        assert_eq!(
            scope.communities_with_health_concerns_scoped(&communities, 4),
            2
        );
    }

    #[test]
    fn health_concerns_scoped() {
        let communities = vec![
            make_community_health_with_health_score("A", ID_A, 10),
            make_community_health_with_health_score("B", ID_B, 2),
        ];
        let scope = scope_for(&[ID_B]);
        assert_eq!(
            scope.communities_with_health_concerns_scoped(&communities, 4),
            0
        );
    }

    // ── Snapshot scoped methods ─────────────────────────────────────

    fn test_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            network: NetworkHealth::empty(),
            communities: test_communities(),
            economic: EconomicHealth::empty(),
            quest: None,
            relay_privacy: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn scope_snapshot_unrestricted_returns_clone() {
        let snapshot = test_snapshot();
        let scope = FederationScope::new();
        let scoped = scope.scope_snapshot(&snapshot);
        assert_eq!(scoped.communities.len(), 3);
    }

    #[test]
    fn scope_snapshot_filters_communities() {
        let snapshot = test_snapshot();
        let scope = scope_for(&[ALPHA_ID, GAMMA_ID]);
        let scoped = scope.scope_snapshot(&snapshot);
        assert_eq!(scoped.communities.len(), 2);
        assert_eq!(scoped.communities[0].community_name, "Alpha Village");
        assert_eq!(scoped.communities[1].community_name, "Gamma City");
    }

    #[test]
    fn scope_snapshot_preserves_network_health() {
        let mut snapshot = test_snapshot();
        snapshot.network = NetworkHealth {
            relay_count: 42,
            connected_count: 30,
            average_score: 0.9,
            total_send_count: 100,
            total_receive_count: 200,
            total_error_count: 5,
            average_latency_ms: Some(50.0),
            intermediary_relay_count: 0,
            privacy_routes_active: 0,
            average_privacy_overhead_ms: None,
            computed_at: Utc::now(),
        };
        let scope = scope_for(&[ALPHA_ID]);
        let scoped = scope.scope_snapshot(&snapshot);
        assert_eq!(scoped.network.relay_count, 42);
        assert_eq!(scoped.network.connected_count, 30);
    }

    #[test]
    fn scope_snapshot_preserves_economic_health() {
        let mut snapshot = test_snapshot();
        snapshot.economic = EconomicHealth {
            max_supply: 100_000,
            in_circulation: 50_000,
            locked_in_cash: 10_000,
            available: 40_000,
            utilization: 50.0,
            active_users: 100,
            total_ideas: 200,
            total_collectives: 5,
            computed_at: Utc::now(),
        };
        let scope = scope_for(&[BETA_ID]);
        let scoped = scope.scope_snapshot(&snapshot);
        assert_eq!(scoped.economic.in_circulation, 50_000);
        assert_eq!(scoped.economic.active_users, 100);
    }

    #[test]
    fn community_count_scoped_unrestricted() {
        let snapshot = test_snapshot();
        let scope = FederationScope::new();
        assert_eq!(scope.community_count_scoped(&snapshot), 3);
    }

    #[test]
    fn community_count_scoped_filtered() {
        let snapshot = test_snapshot();
        let scope = scope_for(&[BETA_ID]);
        assert_eq!(scope.community_count_scoped(&snapshot), 1);
    }

    #[test]
    fn community_count_scoped_no_matches() {
        let snapshot = test_snapshot();
        let scope = FederationScope::from_communities(["nonexistent"]);
        assert_eq!(scope.community_count_scoped(&snapshot), 0);
    }
}
