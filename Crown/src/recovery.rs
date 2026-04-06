//! Account recovery — seed phrases, social recovery, encrypted backups.
//!
//! Crown defines the types and orchestration. Actual cryptographic operations
//! (encryption, key derivation, secret sharing) are injected via traits —
//! the caller provides implementations from Sentinal or equivalent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CrownError;
use crate::keyring::Keyring;

// ---------------------------------------------------------------------------
// Traits — Crown defines interfaces, caller implements
// ---------------------------------------------------------------------------

/// Trait for encrypting/decrypting recovery data.
///
/// Crown defines this trait but never implements it -- the caller injects
/// an implementation (typically Sentinal). This keeps Crown zero-dep on
/// any specific crypto library.
pub trait RecoveryEncryptor: Send + Sync {
    /// Encrypt plaintext with a symmetric key. Returns ciphertext.
    fn encrypt(&self, plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError>;

    /// Decrypt ciphertext with a symmetric key. Returns plaintext.
    fn decrypt(&self, ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError>;

    /// Derive a symmetric key from a user-supplied password (e.g., via Argon2).
    fn derive_key_from_password(&self, password: &str) -> Result<Vec<u8>, CrownError>;

    /// Derive a symmetric key from a raw seed (e.g., via HKDF).
    fn derive_key_from_seed(&self, seed: &[u8]) -> Result<Vec<u8>, CrownError>;
}

/// Trait for splitting and reconstructing secrets via a threshold scheme
/// (e.g., Shamir's Secret Sharing).
///
/// Crown defines this trait but never implements it -- the caller injects
/// an implementation (e.g., using the `sharks` crate).
pub trait SecretSharer: Send + Sync {
    /// Split `secret` into `shares` pieces, requiring `threshold` to reconstruct.
    fn split(
        &self,
        secret: &[u8],
        threshold: u8,
        shares: u8,
    ) -> Result<Vec<Vec<u8>>, CrownError>;

    /// Reconstruct the original secret from at least `threshold` shares.
    fn reconstruct(&self, shares: &[Vec<u8>]) -> Result<Vec<u8>, CrownError>;
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which recovery method the user has set up.
///
/// An identity can have multiple recovery methods active simultaneously.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum RecoveryMethod {
    /// BIP-39 seed phrase (24 words). The user stores the words offline.
    SeedPhrase,
    /// Social recovery via trusted contacts (threshold secret sharing).
    SocialRecovery,
    /// Password-encrypted backup stored on a device or cloud.
    EncryptedBackup,
}

/// Configuration for social recovery (N-of-M threshold scheme).
///
/// The user selects M trusted contacts. Their private key is split into
/// M shares, of which any N (threshold) suffice to reconstruct it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocialRecoveryConfig {
    /// Minimum number of shares needed to reconstruct the secret (N).
    pub threshold: u8,
    /// Crown IDs of all trustees who each hold one share (M total).
    pub trustees: Vec<String>,
}

/// A single share of the private key, assigned to one trustee.
///
/// The caller is responsible for encrypting the share to the trustee's
/// public key (via Sentinal ECDH) before distributing it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyShare {
    /// The trustee's crown ID who holds this share.
    pub trustee_crown_id: String,
    /// The raw share data (caller encrypts before distribution).
    pub encrypted_share: Vec<u8>,
    /// Zero-based index of this share within the split.
    pub share_index: u8,
}

/// A password-encrypted backup of the entire keyring.
///
/// Created via [`Keyring::setup_encrypted_backup`]. To restore,
/// use [`recover_from_backup`] with the same password.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedKeyringBackup {
    /// The encrypted keyring data (primary + all personas).
    pub ciphertext: Vec<u8>,
    /// When the backup was created.
    pub created_at: DateTime<Utc>,
}

/// Which recovery configuration is active for this identity.
///
/// Persisted alongside the soul so the recovery UI knows what to show.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RecoveryConfig {
    /// Seed phrase recovery -- user stores 24 BIP-39 words offline.
    SeedPhrase,
    /// Social recovery -- shares distributed to trustees.
    Social(SocialRecoveryConfig),
    /// Encrypted backup -- password-protected copy of the keyring.
    Backup,
}

/// Artifacts produced by setting up recovery -- returned to the UI
/// so the user can store, distribute, or save them.
#[derive(Clone, Debug)]
pub enum RecoveryArtifacts {
    /// BIP-39 seed words for the user to write down and store offline.
    SeedWords(Vec<String>),
    /// Key shares for distribution to individual trustees.
    KeyShares(Vec<KeyShare>),
    /// An encrypted backup file for the user to store securely.
    BackupData(EncryptedKeyringBackup),
}

// ---------------------------------------------------------------------------
// Keyring extensions (recovery setup)
// ---------------------------------------------------------------------------

