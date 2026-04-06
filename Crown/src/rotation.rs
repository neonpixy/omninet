//! Key rotation — chain of custody for identity keys.
//!
//! When a user rotates their primary key, the OLD key signs an announcement
//! proving the holder authorized the transition. The rotation chain records
//! all previous keys for verification.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CrownError;
use crate::signature::Signature;

/// Record of a primary key that has been rotated out.
///
/// Stored in the [`RotationChain`] as proof that this key once existed
/// and authorized the transition to a new key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviousKey {
    /// Hex-encoded x-only public key (64 chars).
    pub public_key_hex: String,
    /// Bech32-encoded public key (cpub1...).
    pub crown_id: String,
    /// When this key was rotated out.
    pub rotated_at: DateTime<Utc>,
    /// BIP-340 Schnorr signature of the rotation announcement by the old key.
    pub rotation_signature: Vec<u8>,
}

/// Announcement that an identity has rotated its primary key.
///
/// Signed by the OLD key to prove chain of custody -- the previous key
/// holder explicitly authorized the transition to the new key. Publish
/// this to the network so verifiers can follow the identity across rotations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RotationAnnouncement {
    /// Old public key as 64-char hex.
    pub old_pubkey_hex: String,
    /// New public key as 64-char hex.
    pub new_pubkey_hex: String,
    /// Old public key as bech32 crown ID.
    pub old_crown_id: String,
    /// New public key as bech32 crown ID.
    pub new_crown_id: String,
    /// BIP-340 Schnorr signature by the OLD key over the announcement data.
    pub signature: Vec<u8>,
    /// When the rotation occurred.
    pub timestamp: DateTime<Utc>,
    /// Optional human-readable reason for the rotation.
    pub reason: Option<String>,
}

impl RotationAnnouncement {
    /// Produce deterministic bytes for signing/verification.
    ///
    /// Concatenates old_pubkey_hex + new_pubkey_hex + RFC 3339 timestamp.
    pub fn to_signable_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.old_pubkey_hex.as_bytes());
        bytes.extend_from_slice(self.new_pubkey_hex.as_bytes());
        bytes.extend_from_slice(self.timestamp.to_rfc3339().as_bytes());
        bytes
    }

    /// Self-verify: check that the signature was made by the old key.
    pub fn verify(&self) -> Result<bool, CrownError> {
        let pubkey_bytes = hex::decode(&self.old_pubkey_hex).map_err(|e| {
            CrownError::RotationFailed(format!("invalid old pubkey hex: {e}"))
        })?;
        let pubkey: [u8; 32] = pubkey_bytes.try_into().map_err(|v: Vec<u8>| {
            CrownError::RotationFailed(format!(
                "old pubkey wrong length: expected 32, got {}",
                v.len()
            ))
        })?;

        let sig_bytes: [u8; 64] = self.signature.clone().try_into().map_err(|v: Vec<u8>| {
            CrownError::RotationFailed(format!(
                "signature wrong length: expected 64, got {}",
                v.len()
            ))
        })?;

        let signable = self.to_signable_bytes();
        let sig = Signature::new(sig_bytes, self.old_crown_id.clone(), self.timestamp);
        Ok(sig.verify(&signable, &pubkey))
    }
}

/// Chain of all previous primary keys, ordered oldest to newest.
///
/// Each entry records a key that was rotated out and the signature
/// proving the transition was authorized. Use [`verify_chain`](Self::verify_chain)
/// to validate that every link in the chain is cryptographically sound.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RotationChain {
    /// Previous keys in chronological order (oldest first).
    pub previous_keys: Vec<PreviousKey>,
}

impl RotationChain {
    /// Append a previous key record to the chain.
    pub fn push(&mut self, key: PreviousKey) {
        self.previous_keys.push(key);
    }

    /// The most recent rotation, if any.
    pub fn latest_rotation(&self) -> Option<&PreviousKey> {
        self.previous_keys.last()
    }

