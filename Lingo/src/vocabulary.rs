use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use sentinal::SeededRandom;

use crate::symbols::SYMBOLS;

/// Number of symbols reserved per byte alphabet (one per byte value).
const BYTE_ALPHABET_SIZE: usize = 256;

/// Number of byte alphabets for polyalphabetic encoding.
///
/// Each alphabet is a different 256-symbol permutation. During encoding,
/// the alphabet for each byte position is selected by a nonce-derived RNG,
/// defeating letter frequency analysis.
const BYTE_ALPHABET_COUNT: usize = 8;

/// Maximum homophones per common token.
///
/// Each common token maps to up to this many symbols. The encoder picks
/// one at random each time, defeating word frequency analysis.
const MAX_HOMOPHONES: usize = 4;

/// Nonce size in bytes. Prepended to every Babel encode for non-determinism.
pub(crate) const NONCE_SIZE: usize = 16;

/// Seed-deterministic vocabulary mapping with hardened encoding.
///
/// Maps tokens (words/characters) to Unicode symbols and back.
/// The mapping is a pure function of the seed — given the same seed,
/// the same vocabulary is always generated. Nuclear-proof.
///
/// # Hardening features
///
/// 1. **Homophones**: Each common token maps to multiple symbols (up to 4).
///    The encoder picks randomly from the pool, defeating word frequency analysis.
///    The decoder handles any homophone transparently.
///
/// 2. **Polyalphabetic byte encoding**: 8 independent byte alphabets instead of 1.
///    The alphabet for each byte position is selected by a nonce-derived RNG,
///    so the same byte at different positions encodes to different symbols.
///
/// 3. **Nonce per encode**: A random 16-byte nonce seeds the alphabet selection,
///    making every encode unique even for the same input.
///
/// # Two encoding paths
///
/// 1. **Common tokens** (~19K from BIP-39 + Omnidea terms) get single-symbol
///    encoding via homophone pool lookup.
///
/// 2. **Unknown tokens** (any word not in the common list) get reversible
///    polyalphabetic byte-level encoding.
pub struct Vocabulary {
    /// Common token → pool of homophone symbols.
    forward_map: HashMap<String, Vec<String>>,
    /// Any homophone symbol → common token (reverse lookup).
    reverse_map: HashMap<String, String>,
    /// N byte alphabets: `byte_alphabets[alphabet][byte_value]` → symbol.
    byte_alphabets: Vec<Vec<String>>,
    /// N byte reverse maps: `byte_reverse[alphabet][symbol]` → byte_value.
    byte_reverse: Vec<HashMap<String, u8>>,
}

impl Vocabulary {
    /// Create a vocabulary from a seed.
    ///
    /// Uses `SeededRandom` to Fisher-Yates shuffle the full symbol space, then:
    /// - First `8 × 256` shuffled symbols → 8 byte alphabets (polyalphabetic)
    /// - Next `N × homophones` shuffled symbols → common token homophone pools
    pub fn new(seed: &[u8]) -> Self {
        let common_tokens = &*COMMON_TOKENS;
        let mut rng = SeededRandom::new(seed);

        // Fisher-Yates shuffle of indices into the symbol table.
        let symbol_count = SYMBOLS.len();
        let mut indices: Vec<usize> = (0..symbol_count).collect();
        for i in (1..symbol_count).rev() {
            let j = rng.next_bounded(i + 1);
            indices.swap(i, j);
        }

        // First BYTE_ALPHABET_COUNT × 256 shuffled symbols → N byte alphabets.
        let byte_symbols_total = BYTE_ALPHABET_COUNT * BYTE_ALPHABET_SIZE;
        let mut byte_alphabets = Vec::with_capacity(BYTE_ALPHABET_COUNT);
        let mut byte_reverse = Vec::with_capacity(BYTE_ALPHABET_COUNT);

        for a in 0..BYTE_ALPHABET_COUNT {
            let mut alphabet = Vec::with_capacity(BYTE_ALPHABET_SIZE);
            let mut reverse = HashMap::with_capacity(BYTE_ALPHABET_SIZE);
            for byte_val in 0..BYTE_ALPHABET_SIZE {
                let idx = a * BYTE_ALPHABET_SIZE + byte_val;
                let symbol = &SYMBOLS[indices[idx]];
                alphabet.push(symbol.clone());
                reverse.insert(symbol.clone(), byte_val as u8);
            }
            byte_alphabets.push(alphabet);
            byte_reverse.push(reverse);
        }

        // Remaining symbols → common token homophone pools.
        let token_start = byte_symbols_total;
        let available = symbol_count.saturating_sub(token_start);
        let homophones = (available / common_tokens.len().max(1)).clamp(1, MAX_HOMOPHONES);
        let mapped_count = (available / homophones).min(common_tokens.len());

        let mut forward_map = HashMap::with_capacity(mapped_count);
        let mut reverse_map = HashMap::with_capacity(mapped_count * homophones);

        for (i, token) in common_tokens.iter().enumerate().take(mapped_count) {
            let mut pool = Vec::with_capacity(homophones);
            for h in 0..homophones {
                let sym_idx = token_start + i * homophones + h;
                if sym_idx < symbol_count {
                    let symbol = &SYMBOLS[indices[sym_idx]];
                    pool.push(symbol.clone());
                    reverse_map.insert(symbol.clone(), token.clone());
                }
            }
            if !pool.is_empty() {
                forward_map.insert(token.clone(), pool);
            }
        }

        Self {
            forward_map,
            reverse_map,
            byte_alphabets,
            byte_reverse,
        }
    }

