use thiserror::Error;

/// Errors arising from accountability operations within Jail.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum JailError {
    // Trust graph
    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("edge not found: {0}")]
    EdgeNotFound(String),

    #[error("max query depth exceeded: {depth} > {max}")]
    MaxDepthExceeded { depth: usize, max: usize },

    #[error("duplicate edge between {verifier} and {verified}")]
    DuplicateEdge { verifier: String, verified: String },

    // Flags
    #[error("flag not found: {0}")]
    FlagNotFound(String),

    #[error("flag rate limited: {pubkey} has filed {count} flags in the current window (max {max})")]
    FlagRateLimited {
        pubkey: String,
        count: usize,
        max: usize,
    },

    #[error("duplicate flag: {flagger} already flagged {flagged} for {category}")]
    DuplicateFlag {
        flagger: String,
        flagged: String,
        category: String,
    },

    #[error("cannot flag yourself")]
    SelfFlag,

    #[error("flag already reviewed: {0}")]
    FlagAlreadyReviewed(String),

    // Response
    #[error("invalid response level transition: {from} -> {to}")]
    InvalidResponseTransition { from: String, to: String },

    #[error("exclusion not found: {0}")]
    ExclusionNotFound(String),

    #[error("exclusion review overdue: {0}")]
    ExclusionReviewOverdue(String),

    // Re-verification
    #[error("re-verification session not found: {0}")]
    SessionNotFound(String),

    #[error("re-verification session expired: {0}")]
    SessionExpired(String),

    #[error("insufficient attestations: have {have}, need {need}")]
    InsufficientAttestations { have: usize, need: usize },

    #[error("re-verification already completed: {0}")]
    AlreadyCompleted(String),

    #[error("re-verification in terminal state: {0}")]
    TerminalState(String),

    #[error("duplicate attester: {0}")]
    DuplicateAttester(String),

    // Admission
    #[error("insufficient verifications for admission: have {have}, need {need}")]
    InsufficientVerifications { have: usize, need: usize },

    // Appeal
    #[error("appeal not found: {0}")]
    AppealNotFound(String),

    #[error("appeal already resolved: {0}")]
    AppealAlreadyResolved(String),

    #[error("appeal in terminal state: {0}")]
    AppealTerminalState(String),

    // Rights
    #[error("rights violation: {0}")]
    RightsViolation(String),

    // Sustained exclusion
    #[error("sustained exclusion invalid: {0}")]
    SustainedExclusionInvalid(String),

    // General
    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl JailError {
    /// Whether this error indicates a security concern requiring escalation.
    pub fn is_security_concern(&self) -> bool {
        matches!(
            self,
            JailError::RightsViolation(_)
                | JailError::ExclusionReviewOverdue(_)
                | JailError::SustainedExclusionInvalid(_)
        )
    }

    /// Whether this error is transient and the operation may succeed if retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            JailError::FlagRateLimited { .. }
                | JailError::InsufficientAttestations { .. }
                | JailError::InsufficientVerifications { .. }
        )
    }
}

impl From<serde_json::Error> for JailError {
    fn from(e: serde_json::Error) -> Self {
        JailError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = JailError::InsufficientAttestations { have: 1, need: 2 };
        assert!(err.to_string().contains("1"));
        assert!(err.to_string().contains("2"));
    }

    #[test]
    fn error_display_rate_limited() {
        let err = JailError::FlagRateLimited {
            pubkey: "alice".into(),
            count: 5,
            max: 5,
        };
        let msg = err.to_string();
        assert!(msg.contains("alice"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn security_concern_classification() {
        assert!(JailError::RightsViolation("test".into()).is_security_concern());
        assert!(JailError::ExclusionReviewOverdue("test".into()).is_security_concern());
        assert!(!JailError::FlagNotFound("test".into()).is_security_concern());
        assert!(!JailError::SelfFlag.is_security_concern());
    }

    #[test]
    fn retryable_classification() {
        assert!(JailError::FlagRateLimited {
            pubkey: "a".into(),
            count: 5,
            max: 5
        }
        .is_retryable());
        assert!(JailError::InsufficientAttestations { have: 1, need: 2 }.is_retryable());
        assert!(!JailError::SelfFlag.is_retryable());
        assert!(!JailError::RightsViolation("test".into()).is_retryable());
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<JailError>();
    }

    #[test]
    fn from_serde_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let jail_err: JailError = json_err.into();
        assert!(matches!(jail_err, JailError::Serialization(_)));
    }
}
