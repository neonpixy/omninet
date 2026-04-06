use crate::tokenizer::{char_script, script_index, Script, SCRIPT_VARIANTS};

/// Detect the language of text using Unicode script heuristics.
///
/// Returns a BCP 47 language code, or `None` if the text is empty
/// or unrecognizable. This is a best-effort heuristic — for accurate
/// detection, use a `TranslationProvider` if available.
///
/// Supports 20+ scripts covering the vast majority of the world's
/// written languages.
pub fn detect_language(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }

    let mut counts = [0u32; SCRIPT_VARIANTS.len()];
    let mut kana_count = 0u32;
    let mut total = 0u32;

    for c in text.chars() {
        if c.is_whitespace() || c.is_ascii_punctuation() || c.is_ascii_digit() {
            continue;
        }
        total += 1;
        let script = char_script(c);
        counts[script_index(script)] += 1;
        if script == Script::Kana {
            kana_count += 1;
        }
    }

    if total == 0 {
        return None;
    }

    // Find the dominant script (skip Other at the end).
    let (max_idx, max_count) = counts
        .iter()
        .enumerate()
        .take(SCRIPT_VARIANTS.len() - 1) // exclude Other
        .max_by_key(|(_, count)| **count)?;

    if *max_count == 0 {
        return None;
    }

    let dominant = SCRIPT_VARIANTS[max_idx];

    // CJK + Kana disambiguation: kana present → Japanese, else Chinese.
    let cjk_total = counts[script_index(Script::Cjk)] + counts[script_index(Script::Kana)];
    if (dominant == Script::Cjk || dominant == Script::Kana) && cjk_total > 0 {
        return if kana_count > 0 {
            Some("ja".into())
        } else {
            Some("zh-Hans".into())
        };
    }

    // Map dominant script to BCP 47 code.
    // For scripts shared by multiple languages, we pick the most common.
    match dominant {
        Script::Latin => Some("en".into()),
        Script::Cjk => Some("zh-Hans".into()),
        Script::Kana => Some("ja".into()),
        Script::Hangul => Some("ko".into()),
        Script::Arabic => Some("ar".into()),
        Script::Hebrew => Some("he".into()),
        Script::Cyrillic => Some("ru".into()),
        Script::Greek => Some("el".into()),
        Script::Armenian => Some("hy".into()),
        Script::Georgian => Some("ka".into()),
        Script::Devanagari => Some("hi".into()),
        Script::Bengali => Some("bn".into()),
        Script::Tamil => Some("ta".into()),
        Script::Telugu => Some("te".into()),
        Script::Kannada => Some("kn".into()),
        Script::Malayalam => Some("ml".into()),
        Script::Sinhala => Some("si".into()),
        Script::Ethiopic => Some("am".into()),
        Script::Thai => Some("th".into()),
        Script::Khmer => Some("km".into()),
        Script::Lao => Some("lo".into()),
        Script::Myanmar => Some("my".into()),
        Script::Tibetan => Some("bo".into()),
        Script::Other => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_english() {
        assert_eq!(detect_language("Hello world"), Some("en".into()));
    }

    #[test]
    fn detect_japanese() {
        assert_eq!(detect_language("こんにちは世界"), Some("ja".into()));
    }

    #[test]
    fn detect_chinese() {
        assert_eq!(detect_language("你好世界"), Some("zh-Hans".into()));
    }

    #[test]
    fn detect_korean() {
        assert_eq!(detect_language("안녕하세요"), Some("ko".into()));
    }

    #[test]
    fn detect_arabic() {
        assert_eq!(detect_language("مرحبا بالعالم"), Some("ar".into()));
    }

    #[test]
    fn detect_russian() {
        assert_eq!(detect_language("Привет мир"), Some("ru".into()));
    }

    #[test]
    fn detect_thai() {
        assert_eq!(detect_language("สวัสดีครับ"), Some("th".into()));
    }

    #[test]
    fn detect_hebrew() {
        assert_eq!(detect_language("שלום עולם"), Some("he".into()));
    }

    #[test]
    fn detect_hindi() {
        assert_eq!(detect_language("नमस्ते दुनिया"), Some("hi".into()));
    }

    #[test]
    fn detect_greek() {
        assert_eq!(detect_language("Γεια σου κόσμε"), Some("el".into()));
    }

    #[test]
    fn detect_georgian() {
        assert_eq!(detect_language("გამარჯობა"), Some("ka".into()));
    }

    #[test]
    fn detect_armenian() {
        // Armenian script characters: Բ Ա Ր Ե Վ
        assert_eq!(detect_language("\u{0532}\u{0531}\u{0550}\u{0535}\u{054E}"), Some("hy".into()));
    }

    #[test]
    fn detect_bengali() {
        assert_eq!(detect_language("নমস্কার বিশ্ব"), Some("bn".into()));
    }

    #[test]
    fn detect_tamil() {
        assert_eq!(detect_language("வணக்கம் உலகம்"), Some("ta".into()));
    }

    #[test]
    fn detect_khmer() {
        assert_eq!(detect_language("សួស្តី"), Some("km".into()));
    }

    #[test]
    fn detect_empty() {
        assert_eq!(detect_language(""), None);
    }

    #[test]
    fn detect_whitespace_only() {
        assert_eq!(detect_language("   "), None);
    }

    #[test]
    fn detect_numbers_only() {
        assert_eq!(detect_language("12345"), None);
    }
}
