use thiserror::Error;

/// All errors that can occur within the Lingo crate.
#[derive(Error, Debug)]
pub enum LingoError {
    /// Babel encoding failed for the given text.
    #[error("encoding failed: {0}")]
    EncodingFailed(String),

    /// Babel decoding failed for the given encoded content.
    #[error("decoding failed: {0}")]
    DecodingFailed(String),

    /// The vocabulary seed is the wrong length.
    #[error("invalid vocabulary seed: expected {expected} bytes, got {actual}")]
    InvalidSeed { expected: usize, actual: usize },

    /// The requested language is not available for translation.
    #[error("language not available: {0}")]
    LanguageNotAvailable(String),

    /// The translation provider returned an error.
    #[error("translation failed: {0}")]
    TranslationFailed(String),

    /// No translation provider is registered.
    #[error("translation provider not available")]
    ProviderNotAvailable,

    /// An error occurred in the translation cache.
    #[error("cache error: {0}")]
    CacheError(String),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Text tokenization failed.
    #[error("tokenization failed: {0}")]
    TokenizationFailed(String),

    /// The provided BCP 47 language code is not recognized.
    #[error("invalid language code: {0}")]
    InvalidLanguageCode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = LingoError::InvalidSeed {
            expected: 32,
            actual: 16,
        };
        assert!(err.to_string().contains("32"));
        assert!(err.to_string().contains("16"));

        let err = LingoError::LanguageNotAvailable("zh-Hans".into());
        assert!(err.to_string().contains("zh-Hans"));

        let err = LingoError::ProviderNotAvailable;
        assert!(err.to_string().contains("provider"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LingoError>();
    }
}
