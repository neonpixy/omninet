//! Local cache — personal index of previously seen results.
//!
//! Instant results for repeat queries. Grows from your usage.
//! Encrypted in Vault (caller's responsibility — Zeitgeist just
//! provides the data structures).

use std::collections::HashMap;

use magical_index::SearchResult;
use serde::{Deserialize, Serialize};

/// Configuration for the local cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum cached results per query (default: 20).
    pub max_results_per_query: usize,
    /// Maximum number of cached queries (default: 500).
    /// Oldest queries evicted when full (LRU by last_accessed).
    pub max_queries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_results_per_query: 20,
            max_queries: 500,
        }
    }
}

/// A cached query with its results and metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedQuery {
    /// The normalized query text.
    pub query: String,
    /// Cached results.
    pub results: Vec<SearchResult>,
    /// When this entry was created (Unix timestamp).
    pub created_at: i64,
    /// When this entry was last accessed (Unix timestamp).
    pub last_accessed: i64,
    /// How many times this query has been made.
    pub hit_count: u64,
}

/// Personal result cache. Stores recent query results for instant replay.
///
/// Not thread-safe — wrap in a Mutex if shared across threads.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalCache {
    /// Cached queries, keyed by normalized query text.
    entries: HashMap<String, CachedQuery>,
    /// Configuration.
    config: CacheConfig,
}

impl LocalCache {
    /// Create a new empty cache with default config.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            config: CacheConfig::default(),
        }
    }

    /// Create a cache with custom config.
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            entries: HashMap::new(),
            config,
        }
    }

    /// Look up cached results for a query. Returns None if not cached.
    ///
    /// Updates last_accessed and hit_count on cache hit.
    pub fn get(&mut self, query: &str, now: i64) -> Option<&CachedQuery> {
        let key = Self::normalize(query);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_accessed = now;
            entry.hit_count += 1;
            // Re-borrow as immutable.
            return self.entries.get(&key);
        }
        None
    }

    /// Store results for a query. Evicts oldest entry if at capacity.
    pub fn put(&mut self, query: &str, results: Vec<SearchResult>, now: i64) {
        let key = Self::normalize(query);

        // Evict if at capacity (remove least recently accessed).
        if !self.entries.contains_key(&key) && self.entries.len() >= self.config.max_queries {
            self.evict_oldest();
        }

        let mut truncated = results;
        truncated.truncate(self.config.max_results_per_query);

        self.entries.insert(
            key.clone(),
            CachedQuery {
                query: key,
                results: truncated,
                created_at: now,
                last_accessed: now,
                hit_count: 1,
            },
        );
    }

    /// Number of cached queries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Remove a specific query from the cache. Returns true if it existed.
    pub fn remove(&mut self, query: &str) -> bool {
        self.entries.remove(&Self::normalize(query)).is_some()
    }

    /// Snapshot the cache for persistence.
    pub fn snapshot(&self) -> CacheSnapshot {
        CacheSnapshot {
            entries: self.entries.values().cloned().collect(),
            config: self.config.clone(),
        }
    }

    /// Restore from a snapshot.
    pub fn from_snapshot(snapshot: CacheSnapshot) -> Self {
        let mut entries = HashMap::new();
        for entry in snapshot.entries {
            entries.insert(entry.query.clone(), entry);
        }
        Self {
            entries,
            config: snapshot.config,
        }
    }

    /// Normalize a query string for cache keying.
    /// Lowercase, trim whitespace, collapse multiple spaces.
    fn normalize(query: &str) -> String {
        let lower = query.to_lowercase();
        let parts: Vec<&str> = lower.split_whitespace().collect();
        parts.join(" ")
    }

    /// Evict the least recently accessed entry.
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self
            .entries
            .iter()
            .min_by_key(|(_, v)| v.last_accessed)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest_key);
        }
    }
}

