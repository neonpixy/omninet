use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::blinding::{derive_blinded_keypair, BlindingContext};
use crate::error::CrownError;
use crate::keypair::CrownKeypair;
use crate::rotation::{self, PreviousKey, RotationAnnouncement, RotationChain};
use crate::signature::Signature;

/// Manages Crown keypairs — primary identity and named personas.
///
/// The Keyring holds keypairs in memory. `export()` produces JSON bytes
/// for the caller to encrypt (via Sentinal) and persist (via Vault).
/// `load()` accepts those bytes back.
///
/// No `Serialize`/`Deserialize` on Keyring itself — only explicit
/// export/import through `KeyringStorage`.
pub struct Keyring {
    primary: Option<CrownKeypair>,
    personas: HashMap<String, CrownKeypair>,
    rotation_chain: RotationChain,
    /// Cached blinded keypairs, keyed by `"context_id:version"`.
    ///
    /// Derived deterministically from the primary key via HKDF.
    /// Cleared on `lock()` and invalidated on `rotate_primary()`.
    blinded_keys: HashMap<String, CrownKeypair>,
}

/// Internal serialization format for keyring data.
/// Private keys are hex-encoded for JSON safety.
#[derive(Serialize, Deserialize)]
struct KeyringStorage {
    primary_private_key: Option<String>,
    personas: HashMap<String, String>,
    #[serde(default)]
    rotation_chain: RotationChain,
    /// Cached blinded keys (context_key → hex-encoded private key).
    /// Default empty for backward compatibility with pre-blinding exports.
    #[serde(default)]
    blinded_keys: HashMap<String, String>,
}

impl Keyring {
    /// Create a new empty (locked) keyring.
    pub fn new() -> Self {
        Self {
            primary: None,
            personas: HashMap::new(),
            rotation_chain: RotationChain::default(),
            blinded_keys: HashMap::new(),
        }
    }

    // -- State --

    /// Whether a primary identity is loaded (unlocked).
    pub fn is_unlocked(&self) -> bool {
        self.primary.is_some()
    }

    // -- Setup --

    /// Generate a new random primary keypair.
    ///
    /// Fails if a primary already exists.
    pub fn generate_primary(&mut self) -> Result<&CrownKeypair, CrownError> {
        if self.primary.is_some() {
            return Err(CrownError::PersonaAlreadyExists("primary".into()));
        }
        self.primary = Some(CrownKeypair::generate());
        // SAFETY: we just assigned Some on the line above.
        Ok(self.primary.as_ref().expect("primary was just set"))
    }

    /// Import a primary keypair from a csec bech32 string.
    ///
    /// Replaces any existing primary.
    pub fn import_primary(&mut self, csec: &str) -> Result<&CrownKeypair, CrownError> {
        self.primary = Some(CrownKeypair::from_crown_secret(csec)?);
        // SAFETY: we just assigned Some on the line above.
        Ok(self.primary.as_ref().expect("primary was just set"))
    }

    // -- Persistence --

    /// Load keyring state from JSON bytes.
    ///
    /// Deserializes `KeyringStorage`, hex-decodes private keys,
    /// reconstructs keypairs.
    pub fn load(&mut self, data: &[u8]) -> Result<(), CrownError> {
        let mut storage: KeyringStorage =
            serde_json::from_slice(data).map_err(|e| CrownError::SoulCorrupted(e.to_string()))?;

        if let Some(ref mut hex_key) = storage.primary_private_key {
            let mut bytes = hex::decode(&*hex_key)
                .map_err(|e| CrownError::SoulCorrupted(format!("invalid hex: {e}")))?;
            let result = CrownKeypair::from_private_key(&bytes);
            bytes.zeroize();
            hex_key.zeroize();
            self.primary = Some(result?);
        }

        self.personas.clear();
        for (name, hex_key) in &mut storage.personas {
            let mut bytes = hex::decode(&*hex_key)
                .map_err(|e| CrownError::SoulCorrupted(format!("invalid hex: {e}")))?;
            let result = CrownKeypair::from_private_key(&bytes);
            bytes.zeroize();
            hex_key.zeroize();
            let kp = result?;
            self.personas.insert(name.clone(), kp);
        }

        self.rotation_chain = storage.rotation_chain;

        self.blinded_keys.clear();
        for (context_key, hex_key) in &mut storage.blinded_keys {
            let mut bytes = hex::decode(&*hex_key)
                .map_err(|e| CrownError::SoulCorrupted(format!("invalid hex: {e}")))?;
            let result = CrownKeypair::from_private_key(&bytes);
            bytes.zeroize();
            hex_key.zeroize();
            let kp = result?;
            self.blinded_keys.insert(context_key.clone(), kp);
        }

        Ok(())
    }

