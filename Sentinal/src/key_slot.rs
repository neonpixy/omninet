use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

use crate::encryption;
use crate::error::SentinalError;
use crate::key_derivation;
use crate::secure_data::SecureData;

/// A key slot: an encrypted container holding a content key, unlockable
/// with a specific type of credential.
///
/// Multiple key slots can protect the same content key, enabling
/// different unlock paths (password, public key, dimension key).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KeySlot {
    /// Unlocked with a user-entered password (PBKDF2 wrapping).
    Password(PasswordKeySlot),
    /// Unlocked with an X25519 private key (ECDH + HKDF wrapping).
    PublicKey(PublicKeySlot),
    /// Unlocked with a dimension-derived symmetric key (direct AES-GCM wrapping).
    Internal(InternalKeySlot),
}

/// The credential used to unlock a key slot.
pub enum KeySlotCredential<'a> {
    /// A user-entered password (for PasswordKeySlot).
    Password(&'a str),
    /// An X25519 private key (for PublicKeySlot).
    PrivateKey(&'a [u8; 32]),
    /// A dimension-derived symmetric key (for InternalKeySlot).
    DimensionKey(&'a [u8]),
}

impl KeySlot {
    /// Unwrap the content key from this slot using the given credential.
    pub fn unwrap(&self, credential: KeySlotCredential) -> Result<SecureData, SentinalError> {
        match (self, credential) {
            (KeySlot::Password(slot), KeySlotCredential::Password(pw)) => slot.unwrap(pw),
            (KeySlot::PublicKey(slot), KeySlotCredential::PrivateKey(sk)) => slot.unwrap(sk),
            (KeySlot::Internal(slot), KeySlotCredential::DimensionKey(dk)) => slot.unwrap(dk),
            _ => Err(SentinalError::CredentialMismatch),
        }
    }
}

// --- PasswordKeySlot ---

/// A key slot protected by a password.
///
/// The password is run through PBKDF2 (600K iterations) to derive a
/// wrapping key, which encrypts the content key via AES-256-GCM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordKeySlot {
    /// PBKDF2 salt (32 bytes).
    pub salt: Vec<u8>,
    /// AES-256-GCM encrypted content key (combined format: nonce || ct || tag).
    pub wrapped_key: Vec<u8>,
}

impl PasswordKeySlot {
    /// Create a new password-protected key slot wrapping the given content key.
    pub fn create(content_key: &[u8], password: &str) -> Result<KeySlot, SentinalError> {
        let (wrap_key, salt) = key_derivation::derive_master_key(password, None)?;
        let wrapped = encryption::encrypt_combined(content_key, wrap_key.expose())?;

        Ok(KeySlot::Password(PasswordKeySlot {
            salt,
            wrapped_key: wrapped,
        }))
    }

    fn unwrap(&self, password: &str) -> Result<SecureData, SentinalError> {
        let (wrap_key, _) = key_derivation::derive_master_key(password, Some(&self.salt))?;
        let content_key = encryption::decrypt_combined(&self.wrapped_key, wrap_key.expose())?;
        Ok(SecureData::new(content_key))
    }
}

// --- PublicKeySlot ---

/// A key slot protected by X25519 public-key cryptography.
///
/// An ephemeral X25519 keypair performs ECDH with the recipient's
/// public key. The shared secret is run through HKDF to derive a
/// wrapping key, which encrypts the content key via AES-256-GCM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeySlot {
    /// Crown ID of the recipient.
    pub recipient: String,
    /// Ephemeral X25519 public key (32 bytes).
    pub ephemeral_public_key: Vec<u8>,
    /// AES-256-GCM encrypted content key (combined format).
    pub wrapped_key: Vec<u8>,
}

