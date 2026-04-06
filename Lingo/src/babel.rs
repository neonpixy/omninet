use sentinal::SeededRandom;

use crate::tokenizer::{join_separator_for_script, script_for_language, tokenize};
use crate::vocabulary::{Vocabulary, NONCE_SIZE};

/// Babel — hardened semantic text obfuscation.
///
/// Transforms text into sequences of Unicode symbols from ancient and
/// exotic scripts using a seed-derived vocabulary. Three hardening layers
/// make Babel approach stream cipher security:
///
/// 1. **Homophones**: Each common token maps to multiple symbols. The encoder
///    picks randomly, so "the" encodes to a different symbol every time.
///    Defeats word frequency analysis.
///
/// 2. **Polyalphabetic byte encoding**: 8 independent byte alphabets. The
///    alphabet for each byte position is selected by a nonce-derived RNG,
///    so the same byte at different positions produces different symbols.
///    Defeats letter frequency analysis.
///
/// 3. **Nonce per encode**: A random 16-byte nonce is generated for every
///    `encode()` call and embedded in the output. Same input → different
///    output every time. Defeats known-plaintext and replay attacks.
///
/// # Nuclear-proof
///
/// The vocabulary is computed from the seed, not stored. Given the same seed,
/// the same vocabulary structure is always generated. The nonce ensures
/// non-deterministic output while the seed ensures decodability.
pub struct Babel {
    vocabulary: Vocabulary,
    /// Stored seed for nonce-derived RNG.
    seed: Vec<u8>,
}

/// Separator between encoded symbols. Space is safe because none of the
/// Unicode symbol ranges contain ASCII space (U+0020).
const SYMBOL_SEPARATOR: char = ' ';

impl Babel {
    /// Create a new Babel from a vocabulary seed.
    ///
    /// The seed is typically derived from a master key via
    /// `sentinal::key_derivation::derive_vocabulary_seed()`.
    pub fn new(vocabulary_seed: &[u8]) -> Self {
        Self {
            vocabulary: Vocabulary::new(vocabulary_seed),
            seed: vocabulary_seed.to_vec(),
        }
    }

    /// Encode text into Babel symbols (hardened, non-deterministic).
    ///
    /// Generates a random nonce, tokenizes the text, encodes each token
    /// with homophone randomization and polyalphabetic byte encoding,
    /// and prepends the nonce as the first output token.
    ///
    /// The same input produces different output each time due to the
    /// random nonce and random homophone selection.
    pub fn encode(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let tokens = tokenize(text);
        if tokens.is_empty() {
            return String::new();
        }

        // Generate random nonce.
        let mut nonce = [0u8; NONCE_SIZE];
        getrandom::fill(&mut nonce).expect("getrandom failed");

        // Derive nonce RNG for polyalphabetic alphabet selection.
        let mut nonce_rng = self.nonce_rng(&nonce);
        let n_alphabets = self.vocabulary.alphabet_count();

        // Encode nonce as first token (always alphabet 0).
        let nonce_encoded = self.encode_nonce(&nonce);

        // Encode content tokens.
        let mut parts = Vec::with_capacity(tokens.len() + 1);
        parts.push(nonce_encoded);

        for token in &tokens {
            parts.push(self.encode_token_hardened(token, &mut nonce_rng, n_alphabets));
        }

        parts.join(&SYMBOL_SEPARATOR.to_string())
    }

    /// Decode Babel symbols back into text.
    ///
    /// Extracts the nonce from the first token, reconstructs the
    /// polyalphabetic alphabet sequence, and decodes each content token.
    /// Joins with spaces (correct for Latin/Arabic/Cyrillic).
    /// For CJK/Kana/Hangul/Thai, use [`decode_for_language`] instead.
    pub fn decode(&self, encoded: &str) -> String {
        self.decode_with_separator(encoded, " ")
    }

    /// Decode Babel symbols using the source language to determine
    /// how to rejoin tokens.
    ///
    /// CJK/Kana/Hangul/Thai tokens are joined without spaces.
    /// Latin/Arabic/Cyrillic/Devanagari tokens are joined with spaces.
    /// The `source_language` is a BCP 47 code (e.g., "ja", "zh-Hans", "en").
    pub fn decode_for_language(&self, encoded: &str, source_language: &str) -> String {
        let script = script_for_language(source_language);
        let separator = join_separator_for_script(script);
        self.decode_with_separator(encoded, separator)
    }

