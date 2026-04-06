use hkdf::Hkdf;
use sha2::Sha256;
use uuid::Uuid;

use crate::error::SentinalError;
use crate::secure_data::SecureData;

// --- Constants ---

/// PBKDF2 iteration count. 600,000 is the standard for master key derivation.
pub const PBKDF2_ITERATIONS: u32 = 600_000;

/// Standard key length in bytes (256-bit).
pub const KEY_LENGTH: usize = 32;

/// Default salt length in bytes.
pub const SALT_LENGTH: usize = 32;

// HKDF salt strings (domain separation for derived keys).
const CONTENT_SALT: &[u8] = b"omnidea-content-v1";
const VOCABULARY_SALT: &[u8] = b"omnidea-vocabulary-v1";
const DIMENSION_SALT: &[u8] = b"omnidea-dimension-v1";
const STORAGE_SALT: &[u8] = b"omnidea-storage-v1";
const SHARE_SALT: &[u8] = b"omnidea-share-v1";
const IDENTITY_SALT: &[u8] = b"omnidea-identity-v1";
const SOUL_SALT: &[u8] = b"omnidea-soul-v1";

// --- Public API ---

/// Derive a 256-bit master key from a password using PBKDF2-HMAC-SHA256.
///
/// If no salt is provided, a fresh 32-byte random salt is generated.
/// Returns `(master_key, salt)`.
pub fn derive_master_key(
    password: &str,
    salt: Option<&[u8]>,
) -> Result<(SecureData, Vec<u8>), SentinalError> {
    let salt_bytes = match salt {
        Some(s) => s.to_vec(),
        None => generate_salt(SALT_LENGTH)?,
    };

    let mut key = vec![0u8; KEY_LENGTH];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt_bytes, PBKDF2_ITERATIONS, &mut key);

    Ok((SecureData::new(key), salt_bytes))
}

/// Derive a per-idea content key from the master key using HKDF-SHA256.
///
/// Salt: `"omnidea-content-v1"`, Info: `"content-{uuid}"`.
pub fn derive_content_key(
    master_key: &[u8],
    idea_id: &Uuid,
) -> Result<SecureData, SentinalError> {
    let info = format!("content-{idea_id}");
    hkdf_derive(master_key, CONTENT_SALT, info.as_bytes())
}

/// Derive a vocabulary seed from the master key using HKDF-SHA256.
///
/// Salt: `"omnidea-vocabulary-v1"`, Info: `"vocabulary-seed"`.
/// The seed is used by Lingo's Babel for text obfuscation.
pub fn derive_vocabulary_seed(master_key: &[u8]) -> Result<SecureData, SentinalError> {
    hkdf_derive(master_key, VOCABULARY_SALT, b"vocabulary-seed")
}

/// Derive a per-dimension key from the master key using HKDF-SHA256.
///
/// Salt: `"omnidea-dimension-v1"`, Info: `"dimension-{uuid}"`.
pub fn derive_dimension_key(
    master_key: &[u8],
    dimension_id: &Uuid,
) -> Result<SecureData, SentinalError> {
    let info = format!("dimension-{dimension_id}");
    hkdf_derive(master_key, DIMENSION_SALT, info.as_bytes())
}

/// Derive a shared key from an X25519 shared secret using HKDF-SHA256.
///
/// Salt: `"omnidea-share-v1"`, Info: empty.
/// Used internally by PublicKeySlot.
pub fn derive_shared_key(shared_secret: &[u8]) -> Result<SecureData, SentinalError> {
    hkdf_derive(shared_secret, SHARE_SALT, b"")
}

/// Derive a storage encryption key from a private key using HKDF-SHA256.
///
/// Salt: `"omnidea-storage-v1"`, Info: `"storage-{context}"`.
/// Used by Tower and Omnibus to derive SQLCipher keys for relay databases.
/// The `context` parameter provides domain separation (e.g., "tower-relay",
/// "omnibus-relay") so different databases get different keys from the same
/// private key material.
pub fn derive_storage_key(
    private_key_bytes: &[u8],
    context: &str,
) -> Result<SecureData, SentinalError> {
    let info = format!("storage-{context}");
    hkdf_derive(private_key_bytes, STORAGE_SALT, info.as_bytes())
}

/// Derive a 32-byte identity key from a BIP-39 seed using HKDF-SHA256.
///
/// Salt: `"omnidea-identity-v1"`, Info: `"identity-primary"`.
/// The 64-byte BIP-39 seed is the input key material. The output is a
/// 32-byte secp256k1-compatible private key suitable for Crown import.
pub fn derive_identity_key(seed: &[u8]) -> Result<SecureData, SentinalError> {
    if seed.len() < 32 {
        return Err(SentinalError::InvalidKeyLength {
            expected: 64,
            actual: seed.len(),
        });
    }
    hkdf_derive(seed, IDENTITY_SALT, b"identity-primary")
}

/// Derive a soul encryption key from a master key using HKDF-SHA256.
///
/// Salt: `"omnidea-soul-v1"`, Info: `"soul-data"`.
/// Used to encrypt Crown's soul.json at rest.
pub fn derive_soul_key(master_key: &[u8]) -> Result<SecureData, SentinalError> {
    hkdf_derive(master_key, SOUL_SALT, b"soul-data")
}

/// Generate cryptographically random salt bytes.
pub fn generate_salt(length: usize) -> Result<Vec<u8>, SentinalError> {
    let mut salt = vec![0u8; length];
    getrandom::fill(&mut salt).map_err(|e| {
        SentinalError::RandomGenerationFailed(format!("salt generation failed: {e}"))
    })?;
    Ok(salt)
}

