use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::GlobeError;

/// A shared SQLCipher-encrypted database connection for relay storage.
///
/// One database file holds everything: events, assets, gospel snapshots.
/// Encrypted at rest via SQLCipher when a storage key is provided.
/// In-memory mode (no file, no encryption) for tests.
#[derive(Clone, Debug)]
pub struct RelayDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl RelayDatabase {
    /// Open an in-memory database (for tests — no persistence, no encryption).
    pub fn in_memory() -> Self {
        let conn = Connection::open_in_memory().expect("open in-memory SQLite");
        Self::init_schema(&conn);
        Self {
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    /// Open a persistent encrypted database.
    ///
    /// - `path`: filesystem path for the database file (e.g., `data_dir/relay.db`)
    /// - `key`: 32-byte SQLCipher encryption key (derived from Crown via Sentinal HKDF)
    pub fn open(path: &Path, key: &[u8]) -> Result<Self, GlobeError> {
        let conn = Connection::open(path).map_err(|e| GlobeError::StorageError {
            reason: format!("failed to open database: {e}"),
        })?;

        // Apply SQLCipher encryption key.
        let hex_key = hex::encode(key);
        conn.pragma_update(None, "key", format!("x'{hex_key}'"))
            .map_err(|e| GlobeError::StorageError {
                reason: format!("failed to set encryption key: {e}"),
            })?;

        // WAL mode for better write performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| GlobeError::StorageError {
                reason: format!("failed to set WAL mode: {e}"),
            })?;

        Self::init_schema(&conn);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open a persistent encrypted database, auto-generating an encryption key if needed.
    ///
    /// When the caller provides a `storage_key` (e.g., derived from Crown via Sentinal
    /// HKDF), that key is used directly. When no key is provided, a random 32-byte key
    /// is generated on first boot and persisted in `{data_dir}/relay.key` for subsequent
    /// boots. This ensures nothing unencrypted on disk, even when the platform layer
    /// does not provide a key.
    pub fn open_auto(data_dir: &Path) -> Result<Self, GlobeError> {
        let key = Self::load_or_generate_key(data_dir)?;
        let db_path = data_dir.join("relay.db");
        Self::open(&db_path, &key)
    }

    /// Load an existing storage key or generate and persist a new one.
    ///
    /// The key file is `{data_dir}/relay.key` — 32 bytes of raw binary.
    /// File permissions are set to owner-only (0600) on Unix.
    fn load_or_generate_key(data_dir: &Path) -> Result<Vec<u8>, GlobeError> {
        let key_path = data_dir.join("relay.key");

        if key_path.exists() {
            let key = fs::read(&key_path).map_err(|e| GlobeError::StorageError {
                reason: format!("failed to read storage key from {}: {e}", key_path.display()),
            })?;

            if key.len() != 32 {
                return Err(GlobeError::StorageError {
                    reason: format!(
                        "storage key at {} has wrong length: expected 32, got {}",
                        key_path.display(),
                        key.len()
                    ),
                });
            }

            log::info!("loaded existing storage key from {}", key_path.display());
            Ok(key)
        } else {
            let key = Self::generate_random_key()?;
            fs::write(&key_path, &key).map_err(|e| GlobeError::StorageError {
                reason: format!("failed to write storage key to {}: {e}", key_path.display()),
            })?;

            // Restrict file permissions to owner-only on Unix.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::Permissions::from_mode(0o600);
                fs::set_permissions(&key_path, perms).map_err(|e| GlobeError::StorageError {
                    reason: format!(
                        "failed to set permissions on {}: {e}",
                        key_path.display()
                    ),
                })?;
            }

            log::info!("generated new storage key at {}", key_path.display());
            Ok(key)
        }
    }

    /// Generate 32 cryptographically random bytes for use as a storage key.
    fn generate_random_key() -> Result<Vec<u8>, GlobeError> {
        let mut key = vec![0u8; 32];
        getrandom::fill(&mut key).map_err(|e| GlobeError::StorageError {
            reason: format!("failed to generate random storage key: {e}"),
        })?;
        Ok(key)
    }

    /// Path to the auto-generated key file for a given data directory.
    pub fn key_path(data_dir: &Path) -> PathBuf {
        data_dir.join("relay.key")
    }

    /// Opens database WITHOUT encryption. Only for tests.
    #[cfg(test)]
    pub fn open_unencrypted(path: &Path) -> Result<Self, GlobeError> {
        let conn = Connection::open(path).map_err(|e| GlobeError::StorageError {
            reason: format!("failed to open database: {e}"),
        })?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| GlobeError::StorageError {
                reason: format!("failed to set WAL mode: {e}"),
            })?;

