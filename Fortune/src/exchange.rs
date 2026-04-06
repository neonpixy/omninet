use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A proposed trade between two parties.
///
/// From Consortium Art. 2 §2: "All economic relations shall be based on mutual
/// benefit and just exchange."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeProposal {
    pub id: Uuid,
    pub proposer: String,
    pub recipient: String,
    pub offering_cool: i64,
    pub requesting_cool: i64,
    pub offering_items: Vec<String>,
    pub requesting_items: Vec<String>,
    pub message: Option<String>,
    pub status: TradeStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl TradeProposal {
    /// Create a new trade proposal between two parties. Errors if proposer and recipient are the same.
    pub fn new(
        proposer: impl Into<String>,
        recipient: impl Into<String>,
        offering_cool: i64,
        requesting_cool: i64,
    ) -> Result<Self, crate::FortuneError> {
        let proposer = proposer.into();
        let recipient = recipient.into();
        if proposer == recipient {
            return Err(crate::FortuneError::SelfTrade);
        }
        Ok(Self {
            id: Uuid::new_v4(),
            proposer,
            recipient,
            offering_cool,
            requesting_cool,
            offering_items: Vec::new(),
            requesting_items: Vec::new(),
            message: None,
            status: TradeStatus::Proposed,
            created_at: Utc::now(),
            expires_at: None,
            resolved_at: None,
        })
    }

    /// Add item references to the trade — what each party is offering and requesting.
    pub fn with_items(
        mut self,
        offering: Vec<String>,
        requesting: Vec<String>,
    ) -> Self {
        self.offering_items = offering;
        self.requesting_items = requesting;
        self
    }

    /// Set an expiry time for this proposal.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Attach a message to the proposal.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Accept this proposal. Must be in Proposed status and not expired.
    pub fn accept(&mut self) -> Result<(), crate::FortuneError> {
        if self.status != TradeStatus::Proposed {
            return Err(crate::FortuneError::TradeNotFound(self.id.to_string()));
        }
        if self.is_expired() {
            self.status = TradeStatus::Expired;
            return Err(crate::FortuneError::TradeExpired(self.id.to_string()));
        }
        self.status = TradeStatus::Accepted;
        Ok(())
    }

    /// Execute the trade. Must be in Accepted status.
    pub fn execute(&mut self) -> Result<(), crate::FortuneError> {
        if self.status != TradeStatus::Accepted {
            return Err(crate::FortuneError::TradeNotFound(self.id.to_string()));
        }
        self.status = TradeStatus::Executed;
        self.resolved_at = Some(Utc::now());
        Ok(())
    }

    /// Cancel this proposal. Only works if still Proposed or Accepted.
    pub fn cancel(&mut self) {
        if matches!(self.status, TradeStatus::Proposed | TradeStatus::Accepted) {
            self.status = TradeStatus::Cancelled;
            self.resolved_at = Some(Utc::now());
        }
    }

    /// Reject this proposal. Only works if still Proposed.
    pub fn reject(&mut self) {
        if self.status == TradeStatus::Proposed {
            self.status = TradeStatus::Rejected;
            self.resolved_at = Some(Utc::now());
        }
    }

    /// Whether this proposal has passed its expiry time.
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|exp| Utc::now() > exp)
    }

    /// Whether this proposal is still open (Proposed or Accepted and not expired).
    pub fn is_active(&self) -> bool {
        matches!(self.status, TradeStatus::Proposed | TradeStatus::Accepted)
            && !self.is_expired()
    }
}

/// Lifecycle of a trade.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TradeStatus {
    /// Awaiting the recipient's response.
    Proposed,
    /// Recipient agreed — ready to execute.
    Accepted,
    /// Trade completed and funds transferred.
    Executed,
    /// Cancelled by either party before execution.
    Cancelled,
    /// Proposal passed its expiry time without action.
    Expired,
    /// Recipient declined the proposal.
    Rejected,
}

/// An escrow holding — Cool locked until conditions are met.
///
/// From Consortium Art. 2 §2: "Fair compensation, consent-based agreements,
/// and transparency in value flows shall be required."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EscrowRecord {
    pub id: Uuid,
    pub client: String,
    pub provider: String,
    pub amount: i64,
    pub conditions: Vec<ReleaseCondition>,
    pub status: EscrowStatus,
    pub created_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
}

