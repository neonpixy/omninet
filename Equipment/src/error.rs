use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from Phone (request/response RPC).
#[derive(Error, Debug)]
pub enum PhoneError {
    /// No handler registered for the given call ID.
    #[error("no handler registered for call '{0}'")]
    NoHandler(String),

    /// Serialization or deserialization failed.
    #[error("serialization error for call '{call_id}': {source}")]
    Serialization {
        call_id: String,
        source: serde_json::Error,
    },

    /// The handler returned an error.
    #[error("handler for call '{call_id}' failed: {message}")]
    HandlerFailed { call_id: String, message: String },
}

/// Errors from Communicator (real-time sessions).
#[derive(Error, Debug)]
pub enum CommunicatorError {
    /// Session not found.
    #[error("session '{0}' not found")]
    SessionNotFound(String),

    /// No participants provided.
    #[error("session must have at least one participant")]
    NoParticipants,

    /// Session is not active (can't deliver data).
    #[error("session '{0}' is not active")]
    SessionNotActive(String),

    /// Invalid state transition.
    #[error("invalid transition for session '{session_id}': {from:?} -> {to:?}")]
    InvalidTransition {
        session_id: String,
        from: crate::communicator::SessionStatus,
        to: crate::communicator::SessionStatus,
    },

    /// Handler returned an error.
    #[error("handler failed for session '{session_id}': {message}")]
    HandlerFailed { session_id: String, message: String },
}

/// Errors from Contacts (module registry).
#[derive(Error, Debug)]
pub enum ContactsError {
    /// Module already registered.
    #[error("module '{0}' is already registered")]
    AlreadyRegistered(String),

    /// Module not found.
    #[error("module '{0}' not found")]
    NotFound(String),

    /// Declared dependency not found when registering a module.
    #[error("dependency '{0}' not found")]
    DependencyNotFound(String),
}

/// Errors from Mailbox (user mail).
#[derive(Clone, Debug, Error, Serialize, Deserialize, PartialEq, Eq)]
pub enum MailError {
    /// The requested message was not found.
    #[error("message not found: {0}")]
    MessageNotFound(String),

    /// The requested draft was not found.
    #[error("draft not found: {0}")]
    DraftNotFound(String),

    /// Attempted to send a draft with empty subject and body.
    #[error("empty draft cannot be sent")]
    EmptyDraft,
}
