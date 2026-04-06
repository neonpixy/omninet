use std::fmt;
use std::hash::{Hash, Hasher};

use bech32::{Bech32, Hrp};
use secp256k1::{Keypair, PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize;

use crate::error::CrownError;

/// A Crown identity keypair (secp256k1 BIP-340 Schnorr).
///
/// Two modes:
/// - **Full keypair**: has both public and private key (can sign)
/// - **Public-only**: has only the public key (can verify)
///
/// No `Serialize`/`Deserialize` — private keys must not be accidentally
/// serialized. Use [`Keyring`](crate::keyring::Keyring) for explicit export/import.
#[derive(Clone)]
pub struct CrownKeypair {
    /// Public key in bech32 format (e.g., "cpub1...").
    crown_id: String,
    /// Raw public key bytes (32 bytes, x-only BIP-340).
    public_key_data: [u8; 32],
    /// Raw private key bytes (32 bytes). None for public-only.
    private_key_data: Option<[u8; 32]>,
    /// Private key in bech32 format (e.g., "csec1..."). None for public-only.
    crown_secret: Option<String>,
}

impl CrownKeypair {
    // -- Accessors --

    /// The bech32-encoded public key (e.g., "cpub1abc...").
    pub fn crown_id(&self) -> &str {
        &self.crown_id
    }

    /// The bech32-encoded private key, if available.
    pub fn crown_secret(&self) -> Option<&str> {
        self.crown_secret.as_deref()
    }

    /// Raw x-only public key bytes (32 bytes).
    pub fn public_key_data(&self) -> &[u8; 32] {
        &self.public_key_data
    }

    /// Raw private key bytes (32 bytes), if available.
    pub fn private_key_data(&self) -> Option<&[u8; 32]> {
        self.private_key_data.as_ref()
    }

    /// Whether this keypair has a private key (can sign).
    pub fn has_private_key(&self) -> bool {
        self.private_key_data.is_some()
    }

    /// Shortened crown ID for display: "cpub1abcd...wxyz".
    pub fn short_id(&self) -> String {
        let s = &self.crown_id;
        if s.len() > 20 {
            format!("{}...{}", &s[..10], &s[s.len() - 4..])
        } else {
            s.to_string()
        }
    }

    /// Public key as 64-character hex string.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_data)
    }

    // -- Factory methods --

    /// Generate a new random keypair.
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (sk, _pk) = secp.generate_keypair(&mut rand::thread_rng());
        Self::from_secret_key(sk)
    }

    /// Import a keypair from a csec bech32 string.
    pub fn from_crown_secret(csec: &str) -> Result<Self, CrownError> {
        let bytes = Self::decode_bech32(csec, "csec")?;
        let sk = SecretKey::from_slice(&bytes).map_err(|e| CrownError::InvalidCrownSecret {
            reason: format!("invalid secp256k1 key: {e}"),
        })?;
        Ok(Self::from_secret_key(sk))
    }

    /// Import a keypair from raw 32-byte private key.
    pub fn from_private_key(bytes: &[u8]) -> Result<Self, CrownError> {
        if bytes.len() != 32 {
            return Err(CrownError::InvalidPrivateKey {
                reason: format!("expected 32 bytes, got {}", bytes.len()),
            });
        }
        let sk = SecretKey::from_slice(bytes).map_err(|e| CrownError::InvalidPrivateKey {
            reason: format!("invalid secp256k1 key: {e}"),
        })?;
        Ok(Self::from_secret_key(sk))
    }

    /// Create a public-key-only keypair from a cpub string.
    /// Cannot sign, only verify.
    pub fn from_crown_id(cpub: &str) -> Result<Self, CrownError> {
        let bytes = Self::decode_bech32(cpub, "cpub")?;
        // Validate it's a valid x-only public key.
        XOnlyPublicKey::from_slice(&bytes).map_err(|e| CrownError::InvalidCrownId {
            reason: format!("invalid x-only public key: {e}"),
        })?;
        let crown_id_str = Self::encode_public(&bytes);

        Ok(Self {
            crown_id: crown_id_str,
            public_key_data: bytes,
            private_key_data: None,
            crown_secret: None,
        })
    }

    // -- ECDH --

    /// Compute an ECDH shared secret with another party's x-only public key.
    ///
    /// Both sides independently arrive at the same 32-byte shared secret:
    /// `shared_secret(alice_priv, bob_pub) == shared_secret(bob_priv, alice_pub)`
    ///
    /// Used by Lingo to derive shared Babel vocabularies between two users
    /// without any explicit key exchange.
    pub fn shared_secret(
        &self,
        their_public_key: &[u8; 32],
    ) -> Result<[u8; 32], CrownError> {
        let sk_bytes = self.private_key_data.ok_or(CrownError::Locked)?;
        // Reconstruct a full compressed public key from x-only (assume even Y).
        let mut compressed = [0u8; 33];
        compressed[0] = 0x02; // even Y coordinate
        compressed[1..].copy_from_slice(their_public_key);

        let their_pk = PublicKey::from_slice(&compressed).map_err(|e| {
            CrownError::InvalidCrownId {
                reason: format!("invalid public key for ECDH: {e}"),
            }
        })?;

        // Manual ECDH: multiply their public key by our secret key.
        // Then extract only the X coordinate of the result point.
        // This makes the shared secret independent of Y parity,
        // which is critical for x-only (BIP-340) public keys that
        // lose parity information.
        let scalar = Scalar::from_be_bytes(sk_bytes).map_err(|_| {
            CrownError::SignatureFailed("invalid scalar for ECDH".into())
        })?;

        let secp = Secp256k1::new();
        let result_point = their_pk.mul_tweak(&secp, &scalar).map_err(|e| {
            CrownError::SignatureFailed(format!("ECDH multiplication failed: {e}"))
        })?;

        // Extract x-only coordinate (parity-independent).
        let (x_only, _parity) = result_point.x_only_public_key();
        let x_bytes = x_only.serialize();

        // Hash for domain separation.
        let hash: [u8; 32] = Sha256::digest(x_bytes).into();
        Ok(hash)
    }

    // -- Internal --

    fn from_secret_key(sk: SecretKey) -> Self {
        let secp = Secp256k1::new();
        let kp = Keypair::from_secret_key(&secp, &sk);
        let (xonly, _parity) = kp.x_only_public_key();
        let pubkey_bytes = xonly.serialize();
        let privkey_bytes = sk.secret_bytes();

        Self {
            crown_id: Self::encode_public(&pubkey_bytes),
            public_key_data: pubkey_bytes,
            private_key_data: Some(privkey_bytes),
            crown_secret: Some(Self::encode_secret(&privkey_bytes)),
        }
    }

    pub(crate) fn encode_public(pubkey: &[u8; 32]) -> String {
        let hrp = Hrp::parse("cpub").expect("valid hrp");
        bech32::encode::<Bech32>(hrp, pubkey).expect("encoding cannot fail for 32 bytes")
    }

    fn encode_secret(privkey: &[u8; 32]) -> String {
        let hrp = Hrp::parse("csec").expect("valid hrp");
        bech32::encode::<Bech32>(hrp, privkey).expect("encoding cannot fail for 32 bytes")
    }

    pub(crate) fn decode_bech32(input: &str, expected_hrp: &str) -> Result<[u8; 32], CrownError> {
        let (hrp, data) = bech32::decode(input).map_err(|e| {
            if expected_hrp == "csec" {
                CrownError::InvalidCrownSecret {
                    reason: format!("bech32 decode failed: {e}"),
                }
            } else {
                CrownError::InvalidCrownId {
                    reason: format!("bech32 decode failed: {e}"),
                }
            }
        })?;

        if hrp.as_str() != expected_hrp {
            let reason = format!("expected '{expected_hrp}' prefix, got '{}'", hrp.as_str());
            return if expected_hrp == "csec" {
                Err(CrownError::InvalidCrownSecret { reason })
            } else {
                Err(CrownError::InvalidCrownId { reason })
            };
        }

        let bytes: [u8; 32] = data.try_into().map_err(|v: Vec<u8>| {
            let reason = format!("expected 32 bytes, got {}", v.len());
            if expected_hrp == "csec" {
                CrownError::InvalidCrownSecret { reason }
            } else {
                CrownError::InvalidCrownId { reason }
            }
        })?;

        Ok(bytes)
    }
}

