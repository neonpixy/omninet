use chrono::{DateTime, Utc};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::CrownError;
use crate::keypair::CrownKeypair;

/// A BIP-340 Schnorr signature with metadata.
///
/// Signatures are 64 bytes (r || s). Data is SHA-256 hashed before
/// signing (required by secp256k1 Message).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Signature {
    /// Raw signature bytes (64 bytes Schnorr), hex-encoded in JSON.
    #[serde(with = "hex_serde")]
    data: [u8; 64],
    /// Signer's crown ID (bech32 public key).
    signer: String,
    /// When the signature was created.
    timestamp: DateTime<Utc>,
}

impl Signature {
    // -- Accessors --

    /// Raw 64-byte signature data.
    pub fn data(&self) -> &[u8; 64] {
        &self.data
    }

    /// The signer's crown ID string.
    pub fn signer(&self) -> &str {
        &self.signer
    }

    /// When this signature was created.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Hex representation of the signature (128 chars).
    pub fn hex(&self) -> String {
        hex::encode(self.data)
    }

    // -- Construction --

    /// Create a signature from raw components.
    pub fn new(data: [u8; 64], signer: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            data,
            signer,
            timestamp,
        }
    }

    // -- Signing --

    /// Sign arbitrary data with a keypair.
    ///
    /// Data is SHA-256 hashed internally to produce the 32-byte message
    /// required by BIP-340 Schnorr.
    pub fn sign(data: &[u8], keypair: &CrownKeypair) -> Result<Self, CrownError> {
        let privkey = keypair
            .private_key_data()
            .ok_or_else(|| CrownError::SignatureFailed("no private key".into()))?;

        let secp = Secp256k1::new();
        let sk = SecretKey::from_slice(privkey)
            .map_err(|e| CrownError::SignatureFailed(format!("invalid key: {e}")))?;
        let kp = Keypair::from_secret_key(&secp, &sk);

        let hash = Sha256::digest(data);
        let msg = Message::from_digest(hash.into());
        let sig = secp.sign_schnorr(&msg, &kp);

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&sig[..]);

        Ok(Self {
            data: sig_bytes,
            signer: keypair.crown_id().to_string(),
            timestamp: Utc::now(),
        })
    }

    // -- Verification --

    /// Verify against raw x-only public key bytes (32 bytes).
    pub fn verify(&self, data: &[u8], public_key_data: &[u8; 32]) -> bool {
        let secp = Secp256k1::verification_only();

        let xonly = match XOnlyPublicKey::from_slice(public_key_data) {
            Ok(pk) => pk,
            Err(_) => return false,
        };

        let sig = match secp256k1::schnorr::Signature::from_slice(&self.data) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let hash = Sha256::digest(data);
        let msg = Message::from_digest(hash.into());

        secp.verify_schnorr(&sig, &msg, &xonly).is_ok()
    }

    /// Verify against a crown ID bech32 string.
    pub fn verify_crown_id(&self, data: &[u8], crown_id: &str) -> bool {
        match CrownKeypair::decode_bech32(crown_id, "cpub") {
            Ok(bytes) => self.verify(data, &bytes),
            Err(_) => false,
        }
    }

    /// Verify using the stored signer crown ID.
    pub fn verify_signer(&self, data: &[u8]) -> bool {
        self.verify_crown_id(data, &self.signer)
    }
}

/// Custom hex serialization for [u8; 64].
mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let arr: [u8; 64] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| {
                serde::de::Error::custom(format!("expected 64 bytes, got {}", v.len()))
            })?;
        Ok(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let kp = CrownKeypair::generate();
        let data = b"sovereign data";

        let sig = Signature::sign(data, &kp).unwrap();
        assert!(sig.verify(data, kp.public_key_data()));
    }

    #[test]
    fn sign_and_verify_with_crown_id() {
        let kp = CrownKeypair::generate();
        let data = b"hello world";

        let sig = Signature::sign(data, &kp).unwrap();
        assert!(sig.verify_crown_id(data, kp.crown_id()));
    }

    #[test]
    fn sign_and_verify_with_signer() {
        let kp = CrownKeypair::generate();
        let data = b"test message";

        let sig = Signature::sign(data, &kp).unwrap();
        assert!(sig.verify_signer(data));
        assert_eq!(sig.signer(), kp.crown_id());
    }

    #[test]
    fn verify_wrong_data_fails() {
        let kp = CrownKeypair::generate();
        let sig = Signature::sign(b"hello", &kp).unwrap();
        assert!(!sig.verify(b"world", kp.public_key_data()));
    }

    #[test]
    fn verify_wrong_key_fails() {
        let kp1 = CrownKeypair::generate();
        let kp2 = CrownKeypair::generate();

        let sig = Signature::sign(b"data", &kp1).unwrap();
        assert!(!sig.verify(b"data", kp2.public_key_data()));
    }

    #[test]
    fn sign_without_private_key_fails() {
        let kp = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(kp.crown_id()).unwrap();

        let result = Signature::sign(b"data", &pubonly);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CrownError::SignatureFailed(_)
        ));
    }

    #[test]
    fn signature_serde_round_trip() {
        let kp = CrownKeypair::generate();
        let sig = Signature::sign(b"serialize me", &kp).unwrap();

        let json = serde_json::to_string(&sig).unwrap();
        let loaded: Signature = serde_json::from_str(&json).unwrap();
        assert_eq!(sig, loaded);
    }

    #[test]
    fn signature_hex_format() {
        let kp = CrownKeypair::generate();
        let sig = Signature::sign(b"hex test", &kp).unwrap();
        let h = sig.hex();
        assert_eq!(h.len(), 128);
        // All lowercase hex.
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
