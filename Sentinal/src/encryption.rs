use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use serde::{Deserialize, Serialize};

use crate::error::SentinalError;
use crate::key_derivation::KEY_LENGTH;

/// AES-GCM nonce size in bytes.
const NONCE_LENGTH: usize = 12;

/// AES-GCM authentication tag size in bytes.
const TAG_LENGTH: usize = 16;

/// Minimum size of combined encrypted data: nonce + tag (no ciphertext).
const MIN_COMBINED_LENGTH: usize = NONCE_LENGTH + TAG_LENGTH;

/// Structured representation of AES-256-GCM encrypted data.
///
/// Holds the ciphertext, nonce, and authentication tag as separate fields.
/// Use `combined()` to serialize into the wire format, or `from_combined()`
/// to parse it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncryptedData {
    /// The encrypted payload (same length as the original plaintext).
    pub ciphertext: Vec<u8>,
    /// 12-byte nonce (initialization vector).
    pub nonce: Vec<u8>,
    /// 16-byte authentication tag.
    pub tag: Vec<u8>,
}

impl EncryptedData {
    /// Serialize to combined format: `nonce (12) || ciphertext (N) || tag (16)`.
    pub fn combined(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.nonce.len() + self.ciphertext.len() + self.tag.len());
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.ciphertext);
        out.extend_from_slice(&self.tag);
        out
    }

    /// Parse from combined format: `nonce (12) || ciphertext (N) || tag (16)`.
    pub fn from_combined(data: &[u8]) -> Result<Self, SentinalError> {
        if data.len() < MIN_COMBINED_LENGTH {
            return Err(SentinalError::InvalidCombinedData {
                expected: MIN_COMBINED_LENGTH,
                actual: data.len(),
            });
        }
        let nonce = data[..NONCE_LENGTH].to_vec();
        let tag = data[data.len() - TAG_LENGTH..].to_vec();
        let ciphertext = data[NONCE_LENGTH..data.len() - TAG_LENGTH].to_vec();
        Ok(Self {
            ciphertext,
            nonce,
            tag,
        })
    }
}

/// Encrypt plaintext using AES-256-GCM with a fresh random nonce.
///
/// Returns structured `EncryptedData` with separate ciphertext, nonce, and tag.
pub fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<EncryptedData, SentinalError> {
    validate_key(key)?;

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| SentinalError::KeyDerivationFailed(format!("AES key init: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    getrandom::fill(&mut nonce_bytes).map_err(|e| {
        SentinalError::RandomGenerationFailed(format!("nonce generation: {e}"))
    })?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // aes-gcm appends the tag to the ciphertext.
    let ciphertext_with_tag = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| SentinalError::DecryptionFailed)?;

    // Split: ciphertext is everything except the last 16 bytes (tag).
    let ct_len = ciphertext_with_tag.len() - TAG_LENGTH;
    let ciphertext = ciphertext_with_tag[..ct_len].to_vec();
    let tag = ciphertext_with_tag[ct_len..].to_vec();

    Ok(EncryptedData {
        ciphertext,
        nonce: nonce_bytes.to_vec(),
        tag,
    })
}

/// Decrypt structured `EncryptedData` using AES-256-GCM.
pub fn decrypt(encrypted: &EncryptedData, key: &[u8]) -> Result<Vec<u8>, SentinalError> {
    validate_key(key)?;

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| SentinalError::KeyDerivationFailed(format!("AES key init: {e}")))?;

    let nonce = Nonce::from_slice(&encrypted.nonce);

    // aes-gcm expects ciphertext || tag concatenated.
    let mut payload = Vec::with_capacity(encrypted.ciphertext.len() + encrypted.tag.len());
    payload.extend_from_slice(&encrypted.ciphertext);
    payload.extend_from_slice(&encrypted.tag);

    cipher
        .decrypt(nonce, payload.as_ref())
        .map_err(|_| SentinalError::DecryptionFailed)
}

/// Encrypt plaintext and return combined format: `nonce || ciphertext || tag`.
pub fn encrypt_combined(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, SentinalError> {
    let encrypted = encrypt(plaintext, key)?;
    Ok(encrypted.combined())
}

/// Decrypt from combined format: `nonce (12) || ciphertext (N) || tag (16)`.
pub fn decrypt_combined(combined: &[u8], key: &[u8]) -> Result<Vec<u8>, SentinalError> {
    let encrypted = EncryptedData::from_combined(combined)?;
    decrypt(&encrypted, key)
}

/// Encrypt plaintext using AES-256-GCM with authenticated associated data (AAD).
///
/// AAD is authenticated but **not** encrypted — it must be provided identically
/// during decryption or the tag verification will fail. This is useful for binding
/// ciphertext to contextual metadata (e.g., a relay URL or hop index) without
/// encrypting that metadata.
///
/// Returns combined format: `nonce (12) || ciphertext (N) || tag (16)`.
pub fn encrypt_with_aad(
    plaintext: &[u8],
    aad: &[u8],
    key: &[u8],
) -> Result<Vec<u8>, SentinalError> {
    use aes_gcm::aead::Payload;

    validate_key(key)?;

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| SentinalError::KeyDerivationFailed(format!("AES key init: {e}")))?;

    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    getrandom::fill(&mut nonce_bytes).map_err(|e| {
        SentinalError::RandomGenerationFailed(format!("nonce generation: {e}"))
    })?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let payload = Payload { msg: plaintext, aad };

    let ciphertext_with_tag = cipher
        .encrypt(nonce, payload)
        .map_err(|_| SentinalError::DecryptionFailed)?;

    // Combined format: nonce || ciphertext || tag
    // (aes-gcm appends tag to ciphertext, so we just prepend the nonce)
    let mut combined = Vec::with_capacity(NONCE_LENGTH + ciphertext_with_tag.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext_with_tag);
    Ok(combined)
}

