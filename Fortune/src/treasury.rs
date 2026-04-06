use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::policy::FortunePolicy;

/// The capacity-backed treasury — Cool supply tied to real network activity.
///
/// From Consortium Art. 2 §1: "The purpose of trade is not to accumulate private
/// wealth, but to circulate value in ways that nourish reciprocal stewardship
/// and shared abundance."
///
/// Max supply = (users × cool_per_user) + (ideas × cool_per_idea) + (collectives × cool_per_collective)
/// Supply grows with the network. It cannot be inflated beyond capacity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Treasury {
    pub in_circulation: i64,
    pub locked_in_cash: i64,
    pub returned_from_demurrage: i64,
    pub metrics: NetworkMetrics,
    pub mint_history: Vec<MintRecord>,
    policy: FortunePolicy,
}

impl Treasury {
    /// Create a new treasury with the given economic policy and zero circulation.
    pub fn new(policy: FortunePolicy) -> Self {
        Self {
            in_circulation: 0,
            locked_in_cash: 0,
            returned_from_demurrage: 0,
            metrics: NetworkMetrics::default(),
            mint_history: Vec::new(),
            policy,
        }
    }

    /// Maximum supply based on current network capacity.
    pub fn max_supply(&self) -> i64 {
        (self.metrics.active_users as i64 * self.policy.cool_per_active_user)
            + (self.metrics.total_ideas as i64 * self.policy.cool_per_idea)
            + (self.metrics.total_collectives as i64 * self.policy.cool_per_collective)
    }

    /// Cool available to mint.
    pub fn available(&self) -> i64 {
        (self.max_supply() - self.in_circulation).max(0)
    }

    /// Liquid circulation (excludes locked Cash).
    pub fn liquid_circulation(&self) -> i64 {
        self.in_circulation - self.locked_in_cash
    }

    /// Utilization as a percentage.
    pub fn utilization(&self) -> f64 {
        let max = self.max_supply();
        if max == 0 {
            return 0.0;
        }
        (self.in_circulation as f64 / max as f64) * 100.0
    }

    /// Mint Cool into circulation. Returns actual amount minted (may be less if supply limited).
    pub fn mint(
        &mut self,
        amount: i64,
        recipient: &str,
        reason: MintReason,
    ) -> Result<i64, crate::FortuneError> {
        let available = self.available();
        if available <= 0 {
            return Err(crate::FortuneError::TreasuryEmpty);
        }

        let actual = amount.min(available);
        self.in_circulation += actual;
        self.mint_history.push(MintRecord {
            amount: actual,
            recipient: recipient.into(),
            reason,
            timestamp: Utc::now(),
        });

        // Prune history if too large
        if self.mint_history.len() > 1000 {
            let drain_count = self.mint_history.len() - 750;
            self.mint_history.drain(..drain_count);
        }

        Ok(actual)
    }

    /// Mint exact amount — fails if not enough supply.
    pub fn mint_exact(
        &mut self,
        amount: i64,
        recipient: &str,
        reason: MintReason,
    ) -> Result<(), crate::FortuneError> {
        let available = self.available();
        if available < amount {
            return Err(crate::FortuneError::SupplyCapExceeded {
                requested: amount,
                available,
            });
        }
        self.mint(amount, recipient, reason)?;
        Ok(())
    }

    /// Receive Cool back (from demurrage, expired cash, etc).
    pub fn receive(&mut self, amount: i64, reason: ReceiveReason) {
        self.in_circulation = (self.in_circulation - amount).max(0);
        if reason == ReceiveReason::Demurrage {
            self.returned_from_demurrage += amount;
        }
    }

    /// Lock Cool for Cash issuance.
    pub fn lock_for_cash(&mut self, amount: i64) {
        self.locked_in_cash += amount;
    }

    /// Unlock Cool from Cash (redemption or expiry).
    pub fn unlock_from_cash(&mut self, amount: i64) {
        self.locked_in_cash = (self.locked_in_cash - amount).max(0);
    }

    /// Update network metrics.
    pub fn update_metrics(&mut self, metrics: NetworkMetrics) {
        self.metrics = metrics;
    }

    /// Get a snapshot of the treasury's current state.
    pub fn status(&self) -> TreasuryStatus {
        TreasuryStatus {
            max_supply: self.max_supply(),
            in_circulation: self.in_circulation,
            locked_in_cash: self.locked_in_cash,
            available: self.available(),
            liquid_circulation: self.liquid_circulation(),
            utilization: self.utilization(),
            returned_from_demurrage: self.returned_from_demurrage,
            metrics: self.metrics.clone(),
        }
    }
}

/// Network activity that backs the supply.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NetworkMetrics {
    pub active_users: u64,
    pub total_ideas: u64,
    pub total_collectives: u64,
}

/// A record of minting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MintRecord {
    pub amount: i64,
    pub recipient: String,
    pub reason: MintReason,
    pub timestamp: DateTime<Utc>,
}

