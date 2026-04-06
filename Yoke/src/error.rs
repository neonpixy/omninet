use thiserror::Error;

#[derive(Debug, Error)]
pub enum YokeError {
    #[error("link not found: {0}")]
    LinkNotFound(String),

    #[error("version not found: {0}")]
    VersionNotFound(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("branch already exists: {0}")]
    DuplicateBranch(String),

    #[error("branch already merged: {0}")]
    BranchAlreadyMerged(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}
