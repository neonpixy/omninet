//! # Fortune — Economic Primitives
//!
//! Regenerative economics. Capacity-backed supply, UBI, demurrage, bearer cash,
//! cooperative structures, transparent ledger, progressive redistribution.
//!
//! From Consortium Art. 2 §1: "Commerce within the jurisdiction of this Covenant
//! shall be regenerative, participatory, and life-affirming."
//!
//! Fortune doesn't run the economy — it provides the atoms from which economies
//! are composed. Communities choose their own policies, flow-back rates, and
//! exchange mechanisms within Covenant bounds.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — UBI ensures every person has means to live with dignity.
//! **Sovereignty** — communities set their own economic parameters.
//! **Consent** — all exchange is voluntary; accumulation is bounded.

pub mod balance;
pub mod cash;
pub mod commerce;
pub mod cooperative;
pub mod demurrage;
pub mod error;
pub mod exchange;
pub mod federation_scope;
pub mod flow_back;
pub mod pattern_detection;
pub mod policy;
pub mod treasury;
pub mod trust;
pub mod ubi;

// Re-exports for convenience.
pub use balance::{
    Balance, Ledger, Transaction, TransactionReason, TransactionSummary, TransactionType,
};
pub use cash::{CashMint, CashNote, CashRegistry, CashRedemption, CashStatus, RedemptionResult};
pub use commerce::{
    Cart, CartAction, CartItem, CartSuggestion, CheckoutEngine, Inventory, Order, OrderItem,
    Product, ProductVariant, Receipt, Storefront, StorefrontPolicies,
};
pub use cooperative::{
    Cooperative, CooperativeMember, CooperativeStatus, SurplusDistribution,
};
pub use demurrage::{DemurrageCycleResult, DemurrageEngine, DemurragePreview};
pub use error::FortuneError;
pub use federation_scope::EconomicFederationScope;
pub use exchange::{
    EscrowRecord, EscrowStatus, Exchange, ReleaseCondition, TradeProposal, TradeStatus,
};
pub use flow_back::{FlowBack, FlowBackPreview};
pub use pattern_detection::{
    AlertSeverity, ApprovalRequest, ApprovalSignature, ApprovalStatus, AutoAction, DetectorConfig,
    FinancialAlert, FinancialPatternDetector, PatternType, TierScope, TransactionReceipt,
    TransactionTier, TransactionTierPolicy,
};
pub use policy::{FlowBackTier, FortunePolicy};
pub use treasury::{
    MintReason, MintRecord, NetworkMetrics, ReceiveReason, Treasury, TreasuryStatus,
};
pub use trust::{CommonsTrust, StewardshipRecord, TrustAsset, TrustType};
pub use ubi::{ClaimRecord, IneligibilityReason, UbiDistributor, UbiEligibility};
