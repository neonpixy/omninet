use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur during Crown identity operations.
///
/// 29 variants covering state, persona, key, signing, rotation,
/// recovery, sync, profile, storage, blinding, and bridged errors.
/// All variants are `Send + Sync`.
#[derive(Error, Debug)]
pub enum CrownError {
    // -- State errors --

    /// The keyring is locked (no keys in memory). Unlock first.
    #[error("crown is locked: unlock required")]
    Locked,

    /// No primary identity exists. Generate or import one.
    #[error("no identity exists: create one first")]
    NoIdentity,

    /// The soul.json file could not be parsed. May be corrupted or wrong version.
    #[error("soul file corrupted: {0}")]
    SoulCorrupted(String),

    // -- Persona errors --

    /// A named persona was not found in the keyring.
    #[error("persona not found: {0}")]
    PersonaNotFound(String),

    /// Attempted to create a persona with a name that already exists.
    #[error("persona already exists: {0}")]
    PersonaAlreadyExists(String),

    /// Attempted to delete the primary identity (not allowed).
    #[error("cannot delete primary identity")]
    CannotDeletePrimary,

    // -- Key errors --

    /// A bech32 `csec1...` string could not be decoded into a valid private key.
    #[error("invalid crown secret: {reason}")]
    InvalidCrownSecret { reason: String },

    /// A bech32 `cpub1...` string could not be decoded into a valid public key.
    #[error("invalid crown ID: {reason}")]
    InvalidCrownId { reason: String },

    /// Raw private key bytes are invalid (wrong length or not a valid scalar).
    #[error("invalid private key: {reason}")]
    InvalidPrivateKey { reason: String },

    // -- Signing errors --

    /// BIP-340 Schnorr signing failed (missing private key or invalid key material).
    #[error("signature failed: {0}")]
    SignatureFailed(String),

    /// Signature verification failed (used by the founding tree).
    #[error("verification failed")]
    VerificationFailed,

    // -- Rotation errors --

    /// Key rotation failed (signing or chain update error).
    #[error("rotation failed: {0}")]
    RotationFailed(String),

    /// Operation requires a primary key, but none is loaded.
    #[error("no primary key exists")]
    NoPrimaryKey,

    // -- Recovery errors --

    /// Account recovery failed (bad secret, incompatible shares, etc.).
    #[error("recovery failed: {0}")]
    RecoveryFailed(String),

    /// Not enough shares were provided to reconstruct the secret.
    #[error("insufficient shares for secret reconstruction")]
    InsufficientShares,

    /// Symmetric decryption failed (wrong key or corrupted ciphertext).
    #[error("decryption failed")]
    DecryptionFailed,

    // -- Sync errors --

    /// Device sync protocol encountered an error.
    #[error("sync failed: {0}")]
    SyncFailed(String),

    /// The sync offer's 5-minute window has elapsed.
    #[error("sync offer expired")]
    SyncExpired,

    /// The sync accept response is invalid (nonce mismatch, bad signature, etc.).
    #[error("invalid sync response: {0}")]
    InvalidSyncResponse(String),

    // -- Profile errors --

    /// A profile update could not be applied.
    #[error("profile update failed: {0}")]
    ProfileUpdateFailed(String),

    /// Profile data failed validation.
    #[error("invalid profile data: {0}")]
    InvalidProfileData(String),

    // -- Storage errors --

    /// Could not read soul data from disk.
    #[error("load failed: {path}: {reason}")]
    LoadFailed { path: PathBuf, reason: String },

    /// Could not write soul data to disk.
    #[error("save failed: {path}: {reason}")]
    SaveFailed { path: PathBuf, reason: String },

    // -- Blinding errors --

    /// HKDF derivation of a blinded keypair failed.
    #[error("blinding failed: {0}")]
    BlindingFailed(String),

    /// The blinding context is invalid (e.g., empty context_id).
    #[error("invalid blinding context: {0}")]
    InvalidBlindingContext(String),

    /// No cached blinded key exists for the requested context. Derive one first.
    #[error("blinded key not found: {0}")]
    BlindedKeyNotFound(String),

    /// Creating or verifying a blinding proof failed.
    #[error("blinding proof failed: {0}")]
    BlindingProofFailed(String),

    // -- Bridged errors --

    /// An I/O error from the filesystem.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        assert!(CrownError::Locked.to_string().contains("locked"));
        assert!(CrownError::NoIdentity.to_string().contains("no identity"));
        assert!(CrownError::CannotDeletePrimary
            .to_string()
            .contains("primary"));
        assert!(CrownError::PersonaNotFound("work".into())
            .to_string()
            .contains("work"));
        assert!(CrownError::PersonaAlreadyExists("anon".into())
            .to_string()
            .contains("anon"));
        assert!(CrownError::VerificationFailed
            .to_string()
            .contains("verification"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CrownError>();
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let crown_err: CrownError = io_err.into();
        assert!(matches!(crown_err, CrownError::Io(_)));
        assert!(crown_err.to_string().contains("gone"));
    }

    #[test]
    fn json_error_conversion() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let crown_err: CrownError = json_err.into();
        assert!(matches!(crown_err, CrownError::Serialization(_)));
    }

    #[test]
    fn load_save_errors_include_path() {
        let err = CrownError::LoadFailed {
            path: PathBuf::from("/home/user/soul.json"),
            reason: "file not found".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("soul.json"));
        assert!(msg.contains("file not found"));

        let err = CrownError::SaveFailed {
            path: PathBuf::from("/tmp/test"),
            reason: "permission denied".into(),
        };
        assert!(err.to_string().contains("permission denied"));
    }
}
