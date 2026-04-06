use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The result of translating text for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatedText {
    /// The final text to display (translated or original).
    pub text: String,
    /// The original text before translation.
    pub original: String,
    /// BCP 47 language code of the source.
    pub source_language: String,
    /// BCP 47 language code of the target.
    pub target_language: String,
    /// Whether this result came from the cache.
    pub from_cache: bool,
}

impl TranslatedText {
    /// True if source and target are the same (no translation needed).
    pub fn is_original(&self) -> bool {
        self.source_language == self.target_language
    }
}

/// Information about an available language.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageInfo {
    /// BCP 47 language code (e.g., "en", "ja", "zh-Hans").
    pub code: String,
    /// Human-readable name (e.g., "English", "日本語").
    pub name: String,
    /// Whether translation to/from this language is currently available.
    pub is_available: bool,
}

/// A shared vocabulary for Babel decoding between parties.
///
/// Created by one user and encrypted to a recipient's public key.
/// Published to relays so the recipient can decode the sender's
/// Babel-encoded content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationKit {
    /// crown_id of the creator.
    pub from: String,
    /// crown_id of the recipient.
    pub to: String,
    /// When the kit was created.
    pub created: DateTime<Utc>,
    /// The shared vocabulary seed (32 bytes).
    pub vocabulary_seed: Vec<u8>,
    /// Optional Schnorr signature from the creator.
    pub signature: Option<String>,
}

impl TranslationKit {
    /// Create a new `TranslationKit` for sharing a vocabulary.
    pub fn new(from: String, to: String, vocabulary_seed: Vec<u8>) -> Self {
        Self {
            from,
            to,
            created: Utc::now(),
            vocabulary_seed,
            signature: None,
        }
    }
}

/// Cache performance statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    /// Number of entries in the cache.
    pub entry_count: usize,
    /// Total cache hits.
    pub hit_count: u64,
    /// Total cache misses.
    pub miss_count: u64,
}

impl CacheStatistics {
    /// Cache hit rate as a fraction (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_text_serde_roundtrip() {
        let tt = TranslatedText {
            text: "Bonjour le monde".into(),
            original: "Hello world".into(),
            source_language: "en".into(),
            target_language: "fr".into(),
            from_cache: false,
        };
        let json = serde_json::to_string(&tt).unwrap();
        let parsed: TranslatedText = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, tt.text);
        assert_eq!(parsed.source_language, "en");
        assert!(!parsed.from_cache);
    }

    #[test]
    fn translated_text_is_original() {
        let tt = TranslatedText {
            text: "Hello".into(),
            original: "Hello".into(),
            source_language: "en".into(),
            target_language: "en".into(),
            from_cache: false,
        };
        assert!(tt.is_original());
    }

    #[test]
    fn language_info_serde_roundtrip() {
        let info = LanguageInfo {
            code: "ja".into(),
            name: "日本語".into(),
            is_available: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: LanguageInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, info);
    }

    #[test]
    fn translation_kit_serde_roundtrip() {
        let kit = TranslationKit::new(
            "cpub1alice".into(),
            "cpub1bob".into(),
            vec![0x42; 32],
        );
        let json = serde_json::to_string(&kit).unwrap();
        let parsed: TranslationKit = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.from, "cpub1alice");
        assert_eq!(parsed.to, "cpub1bob");
        assert_eq!(parsed.vocabulary_seed.len(), 32);
        assert!(parsed.signature.is_none());
    }

    #[test]
    fn cache_statistics_hit_rate() {
        let stats = CacheStatistics {
            entry_count: 10,
            hit_count: 3,
            miss_count: 7,
        };
        assert!((stats.hit_rate() - 0.3).abs() < f64::EPSILON);

        let empty = CacheStatistics {
            entry_count: 0,
            hit_count: 0,
            miss_count: 0,
        };
        assert!((empty.hit_rate() - 0.0).abs() < f64::EPSILON);
    }
}