    /// Verify every rotation signature in the chain.
    ///
    /// Each PreviousKey's `rotation_signature` must be a valid BIP-340
    /// Schnorr signature by that key over the transition data.
    /// For a chain A -> B -> C, we verify A signed the A->B transition
    /// and B signed the B->C transition.
    ///
    /// A chain with 0 or 1 entries is trivially valid.
    pub fn verify_chain(&self) -> bool {
        if self.previous_keys.len() < 2 {
            return true;
        }

        // Each pair (i, i+1): key[i] signed the transition to key[i+1].
        // The signature in key[i] proves key[i] authorized the rotation.
        // We reconstruct the signable data and verify.
        for i in 0..self.previous_keys.len() - 1 {
            let old_key = &self.previous_keys[i];
            let new_key = &self.previous_keys[i + 1];

            // Reconstruct signable bytes: old_pubkey_hex + new_pubkey_hex + timestamp
            let mut signable = Vec::new();
            signable.extend_from_slice(old_key.public_key_hex.as_bytes());
            signable.extend_from_slice(new_key.public_key_hex.as_bytes());
            signable.extend_from_slice(old_key.rotated_at.to_rfc3339().as_bytes());

            // Parse the old key's public key bytes
            let pubkey_bytes = match hex::decode(&old_key.public_key_hex) {
                Ok(b) => b,
                Err(_) => return false,
            };
            let pubkey: [u8; 32] = match pubkey_bytes.try_into() {
                Ok(b) => b,
                Err(_) => return false,
            };

            // Parse the signature
            let sig_bytes: [u8; 64] = match old_key.rotation_signature.clone().try_into() {
                Ok(b) => b,
                Err(_) => return false,
            };

            let sig = Signature::new(sig_bytes, old_key.crown_id.clone(), old_key.rotated_at);
            if !sig.verify(&signable, &pubkey) {
                return false;
            }
        }

        true
    }

    /// Number of previous keys in the chain.
    pub fn len(&self) -> usize {
        self.previous_keys.len()
    }

    /// Whether the chain is empty (no rotations have occurred).
    pub fn is_empty(&self) -> bool {
        self.previous_keys.is_empty()
    }
}

/// Verify a rotation announcement against a known old public key.
///
/// Checks that the signature in the announcement was made by the given
/// old public key hex.
pub fn verify_rotation(
    old_pubkey_hex: &str,
    announcement: &RotationAnnouncement,
) -> Result<bool, CrownError> {
    if announcement.old_pubkey_hex != old_pubkey_hex {
        return Ok(false);
    }
    announcement.verify()
}

