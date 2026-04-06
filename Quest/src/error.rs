//! Quest error types.

use thiserror::Error;

/// Errors from Quest operations.
#[derive(Debug, Error)]
pub enum QuestError {
    /// A referenced entity was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// An entity with the same identifier already exists.
    #[error("already exists: {0}")]
    AlreadyExists(String),

    /// Operation invalid for the current state (e.g., completing an already-completed mission).
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// The actor does not meet eligibility criteria.
    #[error("not eligible: {0}")]
    NotEligible(String),

    /// A cooldown period is still active.
    #[error("cooldown active: {remaining_seconds}s remaining")]
    CooldownActive {
        /// Seconds remaining until the cooldown expires.
        remaining_seconds: u64,
    },

    /// Covenant consent has not been given for this operation.
    #[error("consent required: Quest participation requires opt-in consent")]
    ConsentRequired,

    /// Configuration error.
    #[error("config error: {0}")]
    ConfigError(String),

    /// Serialization or deserialization failure.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl From<serde_json::Error> for QuestError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_not_found() {
        let e = QuestError::NotFound("mission-42".into());
        assert_eq!(e.to_string(), "not found: mission-42");
    }

    #[test]
    fn display_already_exists() {
        let e = QuestError::AlreadyExists("badge-hero".into());
        assert_eq!(e.to_string(), "already exists: badge-hero");
    }

    #[test]
    fn display_invalid_state() {
        let e = QuestError::InvalidState("mission already completed".into());
        assert_eq!(e.to_string(), "invalid state: mission already completed");
    }

    #[test]
    fn display_not_eligible() {
        let e = QuestError::NotEligible("level too low".into());
        assert_eq!(e.to_string(), "not eligible: level too low");
    }

    #[test]
    fn display_cooldown_active() {
        let e = QuestError::CooldownActive {
            remaining_seconds: 300,
        };
        assert_eq!(e.to_string(), "cooldown active: 300s remaining");
    }

    #[test]
    fn display_consent_required() {
        let e = QuestError::ConsentRequired;
        assert!(e.to_string().contains("consent required"));
    }

    #[test]
    fn display_config_error() {
        let e = QuestError::ConfigError("invalid scaling factor".into());
        assert_eq!(e.to_string(), "config error: invalid scaling factor");
    }

    #[test]
    fn display_serialization_error() {
        let e = QuestError::SerializationError("unexpected EOF".into());
        assert_eq!(e.to_string(), "serialization error: unexpected EOF");
    }

    #[test]
    fn from_serde_json_error() {
        let bad_json = "{ not valid json }";
        let serde_err = serde_json::from_str::<serde_json::Value>(bad_json).unwrap_err();
        let quest_err = QuestError::from(serde_err);
        match quest_err {
            QuestError::SerializationError(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("expected SerializationError, got: {other}"),
        }
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QuestError>();
    }

    #[test]
    fn debug_format() {
        let e = QuestError::CooldownActive {
            remaining_seconds: 60,
        };
        let debug = format!("{e:?}");
        assert!(debug.contains("CooldownActive"));
        assert!(debug.contains("60"));
    }
}
