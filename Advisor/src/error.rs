use thiserror::Error;

/// All errors that can occur within the Advisor crate.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum AdvisorError {
    // ── Lifecycle ────────────────────────────────────────────────
    /// The advisor has not been initialized yet.
    #[error("advisor not initialized")]
    NotInitialized,

    /// Tried to initialize an advisor that is already running.
    #[error("advisor already initialized")]
    AlreadyInitialized,

    /// The advisor is in sleep mode and cannot process requests.
    #[error("advisor is asleep")]
    Asleep,

    /// Tried to wake an advisor that is already awake.
    #[error("advisor already awake")]
    AlreadyAwake,

    // ── Thoughts ─────────────────────────────────────────────────
    /// No thought exists with the given ID.
    #[error("thought not found: {0}")]
    ThoughtNotFound(String),

    /// An error occurred while processing the thought stream.
    #[error("thought stream error: {0}")]
    ThoughtStreamError(String),

    // ── Sessions ─────────────────────────────────────────────────
    /// No session exists with the given ID.
    #[error("session not found: {0}")]
    SessionNotFound(String),

    /// The Home session (inner monologue) cannot be archived or modified.
    #[error("cannot modify home session")]
    CannotModifyHomeSession,

    /// Tried to add a thought to an archived session.
    #[error("session is archived: {0}")]
    SessionArchived(String),

    // ── Pressure ─────────────────────────────────────────────────
    /// A pressure value was outside the valid 0.0..=1.0 range.
    #[error("invalid pressure value: {value} (must be 0.0..=1.0)")]
    InvalidPressure { value: f64 },

    /// A pressure threshold was outside the valid 0.0..=1.0 range.
    #[error("invalid pressure threshold: {value} (must be 0.0..=1.0)")]
    InvalidPressureThreshold { value: f64 },

    // ── Synapses ─────────────────────────────────────────────────
    /// No synapse exists with the given ID.
    #[error("synapse not found: {0}")]
    SynapseNotFound(String),

    /// A synapse would create a circular connection.
    #[error("circular synapse detected: {0}")]
    CircularSynapse(String),

    /// A synapse strength value was outside the allowed min..=max range.
    #[error("synapse strength out of range: {value} (must be {min}..={max})")]
    SynapseStrengthOutOfRange { value: f64, min: f64, max: f64 },

    // ── Engine ───────────────────────────────────────────────────
    /// No cognitive provider is registered with the given ID.
    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    /// A provider exists but is temporarily unavailable.
    #[error("provider unavailable: {id} — {reason}")]
    ProviderUnavailable { id: String, reason: String },

    /// No cognitive providers are available at all.
    #[error("no providers available")]
    NoProvidersAvailable,

    /// An LLM generation request failed.
    #[error("generation failed: {0}")]
    GenerationFailed(String),

    /// The generation context exceeds the provider's token limit.
    #[error("context too large: {tokens} tokens (max {max_tokens})")]
    ContextTooLarge { tokens: usize, max_tokens: usize },

    /// The provider returned a rate-limit response.
    #[error("rate limited — retry after {retry_after_seconds}s")]
    RateLimited { retry_after_seconds: u64 },

    /// Streaming was requested but the provider does not support it.
    #[error("streaming not supported by provider: {0}")]
    StreamingNotSupported(String),

    /// The security tier (Balanced/Hardened/Ultimate) blocked a cloud provider.
    #[error("provider blocked by security tier: {provider_id} blocked at {tier} tier")]
    ProviderBlockedByTier { provider_id: String, tier: String },

    // ── Store ────────────────────────────────────────────────────
    /// A save operation to the cognitive store failed.
    #[error("save failed: {0}")]
    SaveFailed(String),

    /// A load operation from the cognitive store failed.
    #[error("load failed: {0}")]
    LoadFailed(String),

    /// An item was not found in the cognitive store.
    #[error("not found: {type_name} with id {id}")]
    NotFound { type_name: String, id: String },

    /// The cognitive store is locked and cannot be accessed.
    #[error("store locked")]
    StoreLocked,

    // ── Memory ───────────────────────────────────────────────────
    /// No memory entry exists with the given ID.
    #[error("memory not found: {0}")]
    MemoryNotFound(String),

    /// The embedding provider failed to generate an embedding vector.
    #[error("embedding generation failed: {0}")]
    EmbeddingFailed(String),

    // ── Skills ───────────────────────────────────────────────────
    /// No skill is registered with the given ID.
    #[error("skill not found: {0}")]
    SkillNotFound(String),

    /// A skill execution failed at runtime.
    #[error("skill execution failed: {id} — {reason}")]
    SkillFailed { id: String, reason: String },

    /// The parameters passed to a skill were invalid.
    #[error("invalid skill parameters for {id}: {reason}")]
    InvalidSkillParameters { id: String, reason: String },

    // ── Cognitive Loop ───────────────────────────────────────────
    /// The cognitive loop is not currently running.
    #[error("cognitive loop not running")]
    LoopNotRunning,

    /// Tried to start the cognitive loop when it is already running.
    #[error("cognitive loop already running")]
    LoopAlreadyRunning,

    /// The cognitive loop configuration is invalid.
    #[error("invalid loop configuration: {0}")]
    InvalidLoopConfig(String),

    // ── Extensions ───────────────────────────────────────────────
    /// A thought source with the same name is already registered.
    #[error("thought source already registered: {0}")]
    ThoughtSourceAlreadyRegistered(String),

    /// A relationship provider with the same name is already registered.
    #[error("relationship provider already registered: {0}")]
    RelationshipProviderAlreadyRegistered(String),

    // ── Sacred ───────────────────────────────────────────────────
    /// AI companions require a human sponsor per the Covenant.
    #[error("sponsorship required: AI companions must have a human sponsor")]
    SponsorshipRequired,

    /// The advisor tried to express without the human's consent.
    #[error("expression without consent: human approval required")]
    ExpressionWithoutConsent,

    // ── Governance ────────────────────────────────────────────────
    /// Governance delegation was not active when a vote was attempted.
    #[error("governance delegation not active")]
    GovernanceDelegationInactive,

    /// A governance vote could not be cast.
    #[error("governance vote failed: {0}")]
    GovernanceVoteFailed(String),

    // ── Serialization ────────────────────────────────────────────
    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// A configuration value is invalid.
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(String),
}

