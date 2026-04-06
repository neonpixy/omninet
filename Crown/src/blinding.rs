//! Pubkey blinding — deterministic derivation of context-specific keypairs.
//!
//! Blinding lets a single master identity present different public keys in
//! different contexts (communities, relays, topics) so that an observer cannot
//! correlate activity across contexts without the master private key.
//!
//! # How it works
//!
//! Given a master [`CrownKeypair`] and a [`BlindingContext`], HKDF-SHA256
//! deterministically derives a new 32-byte secret, which becomes a fresh
//! secp256k1 keypair. The derivation is one-way: knowing a blinded public key
//! reveals nothing about the master key.
//!
//! The same master + context always produces the same blinded key, so the owner
//! can regenerate any blinded identity from their master key alone.
//!
//! # Limitations
//!
//! Public-only derivation (from just the master public key) is **not possible**
//! because HKDF requires the private key as input keying material. To verify
//! that a blinded key belongs to a particular master, the master must reveal
//! the blinded key — or produce a proof of ownership (future work).
//!
//! # Example
//!
//! ```
//! use crown::{CrownKeypair, BlindingContext};
//! use crown::blinding::{derive_blinded_keypair, recover_blinded_keypair};
//!
//! let master = CrownKeypair::generate();
//! let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
//!
//! let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
//! assert_ne!(blinded.crown_id(), master.crown_id());
//!
//! // Recovery: same result from raw private key bytes
//! let recovered = recover_blinded_keypair(
//!     master.private_key_data().unwrap(),
//!     &ctx,
//! ).unwrap();
//! assert_eq!(blinded.crown_id(), recovered.crown_id());
//! ```

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::CrownError;
use crate::keypair::CrownKeypair;

/// Domain separation salt for all Omnidea blinding derivations.
///
/// Changing this would produce an entirely different set of blinded keys,
/// so it is effectively part of the protocol and must never change.
const BLINDING_SALT: &[u8] = b"omnidea-blinding-v1";

/// A context that selects which blinded keypair to derive.
///
/// The `context_id` identifies the scope (a community, relay, topic, etc.)
/// and the `version` enables rotation within that scope without changing the
/// context identifier.
///
/// # Invariants
///
/// - `context_id` must be non-empty.
/// - Together, `(context_id, version)` uniquely determines the blinded key
///   for a given master key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlindingContext {
    /// Scope identifier, e.g. `"community:woodworkers"` or `"relay:tower.alice.idea"`.
    context_id: String,
    /// Rotation version within this context. Start at 0.
    version: u8,
}

impl BlindingContext {
    /// Create a new blinding context.
    ///
    /// Returns `Err` if `context_id` is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use crown::BlindingContext;
    ///
    /// let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
    /// assert_eq!(ctx.context_id(), "community:woodworkers");
    /// assert_eq!(ctx.version(), 0);
    /// ```
    pub fn new(context_id: impl Into<String>, version: u8) -> Result<Self, CrownError> {
        let context_id = context_id.into();
        if context_id.is_empty() {
            return Err(CrownError::InvalidBlindingContext(
                "context_id must not be empty".into(),
            ));
        }
        Ok(Self {
            context_id,
            version,
        })
    }

    /// The scope identifier.
    pub fn context_id(&self) -> &str {
        &self.context_id
    }

    /// The rotation version.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Build the HKDF `info` parameter: `context_id || version`.
    ///
    /// The version byte is appended raw (not ASCII) so that the info is
    /// unambiguous — a context_id cannot end with a byte that collides
    /// with a version value.
    fn hkdf_info(&self) -> Vec<u8> {
        let mut info = self.context_id.as_bytes().to_vec();
        info.push(self.version);
        info
    }
}

/// Derive a blinded keypair from a master keypair and a context.
///
/// The derivation is deterministic: the same master key and context always
/// produce the same blinded keypair. The blinded key is a fully functional
/// [`CrownKeypair`] — it can sign, verify, and participate in ECDH.
///
/// # Errors
///
/// - [`CrownError::Locked`] if the master keypair has no private key.
/// - [`CrownError::BlindingFailed`] if key derivation produces an invalid
///   secp256k1 scalar (astronomically unlikely, ~1 in 2^128).
#[must_use = "returns the blinded keypair"]
pub fn derive_blinded_keypair(
    master: &CrownKeypair,
    context: &BlindingContext,
) -> Result<CrownKeypair, CrownError> {
    let master_secret = master.private_key_data().ok_or(CrownError::Locked)?;
    derive_from_raw_key(master_secret, context)
}

