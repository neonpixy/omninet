use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::entry::{IdeaFilter, ManifestEntry};
use crate::error::VaultError;

/// Schema for the encrypted manifest database.
const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS manifest (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    title TEXT,
    extended_type TEXT,
    creator TEXT NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    collective_id TEXT,
    header_cache TEXT
);
CREATE INDEX IF NOT EXISTS idx_manifest_path ON manifest(path);
CREATE INDEX IF NOT EXISTS idx_manifest_type ON manifest(extended_type);
CREATE INDEX IF NOT EXISTS idx_manifest_creator ON manifest(creator);
CREATE INDEX IF NOT EXISTS idx_manifest_modified ON manifest(modified_at);

CREATE TABLE IF NOT EXISTS module_state (
    module_id TEXT NOT NULL,
    state_key TEXT NOT NULL,
    data TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (module_id, state_key)
);

CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
    idea_id UNINDEXED,
    title,
    content,
    tags,
    tokenize='porter unicode61'
);
";

/// Encrypted manifest database (SQLCipher) with in-memory HashMap cache.
///
/// The manifest tracks all .idea files known to this vault.
/// Database is encrypted with a key derived from the master key.
/// All reads come from the in-memory cache; writes go to both
/// the database and the cache.
pub struct Manifest {
    /// SQLite connection (encrypted via SQLCipher).
    conn: Option<Connection>,
    /// In-memory cache: id -> entry.
    entries: HashMap<Uuid, ManifestEntry>,
    /// Path index: relative_path -> id.
    path_index: HashMap<String, Uuid>,
}

impl Manifest {
    /// Create a new manifest with no database connection and an empty cache.
    pub fn new() -> Self {
        Self {
            conn: None,
            entries: HashMap::new(),
            path_index: HashMap::new(),
        }
    }

    /// Returns true if the database connection is active.
    pub fn is_open(&self) -> bool {
        self.conn.is_some()
    }

    /// Open the manifest database with the given encryption key.
    ///
    /// Creates the database file and schema if they don't exist.
    /// Loads all entries into the in-memory cache.
    ///
    /// The key is used as a raw hex passphrase via `PRAGMA key = "x'...'"`.
    /// The `x''` prefix tells SQLCipher to use the bytes directly,
    /// bypassing its internal PBKDF2 (we already derived the key via HKDF).
    pub fn open(&mut self, db_path: &Path, encryption_key: &[u8]) -> Result<(), VaultError> {
        let conn = Connection::open(db_path)
            .map_err(|e| VaultError::Database(format!("open failed: {e}")))?;

        // Set SQLCipher raw key (hex-encoded).
        let hex_key = hex::encode(encryption_key);
        conn.pragma_update(None, "key", format!("x'{hex_key}'"))
            .map_err(|e| VaultError::Database(format!("PRAGMA key failed: {e}")))?;

        // Verify the key works (SQLCipher defers key validation).
        conn.execute_batch("SELECT count(*) FROM sqlite_master;")
            .map_err(|_| VaultError::WrongPassword)?;

        // Create schema if needed.
        conn.execute_batch(SCHEMA_SQL)
            .map_err(|e| VaultError::Database(format!("schema creation failed: {e}")))?;

        self.conn = Some(conn);
        self.load_cache()?;

        Ok(())
    }

    /// Close the manifest. Drops the connection but keeps the cache
    /// until `clear_cache()` is called.
    pub fn close(&mut self) {
        self.conn = None;
    }

    /// Clear the in-memory cache (called on lock).
    pub fn clear_cache(&mut self) {
        self.entries.clear();
        self.path_index.clear();
    }

    // --- CRUD ---

