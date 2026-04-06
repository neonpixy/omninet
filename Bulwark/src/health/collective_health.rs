use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::status::CollectiveHealthStatus;
use crate::federation_scope::FederationScope;

/// A snapshot of a community's structural health — the cult detector.
///
/// 5 factors, WEIGHTED. Cross-membership isolation is the heaviest weight (0-5)
/// because communities where members have no outside connections are the
/// primary signal for cult-like behavior.
///
/// Total range: 0-19. Maps to 5 statuses (Thriving → Toxic).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectiveHealthPulse {
    pub collective_id: Uuid,
    pub status: CollectiveHealthStatus,
    pub factors: CollectiveHealthFactors,
    pub computed_at: DateTime<Utc>,
    pub ttl_hours: u32,
    pub contributing_members: u32,
}

impl CollectiveHealthPulse {
    /// Compute a collective health pulse from the given factors and member count.
    pub fn compute(collective_id: Uuid, factors: CollectiveHealthFactors, contributing_members: u32) -> Self {
        let status = CollectiveHealthStatus::from_score(factors.total_score());
        Self {
            collective_id,
            status,
            factors,
            computed_at: Utc::now(),
            ttl_hours: 24,
            contributing_members,
        }
    }

    /// Whether this pulse has expired and should be recomputed.
    pub fn is_expired(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.computed_at);
        age.num_hours() > i64::from(self.ttl_hours)
    }

    /// Compute collective health with federation-scoped cross-membership.
    ///
    /// Cross-membership is the cult detection signal: how many external communities
    /// do members belong to? When federation scope is applied, only memberships in
    /// federated communities count as "external connections."
    ///
    /// All other factors are local to the collective and unaffected by scope.
    ///
    /// # Parameters
    ///
    /// - `collective_id` — the community being measured.
    /// - `factors` — base factors (cross_membership will be overridden).
    /// - `contributing_members` — how many members contributed data.
    /// - `member_communities` — for each member, the set of other communities they belong to.
    /// - `scope` — which communities count as valid "external connections."
    pub fn compute_scoped(
        collective_id: Uuid,
        factors: CollectiveHealthFactors,
        contributing_members: u32,
        member_communities: &[Vec<String>],
        scope: &FederationScope,
    ) -> Self {
        if scope.is_unrestricted() {
            return Self::compute(collective_id, factors, contributing_members);
        }

        // Recompute cross-membership considering only federated communities.
        let scoped_cross = Self::cross_membership_scoped(member_communities, scope);
        let scoped_factors = CollectiveHealthFactors {
            cross_membership: scoped_cross,
            ..factors
        };

        Self::compute(collective_id, scoped_factors, contributing_members)
    }

    /// Determine cross-membership level considering only federated communities.
    ///
    /// A member's "external connections" only count communities visible in the scope.
    /// This means a community that looks well-connected globally might appear isolated
    /// within a particular federation — which is the correct read for federation-local
    /// safety decisions.
    fn cross_membership_scoped(
        member_communities: &[Vec<String>],
        scope: &FederationScope,
    ) -> CrossMembershipLevel {
        if member_communities.is_empty() {
            return CrossMembershipLevel::Isolated;
        }

        // For each member, count external communities visible in scope.
        let external_counts: Vec<usize> = member_communities
            .iter()
            .map(|communities| {
                communities
                    .iter()
                    .filter(|c| scope.is_visible(c))
                    .count()
            })
            .collect();

        // Average external connections per member.
        let total: usize = external_counts.iter().sum();
        let avg = total as f64 / external_counts.len() as f64;

        // Thresholds match the unscoped intuition: 3+ = well connected,
        // 1-3 = some, 0.5-1 = few, <0.5 = isolated.
        if avg >= 3.0 {
            CrossMembershipLevel::WellConnected
        } else if avg >= 1.0 {
            CrossMembershipLevel::SomeConnections
        } else if avg >= 0.5 {
            CrossMembershipLevel::FewConnections
        } else {
            CrossMembershipLevel::Isolated
        }
    }
}

/// The 5 health factors — weighted toward isolation and autocracy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectiveHealthFactors {
    pub engagement: EngagementDistribution,
    pub communication: CollectiveCommunicationPattern,
    pub cross_membership: CrossMembershipLevel,
    pub power_distribution: PowerDistribution,
    pub content_health: CollectiveContentHealth,
}

impl CollectiveHealthFactors {
    pub fn total_score(&self) -> u32 {
        self.engagement.score()
            + self.communication.score()
            + self.cross_membership.score()
            + self.power_distribution.score()
            + self.content_health.score()
    }
}