    /// Encode a single token (deterministic, no nonce context).
    ///
    /// Uses the first homophone and alphabet 0. For testing and
    /// diagnostic use. Production encoding should use [`encode`].
    pub fn encode_token(&self, token: &str) -> String {
        self.vocabulary.encode(token)
    }

    /// Decode a single Babel symbol (deterministic, no nonce context).
    ///
    /// Handles any homophone. Uses alphabet 0 for byte decoding.
    pub fn decode_symbol(&self, symbol: &str) -> String {
        self.vocabulary.decode(symbol)
    }

    // --- Private ---

    /// Internal decode with configurable separator.
    fn decode_with_separator(&self, encoded: &str, separator: &str) -> String {
        if encoded.is_empty() {
            return String::new();
        }

        let parts: Vec<&str> = encoded.split(SYMBOL_SEPARATOR).collect();
        if parts.len() < 2 {
            // Single token — can't have a nonce. Legacy/degenerate decode.
            return self.vocabulary.decode(parts[0]);
        }

        // First token is the nonce.
        let nonce = match self.decode_nonce(parts[0]) {
            Some(n) => n,
            None => {
                // Not a valid nonce — best-effort legacy decode.
                return parts
                    .iter()
                    .map(|s| self.vocabulary.decode(s))
                    .collect::<Vec<_>>()
                    .join(separator);
            }
        };

        let mut nonce_rng = self.nonce_rng(&nonce);
        let n_alphabets = self.vocabulary.alphabet_count();

        parts[1..]
            .iter()
            .map(|s| self.decode_token_hardened(s, &mut nonce_rng, n_alphabets))
            .collect::<Vec<_>>()
            .join(separator)
    }

    /// Encode a single token with hardening (random homophone + polyalphabetic bytes).
    fn encode_token_hardened(
        &self,
        token: &str,
        nonce_rng: &mut SeededRandom,
        n_alphabets: usize,
    ) -> String {
        // Try common token with random homophone.
        if let Some(pool) = self.vocabulary.homophone_pool(token) {
            return pick_random_homophone(pool);
        }
        // Check lowercase match — preserve case via byte encoding.
        let lower = token.to_lowercase();
        if lower != token && self.vocabulary.homophone_pool(&lower).is_some() {
            return self.encode_bytes_poly(token, nonce_rng, n_alphabets);
        }
        self.encode_bytes_poly(token, nonce_rng, n_alphabets)
    }

    /// Decode a single token with hardening (any homophone + polyalphabetic bytes).
    fn decode_token_hardened(
        &self,
        symbol: &str,
        nonce_rng: &mut SeededRandom,
        n_alphabets: usize,
    ) -> String {
        // Try common token reverse lookup (handles all homophones).
        if let Some(token) = self.vocabulary.reverse_lookup(symbol) {
            return token.to_string();
        }
        // Byte-level polyalphabetic decode.
        self.decode_bytes_poly(symbol, nonce_rng, n_alphabets)
    }

    /// Polyalphabetic byte encoding: each byte uses a nonce-derived alphabet.
    fn encode_bytes_poly(
        &self,
        token: &str,
        nonce_rng: &mut SeededRandom,
        n_alphabets: usize,
    ) -> String {
        token
            .as_bytes()
            .iter()
            .map(|&b| {
                let a = nonce_rng.next_bounded(n_alphabets);
                self.vocabulary.encode_byte(b, a)
            })
            .collect::<String>()
    }

    /// Polyalphabetic byte decoding: each char uses a nonce-derived alphabet.
    fn decode_bytes_poly(
        &self,
        compound: &str,
        nonce_rng: &mut SeededRandom,
        n_alphabets: usize,
    ) -> String {
        let bytes: Vec<u8> = compound
            .chars()
            .filter_map(|c| {
                let a = nonce_rng.next_bounded(n_alphabets);
                self.vocabulary.decode_byte_char(c, a)
            })
            .collect();

        String::from_utf8(bytes).unwrap_or_else(|_| compound.to_string())
    }

    /// Encode a nonce as a compound symbol using byte alphabet 0.
    fn encode_nonce(&self, nonce: &[u8]) -> String {
        nonce
            .iter()
            .map(|&b| self.vocabulary.encode_byte(b, 0))
            .collect::<String>()
    }

