//! Economic health aggregation.
//!
//! Wraps Fortune's TreasuryStatus, which is already deidentified. Adds a
//! computed_at timestamp and flattens into Undercroft's metric vocabulary.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use fortune::TreasuryStatus;

/// Aggregated economic health of the network.
///
/// Sourced from Fortune's TreasuryStatus, which is already deidentified
/// (no individual balances, no transaction details, no pubkeys). This type
/// provides the Undercroft's view of economic vitals.
///
/// # Examples
///
/// ```
/// use fortune::{FortunePolicy, Treasury, TreasuryStatus, NetworkMetrics};
/// use undercroft::EconomicHealth;
///
/// let mut treasury = Treasury::new(FortunePolicy::default());
/// treasury.update_metrics(NetworkMetrics {
///     active_users: 100,
///     total_ideas: 50,
///     total_collectives: 2,
/// });
/// let status = treasury.status();
/// let health = EconomicHealth::from_treasury_status(&status);
/// assert_eq!(health.max_supply, 110_500);
/// assert_eq!(health.active_users, 100);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EconomicHealth {
    /// Maximum supply based on network capacity.
    pub max_supply: i64,
    /// Cool currently in circulation.
    pub in_circulation: i64,
    /// Cool locked in bearer cash instruments.
    pub locked_in_cash: i64,
    /// Cool available to mint.
    pub available: i64,
    /// Utilization percentage (0.0-100.0).
    pub utilization: f64,
    /// Number of active users (from NetworkMetrics).
    pub active_users: u64,
    /// Total ideas on the network (from NetworkMetrics).
    pub total_ideas: u64,
    /// Total collectives on the network (from NetworkMetrics).
    pub total_collectives: u64,
    /// When this snapshot was computed.
    pub computed_at: DateTime<Utc>,
}

impl EconomicHealth {
    /// Build economic health from a Fortune TreasuryStatus.
    ///
    /// TreasuryStatus is already deidentified -- it contains no individual
    /// balances, no transaction history, no pubkeys. We extract the aggregate
    /// figures and add a timestamp.
    ///
    /// # Arguments
    ///
    /// * `status` - The treasury status snapshot from Fortune.
    #[must_use]
    pub fn from_treasury_status(status: &TreasuryStatus) -> Self {
        Self {
            max_supply: status.max_supply,
            in_circulation: status.in_circulation,
            locked_in_cash: status.locked_in_cash,
            available: status.available,
            utilization: status.utilization,
            active_users: status.metrics.active_users,
            total_ideas: status.metrics.total_ideas,
            total_collectives: status.metrics.total_collectives,
            computed_at: Utc::now(),
        }
    }

    /// An empty economic health snapshot (no network activity).
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::EconomicHealth;
    ///
    /// let health = EconomicHealth::empty();
    /// assert_eq!(health.max_supply, 0);
    /// assert_eq!(health.active_users, 0);
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self {
            max_supply: 0,
            in_circulation: 0,
            locked_in_cash: 0,
            available: 0,
            utilization: 0.0,
            active_users: 0,
            total_ideas: 0,
            total_collectives: 0,
            computed_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fortune::{FortunePolicy, NetworkMetrics, Treasury};

    fn test_treasury_status() -> TreasuryStatus {
        let mut treasury = Treasury::new(FortunePolicy::default());
        treasury.update_metrics(NetworkMetrics {
            active_users: 100,
            total_ideas: 50,
            total_collectives: 2,
        });
        treasury.mint(5000, "test", fortune::MintReason::Initial).unwrap();
        treasury.lock_for_cash(1000);
        treasury.status()
    }

    #[test]
    fn from_treasury_status() {
        let status = test_treasury_status();
        let health = EconomicHealth::from_treasury_status(&status);

        assert_eq!(health.max_supply, 110_500);
        assert_eq!(health.in_circulation, 5000);
        assert_eq!(health.locked_in_cash, 1000);
        assert_eq!(health.available, 105_500);
        assert_eq!(health.active_users, 100);
        assert_eq!(health.total_ideas, 50);
        assert_eq!(health.total_collectives, 2);
        assert!(health.utilization > 0.0);
    }

    #[test]
    fn empty_economic_health() {
        let health = EconomicHealth::empty();
        assert_eq!(health.max_supply, 0);
        assert_eq!(health.in_circulation, 0);
        assert_eq!(health.locked_in_cash, 0);
        assert_eq!(health.available, 0);
        assert_eq!(health.utilization, 0.0);
        assert_eq!(health.active_users, 0);
        assert_eq!(health.total_ideas, 0);
        assert_eq!(health.total_collectives, 0);
    }

    #[test]
    fn from_zero_activity_treasury() {
        let treasury = Treasury::new(FortunePolicy::default());
        let status = treasury.status();
        let health = EconomicHealth::from_treasury_status(&status);

        assert_eq!(health.max_supply, 0);
        assert_eq!(health.in_circulation, 0);
        assert_eq!(health.utilization, 0.0);
    }

    #[test]
    fn serde_round_trip() {
        let status = test_treasury_status();
        let health = EconomicHealth::from_treasury_status(&status);
        let json = serde_json::to_string(&health).unwrap();
        let restored: EconomicHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.max_supply, health.max_supply);
        assert_eq!(restored.in_circulation, health.in_circulation);
        assert_eq!(restored.locked_in_cash, health.locked_in_cash);
        assert_eq!(restored.available, health.available);
        assert_eq!(restored.active_users, health.active_users);
        assert_eq!(restored.total_ideas, health.total_ideas);
        assert_eq!(restored.total_collectives, health.total_collectives);
    }

    #[test]
    fn no_individual_data_in_output() {
        let status = test_treasury_status();
        let health = EconomicHealth::from_treasury_status(&status);
        let json = serde_json::to_string(&health).unwrap();

        // Treasury status doesn't contain individual data, but verify
        // our output doesn't contain the test mint recipient.
        assert!(!json.contains("test"));
    }
}
