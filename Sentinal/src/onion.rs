//! Onion encryption for relay forwarding.
//!
//! Provides layered encryption where each relay in a forwarding path can
//! only unwrap its own layer, revealing the payload for the next hop (or
//! the final plaintext for the last hop). Uses ephemeral X25519 ECDH +
//! AES-256-GCM per layer.
//!
//! # Wire format per layer
//!
//! ```text
//! [32-byte ephemeral public key][nonce (12) || ciphertext (N) || tag (16)]
//! ```
//!
//! The ephemeral public key lets the relay perform ECDH with its static
//! private key to derive the shared secret, which is then run through
//! HKDF to produce the AES-256-GCM decryption key.
//!
//! # Example
//!
//! ```
//! use sentinal::onion::{wrap_layer, unwrap_layer};
//! use x25519_dalek::{StaticSecret, PublicKey};
//!
//! // Generate relay keypair.
//! let relay_secret = StaticSecret::random();
//! let relay_public = PublicKey::from(&relay_secret);
//!
//! // Wrap a layer of encryption addressed to this relay.
//! let plaintext = b"hello from the onion";
//! let wrapped = wrap_layer(plaintext, relay_public.as_bytes()).unwrap();
//!
//! // The relay unwraps using its private key.
//! let recovered = unwrap_layer(&wrapped, relay_secret.as_bytes()).unwrap();
//! assert_eq!(recovered, plaintext);
//! ```

use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

use crate::encryption::{decrypt_combined, encrypt_combined};
use crate::error::SentinalError;
use crate::key_derivation::derive_shared_key;

/// Size of an X25519 public key in bytes.
const X25519_PUBKEY_LEN: usize = 32;

/// Wrap one layer of onion encryption addressed to `relay_pubkey`.
///
/// Generates an ephemeral X25519 keypair, performs ECDH with the relay's
/// public key, derives an AES-256-GCM key via HKDF, and encrypts the
/// plaintext. The ephemeral public key is prepended to the ciphertext so
/// the relay can derive the same shared secret.
///
/// Returns the encrypted blob: `[ephemeral_pubkey (32) || encrypted_combined]`.
pub fn wrap_layer(plaintext: &[u8], relay_pubkey: &[u8]) -> Result<Vec<u8>, SentinalError> {
    if relay_pubkey.len() != X25519_PUBKEY_LEN {
        return Err(SentinalError::InvalidKeyLength {
            expected: X25519_PUBKEY_LEN,
            actual: relay_pubkey.len(),
        });
    }

    let pubkey_bytes: [u8; 32] = relay_pubkey
        .try_into()
        .map_err(|_| SentinalError::InvalidWrappedKey)?;

    let ephemeral_secret = EphemeralSecret::random();
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    let relay_pk = PublicKey::from(pubkey_bytes);
    let shared_secret = ephemeral_secret.diffie_hellman(&relay_pk);

    let aes_key = derive_shared_key(shared_secret.as_bytes())?;
    let encrypted = encrypt_combined(plaintext, aes_key.expose())?;

    let mut blob = Vec::with_capacity(X25519_PUBKEY_LEN + encrypted.len());
    blob.extend_from_slice(ephemeral_public.as_bytes());
    blob.extend_from_slice(&encrypted);
    Ok(blob)
}