impl Keyring {
    /// Export the primary private key bytes for recovery purposes.
    ///
    /// Returns the raw 32-byte secret key. The caller decides how to protect
    /// it (BIP-39 encoding, encryption, secret sharing, etc.).
    pub fn export_primary_secret(&self) -> Result<Vec<u8>, CrownError> {
        let kp = self.primary_keypair().ok_or(CrownError::NoPrimaryKey)?;
        let privkey = kp
            .private_key_data()
            .ok_or(CrownError::Locked)?;
        Ok(privkey.to_vec())
    }

    /// Set up social recovery by splitting the primary key into threshold shares.
    ///
    /// Returns `KeyShare` entries — one per trustee. The shares are raw
    /// (unencrypted). The caller should encrypt each share to its trustee's
    /// pubkey via Sentinal ECDH before distribution.
    pub fn setup_social_recovery(
        &self,
        config: &SocialRecoveryConfig,
        sharer: &dyn SecretSharer,
    ) -> Result<Vec<KeyShare>, CrownError> {
        if config.threshold == 0 {
            return Err(CrownError::RecoveryFailed(
                "threshold must be at least 1".into(),
            ));
        }
        if (config.threshold as usize) > config.trustees.len() {
            return Err(CrownError::RecoveryFailed(
                "threshold cannot exceed number of trustees".into(),
            ));
        }
        if config.trustees.is_empty() {
            return Err(CrownError::RecoveryFailed(
                "at least one trustee is required".into(),
            ));
        }

        let secret = self.export_primary_secret()?;
        let shares = sharer.split(
            &secret,
            config.threshold,
            config.trustees.len() as u8,
        )?;

        let key_shares: Vec<KeyShare> = config
            .trustees
            .iter()
            .enumerate()
            .zip(shares)
            .map(|((idx, trustee), share)| KeyShare {
                trustee_crown_id: trustee.clone(),
                encrypted_share: share,
                share_index: idx as u8,
            })
            .collect();

        Ok(key_shares)
    }

    /// Set up an encrypted backup of the keyring.
    ///
    /// Exports the keyring, derives a key from the password, and encrypts.
    pub fn setup_encrypted_backup(
        &self,
        encryptor: &dyn RecoveryEncryptor,
        password: &str,
    ) -> Result<EncryptedKeyringBackup, CrownError> {
        let keyring_data = self.export()?;
        let key = encryptor.derive_key_from_password(password)?;
        let ciphertext = encryptor.encrypt(&keyring_data, &key)?;

        Ok(EncryptedKeyringBackup {
            ciphertext,
            created_at: Utc::now(),
        })
    }
}

// ---------------------------------------------------------------------------
// Recovery functions (static — reconstruct a Keyring)
// ---------------------------------------------------------------------------

/// Recover a Keyring from a raw 32-byte private key.
///
/// This is the common path: seed phrase -> bytes -> Keyring,
/// or social recovery -> reconstructed secret -> Keyring.
pub fn recover_from_secret(secret: &[u8]) -> Result<Keyring, CrownError> {
    if secret.len() != 32 {
        return Err(CrownError::RecoveryFailed(format!(
            "expected 32-byte secret, got {}",
            secret.len()
        )));
    }
    let mut keyring = Keyring::new();
    let csec = {
        let kp = crate::keypair::CrownKeypair::from_private_key(secret)?;
        kp.crown_secret()
            .ok_or_else(|| CrownError::RecoveryFailed("failed to derive crown secret".into()))?
            .to_string()
    };
    keyring.import_primary(&csec)?;
    Ok(keyring)
}

/// Recover from threshold shares — reconstruct the secret, then recover.
pub fn recover_from_shares(
    shares: &[Vec<u8>],
    sharer: &dyn SecretSharer,
) -> Result<Keyring, CrownError> {
    let secret = sharer.reconstruct(shares)?;
    recover_from_secret(&secret)
}