// --- Internal ---

fn hkdf_derive(
    input_key_material: &[u8],
    salt: &[u8],
    info: &[u8],
) -> Result<SecureData, SentinalError> {
    let hk = Hkdf::<Sha256>::new(Some(salt), input_key_material);
    let mut output = vec![0u8; KEY_LENGTH];
    hk.expand(info, &mut output).map_err(|e| {
        SentinalError::KeyDerivationFailed(format!("HKDF expand failed: {e}"))
    })?;
    Ok(SecureData::new(output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_master_key_with_generated_salt() {
        let (key, salt) = derive_master_key("test-password", None).unwrap();
        assert_eq!(key.len(), KEY_LENGTH);
        assert_eq!(salt.len(), SALT_LENGTH);
    }

    #[test]
    fn derive_master_key_with_provided_salt() {
        let salt = vec![0xAA; SALT_LENGTH];
        let (key, returned_salt) = derive_master_key("test-password", Some(&salt)).unwrap();
        assert_eq!(key.len(), KEY_LENGTH);
        assert_eq!(returned_salt, salt);
    }

    #[test]
    fn derive_master_key_deterministic() {
        let salt = vec![0xBB; SALT_LENGTH];
        let (key1, _) = derive_master_key("same-password", Some(&salt)).unwrap();
        let (key2, _) = derive_master_key("same-password", Some(&salt)).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_master_key_different_passwords() {
        let salt = vec![0xCC; SALT_LENGTH];
        let (key1, _) = derive_master_key("password-a", Some(&salt)).unwrap();
        let (key2, _) = derive_master_key("password-b", Some(&salt)).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_master_key_different_salts() {
        let (key1, _) = derive_master_key("same-pw", Some(&[0xAA; 32])).unwrap();
        let (key2, _) = derive_master_key("same-pw", Some(&[0xBB; 32])).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_content_key_per_idea() {
        let master = vec![0x42; KEY_LENGTH];
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let key1 = derive_content_key(&master, &id1).unwrap();
        let key2 = derive_content_key(&master, &id2).unwrap();
        assert_eq!(key1.len(), KEY_LENGTH);
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_vocabulary_seed() {
        let master = vec![0x42; KEY_LENGTH];
        let seed = super::derive_vocabulary_seed(&master).unwrap();
        assert_eq!(seed.len(), KEY_LENGTH);
    }

    #[test]
    fn derive_dimension_key_per_dimension() {
        let master = vec![0x42; KEY_LENGTH];
        let dim1 = Uuid::new_v4();
        let dim2 = Uuid::new_v4();
        let key1 = derive_dimension_key(&master, &dim1).unwrap();
        let key2 = derive_dimension_key(&master, &dim2).unwrap();
        assert_eq!(key1.len(), KEY_LENGTH);
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_shared_key_from_secret() {
        let shared_secret = vec![0x55; 32];
        let key = derive_shared_key(&shared_secret).unwrap();
        assert_eq!(key.len(), KEY_LENGTH);
    }

    #[test]
    fn derive_storage_key_deterministic() {
        let private_key = vec![0x42; 32];
        let key1 = derive_storage_key(&private_key, "tower-relay").unwrap();
        let key2 = derive_storage_key(&private_key, "tower-relay").unwrap();
        assert_eq!(key1.len(), KEY_LENGTH);
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_storage_key_different_contexts() {
        let private_key = vec![0x42; 32];
        let key1 = derive_storage_key(&private_key, "tower-relay").unwrap();
        let key2 = derive_storage_key(&private_key, "omnibus-relay").unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_storage_key_different_keys() {
        let key1 = derive_storage_key(&[0x42; 32], "tower-relay").unwrap();
        let key2 = derive_storage_key(&[0x43; 32], "tower-relay").unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_identity_key_from_seed() {
        let seed = vec![0x42; 64];
        let key = derive_identity_key(&seed).unwrap();
        assert_eq!(key.len(), KEY_LENGTH);
    }

    #[test]
    fn derive_identity_key_deterministic() {
        let seed = vec![0x42; 64];
        let key1 = derive_identity_key(&seed).unwrap();
        let key2 = derive_identity_key(&seed).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_identity_key_different_seeds() {
        let key1 = derive_identity_key(&[0x42; 64]).unwrap();
        let key2 = derive_identity_key(&[0x43; 64]).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_identity_key_short_seed_fails() {
        let result = derive_identity_key(&[0x42; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn derive_soul_key_from_master() {
        let master = vec![0x42; KEY_LENGTH];
        let key = super::derive_soul_key(&master).unwrap();
        assert_eq!(key.len(), KEY_LENGTH);
    }

    #[test]
    fn derive_soul_key_deterministic() {
        let master = vec![0x42; KEY_LENGTH];
        let key1 = super::derive_soul_key(&master).unwrap();
        let key2 = super::derive_soul_key(&master).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_soul_key_different_masters() {
        let key1 = super::derive_soul_key(&[0x42; KEY_LENGTH]).unwrap();
        let key2 = super::derive_soul_key(&[0x43; KEY_LENGTH]).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn derive_soul_key_differs_from_content_key() {
        let master = vec![0x42; KEY_LENGTH];
        let soul_key = super::derive_soul_key(&master).unwrap();
        let content_key = derive_content_key(&master, &Uuid::nil()).unwrap();
        assert_ne!(soul_key.expose(), content_key.expose());
    }

    #[test]
    fn generate_salt_correct_length() {
        let salt = generate_salt(32).unwrap();
        assert_eq!(salt.len(), 32);
        // Random salt should not be all zeros.
        assert!(salt.iter().any(|&b| b != 0));
    }
}
