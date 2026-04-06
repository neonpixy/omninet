use std::fmt;
use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur during Hall file I/O operations.
#[derive(Error, Debug)]
pub enum HallError {
    /// The path does not point to a valid .idea directory.
    #[error("not an .idea package: {0}")]
    NotAnIdeaPackage(PathBuf),

    /// The required Header.json file is missing from the package.
    #[error("missing header at {0}")]
    MissingHeader(PathBuf),

    /// Header.json exists but could not be parsed.
    #[error("corrupted header: {0}")]
    CorruptedHeader(String),

    /// An individual digit file could not be decrypted or deserialized.
    #[error("corrupted digit {id}: {reason}")]
    CorruptedDigit { id: Uuid, reason: String },

    /// A binary asset could not be read, decrypted, or verified.
    #[error("corrupted asset {hash}: {reason}")]
    CorruptedAsset { hash: String, reason: String },

    /// The Authority section could not be decrypted or deserialized.
    #[error("corrupted authority: {0}")]
    CorruptedAuthority(String),

    /// The Coinage section could not be decrypted or deserialized.
    #[error("corrupted coinage: {0}")]
    CorruptedCoinage(String),

    /// The Position section could not be decrypted or deserialized.
    #[error("corrupted position: {0}")]
    CorruptedPosition(String),

    /// The SHA-256 hash of a recovered asset does not match the expected hash,
    /// indicating corruption or tampering.
    #[error("asset hash mismatch: expected {expected}, got {actual}")]
    AssetHashMismatch { expected: String, actual: String },

    /// An encryption or decryption operation failed in Sentinal.
    #[error("encryption error: {0}")]
    Encryption(#[from] sentinal::SentinalError),

    /// A directory could not be created on disk.
    #[error("directory creation failed at {path}: {source}")]
    DirectoryCreation { path: PathBuf, source: std::io::Error },

    /// A general filesystem I/O error.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Babel decoding failed (decrypted data was not valid UTF-8).
    #[error("babel decode failed: {0}")]
    BabelDecodeFailed(String),
}

/// Result of a read operation with graceful degradation.
///
/// Non-fatal issues (corrupted digits, missing optional sections)
/// produce warnings instead of errors. Only a missing or corrupted
/// header is fatal.
#[derive(Debug)]
pub struct ReadResult<T> {
    pub value: T,
    pub warnings: Vec<HallWarning>,
}

impl<T> ReadResult<T> {
    /// Create a new result with no warnings.
    pub fn new(value: T) -> Self {
        Self {
            value,
            warnings: Vec::new(),
        }
    }

    /// Create a new result carrying a list of non-fatal warnings.
    pub fn with_warnings(value: T, warnings: Vec<HallWarning>) -> Self {
        Self { value, warnings }
    }

    /// Returns true if any warnings were collected during reading.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// A non-fatal issue encountered during reading.
#[derive(Debug, Clone, Serialize)]
pub struct HallWarning {
    pub message: String,
    pub file: Option<String>,
}

impl HallWarning {
    /// Create a warning with a message and no associated file path.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            file: None,
        }
    }

    /// Create a warning with a message and the file path where the issue was found.
    pub fn with_file(message: impl Into<String>, file: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            file: Some(file.into()),
        }
    }
}

impl fmt::Display for HallWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{} ({})", self.message, file)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = HallError::MissingHeader(PathBuf::from("/test/idea"));
        assert!(err.to_string().contains("/test/idea"));

        let err = HallError::CorruptedDigit {
            id: Uuid::nil(),
            reason: "bad json".into(),
        };
        assert!(err.to_string().contains("bad json"));

        let err = HallError::AssetHashMismatch {
            expected: "abc".into(),
            actual: "def".into(),
        };
        assert!(err.to_string().contains("abc"));
        assert!(err.to_string().contains("def"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HallError>();
    }

    #[test]
    fn read_result_and_warnings() {
        let result = ReadResult::new(42);
        assert!(!result.has_warnings());
        assert_eq!(result.value, 42);

        let w1 = HallWarning::new("corrupted digit");
        let w2 = HallWarning::with_file("bad data", "Content/abc.json");
        assert!(w2.to_string().contains("Content/abc.json"));

        let result = ReadResult::with_warnings("data", vec![w1, w2]);
        assert!(result.has_warnings());
        assert_eq!(result.warnings.len(), 2);
    }
}
