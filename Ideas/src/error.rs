use thiserror::Error;
use uuid::Uuid;

/// All errors that can occur when working with .idea packages and their contents.
///
/// Covers digit validation, header parsing, authority/coinage checks,
/// bond path validation, package I/O, and domain-specific parsing failures.
#[derive(Error, Debug)]
pub enum IdeasError {
    // Digit errors
    #[error("invalid digit type '{0}': {1}")]
    InvalidDigitType(String, String),

    #[error("type '{0}' exceeds max length of {1}")]
    TypeTooLong(String, usize),

    #[error("invalid property key '{0}': {1}")]
    InvalidPropertyKey(String, String),

    #[error("property key '{0}' exceeds max length of {1}")]
    PropertyKeyTooLong(String, usize),

    // Header errors
    #[error("unsupported header version: {0}")]
    UnsupportedVersion(String),

    #[error("unsupported encryption algorithm: {0}")]
    UnsupportedAlgorithm(String),

    #[error("header must have at least one key slot")]
    NoKeySlots,

    #[error("digit count must be > 0")]
    InvalidDigitCount,

    // Authority errors
    #[error("root contribution exceeds 100%: {0}")]
    ContributionExceeds100(i32),

    #[error("invalid contribution weight {1} for root {0}")]
    InvalidContribution(Uuid, i32),

    // Coinage errors
    #[error("splits must equal 100%, got {0}%")]
    SplitsNotEqual100(i32),

    #[error("invalid split percentage: {0}%")]
    InvalidSplitPercentage(i32),

    // Bond errors
    #[error("path must be absolute: {0}")]
    RelativePath(String),

    #[error("path contains directory traversal: {0}")]
    PathTraversal(String),

    // Package errors
    #[error("not a directory: {0}")]
    NotADirectory(String),

    #[error("Header.json not found")]
    HeaderNotFound,

    #[error("content directory not found")]
    ContentNotFound,

    #[error("digit not found: {0}")]
    DigitNotFound(Uuid),

    // Media parsing errors
    #[error("media parsing error: {0}")]
    MediaParsing(String),

    // Domain digit parsing errors
    #[error("sheet parsing error: {0}")]
    SheetParsing(String),

    #[error("slide parsing error: {0}")]
    SlideParsing(String),

    #[error("form parsing error: {0}")]
    FormParsing(String),

    #[error("rich text parsing error: {0}")]
    RichTextParsing(String),

    #[error("interactive parsing error: {0}")]
    InteractiveParsing(String),

    #[error("commerce error: {0}")]
    CommerceError(String),

    #[error("accessibility error: {0}")]
    AccessibilityError(String),

    #[error("binding error: {0}")]
    BindingError(String),

    // Generic
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