/// Recover a blinded keypair from raw master private key bytes and a context.
///
/// This is the recovery path: given the master private key (from a seed phrase
/// backup, social recovery, or encrypted backup), regenerate any blinded
/// keypair without reconstructing the full master [`CrownKeypair`] first.
///
/// Functionally identical to [`derive_blinded_keypair`] but accepts raw bytes
/// instead of a [`CrownKeypair`] reference.
///
/// # Errors
///
/// - [`CrownError::BlindingFailed`] if `master_private_key` is not exactly
///   32 bytes or if key derivation produces an invalid scalar.
#[must_use = "returns the recovered blinded keypair"]
pub fn recover_blinded_keypair(
    master_private_key: &[u8],
    context: &BlindingContext,
) -> Result<CrownKeypair, CrownError> {
    let key: &[u8; 32] = master_private_key.try_into().map_err(|_| {
        CrownError::BlindingFailed(format!(
            "master private key must be 32 bytes, got {}",
            master_private_key.len()
        ))
    })?;
    derive_from_raw_key(key, context)
}

/// Internal: perform the HKDF derivation and construct the blinded keypair.
fn derive_from_raw_key(
    master_secret: &[u8; 32],
    context: &BlindingContext,
) -> Result<CrownKeypair, CrownError> {
    let info = context.hkdf_info();

    // HKDF-SHA256: extract + expand.
    //   salt  = "omnidea-blinding-v1" (domain separation)
    //   ikm   = master private key (32 bytes)
    //   info  = context_id || version
    //   len   = 32 bytes (one secp256k1 scalar)
    let hk = Hkdf::<Sha256>::new(Some(BLINDING_SALT), master_secret);
    let mut okm = [0u8; 32];
    hk.expand(&info, &mut okm).map_err(|e| {
        CrownError::BlindingFailed(format!("HKDF expand failed: {e}"))
    })?;

    // Construct a CrownKeypair from the derived bytes.
    // from_private_key validates that the bytes form a valid secp256k1 scalar.
    CrownKeypair::from_private_key(&okm).map_err(|e| {
        CrownError::BlindingFailed(format!("derived key is not a valid secp256k1 scalar: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Determinism --

    #[test]
    fn same_master_and_context_produce_same_blinded_key() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let a = derive_blinded_keypair(&master, &ctx).unwrap();
        let b = derive_blinded_keypair(&master, &ctx).unwrap();

        assert_eq!(a.crown_id(), b.crown_id());
        assert_eq!(a.private_key_data(), b.private_key_data());
    }

    #[test]
    fn deterministic_across_reconstruct() {
        // Generate, export private key, re-import, derive again — same result.
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("relay:tower.alice.idea", 1).unwrap();

        let blinded1 = derive_blinded_keypair(&master, &ctx).unwrap();

        let master2 =
            CrownKeypair::from_private_key(master.private_key_data().unwrap()).unwrap();
        let blinded2 = derive_blinded_keypair(&master2, &ctx).unwrap();

        assert_eq!(blinded1.crown_id(), blinded2.crown_id());
    }

    // -- Uniqueness --

    #[test]
    fn different_contexts_produce_different_keys() {
        let master = CrownKeypair::generate();
        let ctx_a = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_b = BlindingContext::new("community:gardeners", 0).unwrap();

        let a = derive_blinded_keypair(&master, &ctx_a).unwrap();
        let b = derive_blinded_keypair(&master, &ctx_b).unwrap();

        assert_ne!(a.crown_id(), b.crown_id());
    }

    #[test]
    fn different_versions_produce_different_keys() {
        let master = CrownKeypair::generate();
        let ctx_v0 = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_v1 = BlindingContext::new("community:woodworkers", 1).unwrap();

        let v0 = derive_blinded_keypair(&master, &ctx_v0).unwrap();
        let v1 = derive_blinded_keypair(&master, &ctx_v1).unwrap();

        assert_ne!(v0.crown_id(), v1.crown_id());
    }

    #[test]
    fn blinded_key_differs_from_master() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("any-context", 0).unwrap();

        let blinded = derive_blinded_keypair(&master, &ctx).unwrap();

        assert_ne!(blinded.crown_id(), master.crown_id());
        assert_ne!(
            blinded.private_key_data().unwrap(),
            master.private_key_data().unwrap()
        );
    }

    #[test]
    fn different_masters_same_context_produce_different_keys() {
        let master_a = CrownKeypair::generate();
        let master_b = CrownKeypair::generate();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let a = derive_blinded_keypair(&master_a, &ctx).unwrap();
        let b = derive_blinded_keypair(&master_b, &ctx).unwrap();

        assert_ne!(a.crown_id(), b.crown_id());
    }

    // -- Blinded key is fully functional --

    #[test]
    fn blinded_key_has_private_key() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("test", 0).unwrap();

        let blinded = derive_blinded_keypair(&master, &ctx).unwrap();

        assert!(blinded.has_private_key());
        assert!(blinded.crown_secret().is_some());
        assert!(blinded.crown_id().starts_with("cpub1"));
    }

    #[test]
    fn blinded_key_can_ecdh() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("test", 0).unwrap();
        let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
        let peer = CrownKeypair::generate();

        // ECDH should work in both directions.
        let secret_bp = blinded.shared_secret(peer.public_key_data()).unwrap();
        let secret_pb = peer.shared_secret(blinded.public_key_data()).unwrap();
        assert_eq!(secret_bp, secret_pb);
    }

    // -- Error handling --

    #[test]
    fn derive_requires_private_key() {
        let master = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(master.crown_id()).unwrap();
        let ctx = BlindingContext::new("test", 0).unwrap();

        let result = derive_blinded_keypair(&pubonly, &ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrownError::Locked));
    }

    #[test]
    fn empty_context_id_rejected() {
        let result = BlindingContext::new("", 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CrownError::InvalidBlindingContext(_)
        ));
    }

    #[test]
    fn recover_rejects_wrong_length() {
        let ctx = BlindingContext::new("test", 0).unwrap();

        let result = recover_blinded_keypair(&[0u8; 16], &ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrownError::BlindingFailed(_)));
    }

    #[test]
    fn recover_rejects_empty_key() {
        let ctx = BlindingContext::new("test", 0).unwrap();

        let result = recover_blinded_keypair(&[], &ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrownError::BlindingFailed(_)));
    }

    // -- Recovery roundtrip --

    #[test]
    fn recovery_roundtrip() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let derived = derive_blinded_keypair(&master, &ctx).unwrap();
        let recovered =
            recover_blinded_keypair(master.private_key_data().unwrap(), &ctx).unwrap();

        assert_eq!(derived.crown_id(), recovered.crown_id());
        assert_eq!(
            derived.private_key_data().unwrap(),
            recovered.private_key_data().unwrap()
        );
    }

    #[test]
    fn recovery_across_all_versions() {
        let master = CrownKeypair::generate();
        let master_secret = *master.private_key_data().unwrap();

        for version in [0u8, 1, 127, 255] {
            let ctx = BlindingContext::new("test-context", version).unwrap();

            let derived = derive_blinded_keypair(&master, &ctx).unwrap();
            let recovered = recover_blinded_keypair(&master_secret, &ctx).unwrap();

            assert_eq!(derived.crown_id(), recovered.crown_id());
        }
    }

    // -- BlindingContext --

    #[test]
    fn context_accessors() {
        let ctx = BlindingContext::new("community:woodworkers", 3).unwrap();
        assert_eq!(ctx.context_id(), "community:woodworkers");
        assert_eq!(ctx.version(), 3);
    }

    #[test]
    fn context_serialization_roundtrip() {
        let ctx = BlindingContext::new("relay:tower.alice.idea", 42).unwrap();
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: BlindingContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, deserialized);
    }

    #[test]
    fn context_equality() {
        let a = BlindingContext::new("test", 0).unwrap();
        let b = BlindingContext::new("test", 0).unwrap();
        let c = BlindingContext::new("test", 1).unwrap();
        let d = BlindingContext::new("other", 0).unwrap();

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn context_hash_consistency() {
        use std::collections::HashSet;

        let a = BlindingContext::new("test", 0).unwrap();
        let b = BlindingContext::new("test", 0).unwrap();
        let c = BlindingContext::new("test", 1).unwrap();

        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b); // duplicate, should not increase size
        set.insert(c);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn very_long_context_id_works() {
        let master = CrownKeypair::generate();
        let long_id = "x".repeat(10_000);
        let ctx = BlindingContext::new(long_id, 0).unwrap();

        // Should not panic or error — HKDF handles arbitrary info lengths.
        let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
        assert!(blinded.has_private_key());
    }

    #[test]
    fn unicode_context_id_works() {
        let master = CrownKeypair::generate();
        let ctx = BlindingContext::new("communauté:ébénisterie:日本語", 0).unwrap();

        let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
        assert!(blinded.has_private_key());
    }

    #[test]
    fn all_256_versions_produce_unique_keys() {
        let master = CrownKeypair::generate();
        let mut seen = std::collections::HashSet::new();

        for v in 0..=255u8 {
            let ctx = BlindingContext::new("version-uniqueness-test", v).unwrap();
            let blinded = derive_blinded_keypair(&master, &ctx).unwrap();
            assert!(
                seen.insert(blinded.crown_id().to_string()),
                "version {v} produced a duplicate key"
            );
        }

        assert_eq!(seen.len(), 256);
    }
}
