use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::CrownError;
use crate::preferences::Preferences;
use crate::profile::Profile;
use crate::social::SocialGraph;

// ---------------------------------------------------------------------------
// Trait — Crown defines interface, caller implements
// ---------------------------------------------------------------------------

/// Trait for encrypting/decrypting Soul data at rest.
///
/// Crown defines this trait but doesn't implement it — the caller
/// injects an implementation (typically backed by Sentinal).
/// This keeps Crown zero-dep on Sentinal while ensuring Soul data
/// is always encrypted before hitting disk.
pub trait SoulEncryptor: Send + Sync {
    /// Encrypt plaintext soul data. Returns ciphertext.
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String>;
    /// Decrypt ciphertext soul data. Returns plaintext.
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String>;
}

/// Your digital identity — profile, preferences, and social graph.
///
/// Soul persists to `{path}/soul.json`. The caller provides the path;
/// Soul does not know about Vault's directory layout.
///
/// When an encryptor is provided, soul data is encrypted before writing
/// to disk and decrypted after reading. Without an encryptor, Soul
/// persists as plaintext JSON (backward compatible).
pub struct Soul {
    profile: Profile,
    preferences: Preferences,
    social_graph: SocialGraph,
    path: Option<PathBuf>,
    dirty: bool,
    /// Optional encryptor for at-rest encryption. When present, `save()`
    /// encrypts before writing and `load()` decrypts after reading.
    encryptor: Option<Box<dyn SoulEncryptor>>,
}

impl std::fmt::Debug for Soul {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Soul")
            .field("profile", &self.profile)
            .field("preferences", &self.preferences)
            .field("social_graph", &self.social_graph)
            .field("path", &self.path)
            .field("dirty", &self.dirty)
            .field("encryptor", &self.encryptor.as_ref().map(|_| "<SoulEncryptor>"))
            .finish()
    }
}

/// Serialization format for soul.json.
#[derive(Serialize, Deserialize)]
struct SoulStorage {
    version: u32,
    profile: Profile,
    preferences: Preferences,
    social_graph: SocialGraph,
}

const SOUL_FILE: &str = "soul.json";
const SOUL_VERSION: u32 = 1;

impl Soul {
    /// Create a new soul with defaults (empty profile, default preferences,
    /// empty social graph). Not yet persisted. No encryption.
    pub fn new() -> Self {
        Self {
            profile: Profile::empty(),
            preferences: Preferences::default(),
            social_graph: SocialGraph::empty(),
            path: None,
            dirty: false,
            encryptor: None,
        }
    }

    // -- Persistence --

    /// Load a soul from an existing directory.
    ///
    /// Reads `{path}/soul.json`. If an encryptor is provided, decrypts
    /// after reading. For backward compatibility, if decryption fails
    /// the raw bytes are tried as plaintext JSON (handles migration
    /// from unencrypted to encrypted format).
    pub fn load(
        path: &Path,
        encryptor: Option<Box<dyn SoulEncryptor>>,
    ) -> Result<Self, CrownError> {
        let file_path = path.join(SOUL_FILE);
        let data = std::fs::read(&file_path).map_err(|e| CrownError::LoadFailed {
            path: file_path.clone(),
            reason: e.to_string(),
        })?;

        let storage: SoulStorage = match &encryptor {
            Some(enc) => {
                // Try decrypt -> parse. If that fails at any stage,
                // fall back to parsing the raw bytes as plaintext JSON.
                // This handles migration from unencrypted to encrypted.
                let decrypted_result = enc
                    .decrypt(&data)
                    .ok()
                    .and_then(|plaintext| serde_json::from_slice(&plaintext).ok());

                match decrypted_result {
                    Some(storage) => storage,
                    None => {
                        // Fallback: try raw data as plaintext JSON.
                        serde_json::from_slice(&data)
                            .map_err(|e| CrownError::SoulCorrupted(e.to_string()))?
                    }
                }
            }
            None => serde_json::from_slice(&data)
                .map_err(|e| CrownError::SoulCorrupted(e.to_string()))?,
        };

        Ok(Self {
            profile: storage.profile,
            preferences: storage.preferences,
            social_graph: storage.social_graph,
            path: Some(path.to_path_buf()),
            dirty: false,
            encryptor,
        })
    }

