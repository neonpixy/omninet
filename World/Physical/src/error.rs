//! Error types for World/Physical.

use thiserror::Error;

/// Errors for physical world operations.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum PhysicalError {
    // Place
    #[error("place not found: {0}")]
    PlaceNotFound(String),

    #[error("not place owner: {0}")]
    NotPlaceOwner(String),

    #[error("place name required")]
    PlaceNameRequired,

    // Region
    #[error("region not found: {0}")]
    RegionNotFound(String),

    #[error("circular region nesting: {0}")]
    CircularNesting(String),

    // Rendezvous
    #[error("rendezvous not found: {0}")]
    RendezvousNotFound(String),

    #[error("not organizer: {0}")]
    NotOrganizer(String),

    #[error("rendezvous already {0}")]
    RendezvousAlreadyFinalized(String),

    #[error("scheduled time is in the past")]
    ScheduledInPast,

    #[error("not invited: {0}")]
    NotInvited(String),

    #[error("max participants reached")]
    MaxParticipants,

    // Presence
    #[error("presence TTL exceeds maximum: {requested}s > {maximum}s")]
    TtlExceedsMaximum { requested: u64, maximum: u64 },

    // Lantern
    #[error("lantern share expired")]
    LanternExpired,

    // Handoff
    #[error("handoff state invalid: expected {expected}, got {actual}")]
    InvalidHandoffState { expected: String, actual: String },

    #[error("proximity verification required before signing")]
    ProximityRequired,

    #[error("initiator must sign before counterparty")]
    InitiatorSignatureRequired,

    #[error("cannot handoff with yourself")]
    SelfHandoff,

    // Caravan
    #[error("delivery not found: {0}")]
    DeliveryNotFound(String),

    #[error("invalid delivery state: expected {expected}, got {actual}")]
    InvalidDeliveryState { expected: String, actual: String },

    // OmniTag
    #[error("tag not found: {0}")]
    TagNotFound(String),

    #[error("not tag owner: {0}")]
    NotTagOwner(String),

    // General
    #[error("authorization failed: {0}")]
    Unauthorized(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl PhysicalError {
    /// Whether this error represents a privacy/surveillance concern.
    pub fn is_privacy_concern(&self) -> bool {
        false // No operation in Physical should produce surveillance data
    }

    /// Whether the operation can be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(self, PhysicalError::Serialization(_))
    }
}