/// Unwrap one layer of onion encryption using the relay's private key.
///
/// Extracts the ephemeral public key from the blob, performs ECDH with
/// `relay_private_key`, derives the AES-256-GCM key via HKDF, and
/// decrypts the payload.
///
/// Returns the inner plaintext (which may be another onion layer or the
/// final message).
pub fn unwrap_layer(blob: &[u8], relay_private_key: &[u8]) -> Result<Vec<u8>, SentinalError> {
    if relay_private_key.len() != X25519_PUBKEY_LEN {
        return Err(SentinalError::InvalidKeyLength {
            expected: X25519_PUBKEY_LEN,
            actual: relay_private_key.len(),
        });
    }

    if blob.len() < X25519_PUBKEY_LEN {
        return Err(SentinalError::InvalidCombinedData {
            expected: X25519_PUBKEY_LEN,
            actual: blob.len(),
        });
    }

    let ephemeral_pubkey_bytes: [u8; 32] = blob[..X25519_PUBKEY_LEN]
        .try_into()
        .map_err(|_| SentinalError::InvalidWrappedKey)?;
    let encrypted = &blob[X25519_PUBKEY_LEN..];

    let sk_bytes: [u8; 32] = relay_private_key
        .try_into()
        .map_err(|_| SentinalError::InvalidWrappedKey)?;

    let secret = StaticSecret::from(sk_bytes);
    let ephemeral_pk = PublicKey::from(ephemeral_pubkey_bytes);
    let shared_secret = secret.diffie_hellman(&ephemeral_pk);

    let aes_key = derive_shared_key(shared_secret.as_bytes())?;
    decrypt_combined(encrypted, aes_key.expose())
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::StaticSecret;

    #[test]
    fn wrap_unwrap_roundtrip() {
        let relay_secret = StaticSecret::random();
        let relay_public = PublicKey::from(&relay_secret);

        let plaintext = b"sovereignty is a birthright";
        let wrapped = wrap_layer(plaintext, relay_public.as_bytes()).unwrap();
        let recovered = unwrap_layer(&wrapped, relay_secret.as_bytes()).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_private_key_fails() {
        let relay_secret = StaticSecret::random();
        let relay_public = PublicKey::from(&relay_secret);

        let wrapped = wrap_layer(b"secret payload", relay_public.as_bytes()).unwrap();

        let wrong_secret = StaticSecret::random();
        let result = unwrap_layer(&wrapped, wrong_secret.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn multiple_layers() {
        // Simulate 3-hop onion: wrap for relay3, then relay2, then relay1.
        let relay1_secret = StaticSecret::random();
        let relay1_public = PublicKey::from(&relay1_secret);
        let relay2_secret = StaticSecret::random();
        let relay2_public = PublicKey::from(&relay2_secret);
        let relay3_secret = StaticSecret::random();
        let relay3_public = PublicKey::from(&relay3_secret);

        let plaintext = b"message for the final destination";

        // Wrap innermost first (relay3), then relay2, then relay1.
        let layer3 = wrap_layer(plaintext, relay3_public.as_bytes()).unwrap();
        let layer2 = wrap_layer(&layer3, relay2_public.as_bytes()).unwrap();
        let layer1 = wrap_layer(&layer2, relay1_public.as_bytes()).unwrap();

        // Each relay peels one layer.
        let after_relay1 = unwrap_layer(&layer1, relay1_secret.as_bytes()).unwrap();
        let after_relay2 = unwrap_layer(&after_relay1, relay2_secret.as_bytes()).unwrap();
        let after_relay3 = unwrap_layer(&after_relay2, relay3_secret.as_bytes()).unwrap();

        assert_eq!(after_relay3, plaintext);
    }

    #[test]
    fn empty_plaintext() {
        let relay_secret = StaticSecret::random();
        let relay_public = PublicKey::from(&relay_secret);

        let wrapped = wrap_layer(b"", relay_public.as_bytes()).unwrap();
        let recovered = unwrap_layer(&wrapped, relay_secret.as_bytes()).unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn invalid_pubkey_length() {
        let result = wrap_layer(b"data", &[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_private_key_length() {
        let result = unwrap_layer(&[0u8; 100], &[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn blob_too_short_fails() {
        let result = unwrap_layer(&[0u8; 10], &[0u8; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn wrapped_output_starts_with_pubkey() {
        let relay_secret = StaticSecret::random();
        let relay_public = PublicKey::from(&relay_secret);

        let wrapped = wrap_layer(b"test", relay_public.as_bytes()).unwrap();

        // First 32 bytes are the ephemeral public key (not the relay's).
        // They should be different from the relay's public key.
        let ephemeral_pk = &wrapped[..32];
        assert_ne!(ephemeral_pk, relay_public.as_bytes());
        // The total size: 32 (ephemeral pk) + 12 (nonce) + 4 (ciphertext) + 16 (tag)
        assert_eq!(wrapped.len(), 32 + 12 + 4 + 16);
    }

    #[test]
    fn each_wrap_produces_different_ciphertext() {
        let relay_secret = StaticSecret::random();
        let relay_public = PublicKey::from(&relay_secret);

        let plaintext = b"same message";
        let wrapped1 = wrap_layer(plaintext, relay_public.as_bytes()).unwrap();
        let wrapped2 = wrap_layer(plaintext, relay_public.as_bytes()).unwrap();

        // Different ephemeral keys and nonces mean different output.
        assert_ne!(wrapped1, wrapped2);

        // But both decrypt to the same plaintext.
        let r1 = unwrap_layer(&wrapped1, relay_secret.as_bytes()).unwrap();
        let r2 = unwrap_layer(&wrapped2, relay_secret.as_bytes()).unwrap();
        assert_eq!(r1, plaintext);
        assert_eq!(r2, plaintext);
    }
}
