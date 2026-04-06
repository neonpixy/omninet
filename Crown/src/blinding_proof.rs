//! Blinding proofs — selective disclosure of blinded key ownership.
//!
//! A [`BlindingProof`] lets a master identity prove that a blinded public key
//! belongs to it, without revealing the HKDF derivation path. The master key
//! signs a deterministic message binding the blinded key to the context, and
//! any verifier with the master's public key can check the claim.
//!
//! This completes the P2B workstream: P2A gave us blinding (derive context-
//! specific keys), and P2B gives us selective disclosure (prove ownership
//! when you choose to).
//!
//! # Example
//!
//! ```
//! use crown::{CrownKeypair, BlindingContext, BlindingProof};
//! use crown::blinding::derive_blinded_keypair;
//! use crown::blinding_proof::create_blinding_proof;
//!
//! let master = CrownKeypair::generate();
//! let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
//! let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
//!
//! let proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();
//! assert!(proof.verify().unwrap());
//! ```

use chrono::{DateTime, Utc};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::blinding::BlindingContext;
use crate::error::CrownError;
use crate::keypair::CrownKeypair;

/// Proof that a blinded public key belongs to a master identity.
///
/// The master key produces a BIP-340 Schnorr signature over a deterministic
/// message that binds `blinded_crown_id` to the `(context_id, context_version)`
/// pair. A verifier reconstructs the message from the proof fields and checks
/// the signature against `master_pubkey_hex`.
///
/// The proof reveals the master identity — that is the point of selective
/// disclosure. The holder chooses when (and to whom) to reveal the link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindingProof {
    /// Master public key (64-char hex, x-only).
    master_pubkey_hex: String,
    /// Blinded public key (bech32 crown ID).
    blinded_crown_id: String,
    /// Context scope that produced the blinded key.
    context_id: String,
    /// Context version that produced the blinded key.
    context_version: u8,
    /// BIP-340 Schnorr signature by the master key over the binding message.
    #[serde(with = "hex_serde_64")]
    proof_signature: [u8; 64],
    /// When the proof was created.
    created_at: DateTime<Utc>,
}

impl BlindingProof {
    /// The master public key (hex).
    pub fn master_pubkey_hex(&self) -> &str {
        &self.master_pubkey_hex
    }

    /// The blinded public key (bech32 crown ID).
    pub fn blinded_crown_id(&self) -> &str {
        &self.blinded_crown_id
    }

    /// The context scope identifier.
    pub fn context_id(&self) -> &str {
        &self.context_id
    }

    /// The context version.
    pub fn context_version(&self) -> u8 {
        self.context_version
    }

    /// The raw 64-byte proof signature.
    pub fn proof_signature(&self) -> &[u8; 64] {
        &self.proof_signature
    }

    /// When this proof was created.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Verify that this proof is valid.
    ///
    /// Reconstructs the signable message from the proof fields and verifies
    /// the BIP-340 Schnorr signature against `master_pubkey_hex`.
    ///
    /// Returns `Ok(true)` if the signature is valid, `Ok(false)` if it is not.
    ///
    /// # Errors
    ///
    /// Returns [`CrownError::BlindingProofFailed`] if the master public key
    /// cannot be parsed (invalid hex or not a valid x-only point).
    #[must_use = "check the verification result"]
    pub fn verify(&self) -> Result<bool, CrownError> {
        let master_bytes = hex::decode(&self.master_pubkey_hex).map_err(|e| {
            CrownError::BlindingProofFailed(format!("invalid master_pubkey_hex hex: {e}"))
        })?;

        let master_key: [u8; 32] = master_bytes.try_into().map_err(|v: Vec<u8>| {
            CrownError::BlindingProofFailed(format!(
                "master_pubkey_hex must be 32 bytes, got {}",
                v.len()
            ))
        })?;

        let secp = Secp256k1::verification_only();

        let xonly = XOnlyPublicKey::from_slice(&master_key).map_err(|e| {
            CrownError::BlindingProofFailed(format!("invalid x-only public key: {e}"))
        })?;

        let sig = match secp256k1::schnorr::Signature::from_slice(&self.proof_signature) {
            Ok(s) => s,
            Err(_) => return Ok(false),
        };

        let message = signable_bytes(
            &self.blinded_crown_id,
            &self.context_id,
            self.context_version,
            &self.master_pubkey_hex,
        );
        let hash = Sha256::digest(&message);
        let msg = Message::from_digest(hash.into());

        Ok(secp.verify_schnorr(&sig, &msg, &xonly).is_ok())
    }
}

