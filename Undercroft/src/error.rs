//! Undercroft error types.

use serde::{Deserialize, Serialize};

/// Errors that can occur in the Undercroft observatory.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize, PartialEq)]
pub enum UndercraftError {
    /// No health data is available yet.
    #[error("no health data available")]
    NoData,

    /// The requested community was not found in the health data.
    #[error("community not found: {0}")]
    CommunityNotFound(String),

    /// The snapshot ring buffer has reached its retention limit.
    #[error("snapshot capacity exceeded")]
    CapacityExceeded,
}
