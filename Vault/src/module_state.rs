use chrono::Utc;
use rusqlite::params;

use crate::error::VaultError;
use crate::manifest::Manifest;

/// Generic JSON key-value persistence for other modules.
///
/// Stored in the `module_state` table of the encrypted manifest database.
/// Each module gets its own namespace (module_id), with arbitrary string
/// keys and JSON string values.
impl Manifest {
    /// Save a module state entry.
    pub fn save_module_state(
        &self,
        module_id: &str,
        state_key: &str,
        data: &str,
    ) -> Result<(), VaultError> {
        let conn = self.connection()?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO module_state (module_id, state_key, data, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![module_id, state_key, data, now],
        )
        .map_err(|e| VaultError::Database(format!("save_module_state: {e}")))?;
        Ok(())
    }

    /// Load a module state entry. Returns None if not found.
    pub fn load_module_state(
        &self,
        module_id: &str,
        state_key: &str,
    ) -> Result<Option<String>, VaultError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare("SELECT data FROM module_state WHERE module_id = ?1 AND state_key = ?2")
            .map_err(|e| VaultError::Database(format!("load_module_state prepare: {e}")))?;
        let result = stmt
            .query_row(params![module_id, state_key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|e| VaultError::Database(format!("load_module_state query: {e}")))?;
        Ok(result)
    }

    /// Delete a module state entry.
    pub fn delete_module_state(
        &self,
        module_id: &str,
        state_key: &str,
    ) -> Result<(), VaultError> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM module_state WHERE module_id = ?1 AND state_key = ?2",
            params![module_id, state_key],
        )
        .map_err(|e| VaultError::Database(format!("delete_module_state: {e}")))?;
        Ok(())
    }

    /// List all state keys for a module.
    pub fn list_module_state_keys(&self, module_id: &str) -> Result<Vec<String>, VaultError> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare("SELECT state_key FROM module_state WHERE module_id = ?1 ORDER BY state_key")
            .map_err(|e| VaultError::Database(format!("list_module_state_keys prepare: {e}")))?;
        let keys = stmt
            .query_map(params![module_id], |row| row.get::<_, String>(0))
            .map_err(|e| VaultError::Database(format!("list_module_state_keys query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| VaultError::Database(format!("list_module_state_keys collect: {e}")))?;
        Ok(keys)
    }
}

/// Extension trait import for rusqlite optional query results.
use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        vec![42u8; 32]
    }

    #[test]
    fn save_and_load_module_state() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        manifest
            .save_module_state("crown", "profile", r#"{"name":"Sam"}"#)
            .unwrap();

        let loaded = manifest.load_module_state("crown", "profile").unwrap();
        assert_eq!(loaded.as_deref(), Some(r#"{"name":"Sam"}"#));
    }

    #[test]
    fn load_missing_state_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        let loaded = manifest.load_module_state("crown", "nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn delete_module_state() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        manifest.save_module_state("crown", "key1", "data1").unwrap();
        manifest.delete_module_state("crown", "key1").unwrap();

        let loaded = manifest.load_module_state("crown", "key1").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn list_module_state_keys() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        manifest.save_module_state("crown", "alpha", "a").unwrap();
        manifest.save_module_state("crown", "beta", "b").unwrap();
        manifest.save_module_state("globe", "relay", "r").unwrap();

        let crown_keys = manifest.list_module_state_keys("crown").unwrap();
        assert_eq!(crown_keys, vec!["alpha", "beta"]);

        let globe_keys = manifest.list_module_state_keys("globe").unwrap();
        assert_eq!(globe_keys, vec!["relay"]);
    }
}