/// Factor 1: How broadly are members engaged? (0-3)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EngagementDistribution {
    BroadlyEngaged,
    CoreActive,
    FewActive,
    Inactive,
}

impl EngagementDistribution {
    pub fn score(&self) -> u32 {
        match self {
            EngagementDistribution::BroadlyEngaged => 0,
            EngagementDistribution::CoreActive => 1,
            EngagementDistribution::FewActive => 2,
            EngagementDistribution::Inactive => 3,
        }
    }
}

/// Factor 2: Communication patterns (0-4).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CollectiveCommunicationPattern {
    Healthy,
    ActiveDebate,
    OneWay,
    Hostile,
}

impl CollectiveCommunicationPattern {
    pub fn score(&self) -> u32 {
        match self {
            CollectiveCommunicationPattern::Healthy => 0,
            CollectiveCommunicationPattern::ActiveDebate => 1,
            CollectiveCommunicationPattern::OneWay => 2,
            CollectiveCommunicationPattern::Hostile => 4,
        }
    }
}

/// Factor 3: Cross-membership — THE CULT DETECTOR. (0-5, HEAVIEST WEIGHT)
///
/// Communities where members are ONLY in that one community are the
/// single strongest signal for cult-like isolation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CrossMembershipLevel {
    /// Members have many external connections.
    WellConnected,
    /// Some external connections.
    SomeConnections,
    /// Limited external connections.
    FewConnections,
    /// Members mostly only in this collective. MAJOR RED FLAG.
    Isolated,
}

impl CrossMembershipLevel {
    pub fn score(&self) -> u32 {
        match self {
            CrossMembershipLevel::WellConnected => 0,
            CrossMembershipLevel::SomeConnections => 1,
            CrossMembershipLevel::FewConnections => 3,
            CrossMembershipLevel::Isolated => 5, // heaviest single weight
        }
    }
}

/// Factor 4: How is power distributed? (0-4)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PowerDistribution {
    Distributed,
    CoreLeadership,
    Concentrated,
    Autocratic,
}

impl PowerDistribution {
    pub fn score(&self) -> u32 {
        match self {
            PowerDistribution::Distributed => 0,
            PowerDistribution::CoreLeadership => 1,
            PowerDistribution::Concentrated => 2,
            PowerDistribution::Autocratic => 4,
        }
    }
}

/// Factor 5: Content health (0-3). Structural signals only.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CollectiveContentHealth {
    Positive,
    Neutral,
    Mixed,
    Concerning,
}

impl CollectiveContentHealth {
    pub fn score(&self) -> u32 {
        match self {
            CollectiveContentHealth::Positive => 0,
            CollectiveContentHealth::Neutral => 1,
            CollectiveContentHealth::Mixed => 2,
            CollectiveContentHealth::Concerning => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thriving_collective() {
        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::BroadlyEngaged,
            communication: CollectiveCommunicationPattern::Healthy,
            cross_membership: CrossMembershipLevel::WellConnected,
            power_distribution: PowerDistribution::Distributed,
            content_health: CollectiveContentHealth::Positive,
        };
        assert_eq!(factors.total_score(), 0);
        let pulse = CollectiveHealthPulse::compute(Uuid::new_v4(), factors, 50);
        assert_eq!(pulse.status, CollectiveHealthStatus::Thriving);
    }

    #[test]
    fn toxic_cult_pattern() {
        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::FewActive,
            communication: CollectiveCommunicationPattern::Hostile,
            cross_membership: CrossMembershipLevel::Isolated,   // +5
            power_distribution: PowerDistribution::Autocratic,   // +4
            content_health: CollectiveContentHealth::Concerning,  // +3
        };
        // 2 + 4 + 5 + 4 + 3 = 18
        assert_eq!(factors.total_score(), 18);
        let pulse = CollectiveHealthPulse::compute(Uuid::new_v4(), factors, 10);
        assert_eq!(pulse.status, CollectiveHealthStatus::Toxic);
    }

    #[test]
    fn cross_membership_is_heaviest() {
        // Isolated alone = 5 points
        assert_eq!(CrossMembershipLevel::Isolated.score(), 5);
        // Next heaviest individual scores
        assert_eq!(CollectiveCommunicationPattern::Hostile.score(), 4);
        assert_eq!(PowerDistribution::Autocratic.score(), 4);
    }

