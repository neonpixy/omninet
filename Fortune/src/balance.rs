use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A person's Cool balance — liquid (spendable) and locked (backing Cash).
///
/// From Conjunction Art. 7 §2: "Every Person shall be guaranteed the means
/// to live with dignity... These are not rewards for productivity."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Balance {
    pub liquid: i64,
    pub locked: i64,
    pub last_demurrage: DateTime<Utc>,
}

impl Balance {
    /// Create a zero balance — no liquid, no locked Cool.
    pub fn zero() -> Self {
        Self {
            liquid: 0,
            locked: 0,
            last_demurrage: Utc::now(),
        }
    }

    /// Total Cool held — liquid plus locked.
    pub fn total(&self) -> i64 {
        self.liquid + self.locked
    }
}

impl Default for Balance {
    fn default() -> Self {
        Self::zero()
    }
}

/// The ledger — tracks all balances and records all transactions.
///
/// From Consortium Art. 2 §5: "Consortia shall conduct their economic affairs
/// with full transparency and public accountability."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    balances: HashMap<String, Balance>,
    transactions: Vec<Transaction>,
}

impl Ledger {
    /// Create an empty ledger with no accounts or transactions.
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            transactions: Vec::new(),
        }
    }

    /// Look up a person's balance. Returns zero if the account doesn't exist.
    pub fn balance(&self, pubkey: &str) -> Balance {
        self.balances.get(pubkey).cloned().unwrap_or_default()
    }

    /// Credit liquid balance.
    pub fn credit(
        &mut self,
        pubkey: &str,
        amount: i64,
        reason: TransactionReason,
        reference: Option<String>,
    ) {
        let balance = self.balances.entry(pubkey.into()).or_default();
        balance.liquid += amount;
        let after = balance.total();

        self.transactions.push(Transaction {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            transaction_type: TransactionType::Credit,
            amount,
            reason,
            reference,
            balance_after: after,
            timestamp: Utc::now(),
        });
    }

    /// Debit liquid balance. Fails if insufficient.
    pub fn debit(
        &mut self,
        pubkey: &str,
        amount: i64,
        reason: TransactionReason,
        reference: Option<String>,
    ) -> Result<(), crate::FortuneError> {
        let balance = self.balances.entry(pubkey.into()).or_default();
        if balance.liquid < amount {
            return Err(crate::FortuneError::InsufficientBalance {
                required: amount,
                available: balance.liquid,
            });
        }
        balance.liquid -= amount;
        let after = balance.total();

        self.transactions.push(Transaction {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            transaction_type: TransactionType::Debit,
            amount,
            reason,
            reference,
            balance_after: after,
            timestamp: Utc::now(),
        });
        Ok(())
    }

    /// Atomic transfer: debit sender + credit recipient.
    pub fn transfer(
        &mut self,
        sender: &str,
        recipient: &str,
        amount: i64,
        reference: Option<String>,
    ) -> Result<(), crate::FortuneError> {
        // Check sender has enough before making any changes
        let sender_balance = self.balance(sender);
        if sender_balance.liquid < amount {
            return Err(crate::FortuneError::InsufficientBalance {
                required: amount,
                available: sender_balance.liquid,
            });
        }
        self.debit(sender, amount, TransactionReason::Transfer, reference.clone())?;
        self.credit(recipient, amount, TransactionReason::Transfer, reference);
        Ok(())
    }

    /// Lock Cool (move from liquid to locked for Cash).
    pub fn lock(
        &mut self,
        pubkey: &str,
        amount: i64,
        cash_serial: &str,
    ) -> Result<(), crate::FortuneError> {
        let balance = self.balances.entry(pubkey.into()).or_default();
        if balance.liquid < amount {
            return Err(crate::FortuneError::InsufficientBalance {
                required: amount,
                available: balance.liquid,
            });
        }
        balance.liquid -= amount;
        balance.locked += amount;
        let after = balance.total();

        self.transactions.push(Transaction {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            transaction_type: TransactionType::Lock,
            amount,
            reason: TransactionReason::CashIssuance,
            reference: Some(cash_serial.into()),
            balance_after: after,
            timestamp: Utc::now(),
        });
        Ok(())
    }

    /// Unlock Cool (Cash redeemed or expired).
    pub fn unlock(
        &mut self,
        pubkey: &str,
        amount: i64,
        cash_serial: &str,
        return_to_issuer: bool,
    ) {
        let balance = self.balances.entry(pubkey.into()).or_default();
        balance.locked = (balance.locked - amount).max(0);
        if return_to_issuer {
            balance.liquid += amount;
        }
        let after = balance.total();

        self.transactions.push(Transaction {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            transaction_type: TransactionType::Unlock,
            amount,
            reason: if return_to_issuer {
                TransactionReason::CashExpired
            } else {
                TransactionReason::CashRedeemed
            },
            reference: Some(cash_serial.into()),
            balance_after: after,
            timestamp: Utc::now(),
        });
    }

    /// Apply demurrage decay to liquid balance. Returns actual amount decayed.
    pub fn apply_demurrage(&mut self, pubkey: &str, decay_amount: i64) -> i64 {
        let balance = self.balances.entry(pubkey.into()).or_default();
        let actual = decay_amount.min(balance.liquid);
        if actual > 0 {
            balance.liquid -= actual;
            balance.last_demurrage = Utc::now();
            let after = balance.total();

            self.transactions.push(Transaction {
                id: Uuid::new_v4(),
                pubkey: pubkey.into(),
                transaction_type: TransactionType::Debit,
                amount: actual,
                reason: TransactionReason::Demurrage,
                reference: None,
                balance_after: after,
                timestamp: Utc::now(),
            });
        }
        actual
    }

    /// Update demurrage timestamp without changing balance.
    pub fn update_demurrage_timestamp(&mut self, pubkey: &str) {
        if let Some(balance) = self.balances.get_mut(pubkey) {
            balance.last_demurrage = Utc::now();
        }
    }

    /// Get all accounts (for demurrage cycle processing).
    pub fn all_accounts(&self) -> Vec<(&str, &Balance)> {
        self.balances.iter().map(|(k, v)| (k.as_str(), v)).collect()
    }

    /// Get transactions for a pubkey.
    pub fn transactions_for(&self, pubkey: &str) -> Vec<&Transaction> {
        self.transactions
            .iter()
            .filter(|t| t.pubkey == pubkey)
            .collect()
    }

    /// Get a summary of transactions for a pubkey.
    pub fn summary(&self, pubkey: &str) -> TransactionSummary {
        let txns = self.transactions_for(pubkey);
        let mut summary = TransactionSummary {
            total_credits: 0,
            total_debits: 0,
            transaction_count: txns.len() as u32,
            ubi_received: 0,
            demurrage_paid: 0,
            transfers_sent: 0,
            transfers_received: 0,
        };

        for t in &txns {
            match t.transaction_type {
                TransactionType::Credit => {
                    summary.total_credits += t.amount;
                    if t.reason == TransactionReason::Ubi {
                        summary.ubi_received += t.amount;
                    }
                    if t.reason == TransactionReason::Transfer {
                        summary.transfers_received += t.amount;
                    }
                }
                TransactionType::Debit => {
                    summary.total_debits += t.amount;
                    if t.reason == TransactionReason::Demurrage {
                        summary.demurrage_paid += t.amount;
                    }
                    if t.reason == TransactionReason::Transfer {
                        summary.transfers_sent += t.amount;
                    }
                }
                TransactionType::Lock | TransactionType::Unlock => {}
            }
        }

        summary
    }

    /// Number of accounts tracked by this ledger.
    pub fn account_count(&self) -> usize {
        self.balances.len()
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    /// Get balances for a set of community members, scoped by federation.
    ///
    /// `members` maps community_id to a list of pubkeys in that community.
    /// Only communities visible under the federation scope are included.
    ///
    /// Returns (pubkey, Balance) pairs for all visible members.
    pub fn balances_for_federation(
        &self,
        members: &std::collections::HashMap<String, Vec<String>>,
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> Vec<(String, Balance)> {
        let mut result = Vec::new();
        for (community_id, pubkeys) in members {
            if scope.is_visible(community_id) {
                for pubkey in pubkeys {
                    result.push((pubkey.clone(), self.balance(pubkey)));
                }
            }
        }
        result
    }

    /// Aggregate transaction summary across federation-visible community members.
    ///
    /// `members` maps community_id to pubkeys. Only visible communities
    /// contribute to the summary. Useful for federation-wide economic dashboards.
    pub fn summary_for_federation(
        &self,
        members: &std::collections::HashMap<String, Vec<String>>,
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> TransactionSummary {
        let mut combined = TransactionSummary {
            total_credits: 0,
            total_debits: 0,
            transaction_count: 0,
            ubi_received: 0,
            demurrage_paid: 0,
            transfers_sent: 0,
            transfers_received: 0,
        };

        for (community_id, pubkeys) in members {
            if scope.is_visible(community_id) {
                for pubkey in pubkeys {
                    let s = self.summary(pubkey);
                    combined.total_credits += s.total_credits;
                    combined.total_debits += s.total_debits;
                    combined.transaction_count += s.transaction_count;
                    combined.ubi_received += s.ubi_received;
                    combined.demurrage_paid += s.demurrage_paid;
                    combined.transfers_sent += s.transfers_sent;
                    combined.transfers_received += s.transfers_received;
                }
            }
        }

        combined
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

/// A recorded transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub id: Uuid,
    pub pubkey: String,
    pub transaction_type: TransactionType,
    pub amount: i64,
    pub reason: TransactionReason,
    pub reference: Option<String>,
    pub balance_after: i64,
    pub timestamp: DateTime<Utc>,
}

/// Direction of a transaction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransactionType {
    /// Cool added to an account.
    Credit,
    /// Cool removed from an account.
    Debit,
    /// Cool moved from liquid to locked (backing a Cash note).
    Lock,
    /// Cool moved from locked back to liquid (Cash redeemed or expired).
    Unlock,
}

/// Why the transaction occurred.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransactionReason {
    /// Universal Basic Income distribution.
    Ubi,
    /// Person-to-person transfer.
    Transfer,
    /// Cash note redeemed by the holder.
    CashRedeemed,
    /// Reward for contribution or achievement.
    Reward,
    /// Initial allocation when an account is created.
    Initial,
    /// Purchase of a product or service.
    Purchase,
    /// Demurrage decay on idle balances.
    Demurrage,
    /// Cool locked to back a new Cash note.
    CashIssuance,
    /// Expired Cash note — locked Cool returned to issuer.
    CashExpired,
    /// Service or platform fee.
    Fee,
    /// Manual correction by governance.
    Correction,
}

/// Aggregated transaction stats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionSummary {
    pub total_credits: i64,
    pub total_debits: i64,
    pub transaction_count: u32,
    pub ubi_received: i64,
    pub demurrage_paid: i64,
    pub transfers_sent: i64,
    pub transfers_received: i64,
}