    /// Encode a token deterministically (first homophone, alphabet 0).
    ///
    /// Nuclear-proof: same seed → same output, always.
    /// For hardened (non-deterministic) encoding, use [`Babel`](crate::Babel).
    pub fn encode(&self, token: &str) -> String {
        if let Some(pool) = self.forward_map.get(token) {
            return pool[0].clone();
        }
        let lower = token.to_lowercase();
        if lower != token && self.forward_map.contains_key(&lower) {
            // Common token matched via lowercase — preserve case via byte encoding.
            return self.encode_bytes_alphabet0(token);
        }
        self.encode_bytes_alphabet0(token)
    }

    /// Decode a symbol. Handles any homophone for common tokens.
    ///
    /// Uses alphabet 0 for byte-level decode (deterministic fallback).
    /// For hardened polyalphabetic decode, use [`Babel`](crate::Babel).
    pub fn decode(&self, symbol: &str) -> String {
        if let Some(token) = self.reverse_map.get(symbol) {
            return token.clone();
        }
        self.decode_bytes_alphabet0(symbol)
    }

    // --- Crate-internal API for Babel hardening ---

    /// Get the homophone pool for a common token.
    pub(crate) fn homophone_pool(&self, token: &str) -> Option<&Vec<String>> {
        self.forward_map.get(token)
    }

    /// Reverse-lookup a known symbol to its common token.
    pub(crate) fn reverse_lookup(&self, symbol: &str) -> Option<&str> {
        self.reverse_map.get(symbol).map(|s| s.as_str())
    }

    /// Encode a single byte using a specific alphabet.
    pub(crate) fn encode_byte(&self, byte: u8, alphabet: usize) -> &str {
        &self.byte_alphabets[alphabet % self.byte_alphabets.len()][byte as usize]
    }

    /// Decode a single character from a specific alphabet.
    pub(crate) fn decode_byte_char(&self, ch: char, alphabet: usize) -> Option<u8> {
        let s = ch.to_string();
        let a = alphabet % self.byte_reverse.len();
        self.byte_reverse[a].get(&s).copied()
    }

    /// Number of byte alphabets.
    pub(crate) fn alphabet_count(&self) -> usize {
        self.byte_alphabets.len()
    }

    // --- Private helpers ---

    /// Byte-encode using alphabet 0 (deterministic fallback).
    fn encode_bytes_alphabet0(&self, token: &str) -> String {
        token
            .as_bytes()
            .iter()
            .map(|&b| self.byte_alphabets[0][b as usize].as_str())
            .collect::<String>()
    }

    /// Byte-decode using alphabet 0 (deterministic fallback).
    fn decode_bytes_alphabet0(&self, compound: &str) -> String {
        let bytes: Vec<u8> = compound
            .chars()
            .filter_map(|c| {
                let s = c.to_string();
                self.byte_reverse[0].get(&s).copied()
            })
            .collect();

        String::from_utf8(bytes).unwrap_or_else(|_| compound.to_string())
    }
}

/// BIP-39 word lists from 10 languages, deduplicated + Omnidea domain terms.
/// Lazily generated once, then cached.
static COMMON_TOKENS: LazyLock<Vec<String>> = LazyLock::new(build_common_token_list);