        Self::init_schema(&conn);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Get a lock on the underlying connection.
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn init_schema(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                author TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                kind INTEGER NOT NULL,
                tags TEXT NOT NULL,
                content TEXT NOT NULL,
                sig TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);
            CREATE INDEX IF NOT EXISTS idx_events_author ON events(author);
            CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);

            CREATE TABLE IF NOT EXISTS assets (
                hash TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                size INTEGER NOT NULL,
                inserted_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_assets_inserted ON assets(inserted_at);

            CREATE TABLE IF NOT EXISTS gospel_snapshot (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                snapshot_json TEXT NOT NULL
            );
            ",
        )
        .expect("init database schema");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_creates_tables() {
        let db = RelayDatabase::in_memory();
        let conn = db.lock();

        // Events table exists.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Assets table exists.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Gospel snapshot table exists.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM gospel_snapshot", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn open_unencrypted_persists() {
        let dir = std::env::temp_dir().join(format!("globe_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        // Open and write.
        {
            let db = RelayDatabase::open_unencrypted(&db_path).unwrap();
            let conn = db.lock();
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params!["id1", "author1", 1000i64, 1u32, "[]", "hello", "sig1"],
            )
            .unwrap();
        }

        // Reopen and read.
        {
            let db = RelayDatabase::open_unencrypted(&db_path).unwrap();
            let conn = db.lock();
            let content: String = conn
                .query_row("SELECT content FROM events WHERE id = ?", ["id1"], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(content, "hello");
        }

        // Cleanup.
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clone_shares_connection() {
        let db = RelayDatabase::in_memory();
        let db2 = db.clone();

        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params!["id1", "author1", 1000i64, 1u32, "[]", "test", "sig1"],
            )
            .unwrap();
        }

        let conn2 = db2.lock();
        let count: i64 = conn2
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn open_auto_generates_key_and_encrypts() {
        let dir =
            std::env::temp_dir().join(format!("globe_auto_key_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // First boot — generates key and opens encrypted database.
        {
            let db = RelayDatabase::open_auto(&dir).unwrap();
            let conn = db.lock();
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params!["id1", "author1", 1000i64, 1u32, "[]", "auto", "sig1"],
            )
            .unwrap();
        }

        // Key file should exist and be 32 bytes.
        let key_path = RelayDatabase::key_path(&dir);
        assert!(key_path.exists());
        let key = std::fs::read(&key_path).unwrap();
        assert_eq!(key.len(), 32);

        // Second boot — loads existing key, reads data back.
        {
            let db = RelayDatabase::open_auto(&dir).unwrap();
            let conn = db.lock();
            let content: String = conn
                .query_row("SELECT content FROM events WHERE id = ?", ["id1"], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(content, "auto");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_auto_rejects_wrong_key_length() {
        let dir =
            std::env::temp_dir().join(format!("globe_bad_key_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Write a key file with wrong length.
        let key_path = dir.join("relay.key");
        std::fs::write(&key_path, &[0u8; 16]).unwrap();

        let result = RelayDatabase::open_auto(&dir);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("wrong length"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_auto_key_is_deterministic_across_boots() {
        let dir =
            std::env::temp_dir().join(format!("globe_det_key_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // First boot.
        RelayDatabase::open_auto(&dir).unwrap();
        let key1 = std::fs::read(dir.join("relay.key")).unwrap();

        // Second boot — same key.
        RelayDatabase::open_auto(&dir).unwrap();
        let key2 = std::fs::read(dir.join("relay.key")).unwrap();

        assert_eq!(key1, key2, "key should not change between boots");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_auto_encrypted_db_unreadable_without_key() {
        let dir =
            std::env::temp_dir().join(format!("globe_enc_verify_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Create an encrypted database with auto-generated key.
        {
            let db = RelayDatabase::open_auto(&dir).unwrap();
            let conn = db.lock();
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params!["id1", "a1", 1000i64, 1u32, "[]", "secret", "sig1"],
            )
            .unwrap();
        }

        // Try to open it with a wrong key — should fail.
        let wrong_key = vec![0xFFu8; 32];
        let db_path = dir.join("relay.db");
        let result = RelayDatabase::open(&db_path, &wrong_key);
        // SQLCipher with wrong key will fail on schema init or first query.
        // The open itself might succeed but first operation will fail.
        if let Ok(db) = result {
            let conn = db.lock();
            let query_result: Result<i64, _> =
                conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0));
            assert!(
                query_result.is_err(),
                "should not be able to read encrypted db with wrong key"
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn open_auto_key_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir =
            std::env::temp_dir().join(format!("globe_perms_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        RelayDatabase::open_auto(&dir).unwrap();

        let key_path = dir.join("relay.key");
        let perms = std::fs::metadata(&key_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "key file should be owner-only (0600)"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
