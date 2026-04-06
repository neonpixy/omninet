use uuid::Uuid;

use crate::types::{CacheStatistics, TranslatedText};

/// LRU translation cache.
///
/// Caches translations keyed by `(content_id, target_language)` to avoid
/// re-translating unchanged content. Tracks hit/miss statistics for
/// monitoring performance.
pub struct TranslationCache {
    /// Cache entries keyed by (content_id, target_language).
    entries: Vec<CacheEntry>,
    max_entries: usize,
    hit_count: u64,
    miss_count: u64,
}

struct CacheEntry {
    content_id: Uuid,
    target_language: String,
    translation: TranslatedText,
}

impl TranslationCache {
    /// Create a new cache with the given maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries.min(1024)),
            max_entries,
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// Look up a cached translation.
    ///
    /// On hit, the entry is moved to the back (most recently used).
    pub fn get(&mut self, content_id: &Uuid, target_language: &str) -> Option<TranslatedText> {
        if let Some(pos) = self.entries.iter().position(|e| {
            e.content_id == *content_id && e.target_language == target_language
        }) {
            self.hit_count += 1;
            // Move to back (most recently used).
            let entry = self.entries.remove(pos);
            let mut translation = entry.translation;
            translation.from_cache = true;
            let result = translation.clone();
            self.entries.push(CacheEntry {
                content_id: entry.content_id,
                target_language: entry.target_language,
                translation,
            });
            Some(result)
        } else {
            self.miss_count += 1;
            None
        }
    }

    /// Store a translation in the cache.
    ///
    /// If the cache is full, the least recently used entry is evicted.
    pub fn set(
        &mut self,
        content_id: Uuid,
        target_language: String,
        translation: TranslatedText,
    ) {
        // Remove existing entry for this key, if any.
        self.entries.retain(|e| {
            !(e.content_id == content_id && e.target_language == target_language)
        });

        // Evict oldest if at capacity.
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }

        self.entries.push(CacheEntry {
            content_id,
            target_language,
            translation,
        });
    }

    /// Remove all cached translations for a specific content ID.
    pub fn invalidate(&mut self, content_id: &Uuid) {
        self.entries.retain(|e| e.content_id != *content_id);
    }

    /// Remove all cached translations for a specific target language.
    pub fn invalidate_language(&mut self, language: &str) {
        self.entries.retain(|e| e.target_language != language);
    }

    /// Clear the entire cache and reset statistics.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hit_count = 0;
        self.miss_count = 0;
    }

    /// Get cache performance statistics.
    pub fn statistics(&self) -> CacheStatistics {
        CacheStatistics {
            entry_count: self.entries.len(),
            hit_count: self.hit_count,
            miss_count: self.miss_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_translation(text: &str, source: &str, target: &str) -> TranslatedText {
        TranslatedText {
            text: text.into(),
            original: "original".into(),
            source_language: source.into(),
            target_language: target.into(),
            from_cache: false,
        }
    }

    #[test]
    fn cache_get_set_roundtrip() {
        let mut cache = TranslationCache::new(100);
        let id = Uuid::new_v4();
        let tt = make_translation("bonjour", "en", "fr");

        cache.set(id, "fr".into(), tt);
        let result = cache.get(&id, "fr");
        assert!(result.is_some());
        assert_eq!(result.unwrap().text, "bonjour");
    }

    #[test]
    fn cache_miss_returns_none() {
        let mut cache = TranslationCache::new(100);
        let id = Uuid::new_v4();
        assert!(cache.get(&id, "fr").is_none());
    }

    #[test]
    fn cache_hit_miss_tracking() {
        let mut cache = TranslationCache::new(100);
        let id = Uuid::new_v4();

        cache.get(&id, "fr"); // miss
        cache.set(id, "fr".into(), make_translation("bonjour", "en", "fr"));
        cache.get(&id, "fr"); // hit

        let stats = cache.statistics();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
        assert!((stats.hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn cache_lru_eviction() {
        let mut cache = TranslationCache::new(3);

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let id4 = Uuid::new_v4();

        cache.set(id1, "fr".into(), make_translation("un", "en", "fr"));
        cache.set(id2, "fr".into(), make_translation("deux", "en", "fr"));
        cache.set(id3, "fr".into(), make_translation("trois", "en", "fr"));

        // Cache is full. Adding id4 should evict id1 (oldest).
        cache.set(id4, "fr".into(), make_translation("quatre", "en", "fr"));

        assert!(cache.get(&id1, "fr").is_none());
        assert!(cache.get(&id4, "fr").is_some());
    }

    #[test]
    fn cache_invalidate_content_id() {
        let mut cache = TranslationCache::new(100);
        let id = Uuid::new_v4();

        cache.set(id, "fr".into(), make_translation("bonjour", "en", "fr"));
        cache.set(id, "ja".into(), make_translation("こんにちは", "en", "ja"));

        cache.invalidate(&id);
        assert!(cache.get(&id, "fr").is_none());
        assert!(cache.get(&id, "ja").is_none());
    }

    #[test]
    fn cache_invalidate_language() {
        let mut cache = TranslationCache::new(100);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        cache.set(id1, "fr".into(), make_translation("bonjour", "en", "fr"));
        cache.set(id2, "fr".into(), make_translation("salut", "en", "fr"));
        cache.set(id1, "ja".into(), make_translation("こんにちは", "en", "ja"));

        cache.invalidate_language("fr");
        assert!(cache.get(&id1, "fr").is_none());
        assert!(cache.get(&id2, "fr").is_none());
        assert!(cache.get(&id1, "ja").is_some());
    }

    #[test]
    fn cache_clear_resets_everything() {
        let mut cache = TranslationCache::new(100);
        let id = Uuid::new_v4();

        cache.set(id, "fr".into(), make_translation("bonjour", "en", "fr"));
        cache.get(&id, "fr"); // hit

        cache.clear();
        assert_eq!(cache.statistics().entry_count, 0);
        assert_eq!(cache.statistics().hit_count, 0);
        assert_eq!(cache.statistics().miss_count, 0);
    }
}
