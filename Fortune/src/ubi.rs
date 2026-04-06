use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use crate::balance::Ledger;
use crate::policy::FortunePolicy;
use crate::treasury::{MintReason, Treasury};

/// Universal Basic Income distributor — ensures every verified person
/// can claim a regular allocation of Cool.
///
/// From Conjunction Art. 7 §2: "Every Person shall be guaranteed the means
/// to live with dignity... These are not rewards for productivity. They are
/// the inheritance of kinship among the People."
///
/// The UBI pipeline has 6 eligibility checks, applied in order:
/// 1. Identity verified (Sybil resistance)
/// 2. Not flagged (suspicious activity)
/// 3. System not paused
/// 4. Balance below cap (pressure valve — encourages circulation)
/// 5. Off cooldown (one claim per period)
/// 6. Treasury has funds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UbiDistributor {
    pub verified_identities: HashSet<String>,
    pub flagged_accounts: HashSet<String>,
    pub last_claims: std::collections::HashMap<String, DateTime<Utc>>,
    pub total_distributed: i64,
    pub total_claims: u64,
    pub is_paused: bool,
}

impl UbiDistributor {
    /// Create a new UBI distributor with no verified identities or claims.
    pub fn new() -> Self {
        Self {
            verified_identities: HashSet::new(),
            flagged_accounts: HashSet::new(),
            last_claims: std::collections::HashMap::new(),
            total_distributed: 0,
            total_claims: 0,
            is_paused: false,
        }
    }

    /// Register a verified identity (eligible for UBI).
    pub fn verify_identity(&mut self, pubkey: impl Into<String>) {
        self.verified_identities.insert(pubkey.into());
    }

    /// Flag an account (temporarily ineligible).
    pub fn flag_account(&mut self, pubkey: impl Into<String>) {
        self.flagged_accounts.insert(pubkey.into());
    }

    /// Unflag an account.
    pub fn unflag_account(&mut self, pubkey: &str) {
        self.flagged_accounts.remove(pubkey);
    }

    /// Check eligibility — the 6-step pipeline.
    pub fn check_eligibility(
        &self,
        pubkey: &str,
        ledger: &Ledger,
        treasury: &Treasury,
        policy: &FortunePolicy,
    ) -> UbiEligibility {
        // 1. Identity verification
        if !self.verified_identities.contains(pubkey) {
            return UbiEligibility::ineligible(IneligibilityReason::NotVerified);
        }

        // 2. Flagged check
        if self.flagged_accounts.contains(pubkey) {
            return UbiEligibility::ineligible(IneligibilityReason::Flagged);
        }

        // 3. System paused
        if self.is_paused {
            return UbiEligibility::ineligible(IneligibilityReason::Paused);
        }

        // 4. Balance cap (pressure valve)
        let balance = ledger.balance(pubkey);
        if balance.total() >= policy.ubi_balance_cap {
            return UbiEligibility::ineligible(IneligibilityReason::BalanceCapped);
        }

        // 5. Cooldown
        if let Some(last_claim) = self.last_claims.get(pubkey) {
            let cooldown_seconds = policy.ubi_cooldown_seconds();
            let elapsed = Utc::now()
                .signed_duration_since(*last_claim)
                .num_seconds();
            if elapsed < cooldown_seconds {
                return UbiEligibility {
                    eligible: false,
                    reason: Some(IneligibilityReason::OnCooldown),
                    next_claim_at: Some(
                        *last_claim + chrono::Duration::seconds(cooldown_seconds),
                    ),
                    claim_amount: 0,
                };
            }
        }

        // 6. Treasury has funds
        if treasury.available() < policy.ubi_amount {
            return UbiEligibility::ineligible(IneligibilityReason::TreasuryDepleted);
        }

        UbiEligibility {
            eligible: true,
            reason: None,
            next_claim_at: None,
            claim_amount: policy.ubi_amount,
        }
    }

