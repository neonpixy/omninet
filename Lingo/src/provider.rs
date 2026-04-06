use crate::error::LingoError;
use crate::types::LanguageInfo;

/// Platform translation service abstraction.
///
/// Each platform implements this trait via Divinity FFI:
/// - **Apple**: Translation Framework (iOS 17.4+, macOS 14.4+)
/// - **Android**: ML Kit Translation API
/// - **Web**: Browser translation APIs or LibreTranslate
/// - **Desktop**: Platform-specific or no-op fallback
///
/// If no provider is registered, text is displayed in its original
/// language (after Babel decoding if applicable).
pub trait TranslationProvider: Send + Sync {
    /// Translate text from one language to another.
    ///
    /// Both `from` and `to` are BCP 47 language codes (e.g., "en", "ja").
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String, LingoError>;

    /// Detect the language of the given text.
    ///
    /// Returns a BCP 47 language code, or `None` if detection fails.
    /// Platform providers typically have better detection than the
    /// heuristic in `detection.rs`.
    fn detect_language(&self, text: &str) -> Option<String>;

    /// List all languages available for translation.
    fn available_languages(&self) -> Vec<LanguageInfo>;

    /// Check if a specific language is available for translation.
    fn is_available(&self, language: &str) -> bool;
}

/// Mock provider for testing. Maps (text, from, to) → translated text.
#[cfg(test)]
pub(crate) struct MockTranslationProvider {
    translations: std::collections::HashMap<(String, String, String), String>,
    languages: Vec<LanguageInfo>,
}

#[cfg(test)]
impl MockTranslationProvider {
    pub fn new() -> Self {
        Self {
            translations: std::collections::HashMap::new(),
            languages: vec![
                LanguageInfo {
                    code: "en".into(),
                    name: "English".into(),
                    is_available: true,
                },
                LanguageInfo {
                    code: "fr".into(),
                    name: "French".into(),
                    is_available: true,
                },
                LanguageInfo {
                    code: "ja".into(),
                    name: "Japanese".into(),
                    is_available: true,
                },
            ],
        }
    }

    pub fn add_translation(&mut self, text: &str, from: &str, to: &str, result: &str) {
        self.translations.insert(
            (text.to_string(), from.to_string(), to.to_string()),
            result.to_string(),
        );
    }
}

#[cfg(test)]
impl TranslationProvider for MockTranslationProvider {
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String, LingoError> {
        self.translations
            .get(&(text.to_string(), from.to_string(), to.to_string()))
            .cloned()
            .ok_or_else(|| {
                LingoError::TranslationFailed(format!("no mock translation for '{text}' {from}→{to}"))
            })
    }

    fn detect_language(&self, _text: &str) -> Option<String> {
        Some("en".into())
    }

    fn available_languages(&self) -> Vec<LanguageInfo> {
        self.languages.clone()
    }

    fn is_available(&self, language: &str) -> bool {
        self.languages.iter().any(|l| l.code == language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_trait_is_object_safe() {
        // This compiles only if the trait is object-safe.
        fn _assert_object_safe(_: &dyn TranslationProvider) {}
    }

    #[test]
    fn mock_provider_translates() {
        let mut provider = MockTranslationProvider::new();
        provider.add_translation("hello", "en", "fr", "bonjour");

        let result = provider.translate("hello", "en", "fr").unwrap();
        assert_eq!(result, "bonjour");

        let err = provider.translate("hello", "en", "ja");
        assert!(err.is_err());
    }
}