impl TransactionSummary {
    /// Net change (credits minus debits) across all transactions.
    pub fn net_change(&self) -> i64 {
        self.total_credits - self.total_debits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_defaults() {
        let b = Balance::zero();
        assert_eq!(b.liquid, 0);
        assert_eq!(b.locked, 0);
        assert_eq!(b.total(), 0);
    }

    #[test]
    fn credit_and_debit() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        assert_eq!(ledger.balance("alice").liquid, 100);

        ledger.debit("alice", 30, TransactionReason::Purchase, None).unwrap();
        assert_eq!(ledger.balance("alice").liquid, 70);
    }

    #[test]
    fn debit_fails_insufficient() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 50, TransactionReason::Initial, None);
        let result = ledger.debit("alice", 100, TransactionReason::Purchase, None);
        assert!(result.is_err());
        // Balance unchanged
        assert_eq!(ledger.balance("alice").liquid, 50);
    }

    #[test]
    fn atomic_transfer() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        ledger.transfer("alice", "bob", 40, None).unwrap();

        assert_eq!(ledger.balance("alice").liquid, 60);
        assert_eq!(ledger.balance("bob").liquid, 40);
    }

    #[test]
    fn transfer_fails_insufficient() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 30, TransactionReason::Initial, None);
        let result = ledger.transfer("alice", "bob", 50, None);
        assert!(result.is_err());
        // Neither balance changed
        assert_eq!(ledger.balance("alice").liquid, 30);
        assert_eq!(ledger.balance("bob").liquid, 0);
    }

    #[test]
    fn lock_and_unlock() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);

        ledger.lock("alice", 30, "ABCD-EFGH-JKLM").unwrap();
        let b = ledger.balance("alice");
        assert_eq!(b.liquid, 70);
        assert_eq!(b.locked, 30);
        assert_eq!(b.total(), 100); // total unchanged

        // Unlock back to issuer (expired cash)
        ledger.unlock("alice", 30, "ABCD-EFGH-JKLM", true);
        let b = ledger.balance("alice");
        assert_eq!(b.liquid, 100);
        assert_eq!(b.locked, 0);
    }

    #[test]
    fn unlock_to_redeemer() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        ledger.lock("alice", 50, "XXXX-XXXX-XXXX").unwrap();

        // Unlock WITHOUT returning to issuer (redeemed by someone else)
        ledger.unlock("alice", 50, "XXXX-XXXX-XXXX", false);
        let b = ledger.balance("alice");
        assert_eq!(b.liquid, 50); // only original remainder
        assert_eq!(b.locked, 0);
    }

    #[test]
    fn apply_demurrage() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 1000, TransactionReason::Initial, None);

        let decayed = ledger.apply_demurrage("alice", 30);
        assert_eq!(decayed, 30);
        assert_eq!(ledger.balance("alice").liquid, 970);
    }

    #[test]
    fn demurrage_capped_at_balance() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 10, TransactionReason::Initial, None);

        let decayed = ledger.apply_demurrage("alice", 50);
        assert_eq!(decayed, 10);
        assert_eq!(ledger.balance("alice").liquid, 0);
    }

    #[test]
    fn transaction_summary() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        ledger.credit("alice", 10, TransactionReason::Ubi, None);
        ledger.debit("alice", 5, TransactionReason::Demurrage, None).unwrap();
        ledger.transfer("alice", "bob", 20, None).unwrap();

        let summary = ledger.summary("alice");
        assert_eq!(summary.total_credits, 110);
        assert_eq!(summary.ubi_received, 10);
        assert_eq!(summary.demurrage_paid, 5);
        assert_eq!(summary.transfers_sent, 20);
        assert_eq!(summary.net_change(), 85); // 110 - 25
    }

    #[test]
    fn unknown_account_returns_zero() {
        let ledger = Ledger::new();
        let b = ledger.balance("nobody");
        assert_eq!(b.total(), 0);
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    #[test]
    fn balances_for_federation_scoped() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        ledger.credit("bob", 200, TransactionReason::Initial, None);
        ledger.credit("carol", 300, TransactionReason::Initial, None);

        let mut members = std::collections::HashMap::new();
        members.insert("comm1".to_string(), vec!["alice".to_string()]);
        members.insert("comm2".to_string(), vec!["bob".to_string()]);
        members.insert("comm3".to_string(), vec!["carol".to_string()]);

        // Scope to comm1 + comm2 only
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm2"],
        );
        let balances = ledger.balances_for_federation(&members, &scope);
        assert_eq!(balances.len(), 2);

        let alice_bal = balances.iter().find(|(p, _)| p == "alice").unwrap().1.liquid;
        let bob_bal = balances.iter().find(|(p, _)| p == "bob").unwrap().1.liquid;
        assert_eq!(alice_bal, 100);
        assert_eq!(bob_bal, 200);
        // carol not visible
        assert!(!balances.iter().any(|(p, _)| p == "carol"));
    }

    #[test]
    fn summary_for_federation_aggregates_visible() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Ubi, None);
        ledger.credit("bob", 50, TransactionReason::Ubi, None);
        ledger.credit("carol", 999, TransactionReason::Ubi, None);

        let mut members = std::collections::HashMap::new();
        members.insert("comm1".to_string(), vec!["alice".to_string()]);
        members.insert("comm2".to_string(), vec!["bob".to_string()]);
        members.insert("comm3".to_string(), vec!["carol".to_string()]);

        // Scope to comm1 + comm2 — carol excluded
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm2"],
        );
        let summary = ledger.summary_for_federation(&members, &scope);
        assert_eq!(summary.ubi_received, 150); // 100 + 50, not 999
        assert_eq!(summary.total_credits, 150);
    }

    #[test]
    fn balances_for_federation_unrestricted_returns_all() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        ledger.credit("bob", 200, TransactionReason::Initial, None);

        let mut members = std::collections::HashMap::new();
        members.insert("comm1".to_string(), vec!["alice".to_string()]);
        members.insert("comm2".to_string(), vec!["bob".to_string()]);

        let scope = crate::federation_scope::EconomicFederationScope::new();
        let balances = ledger.balances_for_federation(&members, &scope);
        assert_eq!(balances.len(), 2);
    }

    #[test]
    fn transaction_log_grows() {
        let mut ledger = Ledger::new();
        ledger.credit("alice", 100, TransactionReason::Initial, None);
        ledger.debit("alice", 50, TransactionReason::Purchase, None).unwrap();

        let txns = ledger.transactions_for("alice");
        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].transaction_type, TransactionType::Credit);
        assert_eq!(txns[1].transaction_type, TransactionType::Debit);
    }
}