    /// Insert or update a manifest entry.
    pub fn upsert(&mut self, entry: ManifestEntry) -> Result<(), VaultError> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO manifest
             (id, path, title, extended_type, creator, created_at, modified_at, collective_id, header_cache)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id.to_string(),
                entry.path,
                entry.title,
                entry.extended_type,
                entry.creator,
                entry.created_at.to_rfc3339(),
                entry.modified_at.to_rfc3339(),
                entry.collective_id.map(|id| id.to_string()),
                entry.header_cache,
            ],
        )
        .map_err(|e| VaultError::Database(format!("upsert failed: {e}")))?;

        // Update cache: remove old path entry if the path changed.
        if let Some(old) = self.entries.get(&entry.id) {
            if old.path != entry.path {
                self.path_index.remove(&old.path);
            }
        }
        self.path_index.insert(entry.path.clone(), entry.id);
        self.entries.insert(entry.id, entry);

        Ok(())
    }

    /// Remove an entry by ID.
    pub fn remove(&mut self, id: &Uuid) -> Result<(), VaultError> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM manifest WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(|e| VaultError::Database(format!("remove failed: {e}")))?;

        if let Some(entry) = self.entries.remove(id) {
            self.path_index.remove(&entry.path);
        }
        Ok(())
    }

    /// Get an entry by ID (from cache).
    pub fn get(&self, id: &Uuid) -> Option<&ManifestEntry> {
        self.entries.get(id)
    }

    /// Get an entry by relative path (from cache).
    pub fn get_by_path(&self, path: &str) -> Option<&ManifestEntry> {
        self.path_index
            .get(path)
            .and_then(|id| self.entries.get(id))
    }

    // --- List / Query ---

    /// List all entries.
    pub fn list_all(&self) -> Vec<&ManifestEntry> {
        self.entries.values().collect()
    }

    /// List entries matching a filter (evaluated against cache).
    ///
    /// If the filter includes sorting or pagination parameters, those
    /// are applied after filtering.
    pub fn list_filtered(&self, filter: &IdeaFilter) -> Vec<&ManifestEntry> {
        let filtered: Vec<&ManifestEntry> = self.entries
            .values()
            .filter(|e| filter.matches(e))
            .collect();

        filter.apply_sort_and_paginate(filtered)
    }

    /// List entries in a folder (path prefix match).
    pub fn list_in_folder(&self, folder: &str) -> Vec<&ManifestEntry> {
        let normalized = if folder.ends_with('/') {
            folder.to_string()
        } else {
            format!("{folder}/")
        };
        self.entries
            .values()
            .filter(|e| e.path.starts_with(&normalized))
            .collect()
    }

    /// Number of entries.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Check if an entry exists by ID.
    pub fn contains(&self, id: &Uuid) -> bool {
        self.entries.contains_key(id)
    }

    /// Bulk insert/update entries in a single transaction.
    pub fn bulk_upsert(&mut self, entries: Vec<ManifestEntry>) -> Result<(), VaultError> {
        {
            let conn = self.connection()?;
            conn.execute_batch("BEGIN TRANSACTION;")
                .map_err(|e| VaultError::Database(format!("begin transaction: {e}")))?;

            for entry in &entries {
                conn.execute(
                    "INSERT OR REPLACE INTO manifest
                     (id, path, title, extended_type, creator, created_at, modified_at, collective_id, header_cache)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        entry.id.to_string(),
                        entry.path,
                        entry.title,
                        entry.extended_type,
                        entry.creator,
                        entry.created_at.to_rfc3339(),
                        entry.modified_at.to_rfc3339(),
                        entry.collective_id.map(|id| id.to_string()),
                        entry.header_cache,
                    ],
                )
                .map_err(|e| VaultError::Database(format!("bulk upsert: {e}")))?;
            }

            conn.execute_batch("COMMIT;")
                .map_err(|e| VaultError::Database(format!("commit: {e}")))?;
        }

        // Update cache.
        for entry in entries {
            self.path_index.insert(entry.path.clone(), entry.id);
            self.entries.insert(entry.id, entry);
        }

        Ok(())
    }

    // --- Search ---

    /// Search ideas by text query via FTS5.
    ///
    /// Delegates to [`VaultSearch::search`](crate::VaultSearch::search) using
    /// the manifest's own SQLCipher connection.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<crate::SearchHit>, VaultError> {
        let conn = self.connection()?;
        crate::VaultSearch::search(conn, query, limit)
    }

    /// Index an idea for full-text search.
    ///
    /// Delegates to [`VaultSearch::index_idea`](crate::VaultSearch::index_idea).
    pub fn index_idea(
        &self,
        idea_id: &Uuid,
        title: &str,
        content_text: &str,
        tags: &[String],
    ) -> Result<(), VaultError> {
        let conn = self.connection()?;
        crate::VaultSearch::index_idea(conn, idea_id, title, content_text, tags)
    }

    /// Remove an idea from the search index.
    ///
    /// Delegates to [`VaultSearch::remove_idea`](crate::VaultSearch::remove_idea).
    pub fn remove_from_search(&self, idea_id: &Uuid) -> Result<(), VaultError> {
        let conn = self.connection()?;
        crate::VaultSearch::remove_idea(conn, idea_id)
    }

    /// Rebuild the entire search index from the manifest table.
    ///
    /// Delegates to [`VaultSearch::rebuild_index`](crate::VaultSearch::rebuild_index).
    /// Returns the number of entries indexed.
    pub fn rebuild_search_index(&self) -> Result<usize, VaultError> {
        let conn = self.connection()?;
        crate::VaultSearch::rebuild_index(conn)
    }

    // --- Internal ---

    /// Get the SQLite connection. Errors if not open.
    pub(crate) fn connection(&self) -> Result<&Connection, VaultError> {
        self.conn.as_ref().ok_or(VaultError::Locked)
    }

    /// Load all entries from the database into the in-memory cache.
    fn load_cache(&mut self) -> Result<(), VaultError> {
        self.entries.clear();
        self.path_index.clear();

        // Collect all rows into a temp vec first to release the borrow on self.conn
        // before mutating self.entries and self.path_index.
        let loaded: Vec<ManifestEntry> = {
            let conn = self.connection()?;
            let mut stmt = conn
                .prepare("SELECT id, path, title, extended_type, creator, created_at, modified_at, collective_id, header_cache FROM manifest")
                .map_err(|e| VaultError::Database(format!("load_cache prepare: {e}")))?;

            let rows = stmt
                .query_map([], |row| {
                    let id_str: String = row.get(0)?;
                    let path: String = row.get(1)?;
                    let title: Option<String> = row.get(2)?;
                    let extended_type: Option<String> = row.get(3)?;
                    let creator: String = row.get(4)?;
                    let created_at_str: String = row.get(5)?;
                    let modified_at_str: String = row.get(6)?;
                    let collective_id_str: Option<String> = row.get(7)?;
                    let header_cache: Option<String> = row.get(8)?;

                    Ok((
                        id_str, path, title, extended_type, creator,
                        created_at_str, modified_at_str, collective_id_str, header_cache,
                    ))
                })
                .map_err(|e| VaultError::Database(format!("load_cache query: {e}")))?;

            let mut result = Vec::new();
            for row in rows {
                let (id_str, path, title, extended_type, creator, created_at_str, modified_at_str, collective_id_str, header_cache) =
                    row.map_err(|e| VaultError::Database(format!("load_cache row: {e}")))?;

                let id = Uuid::parse_str(&id_str)
                    .map_err(|e| VaultError::Database(format!("invalid UUID: {e}")))?;
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map_err(|e| VaultError::Database(format!("invalid created_at: {e}")))?
                    .with_timezone(&Utc);
                let modified_at = DateTime::parse_from_rfc3339(&modified_at_str)
                    .map_err(|e| VaultError::Database(format!("invalid modified_at: {e}")))?
                    .with_timezone(&Utc);
                let collective_id = collective_id_str
                    .as_deref()
                    .map(Uuid::parse_str)
                    .transpose()
                    .map_err(|e| VaultError::Database(format!("invalid collective UUID: {e}")))?;

                result.push(ManifestEntry {
                    id, path, title, extended_type, creator,
                    created_at, modified_at, collective_id, header_cache,
                });
            }
            result
        };

        // Now mutate the cache with no outstanding borrows.
        for entry in loaded {
            self.path_index.insert(entry.path.clone(), entry.id);
            self.entries.insert(entry.id, entry);
        }

        Ok(())
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        // A deterministic 32-byte key for tests.
        vec![42u8; 32]
    }

    fn make_entry(path: &str, creator: &str) -> ManifestEntry {
        ManifestEntry {
            id: Uuid::new_v4(),
            path: path.to_string(),
            title: Some("Test".to_string()),
            extended_type: Some("text".to_string()),
            creator: creator.to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            collective_id: None,
            header_cache: None,
        }
    }

    #[test]
    fn open_creates_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();
        assert!(manifest.is_open());
        assert_eq!(manifest.count(), 0);
    }

    #[test]
    fn upsert_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        let entry = make_entry("Personal/test.idea", "cpub1abc");
        let id = entry.id;
        manifest.upsert(entry).unwrap();

        assert_eq!(manifest.count(), 1);
        assert!(manifest.get(&id).is_some());
        assert_eq!(manifest.get(&id).unwrap().path, "Personal/test.idea");
    }

    #[test]
    fn get_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        let entry = make_entry("Personal/song.idea", "cpub1abc");
        manifest.upsert(entry).unwrap();

        assert!(manifest.get_by_path("Personal/song.idea").is_some());
        assert!(manifest.get_by_path("Personal/other.idea").is_none());
    }

    #[test]
    fn remove_entry() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        let entry = make_entry("Personal/test.idea", "cpub1abc");
        let id = entry.id;
        manifest.upsert(entry).unwrap();
        assert_eq!(manifest.count(), 1);

        manifest.remove(&id).unwrap();
        assert_eq!(manifest.count(), 0);
        assert!(manifest.get(&id).is_none());
    }

    #[test]
    fn list_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        manifest.upsert(make_entry("Personal/a.idea", "cpub1abc")).unwrap();
        manifest.upsert(make_entry("Personal/b.idea", "cpub1xyz")).unwrap();

        let filter = IdeaFilter::new().creator("cpub1abc");
        let results = manifest.list_filtered(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].creator, "cpub1abc");
    }

    #[test]
    fn list_in_folder() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        manifest.upsert(make_entry("Personal/a.idea", "cpub1abc")).unwrap();
        manifest.upsert(make_entry("Collectives/shared/b.idea", "cpub1xyz")).unwrap();

        let personal = manifest.list_in_folder("Personal");
        assert_eq!(personal.len(), 1);

        let collectives = manifest.list_in_folder("Collectives");
        assert_eq!(collectives.len(), 1);
    }

    #[test]
    fn persistence_across_close_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let key = test_key();

        let id = {
            let mut manifest = Manifest::new();
            manifest.open(&db_path, &key).unwrap();
            let entry = make_entry("Personal/persist.idea", "cpub1abc");
            let id = entry.id;
            manifest.upsert(entry).unwrap();
            manifest.close();
            id
        };

        // Reopen with same key — data should still be there.
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &key).unwrap();
        assert_eq!(manifest.count(), 1);
        assert!(manifest.get(&id).is_some());
    }

    #[test]
    fn wrong_key_fails() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");

        // Create with one key.
        {
            let mut manifest = Manifest::new();
            manifest.open(&db_path, &[1u8; 32]).unwrap();
            manifest.upsert(make_entry("test.idea", "cpub1")).unwrap();
            manifest.close();
        }

        // Try to open with a different key.
        let mut manifest = Manifest::new();
        let result = manifest.open(&db_path, &[2u8; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn bulk_upsert() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("manifest.db");
        let mut manifest = Manifest::new();
        manifest.open(&db_path, &test_key()).unwrap();

        let entries: Vec<ManifestEntry> = (0..50)
            .map(|i| make_entry(&format!("Personal/idea-{i}.idea"), "cpub1abc"))
            .collect();

        manifest.bulk_upsert(entries).unwrap();
        assert_eq!(manifest.count(), 50);
    }
}