/// Create a blinding proof: the master key signs a binding message proving
/// ownership of the blinded key within a specific context.
///
/// The signed message is: `blinded_crown_id || context_id || version || master_pubkey_hex`
/// (all concatenated as UTF-8 bytes with the version as a raw byte).
///
/// # Errors
///
/// - [`CrownError::BlindingProofFailed`] if the master keypair has no private key.
/// - [`CrownError::BlindingProofFailed`] if signing fails.
#[must_use = "returns the blinding proof"]
pub fn create_blinding_proof(
    master: &CrownKeypair,
    blinded_crown_id: &str,
    context: &BlindingContext,
) -> Result<BlindingProof, CrownError> {
    let privkey = master.private_key_data().ok_or_else(|| {
        CrownError::BlindingProofFailed("master keypair has no private key".into())
    })?;

    let master_hex = master.public_key_hex();

    let message = signable_bytes(
        blinded_crown_id,
        context.context_id(),
        context.version(),
        &master_hex,
    );

    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(privkey).map_err(|e| {
        CrownError::BlindingProofFailed(format!("invalid master secret key: {e}"))
    })?;
    let kp = Keypair::from_secret_key(&secp, &sk);

    let hash = Sha256::digest(&message);
    let msg = Message::from_digest(hash.into());
    let sig = secp.sign_schnorr(&msg, &kp);

    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&sig[..]);

    Ok(BlindingProof {
        master_pubkey_hex: master_hex,
        blinded_crown_id: blinded_crown_id.to_string(),
        context_id: context.context_id().to_string(),
        context_version: context.version(),
        proof_signature: sig_bytes,
        created_at: Utc::now(),
    })
}

/// Build the deterministic message that gets signed/verified.
///
/// Format: `blinded_crown_id || context_id || version_byte || master_pubkey_hex`
///
/// All string components are UTF-8 bytes. The version is a single raw byte
/// (not ASCII), making the encoding unambiguous.
fn signable_bytes(
    blinded_crown_id: &str,
    context_id: &str,
    context_version: u8,
    master_pubkey_hex_hex: &str,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(
        blinded_crown_id.len() + context_id.len() + 1 + master_pubkey_hex_hex.len(),
    );
    buf.extend_from_slice(blinded_crown_id.as_bytes());
    buf.extend_from_slice(context_id.as_bytes());
    buf.push(context_version);
    buf.extend_from_slice(master_pubkey_hex_hex.as_bytes());
    buf
}