impl PartialEq for CrownKeypair {
    fn eq(&self, other: &Self) -> bool {
        self.crown_id == other.crown_id
    }
}

impl Eq for CrownKeypair {}

impl Hash for CrownKeypair {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.crown_id.hash(state);
    }
}

impl fmt::Debug for CrownKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrownKeypair")
            .field("crown_id", &self.crown_id)
            .field("has_private_key", &self.has_private_key())
            .finish()
    }
}

impl Drop for CrownKeypair {
    fn drop(&mut self) {
        self.public_key_data.zeroize();
        if let Some(ref mut key) = self.private_key_data {
            key.zeroize();
        }
        if let Some(ref mut secret) = self.crown_secret {
            secret.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_valid_keypair() {
        let kp = CrownKeypair::generate();
        assert!(kp.crown_id().starts_with("cpub1"));
        assert!(kp.crown_secret().unwrap().starts_with("csec1"));
        assert!(kp.has_private_key());
        assert_eq!(kp.public_key_data().len(), 32);
        assert_eq!(kp.private_key_data().unwrap().len(), 32);
    }

    #[test]
    fn generate_produces_unique_keys() {
        let kp1 = CrownKeypair::generate();
        let kp2 = CrownKeypair::generate();
        assert_ne!(kp1.crown_id(), kp2.crown_id());
    }

    #[test]
    fn from_crown_secret_round_trip() {
        let original = CrownKeypair::generate();
        let csec = original.crown_secret().unwrap().to_string();

        let restored = CrownKeypair::from_crown_secret(&csec).unwrap();
        assert_eq!(original.crown_id(), restored.crown_id());
        assert_eq!(
            original.private_key_data().unwrap(),
            restored.private_key_data().unwrap()
        );
    }

    #[test]
    fn from_crown_secret_invalid_format() {
        let result = CrownKeypair::from_crown_secret("garbage");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrownError::InvalidCrownSecret { .. }));
    }