    #[test]
    fn isolation_plus_autocracy_is_toxic() {
        // Even with other factors healthy, isolation + autocracy triggers tense
        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::BroadlyEngaged, // 0
            communication: CollectiveCommunicationPattern::Healthy, // 0
            cross_membership: CrossMembershipLevel::Isolated,    // 5
            power_distribution: PowerDistribution::Autocratic,    // 4
            content_health: CollectiveContentHealth::Positive,    // 0
        };
        assert_eq!(factors.total_score(), 9);
        assert_eq!(
            CollectiveHealthStatus::from_score(9),
            CollectiveHealthStatus::Quiet
        );
        // But add one more signal and it's tense
        let factors2 = CollectiveHealthFactors {
            communication: CollectiveCommunicationPattern::OneWay, // 2
            ..factors
        };
        assert_eq!(factors2.total_score(), 11);
        assert_eq!(
            CollectiveHealthStatus::from_score(11),
            CollectiveHealthStatus::Tense
        );
    }

    #[test]
    fn quiet_but_not_concerning() {
        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::Inactive,       // 3
            communication: CollectiveCommunicationPattern::Healthy, // 0
            cross_membership: CrossMembershipLevel::SomeConnections, // 1
            power_distribution: PowerDistribution::Distributed,  // 0
            content_health: CollectiveContentHealth::Neutral,    // 1
        };
        // 3 + 0 + 1 + 0 + 1 = 5
        assert_eq!(factors.total_score(), 5);
        let pulse = CollectiveHealthPulse::compute(Uuid::new_v4(), factors, 5);
        assert_eq!(pulse.status, CollectiveHealthStatus::Healthy);
    }

    // ── Federation Scope ──────────────────────────────────────────────────

    #[test]
    fn compute_scoped_unrestricted_matches_compute() {
        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::BroadlyEngaged,
            communication: CollectiveCommunicationPattern::Healthy,
            cross_membership: CrossMembershipLevel::WellConnected,
            power_distribution: PowerDistribution::Distributed,
            content_health: CollectiveContentHealth::Positive,
        };
        let id = Uuid::new_v4();
        let unscoped = CollectiveHealthPulse::compute(id, factors.clone(), 10);
        let scoped = CollectiveHealthPulse::compute_scoped(
            id,
            factors,
            10,
            &[], // no member data needed for unrestricted
            &FederationScope::new(),
        );
        assert_eq!(unscoped.status, scoped.status);
    }

    #[test]
    fn compute_scoped_reduces_cross_membership() {
        // Members have connections to many communities, but the federation
        // only includes "alpha" and "beta".
        let member_communities = vec![
            vec!["alpha".into(), "gamma".into(), "delta".into()],
            vec!["gamma".into(), "epsilon".into()],
            vec!["delta".into(), "zeta".into()],
        ];

        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::BroadlyEngaged,
            communication: CollectiveCommunicationPattern::Healthy,
            cross_membership: CrossMembershipLevel::WellConnected, // will be overridden
            power_distribution: PowerDistribution::Distributed,
            content_health: CollectiveContentHealth::Positive,
        };

        // Federation only sees alpha and beta.
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        let pulse = CollectiveHealthPulse::compute_scoped(
            Uuid::new_v4(),
            factors,
            3,
            &member_communities,
            &scope,
        );

        // Member 0: alpha visible (1 connection). Member 1: none. Member 2: none.
        // Average: 1/3 ≈ 0.33 → FewConnections (0.5 > 0.33, so Isolated).
        // Actually avg = 0.33, threshold for FewConnections is >= 0.5, so Isolated.
        assert_eq!(pulse.factors.cross_membership, CrossMembershipLevel::Isolated);
    }

    #[test]
    fn compute_scoped_well_connected_in_federation() {
        // Members have many connections, all within the federation.
        let member_communities = vec![
            vec!["alpha".into(), "beta".into(), "gamma".into(), "delta".into()],
            vec!["alpha".into(), "gamma".into(), "delta".into()],
            vec!["beta".into(), "gamma".into()],
        ];

        let factors = CollectiveHealthFactors {
            engagement: EngagementDistribution::BroadlyEngaged,
            communication: CollectiveCommunicationPattern::Healthy,
            cross_membership: CrossMembershipLevel::Isolated, // will be overridden
            power_distribution: PowerDistribution::Distributed,
            content_health: CollectiveContentHealth::Positive,
        };

        // Federation includes all communities.
        let scope = FederationScope::from_communities(["alpha", "beta", "gamma", "delta"]);
        let pulse = CollectiveHealthPulse::compute_scoped(
            Uuid::new_v4(),
            factors,
            3,
            &member_communities,
            &scope,
        );

        // Member 0: 4, Member 1: 3, Member 2: 2. Average: 3.0 → WellConnected.
        assert_eq!(pulse.factors.cross_membership, CrossMembershipLevel::WellConnected);
    }
}