impl EscrowRecord {
    /// Create a new escrow between a client and provider for the given amount.
    pub fn new(
        client: impl Into<String>,
        provider: impl Into<String>,
        amount: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            client: client.into(),
            provider: provider.into(),
            amount,
            conditions: Vec::new(),
            status: EscrowStatus::Created,
            created_at: Utc::now(),
            released_at: None,
        }
    }

    /// Attach release conditions to this escrow.
    pub fn with_conditions(mut self, conditions: Vec<ReleaseCondition>) -> Self {
        self.conditions = conditions;
        self
    }

    /// Whether all release conditions have been met.
    pub fn all_conditions_met(&self) -> bool {
        !self.conditions.is_empty() && self.conditions.iter().all(|c| c.met)
    }

    /// Release escrowed funds to the provider. Must be in Created or InProgress status.
    pub fn release(&mut self) -> Result<(), crate::FortuneError> {
        if self.status != EscrowStatus::Created && self.status != EscrowStatus::InProgress {
            return Err(crate::FortuneError::EscrowNotFound(self.id.to_string()));
        }
        self.status = EscrowStatus::Released;
        self.released_at = Some(Utc::now());
        Ok(())
    }

    /// Refund escrowed funds to the client.
    pub fn refund(&mut self) {
        self.status = EscrowStatus::Refunded;
        self.released_at = Some(Utc::now());
    }
}

/// A condition for releasing escrowed funds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseCondition {
    pub description: String,
    pub percentage: u32,
    pub met: bool,
    pub met_at: Option<DateTime<Utc>>,
}

/// Lifecycle of an escrow.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EscrowStatus {
    /// Escrow created but conditions not yet started.
    Created,
    /// Work has begun — conditions being fulfilled.
    InProgress,
    /// All conditions met — funds released to provider.
    Released,
    /// A dispute has been raised on this escrow.
    Disputed,
    /// Funds returned to the client.
    Refunded,
}

/// A pluggable exchange mechanism.
///
/// Communities can implement their own exchange modes beyond the built-in ones.
pub trait Exchange: Send + Sync {
    fn propose(&self, trade: &TradeProposal) -> Result<Uuid, crate::FortuneError>;
    fn accept(&self, trade_id: &Uuid) -> Result<(), crate::FortuneError>;
    fn execute(&self, trade_id: &Uuid) -> Result<(), crate::FortuneError>;
    fn exchange_mode(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_trade_proposal() {
        let trade = TradeProposal::new("alice", "bob", 100, 0).unwrap();
        assert_eq!(trade.status, TradeStatus::Proposed);
        assert!(trade.is_active());
    }

    #[test]
    fn cannot_trade_with_self() {
        let result = TradeProposal::new("alice", "alice", 100, 0);
        assert!(matches!(result, Err(crate::FortuneError::SelfTrade)));
    }

    #[test]
    fn trade_lifecycle() {
        let mut trade = TradeProposal::new("alice", "bob", 50, 50).unwrap();
        trade.accept().unwrap();
        assert_eq!(trade.status, TradeStatus::Accepted);

        trade.execute().unwrap();
        assert_eq!(trade.status, TradeStatus::Executed);
        assert!(trade.resolved_at.is_some());
    }

    #[test]
    fn trade_cancel_and_reject() {
        let mut t1 = TradeProposal::new("alice", "bob", 10, 0).unwrap();
        t1.cancel();
        assert_eq!(t1.status, TradeStatus::Cancelled);

        let mut t2 = TradeProposal::new("alice", "bob", 10, 0).unwrap();
        t2.reject();
        assert_eq!(t2.status, TradeStatus::Rejected);
    }

    #[test]
    fn escrow_with_conditions() {
        let mut escrow = EscrowRecord::new("alice", "bob", 500).with_conditions(vec![
            ReleaseCondition {
                description: "Design delivered".into(),
                percentage: 50,
                met: false,
                met_at: None,
            },
            ReleaseCondition {
                description: "Final revision approved".into(),
                percentage: 50,
                met: false,
                met_at: None,
            },
        ]);

        assert!(!escrow.all_conditions_met());

        escrow.conditions[0].met = true;
        escrow.conditions[0].met_at = Some(Utc::now());
        assert!(!escrow.all_conditions_met());

        escrow.conditions[1].met = true;
        escrow.conditions[1].met_at = Some(Utc::now());
        assert!(escrow.all_conditions_met());

        escrow.release().unwrap();
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    #[test]
    fn escrow_refund() {
        let mut escrow = EscrowRecord::new("alice", "bob", 200);
        escrow.refund();
        assert_eq!(escrow.status, EscrowStatus::Refunded);
    }
}
