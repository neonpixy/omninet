//! DeviceManager error types.

use serde::{Deserialize, Serialize};

/// Errors that can occur in the device management subsystem.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize, PartialEq)]
pub enum DeviceManagerError {
    /// Pairing protocol failed for the given reason.
    #[error("pairing failed: {0}")]
    PairingFailed(String),

    /// The pairing challenge has expired (past its `expires_at` timestamp).
    #[error("pairing expired")]
    PairingExpired,

    /// The response nonce does not match the challenge nonce.
    #[error("nonce mismatch")]
    NonceMismatch,

    /// The BIP-340 Schnorr signature on the pairing response is invalid.
    #[error("signature verification failed")]
    SignatureInvalid,

    /// No device with the given crown_id exists in the fleet.
    #[error("device not found: {0}")]
    DeviceNotFound(String),

    /// A device with the given crown_id is already paired in the fleet.
    #[error("device already paired: {0}")]
    AlreadyPaired(String),

    /// A sync conflict was detected on the specified data type.
    #[error("sync conflict on {data_type}")]
    SyncConflict {
        /// The data type where the conflict occurred.
        data_type: String,
    },
}
