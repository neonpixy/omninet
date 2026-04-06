use serde::{Deserialize, Serialize};

/// All configurable economic parameters for a Fortune instance.
///
/// From Consortium Art. 2 §1: "Commerce within the jurisdiction of this Covenant
/// shall be regenerative, participatory, and life-affirming."
///
/// Three presets: default (production), testing (fast iteration), conservative (early launch).
/// Communities choose parameters within Covenant bounds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FortunePolicy {
    // Supply capacity
    pub cool_per_active_user: i64,
    pub cool_per_idea: i64,
    pub cool_per_collective: i64,

    // UBI parameters
    pub ubi_amount: i64,
    pub ubi_cooldown_hours: u32,
    pub ubi_balance_cap: i64,

    // Demurrage parameters
    pub demurrage_rate: f64,
    pub demurrage_exemption_floor: i64,
    pub demurrage_cycle_hours: u32,

    // Cash parameters
    pub cash_min_amount: i64,
    pub cash_max_amount: i64,
    pub cash_default_expiry_days: u32,
    pub cash_max_per_hour: u32,

    // Flow-back tiers
    pub flow_back_tiers: Vec<FlowBackTier>,

    // Rate limiting
    pub transfer_rate_limit_per_hour: u32,

    // Initial mint for new users
    pub initial_mint_amount: i64,
}

impl FortunePolicy {
    /// Production defaults — balanced for real-world use.
    pub fn default_policy() -> Self {
        Self {
            cool_per_active_user: 1000,
            cool_per_idea: 10,
            cool_per_collective: 5000,
            ubi_amount: 10,
            ubi_cooldown_hours: 24,
            ubi_balance_cap: 500,
            demurrage_rate: 0.03,
            demurrage_exemption_floor: 10,
            demurrage_cycle_hours: 24,
            cash_min_amount: 1,
            cash_max_amount: 10_000,
            cash_default_expiry_days: 365,
            cash_max_per_hour: 10,
            flow_back_tiers: FlowBackTier::default_tiers(),
            transfer_rate_limit_per_hour: 30,
            initial_mint_amount: 100,
        }
    }

    /// Testing preset — fast iteration, generous limits.
    pub fn testing() -> Self {
        Self {
            cool_per_active_user: 10_000,
            cool_per_idea: 100,
            cool_per_collective: 50_000,
            ubi_amount: 100,
            ubi_cooldown_hours: 0, // no cooldown
            ubi_balance_cap: 5000,
            demurrage_rate: 0.10,
            demurrage_exemption_floor: 10,
            demurrage_cycle_hours: 1, // every hour
            cash_min_amount: 1,
            cash_max_amount: 100_000,
            cash_default_expiry_days: 1,
            cash_max_per_hour: 100,
            flow_back_tiers: FlowBackTier::default_tiers(),
            transfer_rate_limit_per_hour: 1000,
            initial_mint_amount: 1000,
        }
    }

    /// Conservative preset — tight constraints for early launch.
    pub fn conservative() -> Self {
        Self {
            cool_per_active_user: 500,
            cool_per_idea: 5,
            cool_per_collective: 2500,
            ubi_amount: 5,
            ubi_cooldown_hours: 24,
            ubi_balance_cap: 250,
            demurrage_rate: 0.02,
            demurrage_exemption_floor: 20,
            demurrage_cycle_hours: 24,
            cash_min_amount: 5,
            cash_max_amount: 1000,
            cash_default_expiry_days: 180,
            cash_max_per_hour: 5,
            flow_back_tiers: FlowBackTier::default_tiers(),
            transfer_rate_limit_per_hour: 10,
            initial_mint_amount: 50,
        }
    }

    /// Daily demurrage rate derived from monthly rate.
    pub fn daily_demurrage_rate(&self) -> f64 {
        self.demurrage_rate / 30.0
    }

    /// UBI cooldown in seconds.
    pub fn ubi_cooldown_seconds(&self) -> i64 {
        i64::from(self.ubi_cooldown_hours) * 3600
    }

