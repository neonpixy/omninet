//! Zeitgeist error types.

use thiserror::Error;

/// Errors from Zeitgeist operations.
#[derive(Debug, Error)]
pub enum ZeitgeistError {
    /// No Towers available for the query.
    #[error("no towers available for query")]
    NoTowersAvailable,

    /// Query text is empty.
    #[error("empty query")]
    EmptyQuery,

    /// Failed to parse a Tower's semantic profile.
    #[error("invalid semantic profile: {0}")]
    InvalidProfile(String),

    /// Failed to parse a Tower's lighthouse announcement.
    #[error("invalid lighthouse announcement: {0}")]
    InvalidAnnouncement(String),

    /// Cache error.
    #[error("cache error: {0}")]
    CacheError(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl From<serde_json::Error> for ZeitgeistError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}