    /// Export keyring state as JSON bytes.
    ///
    /// Only exports keypairs that have private keys. The caller should
    /// encrypt these bytes before persisting.
    pub fn export(&self) -> Result<Vec<u8>, CrownError> {
        let mut primary_hex = self
            .primary
            .as_ref()
            .and_then(|kp| kp.private_key_data())
            .map(hex::encode);

        let mut persona_map = HashMap::new();
        for (name, kp) in &self.personas {
            if let Some(privkey) = kp.private_key_data() {
                persona_map.insert(name.clone(), hex::encode(privkey));
            }
        }

        let mut blinded_map = HashMap::new();
        for (context_key, kp) in &self.blinded_keys {
            if let Some(privkey) = kp.private_key_data() {
                blinded_map.insert(context_key.clone(), hex::encode(privkey));
            }
        }

        let mut storage = KeyringStorage {
            primary_private_key: primary_hex.take(),
            personas: std::mem::take(&mut persona_map),
            rotation_chain: self.rotation_chain.clone(),
            blinded_keys: std::mem::take(&mut blinded_map),
        };

        let result = serde_json::to_vec(&storage).map_err(CrownError::Serialization);

        // Zeroize all intermediate hex-encoded private key strings.
        if let Some(ref mut hex_key) = storage.primary_private_key {
            hex_key.zeroize();
        }
        for value in storage.personas.values_mut() {
            value.zeroize();
        }
        for value in storage.blinded_keys.values_mut() {
            value.zeroize();
        }

        result
    }

    /// Lock the keyring — clear all keys from memory.
    pub fn lock(&mut self) {
        self.primary = None;
        self.personas.clear();
        self.rotation_chain = RotationChain::default();
        self.blinded_keys.clear();
    }

    // -- Identity access --

    /// Primary identity's crown ID.
    pub fn public_key(&self) -> Result<&str, CrownError> {
        self.primary
            .as_ref()
            .map(|kp| kp.crown_id())
            .ok_or(CrownError::NoIdentity)
    }

    /// Primary identity's public key as hex.
    pub fn public_key_hex(&self) -> Result<String, CrownError> {
        self.primary
            .as_ref()
            .map(|kp| kp.public_key_hex())
            .ok_or(CrownError::NoIdentity)
    }

    /// A persona's crown ID.
    pub fn public_key_for(&self, persona: &str) -> Result<&str, CrownError> {
        self.personas
            .get(persona)
            .map(|kp| kp.crown_id())
            .ok_or_else(|| CrownError::PersonaNotFound(persona.into()))
    }

    // -- Signing --

    /// Sign data with the primary identity.
    pub fn sign(&self, data: &[u8]) -> Result<Signature, CrownError> {
        let kp = self.primary.as_ref().ok_or(CrownError::NoIdentity)?;
        Signature::sign(data, kp)
    }

    /// Sign data as a named persona.
    pub fn sign_as(&self, data: &[u8], persona: &str) -> Result<Signature, CrownError> {
        let kp = self
            .personas
            .get(persona)
            .ok_or_else(|| CrownError::PersonaNotFound(persona.into()))?;
        Signature::sign(data, kp)
    }

    /// Verify a signature against a crown ID.
    pub fn verify(&self, sig: &Signature, data: &[u8], crown_id: &str) -> bool {
        sig.verify_crown_id(data, crown_id)
    }

    // -- Persona management --

    /// Generate a new random persona keypair.
    pub fn create_persona(&mut self, name: &str) -> Result<&CrownKeypair, CrownError> {
        if self.personas.contains_key(name) {
            return Err(CrownError::PersonaAlreadyExists(name.into()));
        }
        self.personas
            .insert(name.to_string(), CrownKeypair::generate());
        // SAFETY: we just inserted this key on the line above.
        Ok(self.personas.get(name).expect("persona was just inserted"))
    }

