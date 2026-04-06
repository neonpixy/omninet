//! Top-level health metrics.
//!
//! HealthMetrics is the composite summary used by HQ's Home dashboard.
//! One number per vital, derived from the full HealthSnapshot.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use globe::StoreStats;

use crate::snapshot::HealthSnapshot;

/// Top-level health summary for the network dashboard.
///
/// Provides the key vital signs at a glance: how many nodes, relays,
/// events, Cool in circulation, and communities. All deidentified.
///
/// # Examples
///
/// ```
/// use undercroft::{HealthMetrics, HealthSnapshot, NetworkHealth, EconomicHealth};
///
/// let snapshot = HealthSnapshot {
///     network: NetworkHealth::empty(),
///     communities: vec![],
///     economic: EconomicHealth::empty(),
///     quest: None,
///     relay_privacy: None,
///     timestamp: chrono::Utc::now(),
/// };
/// let metrics = HealthMetrics::from_snapshot(&snapshot, 42, None);
/// assert_eq!(metrics.node_count, 42);
/// assert_eq!(metrics.relay_count, 0);
/// assert_eq!(metrics.community_count, 0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Estimated number of nodes on the network (from gospel peer count).
    pub node_count: u64,
    /// Number of relays (from NetworkHealth).
    pub relay_count: usize,
    /// Estimated event throughput (events per second), derived from store stats.
    pub event_throughput: f64,
    /// Total event count from the store.
    pub content_volume: usize,
    /// Cool in circulation (from EconomicHealth).
    pub cool_circulation: i64,
    /// Number of communities observed.
    pub community_count: usize,
    /// Quest participants (0 if quest data unavailable).
    pub quest_participants: usize,
    /// Quest composite health score (0.0-1.0, 0.0 if quest data unavailable).
    pub quest_health_score: f64,
    /// Active Quest challenges (0 if quest data unavailable).
    pub active_challenges: usize,
    /// Active cooperative raids (0 if quest data unavailable).
    pub active_raids: usize,
    /// Relay privacy composite health score (0.0-1.0, None if unavailable).
    #[serde(default)]
    pub privacy_health_score: Option<f64>,
    /// Number of relays in Intermediary mode (None if unavailable).
    #[serde(default)]
    pub intermediary_count: Option<usize>,
    /// Fraction of traffic using privacy routes (0.0-1.0, None if unavailable).
    #[serde(default)]
    pub privacy_route_fraction: Option<f64>,
    /// When this metric was computed.
    pub computed_at: DateTime<Utc>,
}

impl HealthMetrics {
    /// Build top-level metrics from a health snapshot.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The full health snapshot to summarize.
    /// * `node_count` - Estimated node count (typically from gospel peer registry).
    /// * `store_stats` - Optional event store statistics for throughput calculation.
    #[must_use]
    pub fn from_snapshot(
        snapshot: &HealthSnapshot,
        node_count: u64,
        store_stats: Option<&StoreStats>,
    ) -> Self {
        let (event_throughput, content_volume) = match store_stats {
            Some(stats) => {
                let throughput = Self::compute_throughput(stats);
                (throughput, stats.event_count)
            }
            None => (0.0, 0),
        };

        let (quest_participants, quest_health_score, active_challenges, active_raids) =
            match &snapshot.quest {
                Some(qh) => (
                    qh.total_participants(),
                    qh.health_score(),
                    qh.report.active_challenges,
                    qh.report.active_raids,
                ),
                None => (0, 0.0, 0, 0),
            };

        let (privacy_health_score, intermediary_count, privacy_route_fraction) =
            match &snapshot.relay_privacy {
                Some(rp) => (
                    Some(rp.health_score()),
                    Some(rp.intermediary_count),
                    Some(rp.privacy_route_fraction),
                ),
                None => (None, None, None),
            };

        Self {
            node_count,
            relay_count: snapshot.network.relay_count,
            event_throughput,
            content_volume,
            cool_circulation: snapshot.economic.in_circulation,
            community_count: snapshot.communities.len(),
            quest_participants,
            quest_health_score,
            active_challenges,
            active_raids,
            privacy_health_score,
            intermediary_count,
            privacy_route_fraction,
            computed_at: Utc::now(),
        }
    }