/// Build the common token list from BIP-39 word lists across all available
/// languages, plus Omnidea-specific domain terms.
///
/// BIP-39 provides 2,048 carefully curated common words per language across
/// 10 languages (English, Spanish, French, Italian, Portuguese, Czech,
/// Japanese, Korean, Chinese Simplified, Chinese Traditional).
/// After deduplication: ~19,000 unique tokens, all getting single-symbol encoding.
fn build_common_token_list() -> Vec<String> {
    let mut seen = HashSet::new();
    let mut tokens = Vec::with_capacity(22_000);

    // Add BIP-39 words from all enabled languages (10 languages, 2048 each).
    for lang in bip39::Language::ALL {
        for word in lang.word_list() {
            let w = word.to_string();
            if seen.insert(w.clone()) {
                tokens.push(w);
            }
        }
    }

    // Add Omnidea domain terms (may overlap with BIP-39, dedup handles it).
    let omnidea_terms = [
        "omnidea", "digit", "vault", "crown", "globe", "babel",
        "throne", "sentinel", "hall", "equipment", "fortune", "kingdom",
        "lingo", "magic", "advisor", "polity", "quest", "regalia",
        "universe", "nexus", "oracle", "yoke", "zeitgeist",
        "bulwark", "jail", "pact",
        // Common CJK particles/characters not in BIP-39
        "的", "一", "是", "了", "不", "人", "我", "在", "有", "他",
        "这", "中", "大", "来", "上", "国", "个", "到", "说", "们",
        "为", "子", "和", "你", "地", "出", "会", "时", "要", "也",
        // Japanese particles
        "の", "に", "は", "を", "た", "が", "で", "て", "と", "し",
        "れ", "さ", "ある", "いる", "する", "から", "だ", "まで",
        // Korean particles
        "이", "그", "저", "의", "에", "는", "을", "를", "와", "과",
    ];
    for term in &omnidea_terms {
        let t = term.to_string();
        if seen.insert(t.clone()) {
            tokens.push(t);
        }
    }

    tokens
}

