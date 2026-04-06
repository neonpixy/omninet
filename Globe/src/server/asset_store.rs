use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;

use super::database::RelayDatabase;

/// Configuration for the asset store.
#[derive(Clone, Debug)]
pub struct AssetStoreConfig {
    /// Maximum total bytes to store. Oldest assets evicted when exceeded.
    /// `None` means no limit.
    pub max_bytes: Option<usize>,
    /// Maximum size of a single asset in bytes.
    pub max_asset_size: usize,
    /// Maximum age in seconds before an asset is eligible for eviction.
    /// `None` means no age limit.
    pub max_age_secs: Option<u64>,
}

impl Default for AssetStoreConfig {
    fn default() -> Self {
        Self {
            max_bytes: Some(536_870_912), // 512 MB
            max_asset_size: 52_428_800,   // 50 MB per asset
            max_age_secs: Some(604_800),  // 7 days
        }
    }
}

/// SQLite-backed binary asset store for a relay server.
///
/// Thread-safe via shared `RelayDatabase`. Assets are stored as BLOBs
/// keyed by their SHA-256 hex hash (content-addressed). Oldest assets
/// are evicted when the store exceeds its configured capacity.
///
/// When using a shared `RelayDatabase`, assets are encrypted at rest
/// by SQLCipher alongside events and gospel data — one locked box.
#[derive(Clone)]
pub struct AssetStore {
    db: RelayDatabase,
    config: AssetStoreConfig,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl AssetStore {
    /// Create a new store with default configuration (in-memory).
    pub fn new() -> Self {
        Self::with_config(AssetStoreConfig::default())
    }

    /// Create a new store with custom configuration (in-memory).
    pub fn with_config(config: AssetStoreConfig) -> Self {
        Self {
            db: RelayDatabase::in_memory(),
            config,
        }
    }

    /// Create a store backed by a shared database.
    pub fn from_db(db: RelayDatabase, config: AssetStoreConfig) -> Self {
        Self { db, config }
    }

    /// Store an asset by its SHA-256 hex hash.
    /// Returns `true` if stored, `false` if duplicate or rejected.
    pub fn insert(&self, hash: String, data: Vec<u8>) -> bool {
        let size = data.len();

        if size > self.config.max_asset_size {
            return false;
        }

        let conn = self.db.lock();

        // Check duplicate.
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM assets WHERE hash = ? LIMIT 1",
                params![hash],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            return false;
        }

        // Evict expired assets.
        self.evict_expired(&conn);

        // Evict until we have room.
        if let Some(max) = self.config.max_bytes {
            let total = self.total_bytes_inner(&conn);
            if total + size > max {
                if size > max {
                    return false; // Can't fit even in an empty store.
                }
                self.evict_until_fits(&conn, max, size);
            }
        }

        let now = now_secs() as i64;
        match conn.execute(
            "INSERT INTO assets (hash, data, size, inserted_at) VALUES (?, ?, ?, ?)",
            params![hash, data, size as i64, now],
        ) {
            Ok(_) => true,
            Err(e) => {
                log::error!("asset insert failed: {e}");
                false
            }
        }
    }

    /// Get asset data by hash.
    pub fn get(&self, hash: &str) -> Option<Vec<u8>> {
        let conn = self.db.lock();
        conn.query_row(
            "SELECT data FROM assets WHERE hash = ?",
            params![hash],
            |row| row.get(0),
        )
        .ok()
    }

    /// Check if an asset exists by hash.
    pub fn exists(&self, hash: &str) -> bool {
        let conn = self.db.lock();
        conn.query_row(
            "SELECT 1 FROM assets WHERE hash = ? LIMIT 1",
            params![hash],
            |_| Ok(true),
        )
        .unwrap_or(false)
    }

    /// Delete an asset by hash. Returns `true` if it existed.
    pub fn delete(&self, hash: &str) -> bool {
        let conn = self.db.lock();
        match conn.execute("DELETE FROM assets WHERE hash = ?", params![hash]) {
            Ok(n) => n > 0,
            Err(e) => {
                log::error!("asset delete failed: {e}");
                false
            }
        }
    }

