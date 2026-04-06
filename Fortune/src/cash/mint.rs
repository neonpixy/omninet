use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::note::{generate_serial, CashNote, CashStatus};
use super::registry::CashRegistry;
use crate::balance::Ledger;
use crate::policy::FortunePolicy;
use crate::treasury::Treasury;

/// Issues Cash notes — locks Cool in the issuer's balance.
///
/// Rate-limited: max N issuances per hour per user (configurable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashMint {
    issuance_timestamps: HashMap<String, Vec<DateTime<Utc>>>,
    pub total_issued: i64,
    pub notes_issued: u64,
}

impl CashMint {
    /// Create a new mint with no issuance history.
    pub fn new() -> Self {
        Self {
            issuance_timestamps: HashMap::new(),
            total_issued: 0,
            notes_issued: 0,
        }
    }

    /// Issue a new Cash note. Locks Cool in the issuer's balance.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        &mut self,
        issuer: &str,
        amount: i64,
        memo: Option<String>,
        expiry_days: Option<u32>,
        ledger: &mut Ledger,
        treasury: &mut Treasury,
        registry: &mut CashRegistry,
        policy: &FortunePolicy,
    ) -> Result<CashNote, crate::FortuneError> {
        // Validate amount
        if amount < policy.cash_min_amount {
            return Err(crate::FortuneError::CashIssuanceFailed(format!(
                "Amount must be at least {}",
                policy.cash_min_amount
            )));
        }
        if amount > policy.cash_max_amount {
            return Err(crate::FortuneError::CashIssuanceFailed(format!(
                "Amount cannot exceed {}",
                policy.cash_max_amount
            )));
        }

        // Rate limit
        self.check_rate_limit(issuer, policy)?;

        // Generate serial
        let serial = generate_serial();

        // Lock Cool in ledger
        ledger.lock(issuer, amount, &serial)?;

        // Track in treasury
        treasury.lock_for_cash(amount);

        // Calculate expiry
        let expiry = expiry_days.unwrap_or(policy.cash_default_expiry_days);
        let expires_at = Utc::now() + chrono::Duration::days(i64::from(expiry));

        let note = CashNote {
            serial: serial.clone(),
            amount,
            issuer: issuer.into(),
            issued_at: Utc::now(),
            expires_at,
            memo,
            status: CashStatus::Active,
            redeemer: None,
            redeemed_at: None,
            revocation_reason: None,
        };

        registry.register(note.clone());
        self.record_issuance(issuer);
        self.total_issued += amount;
        self.notes_issued += 1;

        Ok(note)
    }

    fn check_rate_limit(
        &self,
        issuer: &str,
        policy: &FortunePolicy,
    ) -> Result<(), crate::FortuneError> {
        if let Some(timestamps) = self.issuance_timestamps.get(issuer) {
            let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
            let recent = timestamps.iter().filter(|t| **t > one_hour_ago).count();
            if recent >= policy.cash_max_per_hour as usize {
                return Err(crate::FortuneError::RateLimitExceeded(
                    "Cash issuance rate limit exceeded".into(),
                ));
            }
        }
        Ok(())
    }

    fn record_issuance(&mut self, issuer: &str) {
        self.issuance_timestamps
            .entry(issuer.into())
            .or_default()
            .push(Utc::now());
    }

    /// Revoke an active Cash note, returning Cool to issuer.
    pub fn revoke(
        &mut self,
        serial: &str,
        reason: &str,
        ledger: &mut Ledger,
        treasury: &mut Treasury,
        registry: &mut CashRegistry,
    ) -> Result<(), crate::FortuneError> {
        let note = registry
            .note(serial)
            .ok_or_else(|| crate::FortuneError::CashNotFound(serial.into()))?;

        if note.status != CashStatus::Active {
            return Err(crate::FortuneError::CashIssuanceFailed(
                "Note is not active".into(),
            ));
        }

        let issuer = note.issuer.clone();
        let amount = note.amount;

        ledger.unlock(&issuer, amount, serial, true);
        treasury.unlock_from_cash(amount);
        registry.mark_revoked(serial, reason);

        Ok(())
    }
}

impl Default for CashMint {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balance::TransactionReason;
    use crate::treasury::NetworkMetrics;

    fn setup() -> (CashMint, Ledger, Treasury, CashRegistry, FortunePolicy) {
        let policy = FortunePolicy::testing();
        let mut treasury = Treasury::new(policy.clone());
        treasury.update_metrics(NetworkMetrics {
            active_users: 100,
            total_ideas: 0,
            total_collectives: 0,
        });
        let mut ledger = Ledger::new();
        treasury
            .mint(10_000, "alice", crate::treasury::MintReason::Initial)
            .unwrap();
        ledger.credit("alice", 10_000, TransactionReason::Initial, None);

        (CashMint::new(), ledger, treasury, CashRegistry::new(), policy)
    }

    #[test]
    fn issue_cash_note() {
        let (mut mint, mut ledger, mut treasury, mut registry, policy) = setup();
        let note = mint
            .issue(
                "alice",
                100,
                Some("Groceries".into()),
                None,
                &mut ledger,
                &mut treasury,
                &mut registry,
                &policy,
            )
            .unwrap();

        assert_eq!(note.amount, 100);
        assert_eq!(note.issuer, "alice");
        assert!(note.is_active());
        assert_eq!(ledger.balance("alice").liquid, 9900);
        assert_eq!(ledger.balance("alice").locked, 100);
        assert_eq!(mint.notes_issued, 1);
    }

    #[test]
    fn amount_too_small() {
        let (mut mint, mut ledger, mut treasury, mut registry, policy) = setup();
        let result = mint.issue(
            "alice",
            0,
            None,
            None,
            &mut ledger,
            &mut treasury,
            &mut registry,
            &policy,
        );
        assert!(result.is_err());
    }

    #[test]
    fn amount_too_large() {
        let (mut mint, mut ledger, mut treasury, mut registry, policy) = setup();
        let result = mint.issue(
            "alice",
            policy.cash_max_amount + 1,
            None,
            None,
            &mut ledger,
            &mut treasury,
            &mut registry,
            &policy,
        );
        assert!(result.is_err());
    }

    #[test]
    fn insufficient_balance_for_cash() {
        let (mut mint, mut ledger, mut treasury, mut registry, policy) = setup();
        // Alice has 10_000, try to issue 20_000
        let result = mint.issue(
            "alice",
            20_000,
            None,
            None,
            &mut ledger,
            &mut treasury,
            &mut registry,
            &policy,
        );
        assert!(result.is_err());
    }

    #[test]
    fn revoke_cash_note() {
        let (mut mint, mut ledger, mut treasury, mut registry, policy) = setup();
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
        let serial = note.serial.clone();

        mint.revoke(
            &serial,
            "Issued in error",
            &mut ledger,
            &mut treasury,
            &mut registry,
        )
        .unwrap();

        let revoked = registry.note(&serial).unwrap();
        assert_eq!(revoked.status, CashStatus::Revoked);
        assert_eq!(ledger.balance("alice").liquid, 10_000); // Cool returned
        assert_eq!(ledger.balance("alice").locked, 0);
    }
}
