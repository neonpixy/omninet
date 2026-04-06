use unicode_segmentation::UnicodeSegmentation;

/// Unicode script classification for tokenization strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    // Space-separated scripts
    /// Latin alphabet (English, Spanish, French, etc.).
    Latin,
    /// Arabic script (Arabic, Persian, Urdu).
    Arabic,
    /// Cyrillic script (Russian, Ukrainian, Bulgarian).
    Cyrillic,
    /// Devanagari script (Hindi, Marathi, Nepali, Sanskrit).
    Devanagari,
    /// Bengali/Bangla script.
    Bengali,
    /// Tamil script.
    Tamil,
    /// Telugu script.
    Telugu,
    /// Kannada script.
    Kannada,
    /// Malayalam script.
    Malayalam,
    /// Sinhala script.
    Sinhala,
    /// Georgian script.
    Georgian,
    /// Armenian script.
    Armenian,
    /// Greek script.
    Greek,
    /// Hebrew script.
    Hebrew,
    /// Ethiopic/Ge'ez script (Amharic, Tigrinya).
    Ethiopic,
    // Non-space scripts (character-level)
    /// CJK Unified Ideographs (Chinese, Japanese Kanji).
    Cjk,
    /// Japanese Kana (Hiragana + Katakana).
    Kana,
    /// Korean Hangul syllables.
    Hangul,
    // Non-space scripts (grapheme-level)
    /// Thai script (grapheme-cluster tokenization).
    Thai,
    /// Khmer script (grapheme-cluster tokenization).
    Khmer,
    /// Lao script (grapheme-cluster tokenization).
    Lao,
    /// Myanmar/Burmese script (grapheme-cluster tokenization).
    Myanmar,
    /// Tibetan script (grapheme-cluster tokenization).
    Tibetan,
    // Catch-all
    /// Any script not specifically recognized.
    Other,
}

/// Classify a character's Unicode script.
pub fn char_script(c: char) -> Script {
    match c as u32 {
        // Latin (Basic + Extended + Additional)
        0x0041..=0x007A | 0x00C0..=0x024F | 0x1E00..=0x1EFF
        | 0x2C60..=0x2C7F | 0xA720..=0xA7FF => Script::Latin,

        // CJK Unified Ideographs + Extensions
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2CEAF | 0x2F800..=0x2FA1F => Script::Cjk,
        // CJK Compatibility
        0x3300..=0x33FF | 0xFE30..=0xFE4F | 0xF900..=0xFAFF => Script::Cjk,

        // Hiragana + Katakana
        0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF65..=0xFF9F => Script::Kana,

        // Hangul Syllables + Jamo
        0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => Script::Hangul,

        // Arabic + Supplement + Extended
        0x0600..=0x06FF | 0x0750..=0x077F | 0x0870..=0x089F | 0x08A0..=0x08FF => Script::Arabic,

        // Hebrew
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => Script::Hebrew,

        // Cyrillic + Supplement + Extended
        0x0400..=0x04FF | 0x0500..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F => Script::Cyrillic,

        // Greek + Extended
        0x0370..=0x03FF | 0x1F00..=0x1FFF => Script::Greek,

        // Armenian
        0x0530..=0x058F | 0xFB00..=0xFB17 => Script::Armenian,

        // Georgian + Supplement
        0x10A0..=0x10FF | 0x2D00..=0x2D2F | 0x1C90..=0x1CBF => Script::Georgian,

        // Devanagari + Extended
        0x0900..=0x097F | 0xA8E0..=0xA8FF => Script::Devanagari,

        // Bengali / Bangla
        0x0980..=0x09FF => Script::Bengali,

        // Tamil
        0x0B80..=0x0BFF => Script::Tamil,

        // Telugu
        0x0C00..=0x0C7F => Script::Telugu,

        // Kannada
        0x0C80..=0x0CFF => Script::Kannada,

        // Malayalam
        0x0D00..=0x0D7F => Script::Malayalam,

        // Sinhala
        0x0D80..=0x0DFF => Script::Sinhala,

        // Ethiopic + Supplement + Extended
        0x1200..=0x137F | 0x1380..=0x139F | 0x2D80..=0x2DDF
        | 0xAB00..=0xAB2F => Script::Ethiopic,

        // Thai
        0x0E00..=0x0E7F => Script::Thai,

        // Lao
        0x0E80..=0x0EFF => Script::Lao,

        // Khmer + Symbols
        0x1780..=0x17FF | 0x19E0..=0x19FF => Script::Khmer,

        // Myanmar (Burmese)
        0x1000..=0x109F | 0xAA60..=0xAA7F => Script::Myanmar,

        // Tibetan
        0x0F00..=0x0FFF => Script::Tibetan,

        _ => Script::Other,
    }
}