    /// Import a persona keypair from a csec string.
    pub fn import_persona(
        &mut self,
        csec: &str,
        name: &str,
    ) -> Result<&CrownKeypair, CrownError> {
        if self.personas.contains_key(name) {
            return Err(CrownError::PersonaAlreadyExists(name.into()));
        }
        let kp = CrownKeypair::from_crown_secret(csec)?;
        self.personas.insert(name.to_string(), kp);
        // SAFETY: we just inserted this key on the line above.
        Ok(self.personas.get(name).expect("persona was just inserted"))
    }

    /// List persona names, sorted alphabetically.
    pub fn list_personas(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.personas.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Delete a named persona.
    pub fn delete_persona(&mut self, name: &str) -> Result<(), CrownError> {
        if self.personas.remove(name).is_none() {
            return Err(CrownError::PersonaNotFound(name.into()));
        }
        Ok(())
    }

    /// Whether a persona exists.
    pub fn has_persona(&self, name: &str) -> bool {
        self.personas.contains_key(name)
    }

    /// Export the primary identity's csec string.
    pub fn export_primary_csec(&self) -> Result<&str, CrownError> {
        self.primary
            .as_ref()
            .and_then(|kp| kp.crown_secret())
            .ok_or(CrownError::NoIdentity)
    }

    /// Export a persona's csec string.
    pub fn export_persona_csec(&self, name: &str) -> Result<&str, CrownError> {
        self.personas
            .get(name)
            .and_then(|kp| kp.crown_secret())
            .ok_or_else(|| CrownError::PersonaNotFound(name.into()))
    }

    // -- Key rotation --

    /// Rotate the primary key.
    ///
    /// Generates a new keypair, signs an announcement with the OLD key
    /// (proving chain of custody), records the old key in the rotation chain,
    /// then replaces the primary.
    ///
    /// Returns a `RotationAnnouncement` signed by the old key.
    pub fn rotate_primary(&mut self) -> Result<RotationAnnouncement, CrownError> {
        // 1. Verify a primary key exists
        let old_kp = self.primary.as_ref().ok_or(CrownError::NoPrimaryKey)?;
        let old_pubkey_hex = old_kp.public_key_hex();
        let old_crown_id = old_kp.crown_id().to_string();

        // 2. Generate a new keypair
        let new_kp = CrownKeypair::generate();
        let new_pubkey_hex = new_kp.public_key_hex();
        let new_crown_id = new_kp.crown_id().to_string();

        // 3. Build the signable data
        let timestamp = Utc::now();
        let signable = rotation::build_signable_bytes(
            &old_pubkey_hex,
            &new_pubkey_hex,
            &timestamp,
        );

        // 4. Sign with the OLD key
        let sig = Signature::sign(&signable, old_kp)
            .map_err(|e| CrownError::RotationFailed(format!("signing failed: {e}")))?;

        // 5. Create PreviousKey record
        let previous = PreviousKey {
            public_key_hex: old_pubkey_hex.clone(),
            crown_id: old_crown_id.clone(),
            rotated_at: timestamp,
            rotation_signature: sig.data().to_vec(),
        };

        // 6. Push into rotation chain
        self.rotation_chain.push(previous);

        // 7. Replace primary with new keypair (direct assignment, not generate_primary)
        self.primary = Some(new_kp);

        // 8. Invalidate blinded key cache — derived from the old primary
        self.blinded_keys.clear();

        // 9. Return the announcement
        Ok(RotationAnnouncement {
            old_pubkey_hex,
            new_pubkey_hex,
            old_crown_id,
            new_crown_id,
            signature: sig.data().to_vec(),
            timestamp,
            reason: None,
        })
    }

    /// Immutable access to the rotation chain.
    pub fn rotation_chain(&self) -> &RotationChain {
        &self.rotation_chain
    }

    /// Mutable access to the rotation chain (for testing / advanced use).
    pub fn rotation_chain_mut(&mut self) -> &mut RotationChain {
        &mut self.rotation_chain
    }

    /// Direct access to the primary keypair (needed by recovery and device sync).
    pub fn primary_keypair(&self) -> Option<&CrownKeypair> {
        self.primary.as_ref()
    }

    // -- Blinded key management --

    /// Derive a blinded keypair for a context and cache it.
    ///
    /// If a blinded key for this context (including version) already exists
    /// in the cache, it is overwritten with a fresh derivation.
    ///
    /// # Errors
    ///
    /// - [`CrownError::NoIdentity`] if no primary key is loaded.
    /// - [`CrownError::BlindingFailed`] if HKDF derivation fails.
    pub fn derive_blinded_for_context(
        &mut self,
        context: &BlindingContext,
    ) -> Result<&CrownKeypair, CrownError> {
        let primary = self.primary.as_ref().ok_or(CrownError::NoIdentity)?;
        let blinded = derive_blinded_keypair(primary, context)?;
        let cache_key = blinded_cache_key(context);
        self.blinded_keys.insert(cache_key.clone(), blinded);
        // SAFETY: we just inserted, so the key is present.
        Ok(self.blinded_keys.get(&cache_key).expect("blinded key was just inserted"))
    }

    /// Get a cached blinded keypair, or derive and cache it on miss.
    ///
    /// This is the primary entry point for blinded key usage. It returns
    /// the cached key if available, avoiding redundant HKDF derivations.
    ///
    /// # Errors
    ///
    /// - [`CrownError::NoIdentity`] if no primary key is loaded.
    /// - [`CrownError::BlindingFailed`] if HKDF derivation fails (first call only).
    pub fn get_or_derive_blinded(
        &mut self,
        context: &BlindingContext,
    ) -> Result<&CrownKeypair, CrownError> {
        let cache_key = blinded_cache_key(context);
        if self.blinded_keys.contains_key(&cache_key) {
            // SAFETY: we just confirmed the key is present via contains_key.
            return Ok(self.blinded_keys.get(&cache_key).expect("checked contains_key"));
        }
        self.derive_blinded_for_context(context)
    }

    /// Sign data with a cached blinded key.
    ///
    /// The blinded key must already be in the cache (via
    /// [`get_or_derive_blinded`](Self::get_or_derive_blinded) or
    /// [`derive_blinded_for_context`](Self::derive_blinded_for_context)).
    ///
    /// # Errors
    ///
    /// - [`CrownError::BlindedKeyNotFound`] if no cached key exists for this context.
    /// - [`CrownError::SignatureFailed`] if signing fails.
    pub fn sign_blinded(
        &self,
        data: &[u8],
        context: &BlindingContext,
    ) -> Result<Signature, CrownError> {
        let cache_key = blinded_cache_key(context);
        let kp = self
            .blinded_keys
            .get(&cache_key)
            .ok_or(CrownError::BlindedKeyNotFound(cache_key))?;
        Signature::sign(data, kp)
    }

    /// Clear all cached blinded keys from memory.
    pub fn clear_blinded_cache(&mut self) {
        self.blinded_keys.clear();
    }

    /// Number of blinded keys currently cached.
    #[must_use]
    pub fn blinded_key_count(&self) -> usize {
        self.blinded_keys.len()
    }
}

/// Build the HashMap cache key for a blinding context: `"context_id:version"`.
fn blinded_cache_key(context: &BlindingContext) -> String {
    format!("{}:{}", context.context_id(), context.version())
}

impl Default for Keyring {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_keyring_is_locked() {
        let kr = Keyring::new();
        assert!(!kr.is_unlocked());
        assert!(kr.public_key().is_err());
    }

