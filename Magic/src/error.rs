use thiserror::Error;
use uuid::Uuid;

/// Errors arising from Magic operations.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum MagicError {
    /// A digit with the given ID was not found in the document.
    #[error("digit not found: {0}")]
    DigitNotFound(Uuid),

    /// The digit type has no definition in the type registry.
    #[error("digit type not registered: {0}")]
    UnregisteredType(String),

    /// No renderer is registered for this digit type.
    #[error("renderer not found for type: {0}")]
    RendererNotFound(String),

    /// An action could not be executed against the document.
    #[error("action failed: {0}")]
    ActionFailed(String),

    /// The undo or redo stack had no entries to pop.
    #[error("history is empty (nothing to {0})")]
    HistoryEmpty(String),

    /// The requested operation is invalid for the current state.
    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    /// Code projection encountered an error.
    #[error("projection error: {0}")]
    ProjectionError(String),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for MagicError {
    fn from(e: serde_json::Error) -> Self {
        MagicError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let id = Uuid::nil();
        let e = MagicError::DigitNotFound(id);
        assert!(e.to_string().contains("digit not found"));

        let e = MagicError::ActionFailed("bad move".into());
        assert_eq!(e.to_string(), "action failed: bad move");

        let e = MagicError::HistoryEmpty("undo".into());
        assert_eq!(e.to_string(), "history is empty (nothing to undo)");
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MagicError>();
    }

    #[test]
    fn error_equality() {
        let a = MagicError::UnregisteredType("widget".into());
        let b = MagicError::UnregisteredType("widget".into());
        assert_eq!(a, b);
    }

    #[test]
    fn conversion_from_serde() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let magic_err: MagicError = json_err.into();
        assert!(matches!(magic_err, MagicError::Serialization(_)));
    }
}
