use serde::{Deserialize, Serialize};

use crate::policy::FlowBackTier;

/// Progressive redistribution — excess wealth flows back to the Commons.
///
/// From Conjunction Art. 6 §3: "Where holdings exceed personal need and harm
/// the equitable access of others, such holdings shall be subject to lawful
/// redistribution. This is not seizure. This is restoration."
///
/// Default tiers: 1% above 1M, 3% above 10M, 5% above 100M, 7% above 1B.
/// Marginal (each tier only applies to the amount ABOVE its threshold).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowBack {
    pub total_redistributed: i64,
    pub cycles_run: u64,
}

impl FlowBack {
    /// Create a new flow-back tracker with no redistribution history.
    pub fn new() -> Self {
        Self {
            total_redistributed: 0,
            cycles_run: 0,
        }
    }

    /// Calculate flow-back amount for a given balance.
    /// Uses marginal rates — each tier only applies to the excess above its threshold.
    pub fn calculate(&self, balance: i64, tiers: &[FlowBackTier]) -> i64 {
        if tiers.is_empty() || balance <= 0 {
            return 0;
        }

        let mut total_flow_back: f64 = 0.0;
        let mut sorted_tiers: Vec<&FlowBackTier> = tiers.iter().collect();
        sorted_tiers.sort_by_key(|t| t.threshold);

        for (i, tier) in sorted_tiers.iter().enumerate() {
            if balance <= tier.threshold {
                break;
            }

            let ceiling = sorted_tiers
                .get(i + 1)
                .map(|next| next.threshold)
                .unwrap_or(i64::MAX);

            let taxable = (balance.min(ceiling) - tier.threshold).max(0);
            total_flow_back += taxable as f64 * tier.rate;
        }

        total_flow_back.round() as i64
    }