impl PublicKeySlot {
    /// Create a new public-key-protected slot.
    ///
    /// `recipient_public_key` is the recipient's X25519 public key (32 bytes).
    /// `recipient_crown_id` is their Crown identity string.
    pub fn create(
        content_key: &[u8],
        recipient_public_key: &[u8; 32],
        recipient_crown_id: &str,
    ) -> Result<KeySlot, SentinalError> {
        let ephemeral_secret = EphemeralSecret::random();
        let ephemeral_public = PublicKey::from(&ephemeral_secret);

        let recipient_pk = PublicKey::from(*recipient_public_key);
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_pk);

        let wrap_key = key_derivation::derive_shared_key(shared_secret.as_bytes())?;
        let wrapped = encryption::encrypt_combined(content_key, wrap_key.expose())?;

        Ok(KeySlot::PublicKey(PublicKeySlot {
            recipient: recipient_crown_id.to_string(),
            ephemeral_public_key: ephemeral_public.as_bytes().to_vec(),
            wrapped_key: wrapped,
        }))
    }

    fn unwrap(&self, private_key: &[u8; 32]) -> Result<SecureData, SentinalError> {
        let secret = StaticSecret::from(*private_key);
        let ephemeral_pk_bytes: [u8; 32] = self
            .ephemeral_public_key
            .as_slice()
            .try_into()
            .map_err(|_| SentinalError::InvalidWrappedKey)?;
        let ephemeral_pk = PublicKey::from(ephemeral_pk_bytes);

        let shared_secret = secret.diffie_hellman(&ephemeral_pk);
        let wrap_key = key_derivation::derive_shared_key(shared_secret.as_bytes())?;

        let content_key = encryption::decrypt_combined(&self.wrapped_key, wrap_key.expose())?;
        Ok(SecureData::new(content_key))
    }
}

// --- InternalKeySlot ---

/// A key slot protected by a dimension-derived symmetric key.
///
/// Used for internal key wrapping within a vault's dimension hierarchy.
/// The dimension key directly encrypts the content key via AES-256-GCM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalKeySlot {
    /// The dimension this slot belongs to.
    pub dimension_id: Uuid,
    /// AES-256-GCM encrypted content key (combined format).
    pub wrapped_key: Vec<u8>,
}

impl InternalKeySlot {
    /// Create a new internal key slot wrapping the content key with a dimension key.
    pub fn create(
        content_key: &[u8],
        dimension_key: &[u8],
        dimension_id: Uuid,
    ) -> Result<KeySlot, SentinalError> {
        let wrapped = encryption::encrypt_combined(content_key, dimension_key)?;
        Ok(KeySlot::Internal(InternalKeySlot {
            dimension_id,
            wrapped_key: wrapped,
        }))
    }