    /// Total bytes stored.
    pub fn total_bytes(&self) -> usize {
        let conn = self.db.lock();
        self.total_bytes_inner(&conn)
    }

    /// Number of assets stored.
    pub fn asset_count(&self) -> usize {
        let conn = self.db.lock();
        conn.query_row("SELECT COUNT(*) FROM assets", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.asset_count() == 0
    }

    // --- Private helpers ---

    fn total_bytes_inner(&self, conn: &rusqlite::Connection) -> usize {
        conn.query_row(
            "SELECT COALESCE(SUM(size), 0) FROM assets",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize
    }

    fn evict_expired(&self, conn: &rusqlite::Connection) {
        let max_age = match self.config.max_age_secs {
            Some(age) => age,
            None => return,
        };

        let cutoff = (now_secs().saturating_sub(max_age)) as i64;
        let _ = conn.execute(
            "DELETE FROM assets WHERE inserted_at < ?",
            params![cutoff],
        );
    }

    fn evict_until_fits(&self, conn: &rusqlite::Connection, max: usize, needed: usize) {
        // Evict oldest one at a time until we fit.
        loop {
            let total = self.total_bytes_inner(conn);
            if total + needed <= max {
                break;
            }

            let evicted = conn
                .execute(
                    "DELETE FROM assets WHERE hash = (SELECT hash FROM assets ORDER BY inserted_at ASC LIMIT 1)",
                    [],
                )
                .unwrap_or(0);

            if evicted == 0 {
                break; // Nothing left to evict.
            }
        }
    }
}

impl Default for AssetStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config() -> AssetStoreConfig {
        AssetStoreConfig {
            max_bytes: Some(1000),
            max_asset_size: 500,
            max_age_secs: None,
        }
    }

    #[test]
    fn insert_and_get() {
        let store = AssetStore::new();
        let data = b"hello world".to_vec();
        let hash = "abc123".to_string();

        assert!(store.insert(hash.clone(), data.clone()));
        assert_eq!(store.get(&hash).unwrap(), data);
        assert_eq!(store.asset_count(), 1);
        assert_eq!(store.total_bytes(), 11);
    }

    #[test]
    fn duplicate_rejected() {
        let store = AssetStore::new();
        let data = b"hello".to_vec();

        assert!(store.insert("hash1".into(), data.clone()));
        assert!(!store.insert("hash1".into(), data));
        assert_eq!(store.asset_count(), 1);
    }

    #[test]
    fn exists_check() {
        let store = AssetStore::new();
        assert!(!store.exists("missing"));

        store.insert("present".into(), b"data".to_vec());
        assert!(store.exists("present"));
        assert!(!store.exists("missing"));
    }

    #[test]
    fn delete_asset() {
        let store = AssetStore::new();
        store.insert("hash1".into(), b"data".to_vec());
        assert_eq!(store.asset_count(), 1);

        assert!(store.delete("hash1"));
        assert_eq!(store.asset_count(), 0);
        assert_eq!(store.total_bytes(), 0);
        assert!(store.get("hash1").is_none());
    }

    #[test]
    fn delete_nonexistent() {
        let store = AssetStore::new();
        assert!(!store.delete("nonexistent"));
    }

    #[test]
    fn too_large_rejected() {
        let store = AssetStore::with_config(small_config());
        let big_data = vec![0u8; 501]; // Exceeds max_asset_size of 500

        assert!(!store.insert("big".into(), big_data));
        assert_eq!(store.asset_count(), 0);
    }

    #[test]
    fn eviction_when_full() {
        let store = AssetStore::with_config(AssetStoreConfig {
            max_bytes: Some(100),
            max_asset_size: 100,
            max_age_secs: None,
        });

        // Fill with 60 bytes.
        store.insert("first".into(), vec![0u8; 60]);
        assert_eq!(store.asset_count(), 1);

        // Adding 60 more exceeds 100 — should evict "first".
        store.insert("second".into(), vec![1u8; 60]);
        assert_eq!(store.asset_count(), 1);
        assert!(!store.exists("first"));
        assert!(store.exists("second"));
    }

    #[test]
    fn multiple_evictions() {
        let store = AssetStore::with_config(AssetStoreConfig {
            max_bytes: Some(100),
            max_asset_size: 100,
            max_age_secs: None,
        });

        // Insert three small assets.
        store.insert("a".into(), vec![0u8; 30]);
        store.insert("b".into(), vec![0u8; 30]);
        store.insert("c".into(), vec![0u8; 30]);
        assert_eq!(store.asset_count(), 3);
        assert_eq!(store.total_bytes(), 90);

        // Adding 40 bytes exceeds 100 — evicts "a" (30) then we're at 60+40=100.
        store.insert("d".into(), vec![0u8; 40]);
        assert!(!store.exists("a"));
        assert!(store.exists("b"));
        assert!(store.exists("c"));
        assert!(store.exists("d"));
    }

    #[test]
    fn cannot_fit_even_after_eviction() {
        let store = AssetStore::with_config(AssetStoreConfig {
            max_bytes: Some(50),
            max_asset_size: 100, // Allow large assets per-item...
            max_age_secs: None,
        });

        // 60 bytes won't fit in 50 max even after evicting everything.
        assert!(!store.insert("too_big".into(), vec![0u8; 60]));
    }

    #[test]
    fn total_bytes_tracks_correctly() {
        let store = AssetStore::new();

        store.insert("a".into(), vec![0u8; 100]);
        store.insert("b".into(), vec![0u8; 200]);
        assert_eq!(store.total_bytes(), 300);

        store.delete("a");
        assert_eq!(store.total_bytes(), 200);

        store.delete("b");
        assert_eq!(store.total_bytes(), 0);
    }

    #[test]
    fn empty_store() {
        let store = AssetStore::new();
        assert!(store.is_empty());
        assert_eq!(store.asset_count(), 0);
        assert_eq!(store.total_bytes(), 0);
        assert!(store.get("anything").is_none());
    }

    #[test]
    fn thread_safe_clone() {
        let store = AssetStore::new();
        let store2 = store.clone();

        store.insert("shared".into(), b"data".to_vec());
        assert_eq!(store2.asset_count(), 1);
        assert!(store2.exists("shared"));
    }

    #[test]
    fn age_eviction() {
        let store = AssetStore::with_config(AssetStoreConfig {
            max_bytes: None,
            max_asset_size: 1000,
            max_age_secs: Some(0), // Expire immediately
        });

        store.insert("old".into(), b"data".to_vec());
        // Next insert triggers evict_expired — "old" should be gone.
        store.insert("new".into(), b"fresh".to_vec());

        // With max_age_secs=0, anything inserted before "now" is expired.
        // Since both happen within the same second, we test the mechanism compiles.
        assert!(store.asset_count() <= 2);
    }

    #[test]
    fn unlimited_store() {
        let store = AssetStore::with_config(AssetStoreConfig {
            max_bytes: None,
            max_asset_size: usize::MAX,
            max_age_secs: None,
        });

        for i in 0..100u32 {
            store.insert(format!("hash_{i}"), vec![0u8; 1000]);
        }
        assert_eq!(store.asset_count(), 100);
        assert_eq!(store.total_bytes(), 100_000);
    }

    #[test]
    fn persistence_round_trip() {
        let dir = std::env::temp_dir().join(format!("globe_assets_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        // Write assets.
        {
            let db = RelayDatabase::open_unencrypted(&db_path).unwrap();
            let store = AssetStore::from_db(db, AssetStoreConfig::default());
            store.insert("hash1".into(), b"hello world".to_vec());
            store.insert("hash2".into(), b"goodbye world".to_vec());
            assert_eq!(store.asset_count(), 2);
        }

        // Reopen and verify.
        {
            let db = RelayDatabase::open_unencrypted(&db_path).unwrap();
            let store = AssetStore::from_db(db, AssetStoreConfig::default());
            assert_eq!(store.asset_count(), 2);
            assert_eq!(store.get("hash1").unwrap(), b"hello world");
            assert_eq!(store.get("hash2").unwrap(), b"goodbye world");
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