    /// Claim UBI. Checks eligibility, mints from treasury, credits to ledger.
    pub fn claim(
        &mut self,
        pubkey: &str,
        ledger: &mut Ledger,
        treasury: &mut Treasury,
        policy: &FortunePolicy,
    ) -> Result<ClaimRecord, crate::FortuneError> {
        let eligibility = self.check_eligibility(pubkey, ledger, treasury, policy);
        if !eligibility.eligible {
            return match eligibility.reason {
                Some(IneligibilityReason::OnCooldown) => {
                    let next = eligibility
                        .next_claim_at
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_default();
                    Err(crate::FortuneError::UbiOnCooldown(next))
                }
                Some(reason) => Err(crate::FortuneError::UbiNotEligible(format!("{reason:?}"))),
                None => Err(crate::FortuneError::UbiNotEligible("unknown".into())),
            };
        }

        let balance_before = ledger.balance(pubkey).total();

        // Mint from treasury
        let minted = treasury.mint(policy.ubi_amount, pubkey, MintReason::Ubi)?;

        // Credit to ledger
        ledger.credit(
            pubkey,
            minted,
            crate::balance::TransactionReason::Ubi,
            None,
        );

        // Track claim
        self.last_claims.insert(pubkey.into(), Utc::now());
        self.total_distributed += minted;
        self.total_claims += 1;

        let balance_after = ledger.balance(pubkey).total();

        Ok(ClaimRecord {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            amount: minted,
            claimed_at: Utc::now(),
            balance_before,
            balance_after,
        })
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    /// Check UBI eligibility for members across federated communities.
    ///
    /// `members` maps community_id to pubkeys. Only communities visible under
    /// the federation scope are checked. Returns (pubkey, eligibility) pairs.
    ///
    /// Useful for federation-wide dashboards showing UBI coverage.
    pub fn check_eligibility_for_federation(
        &self,
        members: &std::collections::HashMap<String, Vec<String>>,
        ledger: &Ledger,
        treasury: &Treasury,
        policy: &FortunePolicy,
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> Vec<(String, UbiEligibility)> {
        let mut results = Vec::new();
        for (community_id, pubkeys) in members {
            if scope.is_visible(community_id) {
                for pubkey in pubkeys {
                    let eligibility = self.check_eligibility(pubkey, ledger, treasury, policy);
                    results.push((pubkey.clone(), eligibility));
                }
            }
        }
        results
    }
}

impl Default for UbiDistributor {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of an eligibility check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UbiEligibility {
    pub eligible: bool,
    pub reason: Option<IneligibilityReason>,
    pub next_claim_at: Option<DateTime<Utc>>,
    pub claim_amount: i64,
}

impl UbiEligibility {
    fn ineligible(reason: IneligibilityReason) -> Self {
        Self {
            eligible: false,
            reason: Some(reason),
            next_claim_at: None,
            claim_amount: 0,
        }
    }
}

/// Why someone can't claim UBI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IneligibilityReason {
    /// Identity not verified (Sybil resistance).
    NotVerified,
    /// Account flagged for suspicious activity.
    Flagged,
    /// UBI distribution is paused system-wide.
    Paused,
    /// Balance is at or above the cap (encourages spending).
    BalanceCapped,
    /// Haven't waited long enough since last claim.
    OnCooldown,
    /// Treasury has no Cool available.
    TreasuryDepleted,
}

/// Record of a UBI claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaimRecord {
    pub id: Uuid,
    pub pubkey: String,
    pub amount: i64,
    pub claimed_at: DateTime<Utc>,
    pub balance_before: i64,
    pub balance_after: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treasury::NetworkMetrics;

    fn setup() -> (UbiDistributor, Ledger, Treasury, FortunePolicy) {
        let policy = FortunePolicy::testing();
        let mut treasury = Treasury::new(policy.clone());
        treasury.update_metrics(NetworkMetrics {
            active_users: 100,
            total_ideas: 0,
            total_collectives: 0,
        });
        let ledger = Ledger::new();
        let mut ubi = UbiDistributor::new();
        ubi.verify_identity("alice");
        (ubi, ledger, treasury, policy)
    }

    #[test]
    fn eligible_and_claim() {
        let (mut ubi, mut ledger, mut treasury, policy) = setup();
        let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
        assert!(elig.eligible);
        assert_eq!(elig.claim_amount, policy.ubi_amount);

        let record = ubi.claim("alice", &mut ledger, &mut treasury, &policy).unwrap();
        assert_eq!(record.amount, policy.ubi_amount);
        assert_eq!(record.balance_before, 0);
        assert_eq!(record.balance_after, policy.ubi_amount);
        assert_eq!(ubi.total_claims, 1);
    }

    #[test]
    fn not_verified_ineligible() {
        let (ubi, ledger, treasury, policy) = setup();
        let elig = ubi.check_eligibility("unverified", &ledger, &treasury, &policy);
        assert!(!elig.eligible);
        assert_eq!(elig.reason, Some(IneligibilityReason::NotVerified));
    }

    #[test]
    fn flagged_ineligible() {
        let (mut ubi, ledger, treasury, policy) = setup();
        ubi.flag_account("alice");
        let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
        assert!(!elig.eligible);
        assert_eq!(elig.reason, Some(IneligibilityReason::Flagged));

        ubi.unflag_account("alice");
        let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
        assert!(elig.eligible);
    }

    #[test]
    fn paused_ineligible() {
        let (mut ubi, ledger, treasury, policy) = setup();
        ubi.is_paused = true;
        let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
        assert!(!elig.eligible);
        assert_eq!(elig.reason, Some(IneligibilityReason::Paused));
    }

    #[test]
    fn balance_cap_ineligible() {
        let (ubi, mut ledger, treasury, policy) = setup();
        // Give alice more than the cap
        ledger.credit(
            "alice",
            policy.ubi_balance_cap,
            crate::balance::TransactionReason::Initial,
            None,
        );
        let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
        assert!(!elig.eligible);
        assert_eq!(elig.reason, Some(IneligibilityReason::BalanceCapped));
    }

    #[test]
    fn cooldown_ineligible() {
        let (mut ubi, mut ledger, _treasury, _policy) = setup();
        // Use default policy (24h cooldown) for this test
        let strict_policy = FortunePolicy::default_policy();
        let mut strict_treasury = Treasury::new(strict_policy.clone());
        strict_treasury.update_metrics(NetworkMetrics {
            active_users: 100,
            total_ideas: 0,
            total_collectives: 0,
        });

        // First claim succeeds
        ubi.claim("alice", &mut ledger, &mut strict_treasury, &strict_policy)
            .unwrap();

        // Second claim within cooldown fails
        let result = ubi.claim("alice", &mut ledger, &mut strict_treasury, &strict_policy);
        assert!(result.is_err());
    }

    #[test]
    fn treasury_depleted_ineligible() {
        let policy = FortunePolicy::default();
        let treasury = Treasury::new(policy.clone()); // no metrics = 0 supply
        let ledger = Ledger::new();
        let mut ubi = UbiDistributor::new();
        ubi.verify_identity("alice");

        let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
        assert!(!elig.eligible);
        assert_eq!(elig.reason, Some(IneligibilityReason::TreasuryDepleted));
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    #[test]
    fn check_eligibility_for_federation_scoped() {
        let (ubi, ledger, treasury, policy) = setup();

        let mut members = std::collections::HashMap::new();
        members.insert("comm1".to_string(), vec!["alice".to_string()]);
        members.insert("comm2".to_string(), vec!["unverified".to_string()]);

        // Scope to comm1 only — only alice checked
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(["comm1"]);
        let results =
            ubi.check_eligibility_for_federation(&members, &ledger, &treasury, &policy, &scope);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "alice");
        assert!(results[0].1.eligible);
    }

    #[test]
    fn check_eligibility_for_federation_unrestricted() {
        let (ubi, ledger, treasury, policy) = setup();

        let mut members = std::collections::HashMap::new();
        members.insert("comm1".to_string(), vec!["alice".to_string()]);
        members.insert("comm2".to_string(), vec!["unverified".to_string()]);

        // Unrestricted — both checked
        let scope = crate::federation_scope::EconomicFederationScope::new();
        let results =
            ubi.check_eligibility_for_federation(&members, &ledger, &treasury, &policy, &scope);
        assert_eq!(results.len(), 2);

        let alice = results.iter().find(|(p, _)| p == "alice").unwrap();
        assert!(alice.1.eligible);
        let unverified = results.iter().find(|(p, _)| p == "unverified").unwrap();
        assert!(!unverified.1.eligible);
    }

    #[test]
    fn multiple_claims_track_correctly() {
        let (mut ubi, mut ledger, mut treasury, policy) = setup();
        ubi.verify_identity("bob");

        ubi.claim("alice", &mut ledger, &mut treasury, &policy).unwrap();
        ubi.claim("bob", &mut ledger, &mut treasury, &policy).unwrap();

        assert_eq!(ubi.total_claims, 2);
        assert_eq!(ubi.total_distributed, policy.ubi_amount * 2);
    }
}
