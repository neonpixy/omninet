//! # Vault — Encrypted Storage for Omnidea
//!
//! The locked treasury. Vault manages encrypted persistence — it knows where
//! every `.idea` lives, manages the key lifecycle, and owns the manifest
//! database. When locked, nothing is accessible. When unlocked, keys exist
//! only in memory.
//!
//! # Key Hierarchy
//!
//! ```text
//! Password + Salt
//!     | PBKDF2 (via Sentinal)
//!     v
//! Master Key (memory-only, zeroed on lock)
//!     |-- HKDF(idea_id) -> Content Key (per .idea)
//!     |-- HKDF(manifest_key_id) -> Manifest Key (SQLCipher)
//!     |-- HKDF("vocabulary-seed") -> Vocabulary Seed (for Babel)
//!     +-- Collective keys (received externally, cached)
//! ```

pub mod error;
pub mod config;
pub mod state;
pub mod entry;
pub mod collective;
pub mod custodian;
pub mod manifest;
pub mod search;
mod module_state;

// Re-exports
pub use error::VaultError;
pub use config::VaultConfig;
pub use state::VaultState;
pub use entry::{ManifestEntry, IdeaFilter, SortField, SortOrder};
pub use search::{VaultSearch, SearchHit};
pub use collective::{Collective, CollectiveMember, CollectiveRole};
pub use custodian::Custodian;
pub use manifest::Manifest;

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use sentinal::SecureData;
use uuid::Uuid;

/// The Vault — Omnidea's encrypted storage layer.
///
/// Vault manages the key lifecycle, tracks all .idea files in an
/// encrypted manifest database, and supports collective (multi-user)
/// shared spaces. When locked, nothing is accessible. When unlocked,
/// keys exist only in memory and are zeroed on lock.
///
/// # Usage
///
/// ```no_run
/// use vault::Vault;
/// use std::path::PathBuf;
///
/// let mut vault = Vault::new();
/// vault.unlock("my-password", PathBuf::from("/path/to/vault")).unwrap();
/// // ... use the vault ...
/// vault.lock().unwrap();
/// ```
pub struct Vault {
    state: VaultState,
    custodian: Custodian,
    manifest: Manifest,
    collectives: HashMap<Uuid, Collective>,
}

impl Vault {
    /// Create a new vault in the locked state. Call [`unlock`](Vault::unlock)
    /// with a password and root path to begin using it.
    pub fn new() -> Self {
        Self {
            state: VaultState::new(),
            custodian: Custodian::new(),
            manifest: Manifest::new(),
            collectives: HashMap::new(),
        }
    }

    /// Whether the vault is currently unlocked.
    pub fn is_unlocked(&self) -> bool {
        self.state.is_unlocked()
    }

    /// Unlock the vault with a password and root directory path.
    ///
    /// This performs the full unlock sequence:
    /// 1. Validate not already unlocked
    /// 2. Set root path and create .vault/ directory
    /// 3. Load or create config (salt, manifest key ID)
    /// 4. Derive master key via PBKDF2 (Sentinal)
    /// 5. Derive manifest key via HKDF (Sentinal)
    /// 6. Open encrypted manifest database (SQLCipher)
    /// 7. Load persisted collectives
    pub fn unlock(&mut self, password: &str, root_path: PathBuf) -> Result<(), VaultError> {
        if self.state.is_unlocked() {
            return Err(VaultError::AlreadyUnlocked);
        }

        // Set root path.
        self.state.unlock(root_path);

        // Create .vault/ directory if needed.
        let vault_dir = self.state.vault_dir()?;
        std::fs::create_dir_all(&vault_dir)?;

        // Load or create config.
        let config_path = self.state.config_path()?;
        let mut config = VaultConfig::load_or_create(&config_path)?;

        // Handle salt: generate if new vault.
        let salt = match &config.salt {
            Some(s) => s.clone(),
            None => {
                let s = sentinal::key_derivation::generate_salt(32)?;
                config.salt = Some(s.clone());
                s
            }
        };

        // Handle manifest key ID: generate if new vault.
        let manifest_key_id = match config.manifest_key_id {
            Some(id) => id,
            None => {
                let id = Uuid::new_v4();
                config.manifest_key_id = Some(id);
                id
            }
        };

        // Update last_unlocked and save config.
        config.last_unlocked = Some(Utc::now());
        config.save(&config_path)?;

        // Derive master key via PBKDF2.
        self.custodian.unlock(password, Some(&salt))?;
        self.custodian.set_manifest_key_id(manifest_key_id);

        // Derive manifest key via HKDF and open SQLCipher database.
        let manifest_key = self.custodian.manifest_key()?;
        let manifest_path = self.state.manifest_path()?;
        match self.manifest.open(&manifest_path, manifest_key.expose()) {
            Ok(()) => {}
            Err(e) => {
                // Roll back: clear keys and lock state.
                self.custodian.clear();
                self.state.lock();
                return Err(e);
            }
        }

        // Load persisted collectives from module state.
        self.load_collectives();

        Ok(())
    }