    /// Create a new soul at the given path. Creates the directory and
    /// writes `soul.json` with defaults. If an encryptor is provided,
    /// the initial write is encrypted.
    pub fn create(
        path: &Path,
        encryptor: Option<Box<dyn SoulEncryptor>>,
    ) -> Result<Self, CrownError> {
        std::fs::create_dir_all(path).map_err(|e| CrownError::SaveFailed {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

        let mut soul = Self::new();
        soul.path = Some(path.to_path_buf());
        soul.encryptor = encryptor;
        soul.save()?;
        Ok(soul)
    }

    /// Save the soul to disk. Requires a path (from `load` or `create`).
    ///
    /// If the soul was created/loaded with an encryptor, data is
    /// encrypted before writing. Plaintext never leaves memory.
    pub fn save(&mut self) -> Result<(), CrownError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| CrownError::SaveFailed {
                path: PathBuf::from("<no path>"),
                reason: "soul has no path set".into(),
            })?
            .clone();

        let storage = SoulStorage {
            version: SOUL_VERSION,
            profile: self.profile.clone(),
            preferences: self.preferences.clone(),
            social_graph: self.social_graph.clone(),
        };

        let json_bytes =
            serde_json::to_vec_pretty(&storage).map_err(CrownError::Serialization)?;

        let disk_bytes = match &self.encryptor {
            Some(enc) => enc.encrypt(&json_bytes).map_err(|e| CrownError::SaveFailed {
                path: path.clone(),
                reason: format!("encryption failed: {e}"),
            })?,
            None => json_bytes,
        };

        let file_path = path.join(SOUL_FILE);
        std::fs::write(&file_path, &disk_bytes).map_err(|e| CrownError::SaveFailed {
            path: file_path,
            reason: e.to_string(),
        })?;

        self.dirty = false;
        Ok(())
    }

    /// The path this soul is stored at, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Whether unsaved changes exist.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // -- Profile --

    /// The current profile (display name, bio, avatar, etc.).
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Update the profile. Sets `updated_at` and marks dirty.
    pub fn update_profile(&mut self, mut profile: Profile) {
        profile.updated_at = Utc::now();
        self.profile = profile;
        self.dirty = true;
    }

    // -- Preferences --

    /// The current preferences (theme, language, privacy, notifications).
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    /// Replace the preferences. Marks dirty.
    pub fn update_preferences(&mut self, prefs: Preferences) {
        self.preferences = prefs;
        self.dirty = true;
    }

    // -- Social Graph --

    /// The current social graph (following, followers, blocked, muted, trusted, lists).
    pub fn social_graph(&self) -> &SocialGraph {
        &self.social_graph
    }

    /// Replace the entire social graph. Marks dirty. Prefer the shortcut
    /// methods (`follow`, `block`, etc.) for single-connection changes.
    pub fn update_social_graph(&mut self, graph: SocialGraph) {
        self.social_graph = graph;
        self.dirty = true;
    }

    // -- Social shortcuts --

    /// Follow someone. Marks dirty.
    pub fn follow(&mut self, crown_id: &str) {
        self.social_graph.follow(crown_id);
        self.dirty = true;
    }

    /// Unfollow someone. Marks dirty.
    pub fn unfollow(&mut self, crown_id: &str) {
        self.social_graph.unfollow(crown_id);
        self.dirty = true;
    }

    /// Block someone. Also removes them from following. Marks dirty.
    pub fn block(&mut self, crown_id: &str) {
        self.social_graph.block(crown_id);
        self.dirty = true;
    }

    /// Unblock someone. Does not re-follow them. Marks dirty.
    pub fn unblock(&mut self, crown_id: &str) {
        self.social_graph.unblock(crown_id);
        self.dirty = true;
    }

    // -- Encryptor --

    /// Set or replace the soul encryptor. Future `save()` calls will
    /// encrypt data before writing.
    pub fn set_encryptor(&mut self, encryptor: Box<dyn SoulEncryptor>) {
        self.encryptor = Some(encryptor);
    }

    /// Whether this soul has an encryptor configured.
    pub fn has_encryptor(&self) -> bool {
        self.encryptor.is_some()
    }
}

impl Default for Soul {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Mock encryptor for testing
    // -----------------------------------------------------------------------

