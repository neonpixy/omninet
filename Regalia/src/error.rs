use thiserror::Error;

/// Errors arising from design language operations within Regalia.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum RegaliaError {
    /// A hex color string could not be parsed.
    #[error("invalid hex color: {0}")]
    InvalidHex(String),

    /// A sanctum nesting chain exceeded the maximum allowed depth.
    #[error("sanctum nesting exceeds max depth ({max}): {id}")]
    NestingTooDeep { id: String, max: usize },

    /// A referenced sanctum does not exist.
    #[error("sanctum not found: {0}")]
    SanctumNotFound(String),

    /// A referenced formation does not exist.
    #[error("formation not found: {0}")]
    FormationNotFound(String),

    /// A value was out of range or otherwise invalid.
    #[error("invalid value: {0}")]
    InvalidValue(String),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// A material property was invalid or out of range.
    #[error("invalid material property: {0}")]
    InvalidMaterial(String),

    /// A polygon shape was given fewer sides than the minimum.
    #[error("shape has too few sides (minimum {min}): {actual}")]
    TooFewSides { min: u32, actual: u32 },

    /// The requested theme was not found in the collection.
    #[error("theme not found: {0}")]
    ThemeNotFound(String),

    /// Attempted to remove the currently active theme.
    #[error("cannot remove active theme: {0}")]
    CannotRemoveActiveTheme(String),

    /// Attempted to remove the only remaining theme.
    #[error("cannot remove the last theme")]
    CannotRemoveLastTheme,

    /// A theme with this name already exists in the collection.
    #[error("theme already exists: {0}")]
    ThemeAlreadyExists(String),
}

impl From<serde_json::Error> for RegaliaError {
    fn from(e: serde_json::Error) -> Self {
        RegaliaError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = RegaliaError::InvalidHex("xyz".into());
        assert!(err.to_string().contains("xyz"));

        let err = RegaliaError::NestingTooDeep {
            id: "sidebar".into(),
            max: 8,
        };
        assert!(err.to_string().contains("sidebar"));
        assert!(err.to_string().contains("8"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RegaliaError>();
    }

    #[test]
    fn error_equality() {
        let a = RegaliaError::InvalidHex("abc".into());
        let b = RegaliaError::InvalidHex("abc".into());
        let c = RegaliaError::InvalidHex("def".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn serialization_error_conversion() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let regalia_err: RegaliaError = json_err.into();
        assert!(matches!(regalia_err, RegaliaError::Serialization(_)));
    }
}
