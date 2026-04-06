use uuid::Uuid;

use crate::babel::Babel;
use crate::cache::TranslationCache;
use crate::detection;
use crate::error::LingoError;
use crate::provider::TranslationProvider;
use crate::types::{CacheStatistics, TranslatedText};

/// Universal Translator — orchestrates Babel + platform translation.
///
/// # Inbound (reading)
///
/// ```text
/// Stored text → Babel.decode() → original language → provider.translate(source → target) → display
/// ```
///
/// # Outbound (writing)
///
/// ```text
/// User text → Babel.encode() → store with source language tag
/// ```
///
/// No canonical English. Content is stored in its original language.
/// Translation happens on read, from source to reader's language.
/// If no provider is registered, text is displayed in its original
/// language (after Babel decoding if applicable).
pub struct UniversalTranslator {
    babel: Option<Babel>,
    provider: Option<Box<dyn TranslationProvider>>,
    cache: TranslationCache,
}

impl UniversalTranslator {
    /// Create a new translator with no Babel and no provider.
    pub fn new() -> Self {
        Self {
            babel: None,
            provider: None,
            cache: TranslationCache::new(1000),
        }
    }

    /// Configure Babel for semantic text obfuscation.
    pub fn with_babel(mut self, vocabulary_seed: &[u8]) -> Self {
        self.babel = Some(Babel::new(vocabulary_seed));
        self
    }

    /// Register a platform translation provider.
    pub fn with_provider(mut self, provider: Box<dyn TranslationProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the cache capacity.
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache = TranslationCache::new(size);
        self
    }

    /// Translate text for display to a reader.
    ///
    /// Flow: cache check → Babel decode → provider translate → cache store.
    ///
    /// - If the cache has a hit, returns immediately.
    /// - If Babel is configured, decodes the text first.
    /// - If a provider is available and source ≠ target, translates.
    /// - If no provider, returns the decoded text in its original language.
    pub fn translate_for_display(
        &mut self,
        text: &str,
        source_language: &str,
        target_language: &str,
        content_id: &Uuid,
    ) -> Result<TranslatedText, LingoError> {
        // Step 1: cache check.
        if let Some(cached) = self.cache.get(content_id, target_language) {
            return Ok(cached);
        }

        // Step 2: resolve source language (needed for Babel decode).
        let source = if source_language.is_empty() {
            // Try decoding without language awareness first to detect.
            let raw_decoded = match &self.babel {
                Some(babel) => babel.decode(text),
                None => text.to_string(),
            };
            detection::detect_language(&raw_decoded).unwrap_or_else(|| "en".into())
        } else {
            source_language.to_string()
        };

        // Step 3: Babel decode with language-aware rejoin.
        let decoded = match &self.babel {
            Some(babel) => babel.decode_for_language(text, &source),
            None => text.to_string(),
        };

        // Step 4: skip translation if source == target.
        if source == target_language {
            let result = TranslatedText {
                text: decoded.clone(),
                original: text.into(),
                source_language: source,
                target_language: target_language.into(),
                from_cache: false,
            };
            self.cache
                .set(*content_id, target_language.into(), result.clone());
            return Ok(result);
        }

        // Step 5: translate via provider (if available).
        let translated = match &self.provider {
            Some(provider) => provider.translate(&decoded, &source, target_language)?,
            None => {
                // No provider — return decoded text in original language.
                decoded.clone()
            }
        };

        let result = TranslatedText {
            text: translated,
            original: text.into(),
            source_language: source,
            target_language: target_language.into(),
            from_cache: false,
        };

        self.cache
            .set(*content_id, target_language.into(), result.clone());
        Ok(result)
    }

    /// Prepare text for storage.
    ///
    /// Encodes the text with Babel (if configured). No forced translation
    /// to English — the text is stored in its original language. The caller
    /// should store the `source_language` tag alongside the encoded text.
    pub fn prepare_for_storage(
        &self,
        text: &str,
        source_language: &str,
    ) -> Result<StoredText, LingoError> {
        let encoded = match &self.babel {
            Some(babel) => babel.encode(text),
            None => text.to_string(),
        };

        let babel_encoded = self.babel.is_some();

        Ok(StoredText {
            text: encoded,
            source_language: source_language.into(),
            babel_encoded,
        })
    }

    /// Detect the language of text using the provider (if available)
    /// or the built-in heuristic.
    pub fn detect_language(&self, text: &str) -> Option<String> {
        if let Some(provider) = &self.provider {
            if let Some(lang) = provider.detect_language(text) {
                return Some(lang);
            }
        }
        detection::detect_language(text)
    }

    /// Get cache performance statistics.
    pub fn cache_statistics(&self) -> CacheStatistics {
        self.cache.statistics()
    }

