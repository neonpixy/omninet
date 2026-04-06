use thiserror::Error;
use uuid::Uuid;

/// Errors from Vault operations.
#[derive(Error, Debug)]
pub enum VaultError {
    /// An operation was attempted while the vault is locked.
    #[error("vault is locked")]
    Locked,

    /// Tried to unlock a vault that is already open.
    #[error("vault is already unlocked")]
    AlreadyUnlocked,

    /// The password did not match the vault's stored salt/key, or the
    /// manifest database is corrupted.
    #[error("wrong password or corrupted vault")]
    WrongPassword,

    /// No manifest entry exists with the given idea ID.
    #[error("idea not found: {0}")]
    IdeaNotFound(Uuid),

    /// No manifest entry exists at the given relative path.
    #[error("path not found in manifest: {0}")]
    PathNotFound(String),

    /// No collective with the given ID is known to this vault.
    #[error("collective not found: {0}")]
    CollectiveNotFound(Uuid),

    /// The current user is not a member of the specified collective.
    #[error("not a member of collective: {0}")]
    NotCollectiveMember(Uuid),

    /// The user's role is too low for the requested operation.
    #[error("insufficient permissions: have {current}, need {required}")]
    InsufficientPermissions {
        current: String,
        required: String,
    },

    /// An error from the SQLCipher manifest database.
    #[error("database error: {0}")]
    Database(String),

    /// The manifest key ID has not been set in the vault config.
    #[error("manifest key ID not configured")]
    ManifestKeyIdNotSet,

    /// The vault config file is missing or malformed.
    #[error("config error: {0}")]
    Config(String),

    /// An encryption or decryption operation failed in Sentinal.
    #[error("encryption failed: {0}")]
    Encryption(#[from] sentinal::SentinalError),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A general filesystem I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = VaultError::Locked;
        assert_eq!(err.to_string(), "vault is locked");

        let err = VaultError::WrongPassword;
        assert!(err.to_string().contains("wrong password"));

        let id = Uuid::new_v4();
        let err = VaultError::IdeaNotFound(id);
        assert!(err.to_string().contains(&id.to_string()));
    }

    #[test]
    fn error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<VaultError>();
    }

    #[test]
    fn sentinal_error_conversion() {
        let sentinal_err = sentinal::SentinalError::DecryptionFailed;
        let vault_err: VaultError = sentinal_err.into();
        assert!(matches!(vault_err, VaultError::Encryption(_)));
    }
}