    #[test]
    fn from_crown_secret_wrong_hrp() {
        let kp = CrownKeypair::generate();
        let cpub = kp.crown_id().to_string();
        // Pass a cpub where csec is expected.
        let result = CrownKeypair::from_crown_secret(&cpub);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CrownError::InvalidCrownSecret { .. }));
    }

    #[test]
    fn from_private_key_valid() {
        let original = CrownKeypair::generate();
        let privkey = *original.private_key_data().unwrap();

        let restored = CrownKeypair::from_private_key(&privkey).unwrap();
        assert_eq!(original.crown_id(), restored.crown_id());
    }

    #[test]
    fn from_private_key_wrong_length() {
        let result = CrownKeypair::from_private_key(&[0u8; 16]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CrownError::InvalidPrivateKey { .. }
        ));
    }

    #[test]
    fn from_crown_id_creates_public_only() {
        let full = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(full.crown_id()).unwrap();

        assert!(!pubonly.has_private_key());
        assert!(pubonly.crown_secret().is_none());
        assert_eq!(pubonly.public_key_data(), full.public_key_data());
    }

    #[test]
    fn from_crown_id_invalid() {
        let result = CrownKeypair::from_crown_id("garbage");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrownError::InvalidCrownId { .. }));
    }

    #[test]
    fn short_id_format() {
        let kp = CrownKeypair::generate();
        let short = kp.short_id();
        assert!(short.starts_with("cpub1"));
        assert!(short.contains("..."));
        assert!(short.len() < kp.crown_id().len());
    }

    #[test]
    fn equality_based_on_crown_id() {
        let kp = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(kp.crown_id()).unwrap();
        // Full keypair and public-only from same key are equal.
        assert_eq!(kp, pubonly);
    }

    #[test]
    fn ecdh_shared_secret_symmetric() {
        let alice = CrownKeypair::generate();
        let bob = CrownKeypair::generate();

        let secret_ab = alice.shared_secret(bob.public_key_data()).unwrap();
        let secret_ba = bob.shared_secret(alice.public_key_data()).unwrap();

        // Both sides derive the same shared secret.
        assert_eq!(secret_ab, secret_ba);
        // Secret is not all zeros.
        assert!(secret_ab.iter().any(|&b| b != 0));
    }

    #[test]
    fn ecdh_different_pairs_different_secrets() {
        let alice = CrownKeypair::generate();
        let bob = CrownKeypair::generate();
        let carol = CrownKeypair::generate();

        let ab = alice.shared_secret(bob.public_key_data()).unwrap();
        let ac = alice.shared_secret(carol.public_key_data()).unwrap();

        assert_ne!(ab, ac);
    }

    #[test]
    fn ecdh_requires_private_key() {
        let alice = CrownKeypair::generate();
        let bob_pub = CrownKeypair::from_crown_id(alice.crown_id()).unwrap();

        let result = bob_pub.shared_secret(alice.public_key_data());
        assert!(result.is_err());
    }

    #[test]
    fn debug_redacts_private_key() {
        let kp = CrownKeypair::generate();
        let debug = format!("{kp:?}");
        assert!(debug.contains("cpub1"));
        // Should NOT contain the csec or raw private bytes.
        assert!(!debug.contains("csec1"));
        assert!(debug.contains("has_private_key: true"));
    }
}
