use thiserror::Error;

/// Errors from Sentinal cryptographic operations.
///
/// Every fallible function in Sentinal returns one of these variants.
/// All variants are `Send + Sync` so they can cross async boundaries.
#[derive(Error, Debug)]
pub enum SentinalError {
    /// The combined-format blob is too short to contain a valid nonce + tag.
    #[error("combined data too short: expected at least {expected} bytes, got {actual}")]
    InvalidCombinedData { expected: usize, actual: usize },

    /// The wrapped key data could not be parsed (wrong length or corrupt).
    #[error("invalid wrapped key data")]
    InvalidWrappedKey,

    /// The credential type does not match the key slot type
    /// (e.g., passing a password to a public-key slot).
    #[error("credential type does not match key slot type")]
    CredentialMismatch,

    /// AES-256-GCM authentication tag verification failed.
    /// This usually means the wrong key was used, or the ciphertext was tampered with.
    #[error("decryption failed: authentication tag mismatch")]
    DecryptionFailed,

    /// PBKDF2 or HKDF key derivation failed.
    #[error("key derivation failed: {0}")]
    KeyDerivationFailed(String),

    /// A key was the wrong length (e.g., 16 bytes instead of the required 32).
    #[error("invalid key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    /// A salt was the wrong length.
    #[error("invalid salt length: expected {expected} bytes, got {actual}")]
    InvalidSalt { expected: usize, actual: usize },

    /// A BIP-39 recovery phrase failed validation (bad checksum, wrong word count, etc.).
    #[error("invalid recovery phrase: {0}")]
    InvalidRecoveryPhrase(String),

    /// The OS cryptographic random number generator failed.
    #[error("random generation failed: {0}")]
    RandomGenerationFailed(String),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// PKCS#7 padding is invalid (zero pad byte, inconsistent bytes, or exceeds data length).
    #[error("invalid padding: {0}")]
    InvalidPadding(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = SentinalError::InvalidCombinedData {
            expected: 28,
            actual: 10,
        };
        assert!(err.to_string().contains("28"));
        assert!(err.to_string().contains("10"));

        let err = SentinalError::DecryptionFailed;
        assert!(err.to_string().contains("authentication tag"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SentinalError>();
    }
}
