//! Oracle — Guidance, onboarding, and wisdom for the Omnidea network.
//!
//! The front door. 30 seconds from nothing to belonging. Then it steps
//! back — until you need it again.
//!
//! # Architecture
//!
//! - `ActivationStep` trait — pluggable onboarding steps
//! - `ActivationFlow` — state machine orchestrating the journey
//! - `OracleHint` trait — contextual guidance from any crate
//! - `HintEngine` — evaluates and presents relevant hints
//! - `RecoveryMethod` trait — pluggable identity recovery strategies
//! - `RecoveryFlow` — orchestrates device recovery
//! - `SovereigntyTier` — progressive disclosure (sheltered / citizen / steward / architect)
//!
//! Oracle has **zero internal Omnidea dependencies**. It defines traits
//! that other crates implement. Crown implements `ActivationStep` for
//! identity creation. Sentinal implements `RecoveryMethod` for BIP-39.
//! Globe implements steps for network connection. This keeps Oracle
//! dependency-free — any app can embed it.

pub mod activation;
pub mod disclosure;
pub mod error;
pub mod hints;
pub mod recovery;
pub mod workflow;

pub use activation::{ActivationFlow, ActivationStep, FlowConfig, StepId, StepResult, StepStatus};
pub use disclosure::{
    DelegateType, DisclosureConfig, DisclosureSignal, DisclosureTracker, FeatureVisibility,
    NotificationLevel, SovereigntyTier, TierDefaults, UserLevel,
};
pub use error::OracleError;
pub use hints::{HintContext, HintEngine, OracleHint};
pub use recovery::{RecoveryFlow, RecoveryMethod, RecoveryResult, RecoveryStatus};
pub use workflow::{
    ActionContext, ActionExecutor, ActionSpec, AuditEntry, AuditOutcome, Condition, Schedule,
    TagMatch, Trigger, Workflow, WorkflowEvent, WorkflowMatch, WorkflowRegistry, WorkflowScope,
};