    #[test]
    fn generate_primary_unlocks() {
        let mut kr = Keyring::new();
        let kp = kr.generate_primary().unwrap();
        assert!(kp.crown_id().starts_with("cpub1"));
        assert!(kr.is_unlocked());
    }

    #[test]
    fn generate_primary_twice_fails() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let result = kr.generate_primary();
        assert!(matches!(
            result.unwrap_err(),
            CrownError::PersonaAlreadyExists(_)
        ));
    }

    #[test]
    fn import_primary_from_crown_secret() {
        let original = CrownKeypair::generate();
        let crown_secret = original.crown_secret().unwrap().to_string();

        let mut kr = Keyring::new();
        kr.import_primary(&crown_secret).unwrap();
        assert_eq!(kr.public_key().unwrap(), original.crown_id());
    }

    #[test]
    fn export_import_round_trip() {
        let mut kr1 = Keyring::new();
        kr1.generate_primary().unwrap();
        kr1.create_persona("work").unwrap();
        kr1.create_persona("anon").unwrap();

        let exported = kr1.export().unwrap();

        let mut kr2 = Keyring::new();
        kr2.load(&exported).unwrap();

        assert_eq!(kr1.public_key().unwrap(), kr2.public_key().unwrap());
        assert_eq!(
            kr1.public_key_for("work").unwrap(),
            kr2.public_key_for("work").unwrap()
        );
        assert_eq!(
            kr1.public_key_for("anon").unwrap(),
            kr2.public_key_for("anon").unwrap()
        );
    }