    /// Cash default expiry in seconds.
    pub fn cash_default_expiry_seconds(&self) -> i64 {
        i64::from(self.cash_default_expiry_days) * 86400
    }
}

impl Default for FortunePolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/// A progressive redistribution tier.
///
/// From Conjunction Art. 6 §3: "Where holdings exceed personal need and harm
/// the equitable access of others, such holdings shall be subject to lawful
/// redistribution."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowBackTier {
    /// Balance threshold above which this tier applies.
    pub threshold: i64,
    /// Rate of redistribution (0.0 to 1.0).
    pub rate: f64,
}

impl FlowBackTier {
    /// Create a new tier with a threshold and rate (clamped to 0.0-1.0).
    pub fn new(threshold: i64, rate: f64) -> Self {
        Self {
            threshold,
            rate: rate.clamp(0.0, 1.0),
        }
    }

    /// Default progressive tiers: 1% above 1M, 3% above 10M, 5% above 100M, 7% above 1B.
    pub fn default_tiers() -> Vec<Self> {
        vec![
            Self::new(1_000_000, 0.01),
            Self::new(10_000_000, 0.03),
            Self::new(100_000_000, 0.05),
            Self::new(1_000_000_000, 0.07),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_values() {
        let p = FortunePolicy::default();
        assert_eq!(p.cool_per_active_user, 1000);
        assert_eq!(p.ubi_amount, 10);
        assert_eq!(p.ubi_cooldown_hours, 24);
        assert_eq!(p.ubi_balance_cap, 500);
        assert_eq!(p.demurrage_rate, 0.03);
        assert_eq!(p.demurrage_exemption_floor, 10);
        assert_eq!(p.cash_min_amount, 1);
        assert_eq!(p.cash_max_amount, 10_000);
        assert_eq!(p.initial_mint_amount, 100);
    }

    #[test]
    fn testing_policy_generous() {
        let p = FortunePolicy::testing();
        assert!(p.cool_per_active_user > FortunePolicy::default().cool_per_active_user);
        assert!(p.ubi_amount > FortunePolicy::default().ubi_amount);
        assert_eq!(p.ubi_cooldown_hours, 0);
    }

    #[test]
    fn conservative_policy_tight() {
        let p = FortunePolicy::conservative();
        assert!(p.cool_per_active_user < FortunePolicy::default().cool_per_active_user);
        assert!(p.ubi_amount < FortunePolicy::default().ubi_amount);
        assert!(p.cash_max_amount < FortunePolicy::default().cash_max_amount);
    }

    #[test]
    fn daily_demurrage_rate() {
        let p = FortunePolicy::default();
        let daily = p.daily_demurrage_rate();
        assert!((daily - 0.001).abs() < 0.0001);
    }

    #[test]
    fn flow_back_tiers_progressive() {
        let tiers = FlowBackTier::default_tiers();
        assert_eq!(tiers.len(), 4);
        for i in 1..tiers.len() {
            assert!(tiers[i].threshold > tiers[i - 1].threshold);
            assert!(tiers[i].rate > tiers[i - 1].rate);
        }
    }

    #[test]
    fn flow_back_tier_rate_clamped() {
        let tier = FlowBackTier::new(100, 1.5);
        assert_eq!(tier.rate, 1.0);

        let tier = FlowBackTier::new(100, -0.5);
        assert_eq!(tier.rate, 0.0);
    }

    #[test]
    fn policy_serialization_roundtrip() {
        let p = FortunePolicy::default();
        let json = serde_json::to_string(&p).unwrap();
        let restored: FortunePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(p, restored);
    }

    #[test]
    fn cooldown_and_expiry_conversions() {
        let p = FortunePolicy::default();
        assert_eq!(p.ubi_cooldown_seconds(), 86400);
        assert_eq!(p.cash_default_expiry_seconds(), 31_536_000);
    }
}
