use std::collections::HashMap;

use sentinal::{SecureData, key_derivation, encryption};
use uuid::Uuid;

use crate::error::VaultError;

/// Key lifecycle manager.
///
/// Custodian derives all keys from the master key via Sentinal,
/// caches them in memory (in SecureData containers), and zeros
/// everything on lock. Custodian never does crypto directly —
/// it delegates to Sentinal.
///
/// Key hierarchy:
/// ```text
/// Password (user enters)
///     | PBKDF2 (600K iterations, via Sentinal)
///     v
/// Master Key (SecureData, memory-only)
///     |-- derive_content_key(idea_id) -> Content Key (per .idea)
///     |-- derive_content_key(manifest_key_id) -> Manifest Key
///     |-- derive_vocabulary_seed() -> Vocabulary Seed
///     +-- collective keys (received externally, cached here)
/// ```
pub struct Custodian {
    /// Master key (zeroed on lock via SecureData drop).
    master_key: Option<SecureData>,
    /// Salt used for master key derivation.
    salt: Option<Vec<u8>>,
    /// UUID for manifest key derivation (from config).
    manifest_key_id: Option<Uuid>,
    /// Cached collective keys by collective ID.
    collective_keys: HashMap<Uuid, SecureData>,
    /// Cached content keys by idea ID.
    content_key_cache: HashMap<Uuid, SecureData>,
}

impl Custodian {
    /// Create a new custodian with no keys loaded.
    pub fn new() -> Self {
        Self {
            master_key: None,
            salt: None,
            manifest_key_id: None,
            collective_keys: HashMap::new(),
            content_key_cache: HashMap::new(),
        }
    }

    /// Whether a master key is loaded (vault is unlocked).
    pub fn is_unlocked(&self) -> bool {
        self.master_key.is_some()
    }

    /// Set the manifest key ID (called during unlock, from config).
    pub fn set_manifest_key_id(&mut self, id: Uuid) {
        self.manifest_key_id = Some(id);
    }

    /// Derive and cache master key from password + salt.
    ///
    /// If salt is None, Sentinal generates a fresh 32-byte random salt.
    /// Returns the salt used (may be newly generated).
    pub fn unlock(&mut self, password: &str, salt: Option<&[u8]>) -> Result<Vec<u8>, VaultError> {
        // Clear any existing key first (SecureData zeros on drop).
        self.master_key = None;

        let (master_key, used_salt) = key_derivation::derive_master_key(password, salt)?;
        self.master_key = Some(master_key);
        self.salt = Some(used_salt.clone());
        Ok(used_salt)
    }

    /// Derive a content key for a specific idea.
    ///
    /// Results are cached — repeated calls for the same idea ID return
    /// the cached key without re-deriving.
    pub fn content_key(&mut self, idea_id: &Uuid) -> Result<SecureData, VaultError> {
        if let Some(cached) = self.content_key_cache.get(idea_id) {
            return Ok(cached.clone());
        }
        let master = self.master_key_bytes()?;
        let key = key_derivation::derive_content_key(master, idea_id)?;
        self.content_key_cache.insert(*idea_id, key.clone());
        Ok(key)
    }

    /// Derive the manifest encryption key.
    ///
    /// Uses derive_content_key with the manifest_key_id UUID from config.
    pub fn manifest_key(&self) -> Result<SecureData, VaultError> {
        let master = self.master_key_bytes()?;
        let key_id = self.manifest_key_id.ok_or(VaultError::ManifestKeyIdNotSet)?;
        let key = key_derivation::derive_content_key(master, &key_id)?;
        Ok(key)
    }

    /// Derive a soul encryption key for Crown's soul.json at-rest encryption.
    ///
    /// Uses HKDF with domain salt `"omnidea-soul-v1"` and info `"soul-data"`.
    pub fn soul_key(&self) -> Result<SecureData, VaultError> {
        let master = self.master_key_bytes()?;
        let key = key_derivation::derive_soul_key(master)?;
        Ok(key)
    }

    /// Derive the vocabulary seed for Babel obfuscation.
    pub fn vocabulary_seed(&self) -> Result<SecureData, VaultError> {
        let master = self.master_key_bytes()?;
        let seed = key_derivation::derive_vocabulary_seed(master)?;
        Ok(seed)
    }

    /// Encrypt data using the content key for a specific idea.
    pub fn encrypt_for_idea(&mut self, data: &[u8], idea_id: &Uuid) -> Result<Vec<u8>, VaultError> {
        let key = self.content_key(idea_id)?;
        let encrypted = encryption::encrypt_combined(data, key.expose())?;
        Ok(encrypted)
    }

    /// Decrypt data using the content key for a specific idea.
    pub fn decrypt_for_idea(&mut self, data: &[u8], idea_id: &Uuid) -> Result<Vec<u8>, VaultError> {
        let key = self.content_key(idea_id)?;
        let decrypted = encryption::decrypt_combined(data, key.expose())?;
        Ok(decrypted)
    }

    /// Store a collective key (received from network or generated locally).
    pub fn store_collective_key(&mut self, collective_id: Uuid, key: SecureData) {
        self.collective_keys.insert(collective_id, key);
    }

    /// Get a cached collective key.
    pub fn collective_key(&self, collective_id: &Uuid) -> Result<&SecureData, VaultError> {
        self.collective_keys
            .get(collective_id)
            .ok_or(VaultError::CollectiveNotFound(*collective_id))
    }

