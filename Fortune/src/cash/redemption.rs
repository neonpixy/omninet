use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::note::CashStatus;
use super::registry::CashRegistry;
use crate::balance::{Ledger, TransactionReason};
use crate::treasury::Treasury;

/// Handles Cash note redemption — validates and transfers Cool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashRedemption {
    pub total_redeemed: i64,
    pub notes_redeemed: u64,
}

impl CashRedemption {
    /// Create a new redemption tracker with no history.
    pub fn new() -> Self {
        Self {
            total_redeemed: 0,
            notes_redeemed: 0,
        }
    }

    /// Redeem a Cash note. Transfers Cool from issuer's locked to redeemer's liquid.
    pub fn redeem(
        &mut self,
        serial: &str,
        redeemer: &str,
        ledger: &mut Ledger,
        treasury: &mut Treasury,
        registry: &mut CashRegistry,
    ) -> Result<RedemptionResult, crate::FortuneError> {
        let note = registry
            .note(serial)
            .ok_or_else(|| crate::FortuneError::CashNotFound(serial.into()))?;

        // Validate status
        match note.status {
            CashStatus::Redeemed => {
                return Err(crate::FortuneError::CashAlreadyRedeemed(serial.into()));
            }
            CashStatus::Expired => {
                return Err(crate::FortuneError::CashExpired(serial.into()));
            }
            CashStatus::Revoked => {
                return Err(crate::FortuneError::CashRevoked(serial.into()));
            }
            CashStatus::Active => {}
        }

        // Check expiration
        if note.is_expired() {
            registry.mark_expired(serial);
            return Err(crate::FortuneError::CashExpired(serial.into()));
        }

        let issuer = note.issuer.clone();
        let amount = note.amount;

        // Unlock from issuer WITHOUT returning to them
        ledger.unlock(&issuer, amount, serial, false);

        // Credit redeemer
        ledger.credit(redeemer, amount, TransactionReason::CashRedeemed, Some(serial.into()));

        // Update treasury
        treasury.unlock_from_cash(amount);

        // Mark as redeemed
        registry.mark_redeemed(serial, redeemer);

        self.total_redeemed += amount;
        self.notes_redeemed += 1;

        Ok(RedemptionResult {
            serial: serial.into(),
            amount,
            redeemer: redeemer.into(),
            issuer,
            redeemed_at: Utc::now(),
        })
    }

    /// Process all expired Cash notes — return Cool to issuers.
    pub fn process_expirations(
        &self,
        ledger: &mut Ledger,
        treasury: &mut Treasury,
        registry: &mut CashRegistry,
    ) -> ExpirationResult {
        let expired: Vec<(String, String, i64)> = registry
            .expired_unprocessed()
            .iter()
            .map(|n| (n.serial.clone(), n.issuer.clone(), n.amount))
            .collect();

        let mut expired_count = 0u32;
        let mut total_unlocked: i64 = 0;

        for (serial, issuer, amount) in &expired {
            ledger.unlock(issuer, *amount, serial, true);
            treasury.unlock_from_cash(*amount);
            registry.mark_expired(serial);
            expired_count += 1;
            total_unlocked += amount;
        }

        ExpirationResult {
            processed_at: Utc::now(),
            expired_count,
            total_unlocked,
        }
    }
}

impl Default for CashRedemption {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a successful redemption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedemptionResult {
    pub serial: String,
    pub amount: i64,
    pub redeemer: String,
    pub issuer: String,
    pub redeemed_at: DateTime<Utc>,
}

/// Result of processing expired notes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpirationResult {
    pub processed_at: DateTime<Utc>,
    pub expired_count: u32,
    pub total_unlocked: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cash::mint::CashMint;
    use crate::policy::FortunePolicy;
    use crate::treasury::{MintReason, NetworkMetrics};

    fn setup() -> (
        CashMint,
        CashRedemption,
        Ledger,
        Treasury,
        CashRegistry,
        FortunePolicy,
    ) {
        let policy = FortunePolicy::testing();
        let mut treasury = Treasury::new(policy.clone());
        treasury.update_metrics(NetworkMetrics {
            active_users: 100,
            total_ideas: 0,
            total_collectives: 0,
        });
        let mut ledger = Ledger::new();
        treasury.mint(10_000, "alice", MintReason::Initial).unwrap();
        ledger.credit("alice", 10_000, TransactionReason::Initial, None);

        (
            CashMint::new(),
            CashRedemption::new(),
            ledger,
            treasury,
            CashRegistry::new(),
            policy,
        )
    }

    #[test]
    fn full_cash_lifecycle() {
        let (mut mint, mut redemption, mut ledger, mut treasury, mut registry, policy) = setup();

        // Issue
        let note = mint
            .issue(
                "alice",
                100,
                None,
                None,
                &mut ledger,
                &mut treasury,
                &mut registry,
                &policy,
            )
            .unwrap();
        assert_eq!(ledger.balance("alice").liquid, 9900);
        assert_eq!(ledger.balance("alice").locked, 100);

        // Redeem (by bob)
        let result = redemption
            .redeem(
                &note.serial,
                "bob",
                &mut ledger,
                &mut treasury,
                &mut registry,
            )
            .unwrap();

        assert_eq!(result.amount, 100);
        assert_eq!(result.redeemer, "bob");
        assert_eq!(ledger.balance("bob").liquid, 100);
        assert_eq!(ledger.balance("alice").locked, 0);
    }

    #[test]
    fn cannot_redeem_twice() {
        let (mut mint, mut redemption, mut ledger, mut treasury, mut registry, policy) = setup();

        let note = mint
            .issue(
                "alice",
                50,
                None,
                None,
                &mut ledger,
                &mut treasury,
                &mut registry,
                &policy,
            )
            .unwrap();

        redemption
            .redeem(
                &note.serial,
                "bob",
                &mut ledger,
                &mut treasury,
                &mut registry,
            )
            .unwrap();

        let result = redemption.redeem(
            &note.serial,
            "charlie",
            &mut ledger,
            &mut treasury,
            &mut registry,
        );
        assert!(result.is_err());
    }

    #[test]
    fn balance_conservation_through_cash() {
        let (mut mint, mut redemption, mut ledger, mut treasury, mut registry, policy) = setup();

        let before_alice = ledger.balance("alice").total();

        let note = mint
            .issue(
                "alice",
                200,
                None,
                None,
                &mut ledger,
                &mut treasury,
                &mut registry,
                &policy,
            )
            .unwrap();

        // Alice's total unchanged (liquid→locked)
        assert_eq!(ledger.balance("alice").total(), before_alice);

        // Bob redeems
        redemption
            .redeem(
                &note.serial,
                "bob",
                &mut ledger,
                &mut treasury,
                &mut registry,
            )
            .unwrap();

        // Alice lost 200 from total, bob gained 200
        assert_eq!(ledger.balance("alice").total(), before_alice - 200);
        assert_eq!(ledger.balance("bob").total(), 200);
    }
}
