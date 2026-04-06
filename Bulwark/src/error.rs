use thiserror::Error;

/// Errors arising from safety operations within Bulwark.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum BulwarkError {
    // Trust
    #[error("trust layer transition blocked: {0}")]
    LayerTransitionBlocked(String),

    #[error("cannot skip trust layers: {current} -> {target}")]
    CannotSkipLayers { current: String, target: String },

    #[error("already at layer: {0}")]
    AlreadyAtLayer(String),

    // Bonds
    #[error("bond not found: {0}")]
    BondNotFound(String),

    #[error("bond already exists between parties")]
    BondAlreadyExists,

    // Verification
    #[error("verification failed: {0}")]
    VerificationFailed(String),

    #[error("proximity proof required: {0}")]
    ProximityRequired(String),

    #[error("insufficient vouches: have {have}, need {need}")]
    InsufficientVouches { have: usize, need: usize },

    #[error("vouch eligibility failed: {0}")]
    VouchIneligible(String),

    #[error("sponsor eligibility failed: {0}")]
    SponsorIneligible(String),

    // Age / Kids Sphere
    #[error("minor not authorized: {0}")]
    MinorNotAuthorized(String),

    #[error("parent link required")]
    ParentLinkRequired,

    #[error("family bond requires proximity proof")]
    FamilyBondRequiresProximity,

    #[error("parent approval required for kid connection")]
    ParentApprovalRequired,

    #[error("child not in kids sphere: {0}")]
    NotInKidsSphere(String),

    // Child Safety
    #[error("child safety flag: {0}")]
    ChildSafetyViolation(String),

    // KidsSphere Exclusion (R2B)
    #[error("insufficient parental approvals: have {have}, need {need}")]
    InsufficientApprovals { have: usize, need: usize },

    #[error("KidsSphere exclusion invalid: {0}")]
    KidsExclusionInvalid(String),

    // Health
    #[error("health pulse expired: {0}")]
    HealthPulseExpired(String),

    // Reputation
    #[error("standing prevents action: {0}")]
    StandingPreventsAction(String),

    #[error("fraud detected: {0}")]
    FraudDetected(String),

    // Consent
    #[error("consent required: {0}")]
    ConsentRequired(String),

    #[error("consent revoked: {0}")]
    ConsentRevoked(String),

    // Network
    #[error("network in bootstrap phase: {0}")]
    BootstrapRestriction(String),

    // General
    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl BulwarkError {
    /// Whether this error represents a security concern that should be logged or escalated.
    pub fn is_security_concern(&self) -> bool {
        matches!(
            self,
            BulwarkError::ChildSafetyViolation(_)
                | BulwarkError::FraudDetected(_)
                | BulwarkError::MinorNotAuthorized(_)
                | BulwarkError::FamilyBondRequiresProximity
                | BulwarkError::KidsExclusionInvalid(_)
        )
    }

    /// Whether this error is transient and the operation can be retried later.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            BulwarkError::InsufficientVouches { .. }
                | BulwarkError::BootstrapRestriction(_)
                | BulwarkError::HealthPulseExpired(_)
        )
    }
}

impl From<serde_json::Error> for BulwarkError {
    fn from(e: serde_json::Error) -> Self {
        BulwarkError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = BulwarkError::InsufficientVouches { have: 1, need: 3 };
        assert!(err.to_string().contains("1"));
        assert!(err.to_string().contains("3"));
    }

    #[test]
    fn security_concern_classification() {
        assert!(BulwarkError::ChildSafetyViolation("test".into()).is_security_concern());
        assert!(BulwarkError::FraudDetected("test".into()).is_security_concern());
        assert!(!BulwarkError::BondNotFound("test".into()).is_security_concern());
    }

    #[test]
    fn retryable_classification() {
        assert!(BulwarkError::InsufficientVouches { have: 1, need: 2 }.is_retryable());
        assert!(!BulwarkError::ChildSafetyViolation("test".into()).is_retryable());
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BulwarkError>();
    }
}