    #[test]
    fn lock_clears_keys() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        kr.create_persona("test").unwrap();
        assert!(kr.is_unlocked());

        kr.lock();
        assert!(!kr.is_unlocked());
        assert!(kr.public_key().is_err());
        assert!(kr.list_personas().is_empty());
    }

    #[test]
    fn sign_and_verify_via_keyring() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let data = b"keyring signing test";
        let sig = kr.sign(data).unwrap();
        assert!(kr.verify(&sig, data, kr.public_key().unwrap()));
    }

    #[test]
    fn persona_crud() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        kr.create_persona("work").unwrap();
        assert!(kr.has_persona("work"));
        assert_eq!(kr.list_personas(), vec!["work"]);

        kr.create_persona("play").unwrap();
        assert_eq!(kr.list_personas(), vec!["play", "work"]);

        kr.delete_persona("work").unwrap();
        assert!(!kr.has_persona("work"));
        assert_eq!(kr.list_personas(), vec!["play"]);
    }

    #[test]
    fn persona_already_exists() {
        let mut kr = Keyring::new();
        kr.create_persona("dup").unwrap();
        let result = kr.create_persona("dup");
        assert!(matches!(
            result.unwrap_err(),
            CrownError::PersonaAlreadyExists(_)
        ));
    }

    #[test]
    fn delete_nonexistent_persona_fails() {
        let mut kr = Keyring::new();
        let result = kr.delete_persona("ghost");
        assert!(matches!(
            result.unwrap_err(),
            CrownError::PersonaNotFound(_)
        ));
    }

    // -- Blinded keyring tests --

    #[test]
    fn derive_blinded_for_context_caches_key() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let blinded = kr.derive_blinded_for_context(&ctx).unwrap();
        assert!(blinded.crown_id().starts_with("cpub1"));
        assert_eq!(kr.blinded_key_count(), 1);
    }

    #[test]
    fn derive_blinded_requires_primary() {
        let mut kr = Keyring::new();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let result = kr.derive_blinded_for_context(&ctx);
        assert!(matches!(result.unwrap_err(), CrownError::NoIdentity));
    }

    #[test]
    fn get_or_derive_blinded_cache_hit() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        // First call derives and caches.
        let cpub1 = kr.get_or_derive_blinded(&ctx).unwrap().crown_id().to_string();
        // Second call returns cached — same key.
        let cpub2 = kr.get_or_derive_blinded(&ctx).unwrap().crown_id().to_string();
        assert_eq!(cpub1, cpub2);
        assert_eq!(kr.blinded_key_count(), 1);
    }

    #[test]
    fn different_contexts_produce_different_cached_keys() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx_a = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_b = BlindingContext::new("community:gardeners", 0).unwrap();

        let cpub_a = kr.get_or_derive_blinded(&ctx_a).unwrap().crown_id().to_string();
        let cpub_b = kr.get_or_derive_blinded(&ctx_b).unwrap().crown_id().to_string();

        assert_ne!(cpub_a, cpub_b);
        assert_eq!(kr.blinded_key_count(), 2);
    }

    #[test]
    fn different_versions_produce_different_cached_keys() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx_v0 = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_v1 = BlindingContext::new("community:woodworkers", 1).unwrap();

        let cpub_v0 = kr.get_or_derive_blinded(&ctx_v0).unwrap().crown_id().to_string();
        let cpub_v1 = kr.get_or_derive_blinded(&ctx_v1).unwrap().crown_id().to_string();

        assert_ne!(cpub_v0, cpub_v1);
        assert_eq!(kr.blinded_key_count(), 2);
    }

    #[test]
    fn sign_blinded_works_for_cached_context() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let blinded_cpub = kr.get_or_derive_blinded(&ctx).unwrap().crown_id().to_string();

        let data = b"blinded signing test";
        let sig = kr.sign_blinded(data, &ctx).unwrap();

        // Verify against the blinded public key, not the primary.
        assert!(sig.verify_crown_id(data, &blinded_cpub));
        assert_eq!(sig.signer(), blinded_cpub);
    }

    #[test]
    fn sign_blinded_errors_for_uncached_context() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        // Never called get_or_derive_blinded — cache is empty.
        let result = kr.sign_blinded(b"data", &ctx);
        assert!(matches!(
            result.unwrap_err(),
            CrownError::BlindedKeyNotFound(_)
        ));
    }

    #[test]
    fn clear_blinded_cache_removes_all_entries() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx_a = BlindingContext::new("community:woodworkers", 0).unwrap();
        let ctx_b = BlindingContext::new("relay:tower.alice.idea", 0).unwrap();

        kr.get_or_derive_blinded(&ctx_a).unwrap();
        kr.get_or_derive_blinded(&ctx_b).unwrap();
        assert_eq!(kr.blinded_key_count(), 2);

        kr.clear_blinded_cache();
        assert_eq!(kr.blinded_key_count(), 0);
    }

    #[test]
    fn blinded_key_count_reflects_cache_state() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        assert_eq!(kr.blinded_key_count(), 0);

        let ctx = BlindingContext::new("test", 0).unwrap();
        kr.get_or_derive_blinded(&ctx).unwrap();
        assert_eq!(kr.blinded_key_count(), 1);

        let ctx2 = BlindingContext::new("test", 1).unwrap();
        kr.get_or_derive_blinded(&ctx2).unwrap();
        assert_eq!(kr.blinded_key_count(), 2);

        kr.clear_blinded_cache();
        assert_eq!(kr.blinded_key_count(), 0);
    }

    #[test]
    fn export_import_round_trip_with_blinded_keys() {
        let mut kr1 = Keyring::new();
        kr1.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
        let blinded_cpub = kr1.get_or_derive_blinded(&ctx).unwrap().crown_id().to_string();

        let exported = kr1.export().unwrap();

        let mut kr2 = Keyring::new();
        kr2.load(&exported).unwrap();

        assert_eq!(kr2.blinded_key_count(), 1);
        // The cached blinded key should survive the round trip.
        let sig = kr2.sign_blinded(b"round trip", &ctx).unwrap();
        assert!(sig.verify_crown_id(b"round trip", &blinded_cpub));
    }

    #[test]
    fn backward_compat_load_without_blinded_keys() {
        // Simulate pre-blinding JSON (no blinded_keys field).
        let json = r#"{"primary_private_key":null,"personas":{}}"#;

        let mut kr = Keyring::new();
        kr.load(json.as_bytes()).unwrap();
        assert_eq!(kr.blinded_key_count(), 0);
    }

    #[test]
    fn lock_clears_blinded_cache() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
        kr.get_or_derive_blinded(&ctx).unwrap();
        assert_eq!(kr.blinded_key_count(), 1);

        kr.lock();
        assert_eq!(kr.blinded_key_count(), 0);
    }

    #[test]
    fn rotate_primary_invalidates_blinded_cache() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
        kr.get_or_derive_blinded(&ctx).unwrap();
        assert_eq!(kr.blinded_key_count(), 1);

        kr.rotate_primary().unwrap();
        assert_eq!(kr.blinded_key_count(), 0);

        // Re-derive produces a different key (new primary).
        let new_cpub = kr.get_or_derive_blinded(&ctx).unwrap().crown_id().to_string();
        // It should be a valid key, just different from what was cached before.
        assert!(new_cpub.starts_with("cpub1"));
    }

    #[test]
    fn blinded_key_differs_from_primary() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let primary_cpub = kr.public_key().unwrap().to_string();

        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();
        let blinded_cpub = kr.get_or_derive_blinded(&ctx).unwrap().crown_id().to_string();

        assert_ne!(primary_cpub, blinded_cpub);
    }

    #[test]
    fn derive_blinded_overwrites_existing_cache_entry() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let ctx = BlindingContext::new("community:woodworkers", 0).unwrap();

        let cpub1 = kr.derive_blinded_for_context(&ctx).unwrap().crown_id().to_string();
        // Deriving again should overwrite but produce the same key (deterministic).
        let cpub2 = kr.derive_blinded_for_context(&ctx).unwrap().crown_id().to_string();

        assert_eq!(cpub1, cpub2);
        assert_eq!(kr.blinded_key_count(), 1);
    }
}
