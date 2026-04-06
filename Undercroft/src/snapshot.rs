//! Health snapshots and history.
//!
//! A HealthSnapshot captures the full system state at a point in time.
//! HealthHistory maintains a ring buffer of snapshots for trend analysis.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::community::CommunityHealth;
use crate::economic::EconomicHealth;
use crate::network::NetworkHealth;
use crate::privacy_health::RelayPrivacyHealth;
use crate::quest::QuestHealth;

/// A complete health snapshot at a point in time.
///
/// Combines network, community, economic, and quest health into a single
/// timestamped record. All data is deidentified.
///
/// # Examples
///
/// ```
/// use undercroft::{HealthSnapshot, NetworkHealth, EconomicHealth};
///
/// let snapshot = HealthSnapshot {
///     network: NetworkHealth::empty(),
///     communities: vec![],
///     economic: EconomicHealth::empty(),
///     quest: None,
///     relay_privacy: None,
///     timestamp: chrono::Utc::now(),
/// };
/// assert!(snapshot.communities.is_empty());
/// assert!(snapshot.quest.is_none());
/// assert!(snapshot.relay_privacy.is_none());
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthSnapshot {
    /// Network relay health aggregates.
    pub network: NetworkHealth,
    /// Per-community health aggregates.
    pub communities: Vec<CommunityHealth>,
    /// Economic health aggregates.
    pub economic: EconomicHealth,
    /// Quest health aggregates (optional for backward compatibility).
    #[serde(default)]
    pub quest: Option<QuestHealth>,
    /// Relay privacy health aggregates (optional for backward compatibility).
    #[serde(default)]
    pub relay_privacy: Option<RelayPrivacyHealth>,
    /// When this snapshot was taken.
    pub timestamp: DateTime<Utc>,
}

/// Ring buffer of health snapshots for trend analysis.
///
/// Maintains a fixed-size window of snapshots. When the buffer is full,
/// the oldest snapshot is evicted to make room for new ones.
///
/// # Examples
///
/// ```
/// use undercroft::{HealthHistory, HealthSnapshot, NetworkHealth, EconomicHealth};
///
/// let mut history = HealthHistory::new(3);
/// assert!(history.is_empty());
/// assert_eq!(history.len(), 0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthHistory {
    snapshots: VecDeque<HealthSnapshot>,
    max_retention: usize,
}

/// Default retention: one week of hourly snapshots.
const DEFAULT_MAX_RETENTION: usize = 168;

impl HealthHistory {
    /// Create a new history with the given maximum retention.
    ///
    /// # Arguments
    ///
    /// * `max_retention` - Maximum number of snapshots to retain.
    ///   When exceeded, the oldest snapshot is evicted.
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::HealthHistory;
    ///
    /// let history = HealthHistory::new(24);
    /// assert!(history.is_empty());
    /// ```
    #[must_use]
    pub fn new(max_retention: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(max_retention.min(1024)),
            max_retention,
        }
    }

    /// Push a new snapshot. Evicts the oldest if at capacity.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - The health snapshot to record.
    pub fn push(&mut self, snapshot: HealthSnapshot) {
        if self.snapshots.len() >= self.max_retention {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    /// Get the most recent snapshot, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&HealthSnapshot> {
        self.snapshots.back()
    }

    /// Number of snapshots in the history.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Iterate over snapshots from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &HealthSnapshot> {
        self.snapshots.iter()
    }
}

impl Default for HealthHistory {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_RETENTION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot(n: i64) -> HealthSnapshot {
        HealthSnapshot {
            network: NetworkHealth::empty(),
            communities: vec![],
            economic: EconomicHealth::empty(),
            quest: None,
            relay_privacy: None,
            timestamp: DateTime::from_timestamp(1_700_000_000 + n * 3600, 0)
                .unwrap_or_else(Utc::now),
        }
    }

    #[test]
    fn new_history_is_empty() {
        let history = HealthHistory::new(10);
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.latest().is_none());
    }

    #[test]
    fn push_and_latest() {
        let mut history = HealthHistory::new(10);
        history.push(test_snapshot(1));
        assert_eq!(history.len(), 1);
        assert!(!history.is_empty());
        assert!(history.latest().is_some());

        history.push(test_snapshot(2));
        assert_eq!(history.len(), 2);
        // Latest should be the second snapshot (timestamp offset by 2 hours)
        let latest = history.latest().unwrap();
        assert_eq!(
            latest.timestamp.timestamp(),
            1_700_000_000 + 2 * 3600
        );
    }

    #[test]
    fn capacity_eviction() {
        let mut history = HealthHistory::new(3);

        history.push(test_snapshot(1));
        history.push(test_snapshot(2));
        history.push(test_snapshot(3));
        assert_eq!(history.len(), 3);

        // Pushing a 4th should evict snapshot 1 (oldest).
        history.push(test_snapshot(4));
        assert_eq!(history.len(), 3);

        // Oldest remaining should be snapshot 2.
        let oldest = history.iter().next().unwrap();
        assert_eq!(
            oldest.timestamp.timestamp(),
            1_700_000_000 + 2 * 3600
        );

        // Latest should be snapshot 4.
        let latest = history.latest().unwrap();
        assert_eq!(
            latest.timestamp.timestamp(),
            1_700_000_000 + 4 * 3600
        );
    }

    #[test]
    fn iteration_order_oldest_to_newest() {
        let mut history = HealthHistory::new(10);
        for i in 1..=5 {
            history.push(test_snapshot(i));
        }

        let timestamps: Vec<i64> = history
            .iter()
            .map(|s| s.timestamp.timestamp())
            .collect();

        // Should be in ascending order (oldest first).
        for i in 1..timestamps.len() {
            assert!(timestamps[i] > timestamps[i - 1]);
        }
    }

    #[test]
    fn default_retention_is_one_week_hourly() {
        let history = HealthHistory::default();
        assert_eq!(history.max_retention, 168); // 7 * 24
    }

    #[test]
    fn single_capacity() {
        let mut history = HealthHistory::new(1);
        history.push(test_snapshot(1));
        assert_eq!(history.len(), 1);

        history.push(test_snapshot(2));
        assert_eq!(history.len(), 1);

        let latest = history.latest().unwrap();
        assert_eq!(
            latest.timestamp.timestamp(),
            1_700_000_000 + 2 * 3600
        );
    }

    #[test]
    fn snapshot_with_communities() {
        let snapshot = HealthSnapshot {
            network: NetworkHealth::empty(),
            communities: vec![],
            economic: EconomicHealth::empty(),
            quest: None,
            relay_privacy: None,
            timestamp: Utc::now(),
        };

        assert!(snapshot.communities.is_empty());
        assert!(snapshot.quest.is_none());
    }

    #[test]
    fn snapshot_serde_round_trip() {
        let snapshot = test_snapshot(42);
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: HealthSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.timestamp, snapshot.timestamp);
        assert_eq!(restored.network.relay_count, 0);
        assert!(restored.communities.is_empty());
    }

    #[test]
    fn eviction_under_stress() {
        let mut history = HealthHistory::new(5);
        for i in 0..100 {
            history.push(test_snapshot(i));
        }
        assert_eq!(history.len(), 5);

        // The 5 remaining should be the last 5 pushed (95-99).
        let timestamps: Vec<i64> = history
            .iter()
            .map(|s| s.timestamp.timestamp())
            .collect();
        assert_eq!(
            timestamps[0],
            1_700_000_000 + 95 * 3600
        );
        assert_eq!(
            timestamps[4],
            1_700_000_000 + 99 * 3600
        );
    }
}