/// Get the number of common tokens in the BIP-39 + Omnidea vocabulary list.
/// Useful for diagnostics and tests.
pub fn common_token_count() -> usize {
    COMMON_TOKENS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SEED: &[u8] = b"omnidea-test-vocabulary-seed-32b";

    #[test]
    fn encode_decode_roundtrip_common() {
        let vocab = Vocabulary::new(TEST_SEED);
        let words = ["hello", "world", "omnidea", "the", "vault"];
        for word in &words {
            let encoded = vocab.encode(word);
            let decoded = vocab.decode(&encoded);
            assert_eq!(decoded, *word, "round-trip failed for '{word}'");
        }
    }

    #[test]
    fn encode_decode_roundtrip_unknown() {
        let vocab = Vocabulary::new(TEST_SEED);
        let words = [
            "sovereign",
            "xyzzyplughfrobnicate",
            "supercalifragilisticexpialidocious",
            "Omnidea2026!",
            "hello@world.com",
            "{\"json\":true}",
        ];
        for word in &words {
            let encoded = vocab.encode(word);
            let decoded = vocab.decode(&encoded);
            assert_eq!(decoded, *word, "byte-level round-trip failed for '{word}'");
        }
    }

    #[test]
    fn encode_decode_roundtrip_unicode() {
        let vocab = Vocabulary::new(TEST_SEED);
        let words = ["café", "naïve", "日本語テスト", "Ω≈ç√"];
        for word in &words {
            let encoded = vocab.encode(word);
            let decoded = vocab.decode(&encoded);
            assert_eq!(decoded, *word, "unicode round-trip failed for '{word}'");
        }
    }

    #[test]
    fn encode_deterministic() {
        let vocab1 = Vocabulary::new(TEST_SEED);
        let vocab2 = Vocabulary::new(TEST_SEED);
        for word in ["hello", "world", "omnidea", "create", "的", "sovereign"] {
            assert_eq!(
                vocab1.encode(word),
                vocab2.encode(word),
                "determinism failed for '{word}'"
            );
        }
    }

    #[test]
    fn different_seeds_different_symbols() {
        let vocab1 = Vocabulary::new(b"seed-alpha-padding-to-32-bytes!");
        let vocab2 = Vocabulary::new(b"seed-bravo-padding-to-32-bytes!");
        let different = ["hello", "world", "the", "omnidea"]
            .iter()
            .any(|w| vocab1.encode(w) != vocab2.encode(w));
        assert!(different, "different seeds should produce different mappings");
    }

    #[test]
    fn unknown_token_encoded_differently_from_original() {
        let vocab = Vocabulary::new(TEST_SEED);
        let encoded = vocab.encode("xyzzyplughfrobnicate");
        assert!(!encoded.is_empty());
        assert_ne!(encoded, "xyzzyplughfrobnicate");
    }

    #[test]
    fn forward_reverse_consistent() {
        let vocab = Vocabulary::new(TEST_SEED);
        for (token, pool) in &vocab.forward_map {
            for symbol in pool {
                assert_eq!(
                    vocab.reverse_map.get(symbol).unwrap(),
                    token,
                    "inconsistent mapping for token '{token}', symbol '{symbol}'"
                );
            }
        }
    }

    #[test]
    fn vocabulary_regeneration() {
        // Nuclear-proof: create, use, drop, recreate — same deterministic mappings.
        let encoded1 = {
            let vocab = Vocabulary::new(TEST_SEED);
            vocab.encode("sovereign internet")
        };
        let encoded2 = {
            let vocab = Vocabulary::new(TEST_SEED);
            vocab.encode("sovereign internet")
        };
        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn bip39_common_tokens_massive() {
        let count = common_token_count();
        assert!(
            count >= 15_000,
            "expected 15,000+ common tokens from BIP-39, got {count}"
        );
        let tokens: std::collections::HashSet<&str> =
            COMMON_TOKENS.iter().map(|s| s.as_str()).collect();
        assert!(tokens.contains("abandon"));
        assert!(tokens.contains("zoo"));
        assert!(tokens.contains("omnidea"));
    }

    // --- New hardening tests ---

    #[test]
    fn homophones_exist() {
        let vocab = Vocabulary::new(TEST_SEED);
        // Common tokens should have multiple homophones.
        let pool = vocab.forward_map.get("hello").expect("hello should be common");
        assert!(
            pool.len() > 1,
            "expected multiple homophones for 'hello', got {}",
            pool.len()
        );
        assert!(
            pool.len() <= MAX_HOMOPHONES,
            "too many homophones: {}",
            pool.len()
        );
    }

    #[test]
    fn all_homophones_decode_correctly() {
        let vocab = Vocabulary::new(TEST_SEED);
        for (token, pool) in &vocab.forward_map {
            for symbol in pool {
                let decoded = vocab.decode(symbol);
                assert_eq!(
                    &decoded, token,
                    "homophone '{symbol}' should decode to '{token}'"
                );
            }
        }
    }

    #[test]
    fn homophones_are_unique_symbols() {
        let vocab = Vocabulary::new(TEST_SEED);
        let mut all_symbols = HashSet::new();
        for pool in vocab.forward_map.values() {
            for symbol in pool {
                assert!(
                    all_symbols.insert(symbol.clone()),
                    "duplicate homophone symbol: '{symbol}'"
                );
            }
        }
    }

    #[test]
    fn multiple_byte_alphabets_exist() {
        let vocab = Vocabulary::new(TEST_SEED);
        assert_eq!(
            vocab.byte_alphabets.len(),
            BYTE_ALPHABET_COUNT,
            "expected {BYTE_ALPHABET_COUNT} byte alphabets"
        );
        // Each alphabet covers all 256 byte values.
        for (i, alphabet) in vocab.byte_alphabets.iter().enumerate() {
            assert_eq!(alphabet.len(), 256, "alphabet {i} should have 256 entries");
        }
    }

    #[test]
    fn byte_alphabets_differ() {
        let vocab = Vocabulary::new(TEST_SEED);
        // Alphabets should use different symbols (different shuffled positions).
        let a0 = &vocab.byte_alphabets[0];
        let a1 = &vocab.byte_alphabets[1];
        let same_count = a0.iter().zip(a1.iter()).filter(|(a, b)| a == b).count();
        assert!(
            same_count < 256,
            "byte alphabets 0 and 1 should differ (found {same_count}/256 same)"
        );
    }

    #[test]
    fn byte_alphabets_no_overlap_with_homophones() {
        let vocab = Vocabulary::new(TEST_SEED);
        let homophone_symbols: HashSet<&str> = vocab
            .reverse_map
            .keys()
            .map(|s| s.as_str())
            .collect();
        for (i, alphabet) in vocab.byte_alphabets.iter().enumerate() {
            for symbol in alphabet {
                assert!(
                    !homophone_symbols.contains(symbol.as_str()),
                    "byte alphabet {i} symbol '{symbol}' overlaps with homophones"
                );
            }
        }
    }

    #[test]
    fn byte_alphabet_covers_all_256_values() {
        let vocab = Vocabulary::new(TEST_SEED);
        for (i, alphabet) in vocab.byte_alphabets.iter().enumerate() {
            let unique: HashSet<&str> = alphabet.iter().map(|s| s.as_str()).collect();
            assert_eq!(
                unique.len(),
                256,
                "alphabet {i} should have 256 unique symbols"
            );
        }
    }
}