    /// Simple XOR-based mock encryptor for testing.
    struct MockSoulEncryptor {
        key: Vec<u8>,
    }

    impl MockSoulEncryptor {
        fn new(key: &[u8]) -> Self {
            Self { key: key.to_vec() }
        }
    }

    impl SoulEncryptor for MockSoulEncryptor {
        fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
            Ok(xor_bytes(data, &self.key))
        }

        fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
            Ok(xor_bytes(data, &self.key))
        }
    }

    /// An encryptor that always fails decryption.
    struct FailingSoulEncryptor;

    impl SoulEncryptor for FailingSoulEncryptor {
        fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
            // Encrypt works (returns garbage), but decrypt will fail.
            Ok(data.iter().map(|b| b.wrapping_add(1)).collect())
        }

        fn decrypt(&self, _data: &[u8]) -> Result<Vec<u8>, String> {
            Err("decryption failed".into())
        }
    }

    fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect()
    }

    // -----------------------------------------------------------------------
    // Unencrypted tests (backward compatibility)
    // -----------------------------------------------------------------------

    #[test]
    fn new_soul_defaults() {
        let soul = Soul::new();
        assert_eq!(soul.profile().language, "en");
        assert_eq!(soul.preferences().theme, crate::preferences::Theme::System);
        assert!(soul.social_graph().following.is_empty());
        assert!(!soul.is_dirty());
        assert!(soul.path().is_none());
        assert!(!soul.has_encryptor());
    }

    #[test]
    fn create_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("test.idea");

        let mut soul = Soul::create(&soul_path, None).unwrap();
        let mut profile = soul.profile().clone();
        profile.display_name = Some("Test User".into());
        soul.update_profile(profile);
        soul.save().unwrap();

        let loaded = Soul::load(&soul_path, None).unwrap();
        assert_eq!(
            loaded.profile().display_name.as_deref(),
            Some("Test User")
        );
    }

    #[test]
    fn save_writes_json() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("test.idea");
        let mut soul = Soul::create(&soul_path, None).unwrap();

        let mut profile = soul.profile().clone();
        profile.display_name = Some("Sam".into());
        soul.update_profile(profile);
        soul.save().unwrap();

        let raw = std::fs::read_to_string(soul_path.join("soul.json")).unwrap();
        assert!(raw.contains("Sam"));
        assert!(raw.contains("\"version\""));
    }

    #[test]
    fn dirty_tracking() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("test.idea");
        let mut soul = Soul::create(&soul_path, None).unwrap();

        // Just created and saved — not dirty.
        assert!(!soul.is_dirty());

        // Modify profile — dirty.
        let profile = soul.profile().clone();
        soul.update_profile(profile);
        assert!(soul.is_dirty());

        // Save — clean.
        soul.save().unwrap();
        assert!(!soul.is_dirty());
    }

    #[test]
    fn social_shortcut_follow() {
        let mut soul = Soul::new();
        soul.follow("cpub1alice");
        assert!(soul.social_graph().is_following("cpub1alice"));
        assert!(soul.is_dirty());
    }

    #[test]
    fn social_shortcut_block() {
        let mut soul = Soul::new();
        soul.follow("cpub1alice");
        soul.block("cpub1alice");
        assert!(!soul.social_graph().is_following("cpub1alice"));
        assert!(soul.social_graph().is_blocked("cpub1alice"));
    }

    #[test]
    fn load_nonexistent_fails() {
        let result = Soul::load(Path::new("/nonexistent/path"), None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrownError::LoadFailed { .. }));
    }

    // -----------------------------------------------------------------------
    // Encrypted tests
    // -----------------------------------------------------------------------

    #[test]
    fn encrypted_create_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("encrypted.idea");
        let key = b"test-encryption-key-32-bytes!!!!";

        // Create with encryption.
        let mut soul = Soul::create(
            &soul_path,
            Some(Box::new(MockSoulEncryptor::new(key))),
        )
        .unwrap();
        let mut profile = soul.profile().clone();
        profile.display_name = Some("Encrypted User".into());
        soul.update_profile(profile);
        soul.save().unwrap();

        // Verify the file on disk is NOT plaintext JSON.
        let raw = std::fs::read(soul_path.join("soul.json")).unwrap();
        let as_str = String::from_utf8_lossy(&raw);
        assert!(
            !as_str.contains("Encrypted User"),
            "soul.json should be encrypted, not plaintext"
        );

        // Load with encryption — should succeed.
        let loaded = Soul::load(
            &soul_path,
            Some(Box::new(MockSoulEncryptor::new(key))),
        )
        .unwrap();
        assert_eq!(
            loaded.profile().display_name.as_deref(),
            Some("Encrypted User")
        );
        assert!(loaded.has_encryptor());
    }

    #[test]
    fn encrypted_save_then_load_without_encryptor_fails() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("encrypted2.idea");
        let key = b"test-encryption-key-32-bytes!!!!";

        // Create encrypted.
        Soul::create(
            &soul_path,
            Some(Box::new(MockSoulEncryptor::new(key))),
        )
        .unwrap();

        // Try to load without encryptor — should fail (encrypted data isn't valid JSON).
        let result = Soul::load(&soul_path, None);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), CrownError::SoulCorrupted(_)),
            "loading encrypted soul without encryptor should produce SoulCorrupted"
        );
    }

    #[test]
    fn migration_plaintext_to_encrypted() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("migrate.idea");
        let key = b"migration-key-exactly-32-bytes!!";

        // Create unencrypted.
        let mut soul = Soul::create(&soul_path, None).unwrap();
        let mut profile = soul.profile().clone();
        profile.display_name = Some("Migrating User".into());
        soul.update_profile(profile);
        soul.save().unwrap();

        // Verify it's plaintext.
        let raw = std::fs::read_to_string(soul_path.join("soul.json")).unwrap();
        assert!(raw.contains("Migrating User"));

        // Load WITH encryptor — should fall back to plaintext successfully.
        let mut loaded = Soul::load(
            &soul_path,
            Some(Box::new(MockSoulEncryptor::new(key))),
        )
        .unwrap();
        assert_eq!(
            loaded.profile().display_name.as_deref(),
            Some("Migrating User")
        );

        // Save again — now encrypted.
        loaded.save().unwrap();

        // Verify file is now encrypted (not plaintext).
        let raw_after = std::fs::read(soul_path.join("soul.json")).unwrap();
        let as_str = String::from_utf8_lossy(&raw_after);
        assert!(
            !as_str.contains("Migrating User"),
            "after re-save with encryptor, soul.json should be encrypted"
        );
    }

    #[test]
    fn set_encryptor_on_existing_soul() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("set_enc.idea");
        let key = b"set-encryptor-key-32-bytes!!!!!!";

        // Create without encryption.
        let mut soul = Soul::create(&soul_path, None).unwrap();
        assert!(!soul.has_encryptor());

        // Attach encryptor.
        soul.set_encryptor(Box::new(MockSoulEncryptor::new(key)));
        assert!(soul.has_encryptor());

        // Save — should now be encrypted.
        let mut profile = soul.profile().clone();
        profile.display_name = Some("Now Encrypted".into());
        soul.update_profile(profile);
        soul.save().unwrap();

        let raw = std::fs::read(soul_path.join("soul.json")).unwrap();
        let as_str = String::from_utf8_lossy(&raw);
        assert!(!as_str.contains("Now Encrypted"));
    }

    #[test]
    fn encrypted_wrong_key_falls_back_to_plaintext_if_valid() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("wrongkey.idea");

        // Create unencrypted (plaintext JSON on disk).
        Soul::create(&soul_path, None).unwrap();

        // Load with a wrong-key encryptor — decryption will fail,
        // but fallback to plaintext should work.
        let loaded = Soul::load(
            &soul_path,
            Some(Box::new(FailingSoulEncryptor)),
        )
        .unwrap();
        assert_eq!(loaded.profile().language, "en");
    }

    #[test]
    fn encrypted_wrong_key_on_encrypted_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("wrongkey2.idea");
        let key = b"correct-key-exactly-32-bytes!!!!";

        // Create encrypted.
        Soul::create(
            &soul_path,
            Some(Box::new(MockSoulEncryptor::new(key))),
        )
        .unwrap();

        // Load with failing encryptor — decryption fails, fallback to
        // plaintext also fails (data is encrypted garbage, not JSON).
        let result = Soul::load(
            &soul_path,
            Some(Box::new(FailingSoulEncryptor)),
        );
        assert!(result.is_err());
    }
}