/// All script variants in index order for counting.
pub const SCRIPT_VARIANTS: &[Script] = &[
    Script::Latin,      // 0
    Script::Cjk,        // 1
    Script::Kana,       // 2
    Script::Hangul,     // 3
    Script::Arabic,     // 4
    Script::Cyrillic,   // 5
    Script::Devanagari, // 6
    Script::Thai,       // 7
    Script::Bengali,    // 8
    Script::Tamil,      // 9
    Script::Telugu,     // 10
    Script::Kannada,    // 11
    Script::Malayalam,  // 12
    Script::Sinhala,    // 13
    Script::Georgian,   // 14
    Script::Armenian,   // 15
    Script::Greek,      // 16
    Script::Hebrew,     // 17
    Script::Ethiopic,   // 18
    Script::Khmer,      // 19
    Script::Lao,        // 20
    Script::Myanmar,    // 21
    Script::Tibetan,    // 22
    Script::Other,      // 23
];

/// Map a Script to its index in SCRIPT_VARIANTS.
pub fn script_index(s: Script) -> usize {
    match s {
        Script::Latin => 0,
        Script::Cjk => 1,
        Script::Kana => 2,
        Script::Hangul => 3,
        Script::Arabic => 4,
        Script::Cyrillic => 5,
        Script::Devanagari => 6,
        Script::Thai => 7,
        Script::Bengali => 8,
        Script::Tamil => 9,
        Script::Telugu => 10,
        Script::Kannada => 11,
        Script::Malayalam => 12,
        Script::Sinhala => 13,
        Script::Georgian => 14,
        Script::Armenian => 15,
        Script::Greek => 16,
        Script::Hebrew => 17,
        Script::Ethiopic => 18,
        Script::Khmer => 19,
        Script::Lao => 20,
        Script::Myanmar => 21,
        Script::Tibetan => 22,
        Script::Other => 23,
    }
}

/// Detect the dominant script in a text by majority vote.
pub fn dominant_script(text: &str) -> Script {
    let mut counts = [0u32; SCRIPT_VARIANTS.len()];

    for c in text.chars() {
        if c.is_whitespace() || c.is_ascii_punctuation() {
            continue;
        }
        counts[script_index(char_script(c))] += 1;
    }

    let (max_idx, _) = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .unwrap_or((0, &0));

    SCRIPT_VARIANTS.get(max_idx).copied().unwrap_or(Script::Other)
}

/// Returns true if this script tokenizes at the grapheme cluster level
/// (no spaces between words, needs Unicode segmentation).
fn is_grapheme_level(script: Script) -> bool {
    matches!(
        script,
        Script::Thai | Script::Khmer | Script::Lao | Script::Myanmar | Script::Tibetan
    )
}

/// Returns true if this character belongs to a script that tokenizes
/// at the character level (CJK, Kana, Hangul without spaces).
fn is_character_level(c: char) -> bool {
    matches!(char_script(c), Script::Cjk | Script::Kana | Script::Hangul)
}


