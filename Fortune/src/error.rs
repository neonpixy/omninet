use thiserror::Error;

/// Errors arising from economic operations within Fortune.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum FortuneError {
    // Balance
    #[error("insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: i64, available: i64 },

    #[error("invalid recipient: {0}")]
    InvalidRecipient(String),

    #[error("invalid amount: {0}")]
    InvalidAmount(String),

    // Treasury
    #[error("treasury empty: no Cool available to mint")]
    TreasuryEmpty,

    #[error("supply cap exceeded: requested {requested}, available {available}")]
    SupplyCapExceeded { requested: i64, available: i64 },

    // UBI
    #[error("not eligible for UBI: {0}")]
    UbiNotEligible(String),

    #[error("UBI on cooldown: next claim available at {0}")]
    UbiOnCooldown(String),

    // Demurrage
    #[error("demurrage calculation failed: {0}")]
    DemurrageError(String),

    // Cash
    #[error("cash note not found: {0}")]
    CashNotFound(String),

    #[error("cash already redeemed: {0}")]
    CashAlreadyRedeemed(String),

    #[error("cash expired: {0}")]
    CashExpired(String),

    #[error("cash revoked: {0}")]
    CashRevoked(String),

    #[error("invalid cash serial: {0}")]
    InvalidCashSerial(String),

    #[error("cash issuance failed: {0}")]
    CashIssuanceFailed(String),

    // Exchange
    #[error("trade not found: {0}")]
    TradeNotFound(String),

    #[error("trade expired: {0}")]
    TradeExpired(String),

    #[error("cannot trade with self")]
    SelfTrade,

    #[error("escrow not found: {0}")]
    EscrowNotFound(String),

    // Flow-back
    #[error("flow-back calculation failed: {0}")]
    FlowBackError(String),

    // Cooperative
    #[error("cooperative not found: {0}")]
    CooperativeNotFound(String),

    #[error("not a cooperative member: {0}")]
    NotCooperativeMember(String),

    // Commons trust
    #[error("trust not found: {0}")]
    TrustNotFound(String),

    // Rate limiting
    #[error("rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    // Commerce
    #[error("product not found: {0}")]
    ProductNotFound(String),

    #[error("insufficient inventory: product {product}, requested {requested}, available {available}")]
    InsufficientInventory {
        product: String,
        requested: u32,
        available: u32,
    },

    #[error("cart is empty")]
    CartEmpty,

    #[error("checkout failed: {0}")]
    CheckoutFailed(String),

    #[error("order not found: {0}")]
    OrderNotFound(String),

    #[error("invalid order transition: {from} -> {to}")]
    InvalidOrderTransition { from: String, to: String },

    // Pattern detection
    #[error("cash note exceeds denomination cap: {amount} > {cap}")]
    CashNoteDenominationExceeded { amount: i64, cap: i64 },

    #[error("approval request not found: {0}")]
    ApprovalNotFound(String),

    #[error("approval request expired: {0}")]
    ApprovalExpired(String),

    #[error("approval request already resolved: {0}")]
    ApprovalAlreadyResolved(String),

    #[error("transaction requires approval: tier is Approved")]
    TransactionRequiresApproval,

    #[error("duplicate approver: {0}")]
    DuplicateApprover(String),

    // General
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for FortuneError {
    fn from(e: serde_json::Error) -> Self {
        FortuneError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = FortuneError::InsufficientBalance {
            required: 100,
            available: 50,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));

        let err = FortuneError::TreasuryEmpty;
        assert!(err.to_string().contains("empty"));

        let err = FortuneError::CashNotFound("ABCD-EFGH-JKLM".into());
        assert!(err.to_string().contains("ABCD"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FortuneError>();
    }

    #[test]
    fn error_equality() {
        let a = FortuneError::TreasuryEmpty;
        let b = FortuneError::TreasuryEmpty;
        assert_eq!(a, b);
    }

    #[test]
    fn serialization_error_conversion() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let fortune_err: FortuneError = json_err.into();
        assert!(matches!(fortune_err, FortuneError::Serialization(_)));
    }
}
