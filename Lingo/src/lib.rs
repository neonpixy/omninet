//! # Lingo — Language & Translation for Omnidea
//!
//! The shared tongue. Lingo handles:
//!
//! - **Babel**: Semantic text obfuscation via Unicode vocabulary transformation.
//!   Maps words and tokens to symbols from 90,000+ ancient, exotic, and symbolic
//!   Unicode characters. The mapping is a pure function of a seed — nuclear-proof
//!   against corruption.
//!
//! - **Omnilingual tokenization**: Language-aware text splitting. Space-based for
//!   Latin/Arabic/Cyrillic, character-level for CJK/Kana/Hangul, grapheme clusters
//!   for Thai.
//!
//! - **Translation**: Platform translation via the [`TranslationProvider`] trait.
//!   Each platform (Apple, Android, Web) implements this through Divinity FFI.
//!
//! - **Language detection**: Unicode-based script and language identification.
//!
//! # No Canonical English
//!
//! Content is stored in its original language with a BCP 47 tag. Translation
//! happens on read, from source language to reader's language. Every language
//! is first-class. No round-trip translation loss.

pub mod babel;
pub mod cache;
pub mod detection;
pub mod error;
pub mod formula;
pub mod provider;
pub mod symbols;
pub mod tokenizer;
pub mod translator;
pub mod types;
pub mod vocabulary;

// Convenience re-exports.
pub use babel::Babel;
pub use cache::TranslationCache;
pub use error::LingoError;
pub use formula::{
    CellResolver, DependencyGraph, FormulaEvaluator, FormulaLocale, FormulaParser, FormulaValue,
};
pub use provider::TranslationProvider;
pub use tokenizer::{join_separator_for_script, script_for_language, tokenize};
pub use translator::{StoredText, UniversalTranslator};
pub use types::{CacheStatistics, LanguageInfo, TranslatedText, TranslationKit};
pub use vocabulary::Vocabulary;