/// Why Cool was minted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MintReason {
    /// Minted for Universal Basic Income distribution.
    Ubi,
    /// Minted as an initial allocation for a new account.
    Initial,
    /// Minted as a reward for contribution.
    Reward,
    /// Minted as a manual governance correction.
    Correction,
    /// Minted for testing purposes only.
    Testing,
}

/// Why Cool was returned to treasury.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReceiveReason {
    /// Cool returned from demurrage decay.
    Demurrage,
    /// Cool returned from an expired Cash note.
    CashExpired,
    /// Cool returned via manual governance correction.
    Correction,
}

/// Snapshot of treasury state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreasuryStatus {
    pub max_supply: i64,
    pub in_circulation: i64,
    pub locked_in_cash: i64,
    pub available: i64,
    pub liquid_circulation: i64,
    pub utilization: f64,
    pub returned_from_demurrage: i64,
    pub metrics: NetworkMetrics,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_treasury() -> Treasury {
        let policy = FortunePolicy::default();
        let mut t = Treasury::new(policy);
        t.metrics = NetworkMetrics {
            active_users: 100,
            total_ideas: 50,
            total_collectives: 2,
        };
        t
    }

    #[test]
    fn max_supply_formula() {
        let t = test_treasury();
        // 100 * 1000 + 50 * 10 + 2 * 5000 = 100000 + 500 + 10000 = 110500
        assert_eq!(t.max_supply(), 110_500);
        assert_eq!(t.available(), 110_500);
    }

    #[test]
    fn mint_reduces_available() {
        let mut t = test_treasury();
        let minted = t.mint(1000, "alice", MintReason::Initial).unwrap();
        assert_eq!(minted, 1000);
        assert_eq!(t.in_circulation, 1000);
        assert_eq!(t.available(), 109_500);
    }

    #[test]
    fn mint_capped_at_available() {
        let mut t = test_treasury();
        let available = t.available();
        let minted = t.mint(available + 5000, "alice", MintReason::Testing).unwrap();
        assert_eq!(minted, available);
        assert_eq!(t.available(), 0);
    }

    #[test]
    fn mint_exact_fails_if_insufficient() {
        let mut t = test_treasury();
        let available = t.available();
        let result = t.mint_exact(available + 1, "alice", MintReason::Testing);
        assert!(result.is_err());
    }

    #[test]
    fn treasury_empty() {
        let mut t = Treasury::new(FortunePolicy::default());
        // No metrics = 0 max supply
        let result = t.mint(1, "alice", MintReason::Ubi);
        assert!(matches!(result, Err(crate::FortuneError::TreasuryEmpty)));
    }

    #[test]
    fn receive_reduces_circulation() {
        let mut t = test_treasury();
        t.mint(1000, "alice", MintReason::Initial).unwrap();
        t.receive(200, ReceiveReason::Demurrage);
        assert_eq!(t.in_circulation, 800);
        assert_eq!(t.returned_from_demurrage, 200);
    }

    #[test]
    fn cash_lock_unlock() {
        let mut t = test_treasury();
        t.mint(1000, "alice", MintReason::Initial).unwrap();

        t.lock_for_cash(300);
        assert_eq!(t.locked_in_cash, 300);
        assert_eq!(t.liquid_circulation(), 700);

        t.unlock_from_cash(300);
        assert_eq!(t.locked_in_cash, 0);
        assert_eq!(t.liquid_circulation(), 1000);
    }

    #[test]
    fn utilization_percentage() {
        let mut t = test_treasury();
        t.mint(55_250, "alice", MintReason::Testing).unwrap();
        // 55250 / 110500 = 50%
        assert!((t.utilization() - 50.0).abs() < 0.1);
    }

    #[test]
    fn status_snapshot() {
        let t = test_treasury();
        let status = t.status();
        assert_eq!(status.max_supply, 110_500);
        assert_eq!(status.available, 110_500);
        assert_eq!(status.in_circulation, 0);
    }

    #[test]
    fn metrics_update_changes_supply() {
        let mut t = test_treasury();
        assert_eq!(t.max_supply(), 110_500);

        t.update_metrics(NetworkMetrics {
            active_users: 200,
            total_ideas: 100,
            total_collectives: 5,
        });
        // 200*1000 + 100*10 + 5*5000 = 200000 + 1000 + 25000 = 226000
        assert_eq!(t.max_supply(), 226_000);
    }

    #[test]
    fn mint_history_pruning() {
        let mut t = test_treasury();
        t.metrics = NetworkMetrics {
            active_users: 10_000,
            total_ideas: 0,
            total_collectives: 0,
        };
        for i in 0..1100 {
            t.mint(1, &format!("user_{i}"), MintReason::Ubi).unwrap();
        }
        assert!(t.mint_history.len() <= 1000);
    }
}
