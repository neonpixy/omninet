use thiserror::Error;

/// Errors arising from constitutional operations within Polity.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum PolityError {
    /// A right was looked up by ID or name and not found in the registry.
    #[error("right not found: {0}")]
    RightNotFound(String),

    /// Attempted to register a right that already exists (same name and category).
    #[error("duplicate right: {0}")]
    DuplicateRight(String),

    /// A duty was looked up by ID or name and not found in the registry.
    #[error("duty not found: {0}")]
    DutyNotFound(String),

    /// Attempted to register a duty that already exists (same name and category).
    #[error("duplicate duty: {0}")]
    DuplicateDuty(String),

    /// A protection was looked up by ID or name and not found in the registry.
    #[error("protection not found: {0}")]
    ProtectionNotFound(String),

    /// Attempted to register a protection that already exists (same name and type).
    #[error("duplicate protection: {0}")]
    DuplicateProtection(String),

    /// Attempted to modify, remove, or override something that is constitutionally immutable.
    #[error("attempted modification of immutable foundation: {0}")]
    ImmutableViolation(String),

    /// A breach was looked up by ID and not found in the registry.
    #[error("breach not found: {0}")]
    BreachNotFound(String),

    /// An amendment was looked up by ID and not found.
    #[error("amendment not found: {0}")]
    AmendmentNotFound(String),

    /// Attempted an invalid status transition on an amendment (e.g., skipping deliberation).
    #[error("invalid amendment transition: {current} -> {target}")]
    InvalidAmendmentTransition { current: String, target: String },

    /// An amendment's description triggers the immutable foundation guard.
    #[error("amendment contradicts immutable foundation: {0}")]
    AmendmentContradictsFoundation(String),

    /// A ratification vote did not reach the required approval threshold.
    #[error("amendment threshold not met: required {required}, got {actual}")]
    ThresholdNotMet { required: f64, actual: f64 },

    /// An enactment was looked up by enactor and not found.
    #[error("enactment not found for: {0}")]
    EnactmentNotFound(String),

    /// The enactor already has an active enactment and cannot enact again until withdrawn.
    #[error("already enacted: {0}")]
    AlreadyEnacted(String),

    /// Attempted an invalid status transition on an enactment (e.g., suspending a withdrawn one).
    #[error("invalid enactment transition: {current} -> {target}")]
    InvalidEnactmentTransition { current: String, target: String },

    /// A consent record was looked up by ID and not found.
    #[error("consent not found: {0}")]
    ConsentNotFound(String),

    /// Attempted to revoke consent that has already been revoked.
    #[error("consent already revoked: {0}")]
    ConsentAlreadyRevoked(String),

    /// An action requires consent that has not been granted.
    #[error("consent required but not granted for: {0}")]
    ConsentRequired(String),

    /// Consent obtained through coercion, dependency, or necessity is void.
    #[error("consent obtained under coercion is void: {0}")]
    CoercedConsent(String),

    /// A constitutional review identified a violation.
    #[error("review failed: {0}")]
    ReviewFailed(String),

    /// A constitutional clause was looked up by ID and not found.
    #[error("clause not found: {0}")]
    ClauseNotFound(uuid::Uuid),

    /// Attempted to register a clause with an ID that already exists.
    #[error("duplicate clause: {0}")]
    DuplicateClause(uuid::Uuid),

    /// Attempted to amend a Core or Commons clause without the reconstitution process.
    #[error("clause requires reconstitution process (Core/Commons): {0}")]
    ClauseRequiresReconstitution(uuid::Uuid),

    /// Attempted to reconstitute a clause that is not Core or Commons.
    #[error("clause does not require reconstitution (not Core/Commons): {0}")]
    ClauseDoesNotRequireReconstitution(uuid::Uuid),

    /// The clause registry's internal lock was poisoned (another thread panicked while holding it).
    #[error("clause registry lock poisoned")]
    ClauseRegistryPoisoned,

    /// A reconstitution attempt did not meet the extraordinary threshold (90% + 2yr + Star Court).
    #[error("reconstitution threshold not met (requires 90% + 2yr + Star Court unanimous)")]
    ReconstitutionThresholdNotMet,

    /// A reconstitution proposal did not demonstrate alignment with all three axioms.
    #[error("reconstitution axiom alignment incomplete (all three axioms must be demonstrated)")]
    ReconstitutionAxiomAlignmentIncomplete,

    /// A reconstitution proposal would weaken one or more axioms.
    #[error("reconstitution weakens axiom: {0}")]
    ReconstitutionWeakensAxiom(String),

    /// A precedent was looked up by ID and not found.
    #[error("precedent not found: {0}")]
    PrecedentNotFound(uuid::Uuid),

    /// The precedent registry's internal lock was poisoned.
    #[error("precedent registry lock poisoned")]
    PrecedentRegistryPoisoned,

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Converts a `serde_json::Error` into a `PolityError::Serialization`.
impl From<serde_json::Error> for PolityError {
    fn from(e: serde_json::Error) -> Self {
        PolityError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = PolityError::RightNotFound("dignity".into());
        assert!(err.to_string().contains("dignity"));

        let err = PolityError::ImmutableViolation("Core Art. 2".into());
        assert!(err.to_string().contains("immutable"));
        assert!(err.to_string().contains("Core Art. 2"));

        let err = PolityError::ThresholdNotMet {
            required: 0.67,
            actual: 0.45,
        };
        assert!(err.to_string().contains("0.67"));
        assert!(err.to_string().contains("0.45"));

        let err = PolityError::CoercedConsent("dependency-based".into());
        assert!(err.to_string().contains("coercion"));
        assert!(err.to_string().contains("void"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PolityError>();
    }

    #[test]
    fn error_equality() {
        let a = PolityError::RightNotFound("dignity".into());
        let b = PolityError::RightNotFound("dignity".into());
        let c = PolityError::RightNotFound("privacy".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn serialization_error_conversion() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let polity_err: PolityError = json_err.into();
        assert!(matches!(polity_err, PolityError::Serialization(_)));
    }
}