/// Custom hex serialization for `[u8; 64]`.
///
/// Mirrors the pattern in `signature.rs` — stores the 64-byte array as a
/// 128-character lowercase hex string in JSON.
mod hex_serde_64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        let arr: [u8; 64] = bytes.try_into().map_err(|v: Vec<u8>| {
            serde::de::Error::custom(format!("expected 64 bytes, got {}", v.len()))
        })?;
        Ok(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blinding::derive_blinded_keypair;

    /// Helper: generate a master keypair, blinded keypair, and context.
    fn test_fixtures() -> (CrownKeypair, CrownKeypair, BlindingContext) {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
        let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
        (master, blinded, ctx)
    }

    // -- Creation --

    #[test]
    fn proof_creation_succeeds_for_valid_inputs() {
        let (master, blinded, ctx) = test_fixtures();
        let proof = create_blinding_proof(&master, blinded.crown_id(), &ctx);
        assert!(proof.is_ok());

        let proof = proof.unwrap();
        assert_eq!(proof.master_pubkey_hex(), master.public_key_hex());
        assert_eq!(proof.blinded_crown_id(), blinded.crown_id());
        assert_eq!(proof.context_id(), ctx.context_id());
        assert_eq!(proof.context_version(), ctx.version());
    }

    // -- Verification --

    #[test]
    fn proof_verification_succeeds_for_valid_proof() {
        let (master, blinded, ctx) = test_fixtures();
        let proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();
        assert!(proof.verify().unwrap());
    }

    #[test]
    fn tampered_blinded_crown_id_fails_verification() {
        let (master, blinded, ctx) = test_fixtures();
        let mut proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();

        // Swap in a different blinded crown ID.
        let other = CrownKeypair::generate();
        proof.blinded_crown_id = other.crown_id().to_string();

        assert!(!proof.verify().unwrap());
    }

    #[test]
    fn tampered_context_id_fails_verification() {
        let (master, blinded, ctx) = test_fixtures();
        let mut proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();

        proof.context_id = "community:gardeners".to_string();

        assert!(!proof.verify().unwrap());
    }

    #[test]
    fn tampered_master_pubkey_hex_fails_verification() {
        let (master, blinded, ctx) = test_fixtures();
        let mut proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();

        // Replace master_pubkey_hex with a different key's hex.
        let imposter = CrownKeypair::generate();
        proof.master_pubkey_hex = imposter.public_key_hex();

        assert!(!proof.verify().unwrap());
    }

    #[test]
    fn cross_context_proof_fails() {
        let master = CrownKeypair::generate();
        let ctx_a = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_b = BlindingContext::new("community:gardeners", 0).unwrap();
        let blinded_a = derive_blinded_keypair(&master, &ctx_a).unwrap();

        // Create proof for context A, then tamper to claim context B.
        let mut proof = create_blinding_proof(&master, blinded_a.crown_id(), &ctx_a).unwrap();
        proof.context_id = ctx_b.context_id().to_string();

        assert!(!proof.verify().unwrap());
    }

    // -- Serialization --

    #[test]
    fn serialization_roundtrip() {
        let (master, blinded, ctx) = test_fixtures();
        let proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();

        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: BlindingProof = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.master_pubkey_hex(), proof.master_pubkey_hex());
        assert_eq!(deserialized.blinded_crown_id(), proof.blinded_crown_id());
        assert_eq!(deserialized.context_id(), proof.context_id());
        assert_eq!(deserialized.context_version(), proof.context_version());
        assert_eq!(deserialized.proof_signature(), proof.proof_signature());
        // Verify the deserialized proof still passes verification.
        assert!(deserialized.verify().unwrap());
    }

    // -- Edge cases --

    #[test]
    fn proof_with_correct_master_but_wrong_blinded_key_fails() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
        let wrong_key = CrownKeypair::generate();

        // Create a proof that binds a random key (not actually derived from master).
        let proof = create_blinding_proof(&master, wrong_key.crown_id(), &ctx).unwrap();

        // The signature IS valid (master did sign it), but the blinded key
        // is not actually derived from master+context. The proof only attests
        // that the master claims this binding — it does not verify derivation.
        // This is by design: the verifier trusts the master's signature.
        assert!(proof.verify().unwrap());
    }

    #[test]
    fn multiple_proofs_from_same_master_all_verify() {
        let master = CrownKeypair::generate();

        let contexts: Vec<BlindingContext> = (0..5)
            .map(|i| BlindingContext::new(format!("context:{i}"), 0).unwrap())
            .collect();

        let proofs: Vec<BlindingProof> = contexts
            .iter()
            .map(|ctx| {
                let blinded = derive_blinded_keypair(&master, ctx).unwrap();
                create_blinding_proof(&master, blinded.crown_id(), ctx).unwrap()
            })
            .collect();

        for proof in &proofs {
            assert!(proof.verify().unwrap());
        }

        // All proofs share the same master.
        let master_hex = master.public_key_hex();
        for proof in &proofs {
            assert_eq!(proof.master_pubkey_hex(), master_hex);
        }
    }

    #[test]
    fn proof_creation_requires_private_key() {
        let master = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(master.crown_id()).unwrap();
        let ctx = BlindingContext::new("test", 0).unwrap();

        let result = create_blinding_proof(&pubonly, "cpub1fake", &ctx);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CrownError::BlindingProofFailed(_)
        ));
    }

    #[test]
    fn tampered_context_version_fails_verification() {
        let (master, blinded, ctx) = test_fixtures();
        let mut proof = create_blinding_proof(&master, blinded.crown_id(), &ctx).unwrap();

        proof.context_version = 99;

        assert!(!proof.verify().unwrap());
    }

    #[test]
    fn proof_with_different_version_contexts() {
        let master = CrownKeypair::generate();
        let ctx_v0 = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_v1 = BlindingContext::new("community:woodworkers", 1).unwrap();

        let blinded_v0 = derive_blinded_keypair(&master, &ctx_v0).unwrap();
        let blinded_v1 = derive_blinded_keypair(&master, &ctx_v1).unwrap();

        let proof_v0 = create_blinding_proof(&master, blinded_v0.crown_id(), &ctx_v0).unwrap();
        let proof_v1 = create_blinding_proof(&master, blinded_v1.crown_id(), &ctx_v1).unwrap();

        assert!(proof_v0.verify().unwrap());
        assert!(proof_v1.verify().unwrap());

        // Different blinded keys for different versions.
        assert_ne!(proof_v0.blinded_crown_id(), proof_v1.blinded_crown_id());
    }
}
