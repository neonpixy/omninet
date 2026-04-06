//! MagicalIndex errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MagicalError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("index error: {0}")]
    Index(String),

    #[error("query error: {0}")]
    Query(String),
}