impl From<serde_json::Error> for AdvisorError {
    fn from(e: serde_json::Error) -> Self {
        AdvisorError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = AdvisorError::ThoughtNotFound("abc-123".into());
        assert!(err.to_string().contains("abc-123"));

        let err = AdvisorError::ProviderBlockedByTier {
            provider_id: "claude".into(),
            tier: "Ultimate".into(),
        };
        assert!(err.to_string().contains("claude"));
        assert!(err.to_string().contains("Ultimate"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AdvisorError>();
    }

    #[test]
    fn serde_json_error_conversion() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let advisor_err: AdvisorError = json_err.into();
        assert!(matches!(advisor_err, AdvisorError::Serialization(_)));
    }

    #[test]
    fn error_clone_and_eq() {
        let a = AdvisorError::Asleep;
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn pressure_error_fields() {
        let err = AdvisorError::InvalidPressure { value: 1.5 };
        assert!(err.to_string().contains("1.5"));

        let err = AdvisorError::SynapseStrengthOutOfRange {
            value: -0.1,
            min: 0.1,
            max: 1.0,
        };
        assert!(err.to_string().contains("-0.1"));
    }

    #[test]
    fn provider_error_variants() {
        let err = AdvisorError::ContextTooLarge {
            tokens: 50000,
            max_tokens: 32000,
        };
        assert!(err.to_string().contains("50000"));
        assert!(err.to_string().contains("32000"));

        let err = AdvisorError::RateLimited {
            retry_after_seconds: 30,
        };
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn sacred_error_variants() {
        let err = AdvisorError::SponsorshipRequired;
        assert!(err.to_string().contains("sponsor"));

        let err = AdvisorError::ExpressionWithoutConsent;
        assert!(err.to_string().contains("consent"));
    }
}