    /// Remove a collective key from cache.
    pub fn remove_collective_key(&mut self, collective_id: &Uuid) {
        self.collective_keys.remove(collective_id);
    }

    /// List all cached collective IDs.
    pub fn cached_collective_ids(&self) -> Vec<Uuid> {
        self.collective_keys.keys().cloned().collect()
    }

    /// Clear all keys from memory.
    ///
    /// SecureData containers zero their contents on drop.
    pub fn clear(&mut self) {
        self.master_key = None;
        self.salt = None;
        self.manifest_key_id = None;
        self.collective_keys.clear();
        self.content_key_cache.clear();
    }

    /// Get the raw master key bytes. Errors if locked.
    fn master_key_bytes(&self) -> Result<&[u8], VaultError> {
        self.master_key
            .as_ref()
            .map(|k| k.expose())
            .ok_or(VaultError::Locked)
    }
}

impl Default for Custodian {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PASSWORD: &str = "test-vault-password";

    #[test]
    fn starts_without_key() {
        let custodian = Custodian::new();
        assert!(!custodian.is_unlocked());
        assert!(matches!(
            custodian.manifest_key(),
            Err(VaultError::Locked)
        ));
    }

    #[test]
    fn unlock_derives_master_key() {
        let mut custodian = Custodian::new();
        let salt = custodian.unlock(TEST_PASSWORD, None).unwrap();
        assert!(custodian.is_unlocked());
        assert_eq!(salt.len(), 32);
    }

    #[test]
    fn unlock_with_salt_deterministic() {
        let mut c1 = Custodian::new();
        let salt = c1.unlock(TEST_PASSWORD, None).unwrap();

        let mut c2 = Custodian::new();
        c2.unlock(TEST_PASSWORD, Some(&salt)).unwrap();

        // Both should derive the same master key.
        c1.set_manifest_key_id(Uuid::nil());
        c2.set_manifest_key_id(Uuid::nil());
        let k1 = c1.manifest_key().unwrap();
        let k2 = c2.manifest_key().unwrap();
        assert_eq!(k1.expose(), k2.expose());
    }

    #[test]
    fn content_key_per_idea() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let k1 = custodian.content_key(&id1).unwrap();
        let k2 = custodian.content_key(&id2).unwrap();
        assert_ne!(k1.expose(), k2.expose());
    }

    #[test]
    fn content_key_caching() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();

        let id = Uuid::new_v4();
        let k1 = custodian.content_key(&id).unwrap();
        let k2 = custodian.content_key(&id).unwrap();
        assert_eq!(k1.expose(), k2.expose());
    }

    #[test]
    fn soul_key_derivation() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();

        let key = custodian.soul_key().unwrap();
        assert_eq!(key.expose().len(), 32);
    }

    #[test]
    fn soul_key_deterministic() {
        let mut c1 = Custodian::new();
        let salt = c1.unlock(TEST_PASSWORD, None).unwrap();

        let mut c2 = Custodian::new();
        c2.unlock(TEST_PASSWORD, Some(&salt)).unwrap();

        let k1 = c1.soul_key().unwrap();
        let k2 = c2.soul_key().unwrap();
        assert_eq!(k1.expose(), k2.expose());
    }

    #[test]
    fn soul_key_locked_fails() {
        let custodian = Custodian::new();
        assert!(matches!(custodian.soul_key(), Err(VaultError::Locked)));
    }

    #[test]
    fn manifest_key_derivation() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();
        custodian.set_manifest_key_id(Uuid::new_v4());

        let key = custodian.manifest_key().unwrap();
        assert_eq!(key.expose().len(), 32);
    }

    #[test]
    fn manifest_key_without_id_fails() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();
        // Don't set manifest_key_id.
        assert!(matches!(
            custodian.manifest_key(),
            Err(VaultError::ManifestKeyIdNotSet)
        ));
    }

    #[test]
    fn collective_key_store_and_retrieve() {
        let mut custodian = Custodian::new();
        let cid = Uuid::new_v4();
        let key = SecureData::random(32).unwrap();
        let expected = key.expose().to_vec();

        custodian.store_collective_key(cid, key);
        let retrieved = custodian.collective_key(&cid).unwrap();
        assert_eq!(retrieved.expose(), &expected);

        // Unknown collective.
        assert!(matches!(
            custodian.collective_key(&Uuid::new_v4()),
            Err(VaultError::CollectiveNotFound(_))
        ));
    }

    #[test]
    fn encrypt_decrypt_for_idea() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();

        let idea_id = Uuid::new_v4();
        let plaintext = b"sovereign data";
        let encrypted = custodian.encrypt_for_idea(plaintext, &idea_id).unwrap();
        let decrypted = custodian.decrypt_for_idea(&encrypted, &idea_id).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn clear_zeros_everything() {
        let mut custodian = Custodian::new();
        custodian.unlock(TEST_PASSWORD, None).unwrap();
        custodian.set_manifest_key_id(Uuid::new_v4());
        custodian.store_collective_key(Uuid::new_v4(), SecureData::random(32).unwrap());

        let id = Uuid::new_v4();
        custodian.content_key(&id).unwrap();

        custodian.clear();
        assert!(!custodian.is_unlocked());
        assert!(custodian.cached_collective_ids().is_empty());
    }
}