    fn unwrap(&self, dimension_key: &[u8]) -> Result<SecureData, SentinalError> {
        let content_key = encryption::decrypt_combined(&self.wrapped_key, dimension_key)?;
        Ok(SecureData::new(content_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::StaticSecret;

    fn test_content_key() -> Vec<u8> {
        vec![0x42; key_derivation::KEY_LENGTH]
    }

    #[test]
    fn password_slot_create_and_unwrap() {
        let content_key = test_content_key();
        let slot = PasswordKeySlot::create(&content_key, "my-password").unwrap();
        let recovered = slot
            .unwrap(KeySlotCredential::Password("my-password"))
            .unwrap();
        assert_eq!(recovered.expose(), &content_key);
    }

    #[test]
    fn password_slot_wrong_password() {
        let content_key = test_content_key();
        let slot = PasswordKeySlot::create(&content_key, "correct").unwrap();
        let result = slot.unwrap(KeySlotCredential::Password("wrong"));
        assert!(result.is_err());
    }

    #[test]
    fn password_slot_serialization_round_trip() {
        let content_key = test_content_key();
        let slot = PasswordKeySlot::create(&content_key, "serialize-me").unwrap();
        let json = serde_json::to_string(&slot).unwrap();
        let deserialized: KeySlot = serde_json::from_str(&json).unwrap();
        let recovered = deserialized
            .unwrap(KeySlotCredential::Password("serialize-me"))
            .unwrap();
        assert_eq!(recovered.expose(), &content_key);
    }

    #[test]
    fn public_key_slot_create_and_unwrap() {
        let content_key = test_content_key();

        // Generate recipient keypair.
        let recipient_secret = StaticSecret::random();
        let recipient_public = PublicKey::from(&recipient_secret);

        let slot = PublicKeySlot::create(
            &content_key,
            recipient_public.as_bytes(),
            "cpub1test",
        )
        .unwrap();

        let recovered = slot
            .unwrap(KeySlotCredential::PrivateKey(recipient_secret.as_bytes()))
            .unwrap();
        assert_eq!(recovered.expose(), &content_key);
    }

    #[test]
    fn public_key_slot_wrong_private_key() {
        let content_key = test_content_key();
        let recipient_secret = StaticSecret::random();
        let recipient_public = PublicKey::from(&recipient_secret);

        let slot = PublicKeySlot::create(
            &content_key,
            recipient_public.as_bytes(),
            "cpub1test",
        )
        .unwrap();

        let wrong_secret = StaticSecret::random();
        let result = slot.unwrap(KeySlotCredential::PrivateKey(wrong_secret.as_bytes()));
        assert!(result.is_err());
    }

    #[test]
    fn public_key_slot_serialization_round_trip() {
        let content_key = test_content_key();
        let recipient_secret = StaticSecret::random();
        let recipient_public = PublicKey::from(&recipient_secret);

        let slot = PublicKeySlot::create(
            &content_key,
            recipient_public.as_bytes(),
            "cpub1roundtrip",
        )
        .unwrap();

        let json = serde_json::to_string(&slot).unwrap();
        let deserialized: KeySlot = serde_json::from_str(&json).unwrap();
        let recovered = deserialized
            .unwrap(KeySlotCredential::PrivateKey(recipient_secret.as_bytes()))
            .unwrap();
        assert_eq!(recovered.expose(), &content_key);
    }

    #[test]
    fn internal_slot_create_and_unwrap() {
        let content_key = test_content_key();
        let dimension_key = vec![0xDD; key_derivation::KEY_LENGTH];
        let dimension_id = Uuid::new_v4();

        let slot =
            InternalKeySlot::create(&content_key, &dimension_key, dimension_id).unwrap();
        let recovered = slot
            .unwrap(KeySlotCredential::DimensionKey(&dimension_key))
            .unwrap();
        assert_eq!(recovered.expose(), &content_key);
    }

    #[test]
    fn internal_slot_wrong_dimension_key() {
        let content_key = test_content_key();
        let dimension_key = vec![0xDD; key_derivation::KEY_LENGTH];
        let wrong_key = vec![0xEE; key_derivation::KEY_LENGTH];
        let dimension_id = Uuid::new_v4();

        let slot =
            InternalKeySlot::create(&content_key, &dimension_key, dimension_id).unwrap();
        let result = slot.unwrap(KeySlotCredential::DimensionKey(&wrong_key));
        assert!(result.is_err());
    }

    #[test]
    fn credential_mismatch_error() {
        let content_key = test_content_key();
        let slot = PasswordKeySlot::create(&content_key, "password").unwrap();
        // Try to unwrap a password slot with a dimension key credential.
        let result = slot.unwrap(KeySlotCredential::DimensionKey(&[0xAA; 32]));
        assert!(matches!(result, Err(SentinalError::CredentialMismatch)));
    }

    #[test]
    fn key_slot_preserves_dimension_id() {
        let dimension_id = Uuid::new_v4();
        let slot = InternalKeySlot::create(
            &test_content_key(),
            &[0xDD; key_derivation::KEY_LENGTH],
            dimension_id,
        )
        .unwrap();

        if let KeySlot::Internal(inner) = slot {
            assert_eq!(inner.dimension_id, dimension_id);
        } else {
            panic!("Expected Internal slot");
        }
    }
}