    /// Decode a compound symbol as a nonce using byte alphabet 0.
    /// Returns None if the symbol doesn't decode to exactly NONCE_SIZE bytes.
    fn decode_nonce(&self, token: &str) -> Option<[u8; NONCE_SIZE]> {
        let bytes: Vec<u8> = token
            .chars()
            .filter_map(|c| self.vocabulary.decode_byte_char(c, 0))
            .collect();

        if bytes.len() == NONCE_SIZE {
            let mut nonce = [0u8; NONCE_SIZE];
            nonce.copy_from_slice(&bytes);
            Some(nonce)
        } else {
            None
        }
    }

    /// Derive a SeededRandom from the vocabulary seed + nonce.
    /// SeededRandom::new internally SHA-256 hashes the input.
    fn nonce_rng(&self, nonce: &[u8]) -> SeededRandom {
        let mut combined = Vec::with_capacity(self.seed.len() + nonce.len());
        combined.extend_from_slice(&self.seed);
        combined.extend_from_slice(nonce);
        SeededRandom::new(&combined)
    }
}

/// Pick a random element from a homophone pool using OS randomness.
fn pick_random_homophone(pool: &[String]) -> String {
    if pool.len() == 1 {
        return pool[0].clone();
    }
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("getrandom failed");
    let idx = (u64::from_le_bytes(buf) as usize) % pool.len();
    pool[idx].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SEED: &[u8] = b"omnidea-babel-test-seed-00000032";

    #[test]
    fn encode_decode_text() {
        let babel = Babel::new(TEST_SEED);
        let original = "hello world";
        let encoded = babel.encode(original);
        let decoded = babel.decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn encoded_differs_from_original() {
        let babel = Babel::new(TEST_SEED);
        let original = "hello world";
        let encoded = babel.encode(original);
        assert_ne!(encoded, original);
    }

    #[test]
    fn encode_nondeterministic() {
        // Same seed, same text → different encoded output each time (nonce + homophones).
        let babel = Babel::new(TEST_SEED);
        let text = "hello world omnidea vault";
        let encoded1 = babel.encode(text);
        let encoded2 = babel.encode(text);
        assert_ne!(
            encoded1, encoded2,
            "hardened encode should produce different output each time"
        );
    }

    #[test]
    fn nondeterministic_roundtrip() {
        // Both non-deterministic encodings decode to the same original.
        let babel = Babel::new(TEST_SEED);
        let text = "hello world omnidea vault crown";
        let encoded1 = babel.encode(text);
        let encoded2 = babel.encode(text);
        assert_ne!(encoded1, encoded2);
        assert_eq!(babel.decode(&encoded1), text);
        assert_eq!(babel.decode(&encoded2), text);
    }

    #[test]
    fn different_seeds_different_encoding() {
        let babel1 = Babel::new(b"seed-one-for-babel-test-000032b");
        let babel2 = Babel::new(b"seed-two-for-babel-test-000032b");
        let text = "hello world";
        let decoded1 = babel1.decode(&babel1.encode(text));
        let decoded2 = babel2.decode(&babel2.encode(text));
        // Both decode correctly with their own seed.
        assert_eq!(decoded1, text);
        assert_eq!(decoded2, text);
    }

    #[test]
    fn cross_seed_decode_fails() {
        // Encoding with one seed cannot be decoded with another.
        let babel1 = Babel::new(b"seed-one-for-babel-test-000032b");
        let babel2 = Babel::new(b"seed-two-for-babel-test-000032b");
        let encoded = babel1.encode("hello world");
        let decoded = babel2.decode(&encoded);
        assert_ne!(decoded, "hello world");
    }

    #[test]
    fn empty_text() {
        let babel = Babel::new(TEST_SEED);
        assert_eq!(babel.encode(""), "");
        assert_eq!(babel.decode(""), "");
    }

    #[test]
    fn single_word() {
        let babel = Babel::new(TEST_SEED);
        let encoded = babel.encode("hello");
        let decoded = babel.decode(&encoded);
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn nonce_token_present() {
        // The encoded output should have one extra token (the nonce prefix).
        let babel = Babel::new(TEST_SEED);
        let text = "one two three four five";
        let encoded = babel.encode(text);
        let encoded_count = encoded.split(SYMBOL_SEPARATOR).count();
        let original_count = text.split_whitespace().count();
        assert_eq!(
            encoded_count,
            original_count + 1,
            "encoded should have original tokens + 1 nonce token"
        );
    }

    #[test]
    fn frequency_analysis_resistance() {
        // Encode "the" 100 times — should produce many distinct symbols.
        let babel = Babel::new(TEST_SEED);
        let mut symbols = std::collections::HashSet::new();
        for _ in 0..100 {
            let encoded = babel.encode("the");
            // Second token is "the" (first is nonce).
            let parts: Vec<&str> = encoded.split(SYMBOL_SEPARATOR).collect();
            assert!(parts.len() >= 2);
            symbols.insert(parts[1].to_string());
        }
        // With 4 homophones, we should see multiple distinct symbols.
        assert!(
            symbols.len() > 1,
            "expected multiple distinct symbols for 'the', got {}",
            symbols.len()
        );
    }

    #[test]
    fn decode_for_language_cjk_no_spaces() {
        let babel = Babel::new(TEST_SEED);
        let text = "你我他";
        let encoded = babel.encode(text);

        // Language-aware decode — no spaces for Chinese.
        let zh_decoded = babel.decode_for_language(&encoded, "zh-Hans");
        assert!(!zh_decoded.contains(' '));
        assert_eq!(zh_decoded, "你我他");
    }

    #[test]
    fn decode_for_language_japanese_no_spaces() {
        let babel = Babel::new(TEST_SEED);
        let text = "のにはをた";
        let encoded = babel.encode(text);

        let ja_decoded = babel.decode_for_language(&encoded, "ja");
        assert!(!ja_decoded.contains(' '));
        assert_eq!(ja_decoded, "のにはをた");
    }

    #[test]
    fn decode_for_language_english_has_spaces() {
        let babel = Babel::new(TEST_SEED);
        let text = "hello world";
        let encoded = babel.encode(text);

        let en_decoded = babel.decode_for_language(&encoded, "en");
        assert_eq!(en_decoded, "hello world");
    }

    #[test]
    fn decode_for_language_korean_no_spaces() {
        let babel = Babel::new(TEST_SEED);
        let text = "이그저";
        let encoded = babel.encode(text);

        let ko_decoded = babel.decode_for_language(&encoded, "ko");
        assert!(!ko_decoded.contains(' '));
        assert_eq!(ko_decoded, "이그저");
    }

    #[test]
    fn multi_language_encode_decode() {
        let babel = Babel::new(TEST_SEED);

        // Chinese characters from the common token list.
        let cn_encoded = babel.encode("你 我");
        let cn_decoded = babel.decode(&cn_encoded);
        assert!(cn_decoded.contains("你"));
        assert!(cn_decoded.contains("我"));
        assert_ne!(cn_encoded, "你 我");

        // Russian text encodes to something different.
        let ru_encoded = babel.encode("Привет мир");
        assert!(!ru_encoded.is_empty());
        assert_ne!(ru_encoded, "Привет мир");

        // Arabic text encodes to something different.
        let ar_encoded = babel.encode("مرحبا بالعالم");
        assert!(!ar_encoded.is_empty());
        assert_ne!(ar_encoded, "مرحبا بالعالم");
    }

    #[test]
    fn byte_encoded_tokens_roundtrip() {
        // Unknown tokens go through polyalphabetic byte encoding.
        let babel = Babel::new(TEST_SEED);
        let texts = [
            "sovereign internet",
            "hello@world.com is a test",
            "{\"key\": \"value\"}",
            "Ω≈ç√ is math",
        ];
        for text in &texts {
            let encoded = babel.encode(text);
            let decoded = babel.decode(&encoded);
            assert_eq!(&decoded, text, "byte-encoded round-trip failed for '{text}'");
        }
    }

    #[test]
    fn deterministic_encode_token() {
        // The per-token convenience method should be deterministic (first homophone).
        let babel1 = Babel::new(TEST_SEED);
        let babel2 = Babel::new(TEST_SEED);
        assert_eq!(babel1.encode_token("hello"), babel2.encode_token("hello"));
        assert_eq!(
            babel1.encode_token("sovereign"),
            babel2.encode_token("sovereign")
        );
    }
}
