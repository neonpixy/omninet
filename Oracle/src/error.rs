//! Oracle error types.

use thiserror::Error;

/// Errors from Oracle operations.
#[derive(Debug, Error)]
pub enum OracleError {
    /// A step failed during activation.
    #[error("activation step '{step}' failed: {reason}")]
    StepFailed { step: String, reason: String },

    /// A step was skipped but cannot be skipped.
    #[error("step '{0}' cannot be skipped")]
    CannotSkip(String),

    /// Rollback failed.
    #[error("rollback of step '{step}' failed: {reason}")]
    RollbackFailed { step: String, reason: String },

    /// Recovery failed.
    #[error("recovery failed: {0}")]
    RecoveryFailed(String),

    /// Invalid flow state.
    #[error("invalid flow state: {0}")]
    InvalidState(String),

    /// No steps registered.
    #[error("no steps registered in flow")]
    EmptyFlow,

    /// Step not found.
    #[error("step '{0}' not found")]
    StepNotFound(String),

    /// Workflow not found.
    #[error("workflow '{0}' not found")]
    WorkflowNotFound(String),
}