    /// Clear the translation cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Invalidate cached translations for a specific content ID.
    pub fn invalidate_cache(&mut self, content_id: &Uuid) {
        self.cache.invalidate(content_id);
    }
}

impl Default for UniversalTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of preparing text for storage.
#[derive(Debug, Clone)]
pub struct StoredText {
    /// The text to store (Babel-encoded if Babel is configured).
    pub text: String,
    /// BCP 47 language code of the original text.
    pub source_language: String,
    /// Whether Babel encoding was applied.
    pub babel_encoded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::MockTranslationProvider;

    const TEST_SEED: &[u8] = b"omnidea-translator-test-seed-32b";

    #[test]
    fn translate_without_babel_or_provider() {
        let mut translator = UniversalTranslator::new();
        let id = Uuid::new_v4();
        let result = translator
            .translate_for_display("hello world", "en", "en", &id)
            .unwrap();
        assert_eq!(result.text, "hello world");
        assert!(result.is_original());
    }

    #[test]
    fn translate_with_babel_only() {
        let mut translator = UniversalTranslator::new().with_babel(TEST_SEED);
        let id = Uuid::new_v4();

        // Encode then decode through the translator.
        let stored = translator.prepare_for_storage("hello world", "en").unwrap();
        assert!(stored.babel_encoded);
        assert_ne!(stored.text, "hello world");

        let result = translator
            .translate_for_display(&stored.text, "en", "en", &id)
            .unwrap();
        assert_eq!(result.text, "hello world");
    }

    #[test]
    fn translate_with_mock_provider() {
        let mut provider = MockTranslationProvider::new();
        provider.add_translation("hello world", "en", "fr", "bonjour le monde");

        let mut translator = UniversalTranslator::new()
            .with_provider(Box::new(provider));
        let id = Uuid::new_v4();

        let result = translator
            .translate_for_display("hello world", "en", "fr", &id)
            .unwrap();
        assert_eq!(result.text, "bonjour le monde");
        assert_eq!(result.source_language, "en");
        assert_eq!(result.target_language, "fr");
    }

    #[test]
    fn translate_caches_result() {
        let mut provider = MockTranslationProvider::new();
        provider.add_translation("hello", "en", "fr", "bonjour");

        let mut translator = UniversalTranslator::new()
            .with_provider(Box::new(provider));
        let id = Uuid::new_v4();

        // First call — miss.
        let result1 = translator
            .translate_for_display("hello", "en", "fr", &id)
            .unwrap();
        assert!(!result1.from_cache);

        // Second call — hit.
        let result2 = translator
            .translate_for_display("hello", "en", "fr", &id)
            .unwrap();
        assert!(result2.from_cache);
        assert_eq!(result2.text, "bonjour");
    }

    #[test]
    fn prepare_for_storage_without_babel() {
        let translator = UniversalTranslator::new();
        let stored = translator.prepare_for_storage("hello", "en").unwrap();
        assert_eq!(stored.text, "hello");
        assert!(!stored.babel_encoded);
    }

    #[test]
    fn prepare_for_storage_with_babel() {
        let translator = UniversalTranslator::new().with_babel(TEST_SEED);
        let stored = translator.prepare_for_storage("hello", "en").unwrap();
        assert_ne!(stored.text, "hello");
        assert!(stored.babel_encoded);
        assert_eq!(stored.source_language, "en");
    }

    #[test]
    fn omnilingual_babel_roundtrip() {
        // Simulates a multilingual group: same Babel seed, different languages.
        let mut translator = UniversalTranslator::new().with_babel(TEST_SEED);

        // Alice writes in English.
        let stored_en = translator.prepare_for_storage("hello world", "en").unwrap();
        assert!(stored_en.babel_encoded);

        // Bob reads Alice's English content.
        let id_en = Uuid::new_v4();
        let result_en = translator
            .translate_for_display(&stored_en.text, "en", "en", &id_en)
            .unwrap();
        assert_eq!(result_en.text, "hello world");

        // Kenji writes in Japanese (common tokens: の に は を た).
        let stored_ja = translator.prepare_for_storage("のにはをた", "ja").unwrap();
        assert!(stored_ja.babel_encoded);

        // Maria reads Kenji's Japanese content — decoded with correct CJK rejoin.
        let id_ja = Uuid::new_v4();
        let result_ja = translator
            .translate_for_display(&stored_ja.text, "ja", "ja", &id_ja)
            .unwrap();
        // Should have no spaces — CJK characters rejoin without separator.
        assert_eq!(result_ja.text, "のにはをた");
    }

    #[test]
    fn builder_pattern() {
        let mut provider = MockTranslationProvider::new();
        provider.add_translation("test", "en", "fr", "essai");

        let translator = UniversalTranslator::new()
            .with_babel(TEST_SEED)
            .with_provider(Box::new(provider))
            .with_cache_size(500);

        assert_eq!(translator.cache.statistics().entry_count, 0);
    }
}