/// Build the signable bytes for a rotation announcement.
///
/// This is the canonical format: old_pubkey_hex + new_pubkey_hex + timestamp (RFC 3339).
/// Used internally by `Keyring::rotate_primary()` before the announcement struct exists.
pub(crate) fn build_signable_bytes(
    old_pubkey_hex: &str,
    new_pubkey_hex: &str,
    timestamp: &DateTime<Utc>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(old_pubkey_hex.as_bytes());
    bytes.extend_from_slice(new_pubkey_hex.as_bytes());
    bytes.extend_from_slice(timestamp.to_rfc3339().as_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keypair::CrownKeypair;
    use crate::keyring::Keyring;

    #[test]
    fn rotation_chain_push_and_len() {
        let mut chain = RotationChain::default();
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);

        chain.push(PreviousKey {
            public_key_hex: "aa".repeat(32),
            crown_id: "cpub1test".to_string(),
            rotated_at: Utc::now(),
            rotation_signature: vec![0u8; 64],
        });

        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn rotation_chain_latest() {
        let mut chain = RotationChain::default();
        assert!(chain.latest_rotation().is_none());

        chain.push(PreviousKey {
            public_key_hex: "first".to_string(),
            crown_id: "cpub1first".to_string(),
            rotated_at: Utc::now(),
            rotation_signature: vec![0u8; 64],
        });
        chain.push(PreviousKey {
            public_key_hex: "second".to_string(),
            crown_id: "cpub1second".to_string(),
            rotated_at: Utc::now(),
            rotation_signature: vec![0u8; 64],
        });

        assert_eq!(
            chain.latest_rotation().unwrap().public_key_hex,
            "second"
        );
    }

    #[test]
    fn empty_chain_verifies() {
        let chain = RotationChain::default();
        assert!(chain.verify_chain());
    }

    #[test]
    fn single_entry_chain_verifies() {
        let mut chain = RotationChain::default();
        chain.push(PreviousKey {
            public_key_hex: "aa".repeat(32),
            crown_id: "cpub1test".to_string(),
            rotated_at: Utc::now(),
            rotation_signature: vec![0u8; 64],
        });
        assert!(chain.verify_chain());
    }

    #[test]
    fn rotate_primary_produces_valid_announcement() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let old_cpub = kr.public_key().unwrap().to_string();
        let old_hex = kr.public_key_hex().unwrap();

        let announcement = kr.rotate_primary().unwrap();

        // The announcement references the old key
        assert_eq!(announcement.old_pubkey_hex, old_hex);
        assert_eq!(announcement.old_crown_id, old_cpub);

        // The keyring now has a different primary
        let new_cpub = kr.public_key().unwrap().to_string();
        assert_ne!(new_cpub, old_cpub);
        assert_eq!(announcement.new_crown_id, new_cpub);
        assert_eq!(announcement.new_pubkey_hex, kr.public_key_hex().unwrap());

        // The announcement self-verifies
        assert!(announcement.verify().unwrap());
    }

    #[test]
    fn rotate_primary_builds_chain() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let first_cpub = kr.public_key().unwrap().to_string();

        kr.rotate_primary().unwrap();
        assert_eq!(kr.rotation_chain().len(), 1);
        assert_eq!(
            kr.rotation_chain().latest_rotation().unwrap().crown_id,
            first_cpub
        );

        let second_cpub = kr.public_key().unwrap().to_string();
        kr.rotate_primary().unwrap();
        assert_eq!(kr.rotation_chain().len(), 2);
        assert_eq!(
            kr.rotation_chain().latest_rotation().unwrap().crown_id,
            second_cpub
        );
    }

    #[test]
    fn rotate_primary_without_primary_fails() {
        let mut kr = Keyring::new();
        let result = kr.rotate_primary();
        assert!(matches!(result.unwrap_err(), CrownError::NoPrimaryKey));
    }

    #[test]
    fn verify_rotation_valid() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let old_hex = kr.public_key_hex().unwrap();

        let announcement = kr.rotate_primary().unwrap();
        assert!(verify_rotation(&old_hex, &announcement).unwrap());
    }

    #[test]
    fn verify_rotation_wrong_key() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let announcement = kr.rotate_primary().unwrap();

        // Verify with a completely different key
        let other = CrownKeypair::generate();
        assert!(!verify_rotation(&other.public_key_hex(), &announcement).unwrap());
    }

    #[test]
    fn verify_rotation_tampered_announcement() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let old_hex = kr.public_key_hex().unwrap();

        let mut announcement = kr.rotate_primary().unwrap();
        // Tamper with the new pubkey
        announcement.new_pubkey_hex = "ff".repeat(32);

        // Self-verify should fail because signable bytes changed
        assert!(!announcement.verify().unwrap());
        assert!(!verify_rotation(&old_hex, &announcement).unwrap());
    }

    #[test]
    fn rotation_chain_verify_multi_step() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        // Rotate 3 times to build a chain of 3 previous keys
        kr.rotate_primary().unwrap();
        kr.rotate_primary().unwrap();
        kr.rotate_primary().unwrap();

        assert_eq!(kr.rotation_chain().len(), 3);
        assert!(kr.rotation_chain().verify_chain());
    }

    #[test]
    fn rotation_chain_verify_tampered_fails() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        kr.rotate_primary().unwrap();
        kr.rotate_primary().unwrap();

        // Tamper with a signature in the chain
        let chain = kr.rotation_chain_mut();
        chain.previous_keys[0].rotation_signature = vec![0u8; 64];

        assert!(!kr.rotation_chain().verify_chain());
    }

    #[test]
    fn rotation_announcement_to_signable_bytes_deterministic() {
        let announcement = RotationAnnouncement {
            old_pubkey_hex: "aa".repeat(32),
            new_pubkey_hex: "bb".repeat(32),
            old_crown_id: "cpub1old".to_string(),
            new_crown_id: "cpub1new".to_string(),
            signature: vec![0u8; 64],
            timestamp: chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00+00:00")
                .unwrap()
                .with_timezone(&Utc),
            reason: None,
        };

        let bytes1 = announcement.to_signable_bytes();
        let bytes2 = announcement.to_signable_bytes();
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn export_load_preserves_rotation_chain() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        kr.rotate_primary().unwrap();
        kr.rotate_primary().unwrap();
        assert_eq!(kr.rotation_chain().len(), 2);

        let exported = kr.export().unwrap();

        let mut kr2 = Keyring::new();
        kr2.load(&exported).unwrap();

        assert_eq!(kr2.rotation_chain().len(), 2);
        assert_eq!(
            kr.rotation_chain().previous_keys[0].public_key_hex,
            kr2.rotation_chain().previous_keys[0].public_key_hex
        );
        assert_eq!(
            kr.rotation_chain().previous_keys[1].public_key_hex,
            kr2.rotation_chain().previous_keys[1].public_key_hex
        );
    }

    #[test]
    fn lock_clears_rotation_chain() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        kr.rotate_primary().unwrap();
        assert_eq!(kr.rotation_chain().len(), 1);

        kr.lock();
        assert!(kr.rotation_chain().is_empty());
    }
}