/// Map a BCP 47 language code to its primary script.
///
/// Used by `Babel::decode_for_language` to determine how to rejoin
/// decoded tokens (spaces for Latin scripts, no separator for CJK).
pub fn script_for_language(language: &str) -> Script {
    // Normalize: take just the primary subtag (e.g., "zh-Hans" → "zh").
    let primary = language.split('-').next().unwrap_or(language);

    match primary {
        // Character-level (no spaces)
        "zh" => Script::Cjk,
        "ja" => Script::Kana,
        "ko" => Script::Hangul,

        // Grapheme-level (no spaces)
        "th" => Script::Thai,
        "km" => Script::Khmer,
        "lo" => Script::Lao,
        "my" => Script::Myanmar,
        "bo" | "dz" => Script::Tibetan,

        // Space-separated scripts
        "ar" | "fa" | "ur" => Script::Arabic,
        "he" | "yi" => Script::Hebrew,
        "ru" | "uk" | "bg" | "sr" | "mk" | "be" => Script::Cyrillic,
        "el" => Script::Greek,
        "hy" => Script::Armenian,
        "ka" => Script::Georgian,
        "hi" | "mr" | "ne" | "sa" => Script::Devanagari,
        "bn" | "as" => Script::Bengali,
        "ta" => Script::Tamil,
        "te" => Script::Telugu,
        "kn" => Script::Kannada,
        "ml" => Script::Malayalam,
        "si" => Script::Sinhala,
        "am" | "ti" => Script::Ethiopic,

        // Default: Latin (en, es, fr, de, pt, it, nl, sv, pl, tr, vi, id, etc.)
        _ => Script::Latin,
    }
}

/// Returns the join separator for decoded Babel tokens based on script.
///
/// Space-separated scripts (Latin, Arabic, Cyrillic, Devanagari) use " ".
/// Character-based scripts (CJK, Kana, Hangul, Thai) use "" (no separator).
pub fn join_separator_for_script(script: Script) -> &'static str {
    match script {
        // No separator for character-level and grapheme-level scripts.
        Script::Cjk | Script::Kana | Script::Hangul
        | Script::Thai | Script::Khmer | Script::Lao
        | Script::Myanmar | Script::Tibetan => "",
        // Space for everything else.
        _ => " ",
    }
}

/// Omnilingual tokenizer.
///
/// Splits text into tokens appropriate for the detected script:
/// - Space-separated scripts (Latin, Arabic, Cyrillic, Devanagari): split on whitespace
/// - Character-based scripts (CJK, Kana, Hangul): each character is a token
/// - Grapheme-based scripts (Thai): each grapheme cluster is a token
/// - Mixed text: whitespace split first, then sub-tokenize character-level segments
pub fn tokenize(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let script = dominant_script(text);

    if is_grapheme_level(script) {
        tokenize_grapheme(text)
    } else if matches!(script, Script::Cjk | Script::Kana | Script::Hangul) {
        tokenize_character_level(text)
    } else {
        tokenize_space_aware(text)
    }
}

/// Split on whitespace, but sub-tokenize any CJK/Kana/Hangul segments
/// at the character level.
fn tokenize_space_aware(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    for word in text.split_whitespace() {
        if word.chars().any(is_character_level) {
            // Mixed or pure CJK within a whitespace-delimited segment:
            // sub-tokenize character-level chars, keep others as runs.
            let mut run = String::new();
            for c in word.chars() {
                if is_character_level(c) {
                    if !run.is_empty() {
                        tokens.push(std::mem::take(&mut run));
                    }
                    tokens.push(c.to_string());
                } else {
                    run.push(c);
                }
            }
            if !run.is_empty() {
                tokens.push(run);
            }
        } else {
            tokens.push(word.to_string());
        }
    }

    tokens
}

/// Tokenize at the character level for CJK/Kana/Hangul-dominant text.
/// Non-CJK segments (like embedded Latin) are kept as whitespace-split words.
fn tokenize_character_level(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut run = String::new();

    for c in text.chars() {
        if c.is_whitespace() {
            if !run.is_empty() {
                tokens.push(std::mem::take(&mut run));
            }
            continue;
        }

        if is_character_level(c) {
            if !run.is_empty() {
                tokens.push(std::mem::take(&mut run));
            }
            tokens.push(c.to_string());
        } else {
            run.push(c);
        }
    }

    if !run.is_empty() {
        tokens.push(run);
    }

    tokens
}

