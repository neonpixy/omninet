use lingo::{Babel, TranslationCache, UniversalTranslator, Vocabulary};
use uuid::Uuid;

const TEST_SEED: &[u8] = b"omnidea-integration-test-seed-32";

// --- Babel end-to-end ---

#[test]
fn babel_end_to_end() {
    let babel = Babel::new(TEST_SEED);
    let original = "hello world omnidea vault crown";
    let encoded = babel.encode(original);
    let decoded = babel.decode(&encoded);

    assert_ne!(encoded, original);
    assert_eq!(decoded, original);
}

#[test]
fn babel_nondeterministic() {
    // Hardened Babel produces different output each time (nonce + homophones).
    let text = "the quick brown fox jumps over the lazy dog";
    let babel = Babel::new(TEST_SEED);

    let encoded1 = babel.encode(text);
    let encoded2 = babel.encode(text);
    assert_ne!(encoded1, encoded2, "hardened encode should be non-deterministic");

    // Both decode correctly.
    assert_eq!(babel.decode(&encoded1), text);
    assert_eq!(babel.decode(&encoded2), text);
}

#[test]
fn babel_different_seeds_incompatible() {
    // Different seed → different vocabulary → cannot decode each other's output.
    let babel1 = Babel::new(TEST_SEED);
    let babel2 = Babel::new(b"a-completely-different-seed-0032");

    let encoded = babel1.encode("hello world");
    let decoded_wrong = babel2.decode(&encoded);
    assert_ne!(decoded_wrong, "hello world");
}

// --- Omnilingual ---

#[test]
fn omnilingual_chinese_common_tokens() {
    let babel = Babel::new(TEST_SEED);
    let text = "的 是 人 我 在";
    let encoded = babel.encode(text);
    let decoded = babel.decode(&encoded);
    assert_eq!(decoded, text);
}

#[test]
fn omnilingual_japanese_common_tokens() {
    let babel = Babel::new(TEST_SEED);
    let text = "の に は を た";
    let encoded = babel.encode(text);
    let decoded = babel.decode(&encoded);
    assert_eq!(decoded, text);
}

#[test]
fn omnilingual_encoding_produces_babel_symbols() {
    let babel = Babel::new(TEST_SEED);

    let ar = "مرحبا بالعالم";
    let ar_encoded = babel.encode(ar);
    assert_ne!(ar_encoded, ar);
    assert!(!ar_encoded.is_empty());

    let ru = "Привет мир";
    let ru_encoded = babel.encode(ru);
    assert_ne!(ru_encoded, ru);
    assert!(!ru_encoded.is_empty());
}

// --- Nuclear-proof vocabulary regeneration ---

#[test]
fn vocabulary_regeneration_from_seed() {
    // Vocabulary's deterministic encode (first homophone, alphabet 0) is still nuclear-proof.
    let words = ["hello", "world", "omnidea", "vault", "crown", "the", "of"];

    let encodings: Vec<String> = {
        let vocab = Vocabulary::new(TEST_SEED);
        words.iter().map(|w| vocab.encode(w)).collect()
    };

    let encodings2: Vec<String> = {
        let vocab = Vocabulary::new(TEST_SEED);
        words.iter().map(|w| vocab.encode(w)).collect()
    };

    assert_eq!(encodings, encodings2);
}

// --- Sentinal integration ---

#[test]
fn sentinal_seed_derivation_to_babel() {
    let master_key = vec![0x42u8; 32];
    let seed = sentinal::key_derivation::derive_vocabulary_seed(&master_key).unwrap();

    let babel = Babel::new(seed.expose());
    let encoded = babel.encode("hello world");
    let decoded = babel.decode(&encoded);
    assert_eq!(decoded, "hello world");

    // Same master key → same seed → both decode correctly.
    let seed2 = sentinal::key_derivation::derive_vocabulary_seed(&master_key).unwrap();
    let babel2 = Babel::new(seed2.expose());
    let encoded2 = babel2.encode("hello world");
    assert_eq!(babel2.decode(&encoded2), "hello world");
}

// --- UniversalTranslator ---

#[test]
fn translator_babel_roundtrip() {
    let mut translator = UniversalTranslator::new().with_babel(TEST_SEED);
    let id = Uuid::new_v4();

    let stored = translator.prepare_for_storage("hello world", "en").unwrap();
    assert!(stored.babel_encoded);
    assert_ne!(stored.text, "hello world");

    let result = translator
        .translate_for_display(&stored.text, "en", "en", &id)
        .unwrap();
    assert_eq!(result.text, "hello world");
    assert_eq!(result.source_language, "en");
    assert!(!result.from_cache);
}

// --- Large text ---

