use std::collections::HashMap;
use uuid::Uuid;

use super::render::{RenderMode, RenderSpec};

/// LRU render cache key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    digit_id: Uuid,
    mode: RenderMode,
}

/// LRU render cache. Stores RenderSpecs to avoid re-rendering unchanged digits.
pub struct RenderCache {
    entries: HashMap<CacheKey, RenderSpec>,
    access_order: Vec<CacheKey>,
    max_size: usize,
    hits: u64,
    misses: u64,
}

impl RenderCache {
    pub const DEFAULT_MAX_SIZE: usize = 200;

    /// Create a cache with the default capacity (200 entries).
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
            max_size: Self::DEFAULT_MAX_SIZE,
            hits: 0,
            misses: 0,
        }
    }

    /// Create a cache with a custom capacity.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
            max_size,
            hits: 0,
            misses: 0,
        }
    }

    /// Get a cached spec, updating LRU order.
    pub fn get(&mut self, digit_id: Uuid, mode: RenderMode) -> Option<&RenderSpec> {
        let key = CacheKey { digit_id, mode };
        if self.entries.contains_key(&key) {
            self.hits += 1;
            // Move to end of access order (most recent)
            self.access_order.retain(|k| k != &key);
            self.access_order.push(key.clone());
            self.entries.get(&key)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a spec, evicting LRU if at capacity.
    pub fn insert(&mut self, spec: RenderSpec) {
        let key = CacheKey {
            digit_id: spec.digit_id,
            mode: spec.mode,
        };
        // Remove existing entry from access order if present
        self.access_order.retain(|k| k != &key);

        // Evict LRU if at capacity
        while self.entries.len() >= self.max_size && !self.access_order.is_empty() {
            let evict = self.access_order.remove(0);
            self.entries.remove(&evict);
        }

        self.access_order.push(key.clone());
        self.entries.insert(key, spec);
    }

    /// Remove all cached specs for a digit (all modes).
    pub fn invalidate(&mut self, digit_id: Uuid) {
        self.entries.retain(|k, _| k.digit_id != digit_id);
        self.access_order.retain(|k| k.digit_id != digit_id);
    }

    /// Clear the entire cache.
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    /// Number of entries currently in the cache.
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Total cache hits since creation.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Total cache misses since creation.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Hit rate as a ratio (0.0-1.0). Returns 0.0 if no lookups have occurred.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::accessibility::{AccessibilityRole, AccessibilitySpec};

    fn make_spec(digit_id: Uuid, mode: RenderMode) -> RenderSpec {
        RenderSpec::new(digit_id, "text", mode)
            .with_size(100.0, 20.0)
            .with_accessibility(AccessibilitySpec::new(AccessibilityRole::Custom("text".into()), "Test"))
    }

    #[test]
    fn insert_and_get() {
        let mut cache = RenderCache::new();
        let id = Uuid::new_v4();
        cache.insert(make_spec(id, RenderMode::Display));
        let spec = cache.get(id, RenderMode::Display).unwrap();
        assert_eq!(spec.digit_id, id);
    }

    #[test]
    fn cache_miss_returns_none() {
        let mut cache = RenderCache::new();
        assert!(cache.get(Uuid::new_v4(), RenderMode::Display).is_none());
    }

    #[test]
    fn invalidate_removes_all_modes() {
        let mut cache = RenderCache::new();
        let id = Uuid::new_v4();
        cache.insert(make_spec(id, RenderMode::Display));
        cache.insert(make_spec(id, RenderMode::Thumbnail));
        assert_eq!(cache.size(), 2);
        cache.invalidate(id);
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn invalidate_all_clears() {
        let mut cache = RenderCache::new();
        cache.insert(make_spec(Uuid::new_v4(), RenderMode::Display));
        cache.insert(make_spec(Uuid::new_v4(), RenderMode::Display));
        cache.invalidate_all();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn lru_eviction() {
        let mut cache = RenderCache::with_max_size(3);
        let ids: Vec<_> = (0..4).map(|_| Uuid::new_v4()).collect();
        for id in &ids {
            cache.insert(make_spec(*id, RenderMode::Display));
        }
        assert_eq!(cache.size(), 3);
        // First one should have been evicted
        assert!(cache.get(ids[0], RenderMode::Display).is_none());
        // Last three should be present
        assert!(cache.get(ids[1], RenderMode::Display).is_some());
        assert!(cache.get(ids[2], RenderMode::Display).is_some());
        assert!(cache.get(ids[3], RenderMode::Display).is_some());
    }

    #[test]
    fn hit_miss_counting() {
        let mut cache = RenderCache::new();
        let id = Uuid::new_v4();
        cache.get(id, RenderMode::Display); // miss
        cache.insert(make_spec(id, RenderMode::Display));
        cache.get(id, RenderMode::Display); // hit
        cache.get(id, RenderMode::Display); // hit
        assert_eq!(cache.hits(), 2);
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn hit_rate_calculation() {
        let mut cache = RenderCache::new();
        assert_eq!(cache.hit_rate(), 0.0); // no operations
        let id = Uuid::new_v4();
        cache.insert(make_spec(id, RenderMode::Display));
        cache.get(id, RenderMode::Display); // hit
        cache.get(Uuid::new_v4(), RenderMode::Display); // miss
        assert!((cache.hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn multiple_modes_cached_separately() {
        let mut cache = RenderCache::new();
        let id = Uuid::new_v4();
        cache.insert(make_spec(id, RenderMode::Display));
        cache.insert(make_spec(id, RenderMode::Thumbnail));
        assert_eq!(cache.size(), 2);
        assert!(cache.get(id, RenderMode::Display).is_some());
        assert!(cache.get(id, RenderMode::Thumbnail).is_some());
        assert!(cache.get(id, RenderMode::Editing).is_none());
    }
}