/// Decrypt from combined format with authenticated associated data (AAD).
///
/// The `aad` must exactly match what was provided during encryption, otherwise
/// decryption fails with `DecryptionFailed`.
///
/// Input format: `nonce (12) || ciphertext (N) || tag (16)`.
pub fn decrypt_with_aad(
    combined: &[u8],
    aad: &[u8],
    key: &[u8],
) -> Result<Vec<u8>, SentinalError> {
    use aes_gcm::aead::Payload;

    validate_key(key)?;

    if combined.len() < MIN_COMBINED_LENGTH {
        return Err(SentinalError::InvalidCombinedData {
            expected: MIN_COMBINED_LENGTH,
            actual: combined.len(),
        });
    }

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| SentinalError::KeyDerivationFailed(format!("AES key init: {e}")))?;

    let nonce = Nonce::from_slice(&combined[..NONCE_LENGTH]);
    let ciphertext_with_tag = &combined[NONCE_LENGTH..];

    let payload = Payload {
        msg: ciphertext_with_tag,
        aad,
    };

    cipher
        .decrypt(nonce, payload)
        .map_err(|_| SentinalError::DecryptionFailed)
}

fn validate_key(key: &[u8]) -> Result<(), SentinalError> {
    if key.len() != KEY_LENGTH {
        return Err(SentinalError::InvalidKeyLength {
            expected: KEY_LENGTH,
            actual: key.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        vec![0x42; KEY_LENGTH]
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = test_key();
        let plaintext = b"Doctor appointment Tuesday";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_combined_round_trip() {
        let key = test_key();
        let plaintext = b"Sovereignty is a birthright";
        let combined = encrypt_combined(plaintext, &key).unwrap();
        let decrypted = decrypt_combined(&combined, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn combined_format_structure() {
        let key = test_key();
        let plaintext = b"hello";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let combined = encrypted.combined();
        // nonce (12) + ciphertext (5) + tag (16) = 33
        assert_eq!(combined.len(), NONCE_LENGTH + plaintext.len() + TAG_LENGTH);
        assert_eq!(&combined[..NONCE_LENGTH], &encrypted.nonce);
        assert_eq!(&combined[combined.len() - TAG_LENGTH..], &encrypted.tag);
    }

    #[test]
    fn from_combined_round_trip() {
        let key = test_key();
        let plaintext = b"round trip";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let combined = encrypted.combined();
        let parsed = EncryptedData::from_combined(&combined).unwrap();
        assert_eq!(parsed, encrypted);
    }

    #[test]
    fn from_combined_too_short() {
        let result = EncryptedData::from_combined(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let key1 = vec![0x42; KEY_LENGTH];
        let key2 = vec![0x43; KEY_LENGTH];
        let encrypted = encrypt(b"secret", &key1).unwrap();
        let result = decrypt(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn decrypt_tampered_ciphertext_fails() {
        let key = test_key();
        let mut encrypted = encrypt(b"secret", &key).unwrap();
        if !encrypted.ciphertext.is_empty() {
            encrypted.ciphertext[0] ^= 0xFF;
        }
        let result = decrypt(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_key_length() {
        let short_key = vec![0x42; 16]; // AES-128, not AES-256
        let result = encrypt(b"data", &short_key);
        assert!(result.is_err());
    }

    #[test]
    fn empty_plaintext() {
        let key = test_key();
        let encrypted = encrypt(b"", &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert!(decrypted.is_empty());
    }

    // -- AAD tests --

    #[test]
    fn aad_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = b"relay forwarding payload";
        let aad = b"hop-index:2";
        let combined = encrypt_with_aad(plaintext, aad, &key).unwrap();
        let decrypted = decrypt_with_aad(&combined, aad, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn aad_tampered_aad_fails() {
        let key = test_key();
        let plaintext = b"sensitive data";
        let aad = b"correct-context";
        let combined = encrypt_with_aad(plaintext, aad, &key).unwrap();

        let result = decrypt_with_aad(&combined, b"wrong-context", &key);
        assert!(result.is_err());
    }

    #[test]
    fn aad_empty_aad_roundtrip() {
        let key = test_key();
        let plaintext = b"no associated data";
        let combined = encrypt_with_aad(plaintext, b"", &key).unwrap();
        let decrypted = decrypt_with_aad(&combined, b"", &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn aad_wrong_key_fails() {
        let key1 = vec![0x42; KEY_LENGTH];
        let key2 = vec![0x43; KEY_LENGTH];
        let combined = encrypt_with_aad(b"secret", b"context", &key1).unwrap();
        let result = decrypt_with_aad(&combined, b"context", &key2);
        assert!(result.is_err());
    }

    #[test]
    fn aad_combined_too_short_fails() {
        let key = test_key();
        let result = decrypt_with_aad(&[0u8; 10], b"aad", &key);
        assert!(result.is_err());
    }

    #[test]
    fn aad_empty_plaintext() {
        let key = test_key();
        let combined = encrypt_with_aad(b"", b"some-aad", &key).unwrap();
        let decrypted = decrypt_with_aad(&combined, b"some-aad", &key).unwrap();
        assert!(decrypted.is_empty());
    }
}