/// Serializable snapshot of the cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheSnapshot {
    pub entries: Vec<CachedQuery>,
    pub config: CacheConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(event_id: &str, relevance: f64) -> SearchResult {
        SearchResult {
            event_id: event_id.into(),
            author: "a".into(),
            kind: 1,
            created_at: 1000,
            relevance,
            snippet: None,
            suggestions: vec![],
        }
    }

    #[test]
    fn empty_cache() {
        let mut cache = LocalCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(cache.get("test", 1000).is_none());
    }

    #[test]
    fn put_and_get() {
        let mut cache = LocalCache::new();
        let results = vec![make_result("e1", 0.9), make_result("e2", 0.5)];

        cache.put("woodworking", results, 1000);
        assert_eq!(cache.len(), 1);

        let cached = cache.get("woodworking", 1001).unwrap();
        assert_eq!(cached.results.len(), 2);
        assert_eq!(cached.hit_count, 2); // 1 from put + 1 from get
        assert_eq!(cached.last_accessed, 1001);
    }

    #[test]
    fn normalization() {
        let mut cache = LocalCache::new();
        cache.put("  Woodworking  Joints  ", vec![make_result("e1", 0.9)], 1000);

        // Different casing and spacing should hit the same entry.
        let cached = cache.get("woodworking joints", 1001);
        assert!(cached.is_some());

        let cached = cache.get("WOODWORKING   JOINTS", 1002);
        assert!(cached.is_some());
    }

    #[test]
    fn eviction_at_capacity() {
        let config = CacheConfig {
            max_queries: 2,
            max_results_per_query: 10,
        };
        let mut cache = LocalCache::with_config(config);

        cache.put("query1", vec![make_result("e1", 0.9)], 1000);
        cache.put("query2", vec![make_result("e2", 0.8)], 2000);
        assert_eq!(cache.len(), 2);

        // Access query2 to make query1 the oldest.
        cache.get("query2", 2500);

        // Adding a third should evict query1 (least recently accessed).
        cache.put("query3", vec![make_result("e3", 0.7)], 3000);
        assert_eq!(cache.len(), 2);
        assert!(cache.get("query1", 3001).is_none());
        assert!(cache.get("query2", 3002).is_some());
        assert!(cache.get("query3", 3003).is_some());
    }

    #[test]
    fn results_truncated_to_max() {
        let config = CacheConfig {
            max_results_per_query: 3,
            max_queries: 100,
        };
        let mut cache = LocalCache::with_config(config);

        let results: Vec<SearchResult> = (0..10)
            .map(|i| make_result(&format!("e{i}"), 0.5))
            .collect();
        cache.put("big query", results, 1000);

        let cached = cache.get("big query", 1001).unwrap();
        assert_eq!(cached.results.len(), 3);
    }

    #[test]
    fn clear() {
        let mut cache = LocalCache::new();
        cache.put("q1", vec![], 1000);
        cache.put("q2", vec![], 2000);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn remove() {
        let mut cache = LocalCache::new();
        cache.put("q1", vec![], 1000);
        assert_eq!(cache.len(), 1);

        assert!(cache.remove("q1"));
        assert!(cache.is_empty());
        assert!(!cache.remove("q1")); // Already removed.
    }

    #[test]
    fn snapshot_and_restore() {
        let mut cache = LocalCache::new();
        cache.put("woodworking", vec![make_result("e1", 0.9)], 1000);
        cache.put("art", vec![make_result("e2", 0.8)], 2000);

        let snap = cache.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let loaded: CacheSnapshot = serde_json::from_str(&json).unwrap();
        let mut restored = LocalCache::from_snapshot(loaded);

        assert_eq!(restored.len(), 2);
        assert!(restored.get("woodworking", 3000).is_some());
        assert!(restored.get("art", 3000).is_some());
    }

    #[test]
    fn update_existing_query() {
        let mut cache = LocalCache::new();
        cache.put("test", vec![make_result("old", 0.5)], 1000);
        cache.put("test", vec![make_result("new", 0.9)], 2000);

        assert_eq!(cache.len(), 1);
        let cached = cache.get("test", 2001).unwrap();
        assert_eq!(cached.results[0].event_id, "new");
        assert_eq!(cached.created_at, 2000); // New entry replaces old.
    }

    #[test]
    fn hit_count_increments() {
        let mut cache = LocalCache::new();
        cache.put("test", vec![], 1000);

        cache.get("test", 1001);
        cache.get("test", 1002);
        cache.get("test", 1003);

        let cached = cache.get("test", 1004).unwrap();
        assert_eq!(cached.hit_count, 5); // 1 (put) + 4 (gets)
    }

    #[test]
    fn config_serde() {
        let config = CacheConfig {
            max_results_per_query: 42,
            max_queries: 100,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: CacheConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.max_results_per_query, 42);
        assert_eq!(loaded.max_queries, 100);
    }
}