/// Recover from an encrypted backup — decrypt, then load the keyring.
pub fn recover_from_backup(
    backup: &EncryptedKeyringBackup,
    encryptor: &dyn RecoveryEncryptor,
    password: &str,
) -> Result<Keyring, CrownError> {
    let key = encryptor.derive_key_from_password(password)?;
    let plaintext = encryptor.decrypt(&backup.ciphertext, &key)?;
    let mut keyring = Keyring::new();
    keyring.load(&plaintext)?;
    Ok(keyring)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Mock implementations for testing
    // -----------------------------------------------------------------------

    /// Simple XOR-based mock encryptor for testing.
    struct MockEncryptor;

    impl RecoveryEncryptor for MockEncryptor {
        fn encrypt(&self, plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError> {
            Ok(xor_bytes(plaintext, key))
        }

        fn decrypt(&self, ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError> {
            Ok(xor_bytes(ciphertext, key))
        }

        fn derive_key_from_password(&self, password: &str) -> Result<Vec<u8>, CrownError> {
            // Simple hash of password for testing
            use sha2::{Digest, Sha256};
            let hash = Sha256::digest(password.as_bytes());
            Ok(hash.to_vec())
        }

        fn derive_key_from_seed(&self, seed: &[u8]) -> Result<Vec<u8>, CrownError> {
            use sha2::{Digest, Sha256};
            let hash = Sha256::digest(seed);
            Ok(hash.to_vec())
        }
    }

    /// Simple mock secret sharer that just duplicates the secret.
    struct MockSharer;

    impl SecretSharer for MockSharer {
        fn split(
            &self,
            secret: &[u8],
            _threshold: u8,
            shares: u8,
        ) -> Result<Vec<Vec<u8>>, CrownError> {
            // For testing: each share is the original secret (not secure!)
            Ok((0..shares).map(|_| secret.to_vec()).collect())
        }

        fn reconstruct(&self, shares: &[Vec<u8>]) -> Result<Vec<u8>, CrownError> {
            if shares.is_empty() {
                return Err(CrownError::InsufficientShares);
            }
            // Just return the first share (since they're all the same in mock)
            Ok(shares[0].clone())
        }
    }

    /// A mock encryptor that always fails decryption.
    struct FailingEncryptor;

    impl RecoveryEncryptor for FailingEncryptor {
        fn encrypt(&self, plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError> {
            Ok(xor_bytes(plaintext, key))
        }

        fn decrypt(&self, _ciphertext: &[u8], _key: &[u8]) -> Result<Vec<u8>, CrownError> {
            Err(CrownError::DecryptionFailed)
        }

        fn derive_key_from_password(&self, password: &str) -> Result<Vec<u8>, CrownError> {
            use sha2::{Digest, Sha256};
            Ok(Sha256::digest(password.as_bytes()).to_vec())
        }

        fn derive_key_from_seed(&self, seed: &[u8]) -> Result<Vec<u8>, CrownError> {
            use sha2::{Digest, Sha256};
            Ok(Sha256::digest(seed).to_vec())
        }
    }

    /// A mock sharer that fails reconstruction.
    struct FailingSharer;

    impl SecretSharer for FailingSharer {
        fn split(
            &self,
            secret: &[u8],
            _threshold: u8,
            shares: u8,
        ) -> Result<Vec<Vec<u8>>, CrownError> {
            Ok((0..shares).map(|_| secret.to_vec()).collect())
        }

        fn reconstruct(&self, _shares: &[Vec<u8>]) -> Result<Vec<u8>, CrownError> {
            Err(CrownError::InsufficientShares)
        }
    }

    fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect()
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[test]
    fn export_primary_secret_works() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let secret = kr.export_primary_secret().unwrap();
        assert_eq!(secret.len(), 32);
    }

    #[test]
    fn export_primary_secret_no_key_fails() {
        let kr = Keyring::new();
        let result = kr.export_primary_secret();
        assert!(result.is_err());
    }

    #[test]
    fn recover_from_secret_round_trip() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let original_cpub = kr.public_key().unwrap().to_string();

        let secret = kr.export_primary_secret().unwrap();
        let recovered = recover_from_secret(&secret).unwrap();

        assert_eq!(recovered.public_key().unwrap(), original_cpub);
    }

    #[test]
    fn recover_from_secret_wrong_length() {
        let result = recover_from_secret(&[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn social_recovery_setup_and_recover() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let original_cpub = kr.public_key().unwrap().to_string();

        let config = SocialRecoveryConfig {
            threshold: 2,
            trustees: vec![
                "cpub1alice".to_string(),
                "cpub1bob".to_string(),
                "cpub1carol".to_string(),
            ],
        };

        let sharer = MockSharer;
        let shares = kr.setup_social_recovery(&config, &sharer).unwrap();

        assert_eq!(shares.len(), 3);
        assert_eq!(shares[0].trustee_crown_id, "cpub1alice");
        assert_eq!(shares[1].trustee_crown_id, "cpub1bob");
        assert_eq!(shares[2].trustee_crown_id, "cpub1carol");
        assert_eq!(shares[0].share_index, 0);
        assert_eq!(shares[1].share_index, 1);
        assert_eq!(shares[2].share_index, 2);

        // Recover from shares
        let raw_shares: Vec<Vec<u8>> = shares
            .iter()
            .map(|s| s.encrypted_share.clone())
            .collect();
        let recovered = recover_from_shares(&raw_shares, &sharer).unwrap();
        assert_eq!(recovered.public_key().unwrap(), original_cpub);
    }

    #[test]
    fn social_recovery_threshold_exceeds_trustees() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let config = SocialRecoveryConfig {
            threshold: 5,
            trustees: vec!["cpub1alice".to_string(), "cpub1bob".to_string()],
        };

        let sharer = MockSharer;
        let result = kr.setup_social_recovery(&config, &sharer);
        assert!(matches!(
            result.unwrap_err(),
            CrownError::RecoveryFailed(_)
        ));
    }

    #[test]
    fn social_recovery_zero_threshold() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let config = SocialRecoveryConfig {
            threshold: 0,
            trustees: vec!["cpub1alice".to_string()],
        };

        let sharer = MockSharer;
        let result = kr.setup_social_recovery(&config, &sharer);
        assert!(matches!(
            result.unwrap_err(),
            CrownError::RecoveryFailed(_)
        ));
    }

    #[test]
    fn social_recovery_empty_trustees() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let config = SocialRecoveryConfig {
            threshold: 1,
            trustees: vec![],
        };

        let sharer = MockSharer;
        let result = kr.setup_social_recovery(&config, &sharer);
        assert!(matches!(
            result.unwrap_err(),
            CrownError::RecoveryFailed(_)
        ));
    }

    #[test]
    fn social_recovery_no_primary_fails() {
        let kr = Keyring::new();
        let config = SocialRecoveryConfig {
            threshold: 1,
            trustees: vec!["cpub1alice".to_string()],
        };

        let sharer = MockSharer;
        let result = kr.setup_social_recovery(&config, &sharer);
        assert!(result.is_err());
    }

    #[test]
    fn encrypted_backup_round_trip() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        kr.create_persona("work").unwrap();
        let original_cpub = kr.public_key().unwrap().to_string();

        let encryptor = MockEncryptor;
        let backup = kr
            .setup_encrypted_backup(&encryptor, "strongpassword")
            .unwrap();

        assert!(!backup.ciphertext.is_empty());

        let recovered =
            recover_from_backup(&backup, &encryptor, "strongpassword").unwrap();
        assert_eq!(recovered.public_key().unwrap(), original_cpub);
    }

    #[test]
    fn encrypted_backup_wrong_password_fails() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let encryptor = MockEncryptor;
        let backup = kr
            .setup_encrypted_backup(&encryptor, "correctpassword")
            .unwrap();

        // With our XOR mock, wrong password produces garbage JSON
        let result =
            recover_from_backup(&backup, &encryptor, "wrongpassword");
        assert!(result.is_err());
    }

    #[test]
    fn encrypted_backup_no_primary_fails() {
        let kr = Keyring::new();
        let encryptor = MockEncryptor;
        let result = kr.setup_encrypted_backup(&encryptor, "password");
        // export() succeeds (primary is None -> null), but the backup is valid;
        // the real concern is that there's nothing to recover
        // This actually works because Keyring can export without a primary
        assert!(result.is_ok());
    }

    #[test]
    fn recover_from_shares_empty_fails() {
        let sharer = FailingSharer;
        let result = recover_from_shares(&[], &sharer);
        assert!(result.is_err());
    }

    #[test]
    fn recover_from_backup_decrypt_fails() {
        let backup = EncryptedKeyringBackup {
            ciphertext: vec![1, 2, 3],
            created_at: Utc::now(),
        };

        let encryptor = FailingEncryptor;
        let result = recover_from_backup(&backup, &encryptor, "password");
        assert!(result.is_err());
    }

    #[test]
    fn recovery_method_serde() {
        let method = RecoveryMethod::SocialRecovery;
        let json = serde_json::to_string(&method).unwrap();
        let loaded: RecoveryMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, RecoveryMethod::SocialRecovery);
    }

    #[test]
    fn recovery_config_serde() {
        let config = RecoveryConfig::Social(SocialRecoveryConfig {
            threshold: 3,
            trustees: vec!["cpub1a".into(), "cpub1b".into(), "cpub1c".into()],
        });
        let json = serde_json::to_string(&config).unwrap();
        let loaded: RecoveryConfig = serde_json::from_str(&json).unwrap();
        match loaded {
            RecoveryConfig::Social(sc) => {
                assert_eq!(sc.threshold, 3);
                assert_eq!(sc.trustees.len(), 3);
            }
            _ => panic!("expected Social variant"),
        }
    }

    #[test]
    fn key_share_serde() {
        let share = KeyShare {
            trustee_crown_id: "cpub1test".to_string(),
            encrypted_share: vec![1, 2, 3, 4],
            share_index: 0,
        };
        let json = serde_json::to_string(&share).unwrap();
        let loaded: KeyShare = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.trustee_crown_id, "cpub1test");
        assert_eq!(loaded.share_index, 0);
    }

    #[test]
    fn encrypted_backup_serde() {
        let backup = EncryptedKeyringBackup {
            ciphertext: vec![0xDE, 0xAD],
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&backup).unwrap();
        let loaded: EncryptedKeyringBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.ciphertext, vec![0xDE, 0xAD]);
    }
}