#[test]
fn large_text_roundtrip() {
    let babel = Babel::new(TEST_SEED);

    let words = [
        "the", "hello", "world", "omnidea", "vault", "crown",
        "globe", "throne", "hall", "good", "time", "people",
        "work", "make", "give",
    ];
    let large_text: String = (0..1000)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ");

    let encoded = babel.encode(&large_text);
    let decoded = babel.decode(&encoded);
    assert_eq!(decoded, large_text);
}

// --- TranslationKit serialization ---

#[test]
fn translation_kit_serialization() {
    let kit = lingo::TranslationKit::new(
        "cpub1alice".into(),
        "cpub1bob".into(),
        TEST_SEED.to_vec(),
    );

    let json = serde_json::to_string_pretty(&kit).unwrap();
    let parsed: lingo::TranslationKit = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.from, "cpub1alice");
    assert_eq!(parsed.to, "cpub1bob");
    assert_eq!(parsed.vocabulary_seed, TEST_SEED);
    assert!(parsed.signature.is_none());

    // Shared seed → both decode each other's encodings.
    let babel1 = Babel::new(&kit.vocabulary_seed);
    let babel2 = Babel::new(&parsed.vocabulary_seed);
    let encoded = babel1.encode("hello");
    assert_eq!(babel2.decode(&encoded), "hello");
}

// --- Language detection ---

#[test]
fn language_detection_integration() {
    let translator = UniversalTranslator::new();

    assert_eq!(translator.detect_language("Hello world"), Some("en".into()));
    assert_eq!(translator.detect_language("こんにちは世界"), Some("ja".into()));
    assert_eq!(translator.detect_language("你好世界"), Some("zh-Hans".into()));
    assert_eq!(translator.detect_language("안녕하세요"), Some("ko".into()));
}

// --- Cache isolation ---

#[test]
fn cache_isolation_by_content_id() {
    let mut cache = TranslationCache::new(100);
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    cache.set(
        id1,
        "fr".into(),
        lingo::TranslatedText {
            text: "bonjour".into(),
            original: "hello".into(),
            source_language: "en".into(),
            target_language: "fr".into(),
            from_cache: false,
        },
    );

    assert!(cache.get(&id1, "fr").is_some());
    assert!(cache.get(&id2, "fr").is_none());

    let stats = cache.statistics();
    assert_eq!(stats.hit_count, 1);
    assert_eq!(stats.miss_count, 1);
}

// --- Symbol space verification ---

#[test]
fn symbol_space_is_massive() {
    let count = lingo::symbols::symbol_count();
    assert!(
        count >= 80_000,
        "expected 80,000+ symbols, got {count}"
    );
}

// --- Hardening-specific integration tests ---

#[test]
fn hardened_babel_frequency_resistance() {
    // Encode the same common word many times. With homophones,
    // it should produce multiple distinct encoded symbols.
    let babel = Babel::new(TEST_SEED);
    let mut encoded_symbols = std::collections::HashSet::new();

    for _ in 0..200 {
        let encoded = babel.encode("the");
        let parts: Vec<&str> = encoded.split(' ').collect();
        // parts[0] = nonce, parts[1] = "the" encoded
        if parts.len() >= 2 {
            encoded_symbols.insert(parts[1].to_string());
        }
    }

    assert!(
        encoded_symbols.len() > 1,
        "expected multiple homophones for 'the', got {} distinct symbols",
        encoded_symbols.len()
    );
}

#[test]
fn hardened_babel_byte_encoding_nondeterministic() {
    // Unknown tokens go through polyalphabetic byte encoding.
    // Different nonces → different byte symbols for the same input.
    let babel = Babel::new(TEST_SEED);
    let mut encodings = std::collections::HashSet::new();

    for _ in 0..20 {
        let encoded = babel.encode("sovereign");
        let parts: Vec<&str> = encoded.split(' ').collect();
        if parts.len() >= 2 {
            encodings.insert(parts[1].to_string());
        }
    }

    assert!(
        encodings.len() > 1,
        "polyalphabetic byte encoding should produce different symbols per nonce"
    );
}

#[test]
fn hardened_babel_mixed_content_roundtrip() {
    // Mix of common tokens and byte-encoded tokens.
    let babel = Babel::new(TEST_SEED);
    let texts = [
        "hello sovereign world",
        "the omnidea@vault.com is great",
        "crown 42 globe {\"key\":\"value\"}",
        "vault Привет world",
    ];

    for text in &texts {
        let encoded = babel.encode(text);
        let decoded = babel.decode(&encoded);
        assert_eq!(
            &decoded, text,
            "mixed content round-trip failed for '{text}'"
        );
    }

    // CJK mixed text needs decode_for_language for correct joining.
    let cjk_text = "日本語テスト";
    let encoded = babel.encode(cjk_text);
    let decoded = babel.decode_for_language(&encoded, "ja");
    assert_eq!(decoded, cjk_text);
}