/// Tokenize using Unicode grapheme clusters (for Thai, Khmer, etc.).
fn tokenize_grapheme(text: &str) -> Vec<String> {
    text.graphemes(true)
        .filter(|g| !g.chars().all(char::is_whitespace))
        .map(|g| g.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_english() {
        let tokens = tokenize("Hello world");
        assert_eq!(tokens, vec!["Hello", "world"]);
    }

    #[test]
    fn tokenize_english_multiple_spaces() {
        let tokens = tokenize("Hello   world   test");
        assert_eq!(tokens, vec!["Hello", "world", "test"]);
    }

    #[test]
    fn tokenize_chinese() {
        let tokens = tokenize("你好世界");
        assert_eq!(tokens, vec!["你", "好", "世", "界"]);
    }

    #[test]
    fn tokenize_japanese_mixed() {
        // Hiragana + Kanji
        let tokens = tokenize("こんにちは世界");
        assert_eq!(tokens, vec!["こ", "ん", "に", "ち", "は", "世", "界"]);
    }

    #[test]
    fn tokenize_korean() {
        let tokens = tokenize("안녕하세요");
        assert_eq!(tokens, vec!["안", "녕", "하", "세", "요"]);
    }

    #[test]
    fn tokenize_arabic() {
        let tokens = tokenize("مرحبا بالعالم");
        assert_eq!(tokens, vec!["مرحبا", "بالعالم"]);
    }

    #[test]
    fn tokenize_russian() {
        let tokens = tokenize("Привет мир");
        assert_eq!(tokens, vec!["Привет", "мир"]);
    }

    #[test]
    fn tokenize_thai() {
        // Each grapheme cluster is a token
        let tokens = tokenize("สวัสดี");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn tokenize_mixed_english_chinese() {
        let tokens = tokenize("Hello你好world");
        // "Hello" is Latin run, then 你 好 as individual chars, then "world" as Latin
        assert!(tokens.contains(&"Hello".to_string()));
        assert!(tokens.contains(&"你".to_string()));
        assert!(tokens.contains(&"好".to_string()));
        assert!(tokens.contains(&"world".to_string()));
    }

    #[test]
    fn tokenize_empty() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_whitespace_only() {
        let tokens = tokenize("   ");
        assert!(tokens.is_empty());
    }

    #[test]
    fn detect_script_latin() {
        assert_eq!(char_script('A'), Script::Latin);
        assert_eq!(char_script('z'), Script::Latin);
    }

    #[test]
    fn detect_script_cjk() {
        assert_eq!(char_script('你'), Script::Cjk);
        assert_eq!(char_script('世'), Script::Cjk);
    }

    #[test]
    fn detect_script_kana() {
        assert_eq!(char_script('あ'), Script::Kana);
        assert_eq!(char_script('ア'), Script::Kana);
    }

    #[test]
    fn detect_script_hangul() {
        assert_eq!(char_script('한'), Script::Hangul);
    }

    #[test]
    fn detect_script_arabic() {
        assert_eq!(char_script('م'), Script::Arabic);
    }

    #[test]
    fn detect_script_cyrillic() {
        assert_eq!(char_script('Д'), Script::Cyrillic);
    }

    #[test]
    fn dominant_script_detection() {
        assert_eq!(dominant_script("Hello world"), Script::Latin);
        assert_eq!(dominant_script("你好世界"), Script::Cjk);
        assert_eq!(dominant_script("Привет мир"), Script::Cyrillic);
    }

    #[test]
    fn script_for_language_mapping() {
        // Character-level (no spaces)
        assert_eq!(script_for_language("zh-Hans"), Script::Cjk);
        assert_eq!(script_for_language("zh"), Script::Cjk);
        assert_eq!(script_for_language("ja"), Script::Kana);
        assert_eq!(script_for_language("ko"), Script::Hangul);

        // Grapheme-level (no spaces)
        assert_eq!(script_for_language("th"), Script::Thai);
        assert_eq!(script_for_language("km"), Script::Khmer);
        assert_eq!(script_for_language("lo"), Script::Lao);
        assert_eq!(script_for_language("my"), Script::Myanmar);
        assert_eq!(script_for_language("bo"), Script::Tibetan);

        // Space-separated
        assert_eq!(script_for_language("en"), Script::Latin);
        assert_eq!(script_for_language("es"), Script::Latin);
        assert_eq!(script_for_language("fr"), Script::Latin);
        assert_eq!(script_for_language("ar"), Script::Arabic);
        assert_eq!(script_for_language("he"), Script::Hebrew);
        assert_eq!(script_for_language("ru"), Script::Cyrillic);
        assert_eq!(script_for_language("el"), Script::Greek);
        assert_eq!(script_for_language("hy"), Script::Armenian);
        assert_eq!(script_for_language("ka"), Script::Georgian);
        assert_eq!(script_for_language("hi"), Script::Devanagari);
        assert_eq!(script_for_language("bn"), Script::Bengali);
        assert_eq!(script_for_language("ta"), Script::Tamil);
        assert_eq!(script_for_language("te"), Script::Telugu);
        assert_eq!(script_for_language("kn"), Script::Kannada);
        assert_eq!(script_for_language("ml"), Script::Malayalam);
        assert_eq!(script_for_language("si"), Script::Sinhala);
        assert_eq!(script_for_language("am"), Script::Ethiopic);
    }

    #[test]
    fn join_separator_space_vs_none() {
        // Space-separated
        assert_eq!(join_separator_for_script(Script::Latin), " ");
        assert_eq!(join_separator_for_script(Script::Arabic), " ");
        assert_eq!(join_separator_for_script(Script::Hebrew), " ");
        assert_eq!(join_separator_for_script(Script::Cyrillic), " ");
        assert_eq!(join_separator_for_script(Script::Greek), " ");
        assert_eq!(join_separator_for_script(Script::Devanagari), " ");
        assert_eq!(join_separator_for_script(Script::Bengali), " ");
        assert_eq!(join_separator_for_script(Script::Tamil), " ");
        assert_eq!(join_separator_for_script(Script::Ethiopic), " ");

        // No separator
        assert_eq!(join_separator_for_script(Script::Cjk), "");
        assert_eq!(join_separator_for_script(Script::Kana), "");
        assert_eq!(join_separator_for_script(Script::Hangul), "");
        assert_eq!(join_separator_for_script(Script::Thai), "");
        assert_eq!(join_separator_for_script(Script::Khmer), "");
        assert_eq!(join_separator_for_script(Script::Lao), "");
        assert_eq!(join_separator_for_script(Script::Myanmar), "");
        assert_eq!(join_separator_for_script(Script::Tibetan), "");
    }

    #[test]
    fn detect_new_scripts() {
        assert_eq!(char_script('α'), Script::Greek);       // Greek alpha
        assert_eq!(char_script('შ'), Script::Georgian);     // Georgian
        assert_eq!(char_script('ב'), Script::Hebrew);       // Hebrew bet
        assert_eq!(char_script('ব'), Script::Bengali);      // Bengali
        assert_eq!(char_script('த'), Script::Tamil);        // Tamil
        assert_eq!(char_script('త'), Script::Telugu);       // Telugu
        assert_eq!(char_script('ಕ'), Script::Kannada);      // Kannada
        assert_eq!(char_script('മ'), Script::Malayalam);     // Malayalam
        assert_eq!(char_script('ස'), Script::Sinhala);      // Sinhala
        assert_eq!(char_script('ᎀ'), Script::Ethiopic);     // Ethiopic
        assert_eq!(char_script('ខ'), Script::Khmer);        // Khmer
        assert_eq!(char_script('ກ'), Script::Lao);          // Lao
        assert_eq!(char_script('က'), Script::Myanmar);       // Myanmar
        assert_eq!(char_script('ག'), Script::Tibetan);      // Tibetan
    }
}