    /// Lock the vault. Zeros all keys and closes the manifest.
    pub fn lock(&mut self) -> Result<(), VaultError> {
        if !self.state.is_unlocked() {
            return Err(VaultError::Locked);
        }

        // Persist collectives before locking.
        self.persist_collectives();

        // Zero all keys (SecureData drops zero memory).
        self.custodian.clear();

        // Close and clear manifest.
        self.manifest.close();
        self.manifest.clear_cache();

        // Clear collectives.
        self.collectives.clear();

        // Mark locked.
        self.state.lock();

        Ok(())
    }

    // --- Manifest operations ---

    /// Register a .idea entry in the manifest.
    pub fn register_idea(&mut self, entry: ManifestEntry) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.manifest.upsert(entry)
    }

    /// Remove a .idea entry from the manifest.
    pub fn unregister_idea(&mut self, id: &Uuid) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.manifest.remove(id)
    }

    /// Get a manifest entry by ID.
    pub fn get_idea(&self, id: &Uuid) -> Result<Option<&ManifestEntry>, VaultError> {
        self.guard_unlocked()?;
        Ok(self.manifest.get(id))
    }

    /// Get a manifest entry by relative path.
    pub fn get_idea_by_path(&self, path: &str) -> Result<Option<&ManifestEntry>, VaultError> {
        self.guard_unlocked()?;
        Ok(self.manifest.get_by_path(path))
    }

    /// List ideas matching a filter.
    pub fn list_ideas(&self, filter: &IdeaFilter) -> Result<Vec<&ManifestEntry>, VaultError> {
        self.guard_unlocked()?;
        Ok(self.manifest.list_filtered(filter))
    }

    /// List ideas in a folder (path prefix match).
    pub fn list_ideas_in_folder(&self, folder: &str) -> Result<Vec<&ManifestEntry>, VaultError> {
        self.guard_unlocked()?;
        Ok(self.manifest.list_in_folder(folder))
    }

    /// Number of registered ideas.
    pub fn idea_count(&self) -> Result<usize, VaultError> {
        self.guard_unlocked()?;
        Ok(self.manifest.count())
    }

    // --- Search ---

    /// Search ideas by text query via FTS5.
    ///
    /// Returns matching ideas ordered by relevance (best first).
    /// Uses `limit` as the maximum number of results.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, VaultError> {
        self.guard_unlocked()?;
        self.manifest.search(query, limit)
    }

    /// Index an idea's content for full-text search.
    ///
    /// Call this when registering or updating an idea.
    /// If the idea is already indexed, the existing entry is replaced.
    pub fn index_idea_for_search(
        &self,
        idea_id: &Uuid,
        title: &str,
        content_text: &str,
        tags: &[String],
    ) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.manifest.index_idea(idea_id, title, content_text, tags)
    }

    /// Remove an idea from the search index.
    pub fn remove_from_search_index(&self, idea_id: &Uuid) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.manifest.remove_from_search(idea_id)
    }

    /// Rebuild the entire search index from the manifest table.
    ///
    /// Clears the FTS5 table and re-indexes every manifest entry that
    /// has a title. Returns the number of entries indexed.
    pub fn rebuild_search_index(&self) -> Result<usize, VaultError> {
        self.guard_unlocked()?;
        self.manifest.rebuild_search_index()
    }

    // --- Encryption ---

    /// Encrypt data using the content key for a specific idea.
    pub fn encrypt_for_idea(&mut self, data: &[u8], idea_id: &Uuid) -> Result<Vec<u8>, VaultError> {
        self.guard_unlocked()?;
        self.custodian.encrypt_for_idea(data, idea_id)
    }

    /// Decrypt data using the content key for a specific idea.
    pub fn decrypt_for_idea(&mut self, data: &[u8], idea_id: &Uuid) -> Result<Vec<u8>, VaultError> {
        self.guard_unlocked()?;
        self.custodian.decrypt_for_idea(data, idea_id)
    }

    /// Get the content key for a specific idea (as SecureData).
    pub fn content_key(&mut self, idea_id: &Uuid) -> Result<SecureData, VaultError> {
        self.guard_unlocked()?;
        self.custodian.content_key(idea_id)
    }

    /// Get the vocabulary seed for Babel obfuscation.
    pub fn vocabulary_seed(&self) -> Result<SecureData, VaultError> {
        self.guard_unlocked()?;
        self.custodian.vocabulary_seed()
    }

    /// Get the soul encryption key for Crown's soul.json at-rest encryption.
    ///
    /// Derived from the master key via HKDF with domain salt `"omnidea-soul-v1"`.
    pub fn soul_key(&self) -> Result<SecureData, VaultError> {
        self.guard_unlocked()?;
        self.custodian.soul_key()
    }

    // --- Collectives ---

    /// Create a new collective. Generates a random 256-bit key.
    pub fn create_collective(
        &mut self,
        name: String,
        owner_public_key: String,
    ) -> Result<&Collective, VaultError> {
        self.guard_unlocked()?;

        let collective = Collective::create(name, owner_public_key);
        let id = collective.id;

        // Generate and store a random 256-bit collective key.
        let key = SecureData::random(32)?;
        self.custodian.store_collective_key(id, key);
        self.collectives.insert(id, collective);

        Ok(self.collectives.get(&id).expect("just inserted"))
    }

    /// Join an existing collective (with a key received externally).
    pub fn join_collective(
        &mut self,
        id: Uuid,
        name: String,
        key: SecureData,
        our_role: CollectiveRole,
    ) -> Result<(), VaultError> {
        self.guard_unlocked()?;

        let collective = Collective {
            id,
            name,
            created_at: Utc::now(),
            members: vec![],
            our_role,
        };

        self.custodian.store_collective_key(id, key);
        self.collectives.insert(id, collective);
        Ok(())
    }

    /// Leave a collective. Removes the key from memory.
    pub fn leave_collective(&mut self, id: &Uuid) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.collectives.remove(id);
        self.custodian.remove_collective_key(id);
        Ok(())
    }

    /// List all collectives.
    pub fn list_collectives(&self) -> Result<Vec<&Collective>, VaultError> {
        self.guard_unlocked()?;
        Ok(self.collectives.values().collect())
    }

    /// Get a collective's encryption key.
    pub fn collective_key(&self, id: &Uuid) -> Result<&SecureData, VaultError> {
        self.guard_unlocked()?;
        self.custodian.collective_key(id)
    }

    /// Add a member to a collective. Requires Admin or higher.
    pub fn collective_add_member(
        &mut self,
        collective_id: &Uuid,
        public_key: String,
        role: CollectiveRole,
    ) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        let collective = self
            .collectives
            .get_mut(collective_id)
            .ok_or(VaultError::CollectiveNotFound(*collective_id))?;
        collective.add_member(public_key, role)
    }

    /// Remove a member from a collective by public key. Requires Owner.
    pub fn collective_remove_member(
        &mut self,
        collective_id: &Uuid,
        public_key: &str,
    ) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        let collective = self
            .collectives
            .get_mut(collective_id)
            .ok_or(VaultError::CollectiveNotFound(*collective_id))?;
        collective.remove_member(public_key)
    }

    /// Check if a public key is a member of a collective.
    pub fn collective_is_member(
        &self,
        collective_id: &Uuid,
        public_key: &str,
    ) -> Result<bool, VaultError> {
        self.guard_unlocked()?;
        let collective = self
            .collectives
            .get(collective_id)
            .ok_or(VaultError::CollectiveNotFound(*collective_id))?;
        Ok(collective.is_member(public_key))
    }

    /// Get a member's role in a collective. Returns None if not a member.
    pub fn collective_member_role(
        &self,
        collective_id: &Uuid,
        public_key: &str,
    ) -> Result<Option<CollectiveRole>, VaultError> {
        self.guard_unlocked()?;
        let collective = self
            .collectives
            .get(collective_id)
            .ok_or(VaultError::CollectiveNotFound(*collective_id))?;
        Ok(collective.member_role(public_key))
    }

    // --- Module state ---

    /// Save a module state entry.
    pub fn save_module_state(
        &self,
        module_id: &str,
        state_key: &str,
        data: &str,
    ) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.manifest.save_module_state(module_id, state_key, data)
    }

    /// Load a module state entry.
    pub fn load_module_state(
        &self,
        module_id: &str,
        state_key: &str,
    ) -> Result<Option<String>, VaultError> {
        self.guard_unlocked()?;
        self.manifest.load_module_state(module_id, state_key)
    }

    /// Delete a module state entry.
    pub fn delete_module_state(
        &self,
        module_id: &str,
        state_key: &str,
    ) -> Result<(), VaultError> {
        self.guard_unlocked()?;
        self.manifest.delete_module_state(module_id, state_key)
    }

    /// List all state keys for a module.
    pub fn list_module_state_keys(&self, module_id: &str) -> Result<Vec<String>, VaultError> {
        self.guard_unlocked()?;
        self.manifest.list_module_state_keys(module_id)
    }

    // --- Path resolution ---

    /// Get the vault root path.
    pub fn root_path(&self) -> Result<&std::path::Path, VaultError> {
        self.state.root_path()
    }

    /// Get the personal ideas directory path.
    pub fn personal_path(&self) -> Result<PathBuf, VaultError> {
        self.state.personal_path()
    }

    /// Get the collectives directory path.
    pub fn collectives_path(&self) -> Result<PathBuf, VaultError> {
        self.state.collectives_path()
    }

    /// Resolve a relative path within the vault root.
    pub fn resolve_path(&self, relative: &str) -> Result<PathBuf, VaultError> {
        self.state.resolve_path(relative)
    }

    // --- Internal ---

    fn guard_unlocked(&self) -> Result<(), VaultError> {
        if !self.state.is_unlocked() {
            return Err(VaultError::Locked);
        }
        Ok(())
    }

    /// Persist collectives to module state (called before lock).
    fn persist_collectives(&self) {
        if !self.manifest.is_open() {
            return;
        }
        if let Ok(json) = serde_json::to_string(&self.collectives) {
            let _ = self.manifest.save_module_state("vault", "collectives", &json);
        }
    }

    /// Load collectives from module state (called after unlock).
    fn load_collectives(&mut self) {
        if let Ok(Some(json)) = self.manifest.load_module_state("vault", "collectives") {
            if let Ok(collectives) = serde_json::from_str::<HashMap<Uuid, Collective>>(&json) {
                self.collectives = collectives;
            }
        }
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PASSWORD: &str = "test-vault-password-123";

    fn temp_vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::new();
        (dir, vault)
    }

    #[test]
    fn starts_locked() {
        let vault = Vault::new();
        assert!(!vault.is_unlocked());
    }

    #[test]
    fn unlock_and_lock() {
        let (dir, mut vault) = temp_vault();
        vault.unlock(TEST_PASSWORD, dir.path().to_path_buf()).unwrap();
        assert!(vault.is_unlocked());

        vault.lock().unwrap();
        assert!(!vault.is_unlocked());
    }

    #[test]
    fn double_unlock_fails() {
        let (dir, mut vault) = temp_vault();
        vault.unlock(TEST_PASSWORD, dir.path().to_path_buf()).unwrap();
        let result = vault.unlock(TEST_PASSWORD, dir.path().to_path_buf());
        assert!(matches!(result, Err(VaultError::AlreadyUnlocked)));
    }

    #[test]
    fn operations_fail_when_locked() {
        let vault = Vault::new();
        assert!(matches!(vault.get_idea(&Uuid::new_v4()), Err(VaultError::Locked)));
        assert!(matches!(vault.list_collectives(), Err(VaultError::Locked)));
        assert!(matches!(vault.vocabulary_seed(), Err(VaultError::Locked)));
    }

    #[test]
    fn creates_vault_directory_structure() {
        let (dir, mut vault) = temp_vault();
        vault.unlock(TEST_PASSWORD, dir.path().to_path_buf()).unwrap();

        assert!(dir.path().join(".vault").exists());
        assert!(dir.path().join(".vault/config.json").exists());
        assert!(dir.path().join(".vault/manifest.db").exists());
    }

    #[test]
    fn config_persists_across_sessions() {
        let dir = tempfile::tempdir().unwrap();

        // First session.
        {
            let mut vault = Vault::new();
            vault.unlock(TEST_PASSWORD, dir.path().to_path_buf()).unwrap();
            vault.lock().unwrap();
        }

        // Config should have salt and manifest_key_id.
        let config = VaultConfig::load_or_create(&dir.path().join(".vault/config.json")).unwrap();
        assert!(config.salt.is_some());
        assert!(config.manifest_key_id.is_some());
    }
}