    /// Compute event throughput (events/second) from store statistics.
    ///
    /// Uses the time range between oldest and newest events to estimate
    /// the average throughput. Returns 0.0 if insufficient data.
    fn compute_throughput(stats: &StoreStats) -> f64 {
        match (stats.oldest_event, stats.newest_event) {
            (Some(oldest), Some(newest)) if newest > oldest => {
                let duration_secs = (newest - oldest) as f64;
                if duration_secs > 0.0 {
                    stats.event_count as f64 / duration_secs
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economic::EconomicHealth;
    use crate::network::NetworkHealth;
    use std::collections::HashMap;

    fn test_snapshot() -> HealthSnapshot {
        HealthSnapshot {
            network: NetworkHealth::empty(),
            communities: vec![],
            economic: EconomicHealth::empty(),
            quest: None,
            relay_privacy: None,
            timestamp: Utc::now(),
        }
    }

    fn test_snapshot_with_data() -> HealthSnapshot {
        let mut economic = EconomicHealth::empty();
        economic.in_circulation = 50_000;

        HealthSnapshot {
            network: NetworkHealth {
                relay_count: 5,
                connected_count: 3,
                average_score: 0.8,
                total_send_count: 100,
                total_receive_count: 200,
                total_error_count: 5,
                average_latency_ms: Some(75.0),
                intermediary_relay_count: 0,
                privacy_routes_active: 0,
                average_privacy_overhead_ms: None,
                computed_at: Utc::now(),
            },
            communities: vec![],
            economic,
            quest: None,
            relay_privacy: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn from_empty_snapshot_no_stats() {
        let snapshot = test_snapshot();
        let metrics = HealthMetrics::from_snapshot(&snapshot, 0, None);

        assert_eq!(metrics.node_count, 0);
        assert_eq!(metrics.relay_count, 0);
        assert_eq!(metrics.event_throughput, 0.0);
        assert_eq!(metrics.content_volume, 0);
        assert_eq!(metrics.cool_circulation, 0);
        assert_eq!(metrics.community_count, 0);
    }

    #[test]
    fn from_snapshot_with_node_count() {
        let snapshot = test_snapshot();
        let metrics = HealthMetrics::from_snapshot(&snapshot, 42, None);
        assert_eq!(metrics.node_count, 42);
    }

    #[test]
    fn from_snapshot_with_data() {
        let snapshot = test_snapshot_with_data();
        let metrics = HealthMetrics::from_snapshot(&snapshot, 10, None);

        assert_eq!(metrics.relay_count, 5);
        assert_eq!(metrics.cool_circulation, 50_000);
    }

    #[test]
    fn with_store_stats() {
        let snapshot = test_snapshot();
        let stats = StoreStats {
            event_count: 1000,
            oldest_event: Some(1_700_000_000),
            newest_event: Some(1_700_001_000), // 1000 seconds later
            events_by_kind: HashMap::new(),
        };

        let metrics = HealthMetrics::from_snapshot(&snapshot, 5, Some(&stats));
        assert_eq!(metrics.content_volume, 1000);
        // 1000 events / 1000 seconds = 1.0 events/sec
        assert!((metrics.event_throughput - 1.0).abs() < 0.01);
    }

    #[test]
    fn throughput_zero_when_no_time_range() {
        let stats = StoreStats {
            event_count: 100,
            oldest_event: None,
            newest_event: None,
            events_by_kind: HashMap::new(),
        };

        let throughput = HealthMetrics::compute_throughput(&stats);
        assert_eq!(throughput, 0.0);
    }

    #[test]
    fn throughput_zero_when_same_timestamp() {
        let stats = StoreStats {
            event_count: 100,
            oldest_event: Some(1_700_000_000),
            newest_event: Some(1_700_000_000),
            events_by_kind: HashMap::new(),
        };

        let throughput = HealthMetrics::compute_throughput(&stats);
        assert_eq!(throughput, 0.0);
    }

    #[test]
    fn community_count_from_snapshot() {
        use crate::community::CommunityHealth;
        use kingdom::{Community, CommunityBasis};

        let c1 = Community::new("A", CommunityBasis::Digital);
        let c2 = Community::new("B", CommunityBasis::Place);

        let h1 = CommunityHealth::from_community(&c1, &[], None);
        let h2 = CommunityHealth::from_community(&c2, &[], None);

        let snapshot = HealthSnapshot {
            network: NetworkHealth::empty(),
            communities: vec![h1, h2],
            economic: EconomicHealth::empty(),
            quest: None,
            relay_privacy: None,
            timestamp: Utc::now(),
        };

        let metrics = HealthMetrics::from_snapshot(&snapshot, 0, None);
        assert_eq!(metrics.community_count, 2);
    }

    #[test]
    fn serde_round_trip() {
        let snapshot = test_snapshot_with_data();
        let metrics = HealthMetrics::from_snapshot(&snapshot, 15, None);
        let json = serde_json::to_string(&metrics).unwrap();
        let restored: HealthMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.node_count, metrics.node_count);
        assert_eq!(restored.relay_count, metrics.relay_count);
        assert_eq!(restored.cool_circulation, metrics.cool_circulation);
        assert_eq!(restored.community_count, metrics.community_count);
        assert_eq!(restored.content_volume, metrics.content_volume);
    }
}
