use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VaultError;

/// Configuration stored in `.vault/config.json`.
///
/// Persisted as cleartext JSON. Contains only non-secret metadata:
/// salt (needed for PBKDF2), manifest key ID (needed for HKDF),
/// owner identity, and version info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Config format version.
    pub version: String,
    /// When this vault was created.
    pub created_at: DateTime<Utc>,
    /// Last successful unlock timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_unlocked: Option<DateTime<Utc>>,
    /// Owner's public key (crown_id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_public_key: Option<String>,
    /// PBKDF2 salt (32 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salt: Option<Vec<u8>>,
    /// UUID for manifest key derivation (random per vault).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_key_id: Option<Uuid>,
}

impl VaultConfig {
    /// Create a fresh config for a new vault.
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            created_at: Utc::now(),
            last_unlocked: None,
            owner_public_key: None,
            salt: None,
            manifest_key_id: None,
        }
    }

    /// Load config from a JSON file, or create a new one if the file is missing.
    pub fn load_or_create(path: &Path) -> Result<Self, VaultError> {
        if path.exists() {
            let data = std::fs::read_to_string(path)?;
            let config: VaultConfig = serde_json::from_str(&data)
                .map_err(|e| VaultError::Config(format!("invalid config JSON: {e}")))?;
            Ok(config)
        } else {
            Ok(Self::new())
        }
    }

    /// Save config to a JSON file.
    ///
    /// Writes to a temporary file first, then renames for atomicity.
    pub fn save(&self, path: &Path) -> Result<(), VaultError> {
        let json = serde_json::to_string_pretty(self)?;
        // Write to temp file then rename for atomic write.
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json.as_bytes())?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_config_defaults() {
        let config = VaultConfig::new();
        assert_eq!(config.version, "1.0");
        assert!(config.salt.is_none());
        assert!(config.manifest_key_id.is_none());
        assert!(config.owner_public_key.is_none());
        assert!(config.last_unlocked.is_none());
    }

    #[test]
    fn config_serde_round_trip() {
        let mut config = VaultConfig::new();
        config.salt = Some(vec![1, 2, 3, 4]);
        config.manifest_key_id = Some(Uuid::new_v4());
        config.owner_public_key = Some("cpub1test".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let restored: VaultConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.version, config.version);
        assert_eq!(restored.salt, config.salt);
        assert_eq!(restored.manifest_key_id, config.manifest_key_id);
        assert_eq!(restored.owner_public_key, config.owner_public_key);
    }

    #[test]
    fn load_or_create_missing_file() {
        let path = std::path::PathBuf::from("/tmp/nonexistent_vault_config.json");
        let _ = std::fs::remove_file(&path);
        let config = VaultConfig::load_or_create(&path).unwrap();
        assert_eq!(config.version, "1.0");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let mut config = VaultConfig::new();
        config.salt = Some(vec![10, 20, 30]);
        config.manifest_key_id = Some(Uuid::new_v4());
        config.save(&path).unwrap();

        let loaded = VaultConfig::load_or_create(&path).unwrap();
        assert_eq!(loaded.salt, config.salt);
        assert_eq!(loaded.manifest_key_id, config.manifest_key_id);
    }
}