    /// Preview flow-back for a given balance.
    pub fn preview(&self, balance: i64, tiers: &[FlowBackTier]) -> FlowBackPreview {
        let amount = self.calculate(balance, tiers);
        FlowBackPreview {
            balance,
            flow_back_amount: amount,
            balance_after: balance - amount,
            effective_rate: if balance > 0 {
                amount as f64 / balance as f64
            } else {
                0.0
            },
        }
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    /// Preview flow-back for all members across federated communities.
    ///
    /// `member_balances` maps community_id to a list of (pubkey, balance) pairs.
    /// Only communities visible under the federation scope are included.
    ///
    /// Returns a preview per member, useful for federation-wide redistribution dashboards.
    pub fn preview_for_federation(
        &self,
        member_balances: &std::collections::HashMap<String, Vec<(String, i64)>>,
        tiers: &[FlowBackTier],
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> Vec<(String, FlowBackPreview)> {
        let mut previews = Vec::new();
        for (community_id, members) in member_balances {
            if scope.is_visible(community_id) {
                for (pubkey, balance) in members {
                    let preview = self.preview(*balance, tiers);
                    previews.push((pubkey.clone(), preview));
                }
            }
        }
        previews
    }

    /// Calculate total flow-back across all members in federated communities.
    ///
    /// Returns the aggregate flow-back amount from all visible community members.
    pub fn total_for_federation(
        &self,
        member_balances: &std::collections::HashMap<String, Vec<(String, i64)>>,
        tiers: &[FlowBackTier],
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> i64 {
        let mut total = 0i64;
        for (community_id, members) in member_balances {
            if scope.is_visible(community_id) {
                for (_, balance) in members {
                    total += self.calculate(*balance, tiers);
                }
            }
        }
        total
    }
}

impl Default for FlowBack {
    fn default() -> Self {
        Self::new()
    }
}

/// Preview of flow-back for an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowBackPreview {
    pub balance: i64,
    pub flow_back_amount: i64,
    pub balance_after: i64,
    pub effective_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_tiers() -> Vec<FlowBackTier> {
        FlowBackTier::default_tiers()
    }

    #[test]
    fn zero_balance_no_flowback() {
        let fb = FlowBack::new();
        assert_eq!(fb.calculate(0, &default_tiers()), 0);
    }

    #[test]
    fn below_first_tier_no_flowback() {
        let fb = FlowBack::new();
        assert_eq!(fb.calculate(500_000, &default_tiers()), 0);
    }

    #[test]
    fn first_tier_only() {
        let fb = FlowBack::new();
        // 2M balance, first tier 1% above 1M
        // Taxable: 1M at 1% = 10,000
        let amount = fb.calculate(2_000_000, &default_tiers());
        assert_eq!(amount, 10_000);
    }

    #[test]
    fn two_tiers() {
        let fb = FlowBack::new();
        // 15M balance
        // Tier 1: 1% on 1M-10M = 9M * 0.01 = 90,000
        // Tier 2: 3% on 10M-15M = 5M * 0.03 = 150,000
        // Total: 240,000
        let amount = fb.calculate(15_000_000, &default_tiers());
        assert_eq!(amount, 240_000);
    }

    #[test]
    fn all_four_tiers() {
        let fb = FlowBack::new();
        // 2B balance
        // Tier 1: 1% on 1M-10M = 9M * 0.01 = 90,000
        // Tier 2: 3% on 10M-100M = 90M * 0.03 = 2,700,000
        // Tier 3: 5% on 100M-1B = 900M * 0.05 = 45,000,000
        // Tier 4: 7% on 1B-2B = 1B * 0.07 = 70,000,000
        // Total: 117,790,000
        let amount = fb.calculate(2_000_000_000, &default_tiers());
        assert_eq!(amount, 117_790_000);
    }

    #[test]
    fn marginal_rates_are_progressive() {
        let fb = FlowBack::new();
        let small = fb.calculate(2_000_000, &default_tiers());
        let medium = fb.calculate(20_000_000, &default_tiers());
        let large = fb.calculate(200_000_000, &default_tiers());

        // Effective rate increases with wealth
        let rate_small = small as f64 / 2_000_000.0;
        let rate_medium = medium as f64 / 20_000_000.0;
        let rate_large = large as f64 / 200_000_000.0;

        assert!(rate_small < rate_medium);
        assert!(rate_medium < rate_large);
    }

    #[test]
    fn preview_shows_effective_rate() {
        let fb = FlowBack::new();
        let preview = fb.preview(2_000_000, &default_tiers());
        assert_eq!(preview.flow_back_amount, 10_000);
        assert_eq!(preview.balance_after, 1_990_000);
        assert!((preview.effective_rate - 0.005).abs() < 0.001);
    }

    #[test]
    fn empty_tiers_no_flowback() {
        let fb = FlowBack::new();
        assert_eq!(fb.calculate(1_000_000_000, &[]), 0);
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    #[test]
    fn preview_for_federation_scoped() {
        let fb = FlowBack::new();
        let tiers = default_tiers();

        let mut member_balances = std::collections::HashMap::new();
        member_balances.insert(
            "comm1".to_string(),
            vec![("alice".to_string(), 2_000_000)],
        );
        member_balances.insert(
            "comm2".to_string(),
            vec![("bob".to_string(), 15_000_000)],
        );
        member_balances.insert(
            "comm3".to_string(),
            vec![("carol".to_string(), 500_000_000)],
        );

        // Scope to comm1 + comm2 only
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm2"],
        );
        let previews = fb.preview_for_federation(&member_balances, &tiers, &scope);
        assert_eq!(previews.len(), 2);

        let alice_preview = previews.iter().find(|(p, _)| p == "alice").unwrap();
        assert_eq!(alice_preview.1.flow_back_amount, 10_000); // 1% above 1M
        let bob_preview = previews.iter().find(|(p, _)| p == "bob").unwrap();
        assert_eq!(bob_preview.1.flow_back_amount, 240_000);
        // carol not visible
        assert!(!previews.iter().any(|(p, _)| p == "carol"));
    }

    #[test]
    fn total_for_federation_aggregates() {
        let fb = FlowBack::new();
        let tiers = default_tiers();

        let mut member_balances = std::collections::HashMap::new();
        member_balances.insert(
            "comm1".to_string(),
            vec![("alice".to_string(), 2_000_000)], // 10,000 flow-back
        );
        member_balances.insert(
            "comm2".to_string(),
            vec![("bob".to_string(), 2_000_000)], // 10,000 flow-back
        );
        member_balances.insert(
            "comm3".to_string(),
            vec![("carol".to_string(), 2_000_000)], // excluded
        );

        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm2"],
        );
        let total = fb.total_for_federation(&member_balances, &tiers, &scope);
        assert_eq!(total, 20_000); // 10,000 + 10,000
    }

    #[test]
    fn custom_tiers() {
        let fb = FlowBack::new();
        let tiers = vec![FlowBackTier::new(100, 0.10)]; // 10% above 100
        let amount = fb.calculate(200, &tiers);
        assert_eq!(amount, 10); // 100 * 0.10
    }
}
