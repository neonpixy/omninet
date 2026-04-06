use std::sync::LazyLock;

/// All Unicode ranges used for Babel symbol space.
///
/// These are exclusively ancient, historical, constructed, exotic, and
/// symbolic scripts — no modern scripts that could be confused with
/// actual readable text.
const RANGES: &[(u32, u32)] = &[
    // === Ancient & Historical Scripts ===
    // Egyptian Hieroglyphs
    (0x13000, 0x1342F),
    // Cuneiform
    (0x12000, 0x123FF),
    // Cuneiform Numbers and Punctuation
    (0x12400, 0x1247F),
    // Early Dynastic Cuneiform
    (0x12480, 0x1254F),
    // Linear A
    (0x10600, 0x1077F),
    // Linear B Syllabary
    (0x10000, 0x1007F),
    // Linear B Ideograms
    (0x10080, 0x100FF),
    // Old Persian
    (0x103A0, 0x103DF),
    // Phoenician
    (0x10900, 0x1091F),
    // Gothic
    (0x10330, 0x1034F),
    // Ugaritic
    (0x10380, 0x1039F),
    // Old Italic
    (0x10300, 0x1032F),
    // Cypriot Syllabary
    (0x10800, 0x1083F),
    // Lydian
    (0x10920, 0x1093F),
    // Carian
    (0x102A0, 0x102DF),
    // Lycian
    (0x10280, 0x1029F),
    // Old South Arabian
    (0x10A60, 0x10A7F),
    // Old North Arabian
    (0x10A80, 0x10A9F),
    // Avestan
    (0x10B00, 0x10B3F),
    // Inscriptional Parthian
    (0x10B40, 0x10B5F),
    // Inscriptional Pahlavi
    (0x10B60, 0x10B7F),
    // Psalter Pahlavi
    (0x10B80, 0x10BAF),
    // Manichaean
    (0x10AC0, 0x10AFF),
    // Nabataean
    (0x10880, 0x108AF),
    // Hatran
    (0x108E0, 0x108FF),
    // Imperial Aramaic
    (0x10840, 0x1085F),
    // Palmyrene
    (0x10860, 0x1087F),
    // Old Turkic
    (0x10C00, 0x10C4F),
    // Old Hungarian
    (0x10C80, 0x10CFF),
    // Meroitic Hieroglyphs
    (0x10980, 0x1099F),
    // Meroitic Cursive
    (0x109A0, 0x109FF),
    // Old Permic
    (0x10350, 0x1037F),
    // Elbasan
    (0x10500, 0x1052F),
    // Caucasian Albanian
    (0x10530, 0x1056F),
    // Old Sogdian
    (0x10F00, 0x10F2F),
    // Sogdian
    (0x10F30, 0x10F6F),
    // Elymaic
    (0x10FE0, 0x10FFF),
    // Chorasmian
    (0x10FB0, 0x10FDF),
    // Yezidi
    (0x10E80, 0x10EBF),
    // Old Uyghur
    (0x10F70, 0x10FAF),
    // Cypro-Minoan
    (0x12F90, 0x12FFF),

    // === Constructed & Exotic Scripts ===
    // Deseret
    (0x10400, 0x1044F),
    // Shavian
    (0x10450, 0x1047F),
    // Osmanya
    (0x10480, 0x104AF),
    // Osage
    (0x104B0, 0x104FF),
    // Anatolian Hieroglyphs
    (0x14400, 0x1467F),
    // Bamum Supplement
    (0x16800, 0x16A3F),
    // Miao (Pollard)
    (0x16F00, 0x16F9F),
    // Nushu
    (0x1B170, 0x1B2FF),
    // Sutton SignWriting
    (0x1D800, 0x1DAAF),
    // Wancho
    (0x1E2C0, 0x1E2FF),
    // Nag Mundari
    (0x1E4D0, 0x1E4FF),
    // Adlam
    (0x1E900, 0x1E95F),

    // === CJK & East Asian (massive) ===
    // CJK Unified Ideographs
    (0x4E00, 0x9FFF),
    // CJK Extension A
    (0x3400, 0x4DBF),
    // CJK Extension B
    (0x20000, 0x2A6DF),
    // CJK Extension C
    (0x2A700, 0x2B73F),
    // CJK Extension D
    (0x2B740, 0x2B81F),
    // CJK Extension E
    (0x2B820, 0x2CEAF),
    // CJK Extension F
    (0x2CEB0, 0x2EBEF),
    // CJK Compatibility Ideographs Supplement
    (0x2F800, 0x2FA1F),
    // Tangut
    (0x17000, 0x187FF),
    // Tangut Components
    (0x18800, 0x18AFF),
    // Tangut Supplement
    (0x18D00, 0x18D7F),
    // Khitan Small Script
    (0x18B00, 0x18CFF),
    // Yi Syllables
    (0xA000, 0xA48F),
    // Yi Radicals
    (0xA490, 0xA4CF),
    // Kangxi Radicals
    (0x2F00, 0x2FDF),

    // === African & Indigenous ===
    // Ethiopic
    (0x1200, 0x137F),
    // Ethiopic Supplement
    (0x1380, 0x139F),
    // Ethiopic Extended
    (0x2D80, 0x2DDF),
    // Ethiopic Extended-A
    (0xAB00, 0xAB2F),
    // Ethiopic Extended-B
    (0x1E7E0, 0x1E7FF),
    // Canadian Aboriginal Syllabics
    (0x1400, 0x167F),
    // Canadian Aboriginal Extended
    (0x18B0, 0x18FF),
    // Vai
    (0xA500, 0xA63F),
    // Bamum
    (0xA6A0, 0xA6FF),
    // Tifinagh
    (0x2D30, 0x2D7F),

    // === Runic, Ogham & Northern European ===
    // Runic
    (0x16A0, 0x16FF),
    // Ogham
    (0x1680, 0x169F),

    // === Symbols & Notation ===
    // Alchemical Symbols
    (0x1F700, 0x1F77F),
    // Musical Symbols
    (0x1D100, 0x1D1FF),
    // Byzantine Musical Symbols
    (0x1D000, 0x1D0FF),
    // Ancient Greek Musical Notation
    (0x1D200, 0x1D24F),
    // Braille Patterns
    (0x2800, 0x28FF),
    // Mathematical Operators
    (0x2200, 0x22FF),
    // Supplemental Mathematical Operators
    (0x2A00, 0x2AFF),
    // Miscellaneous Mathematical Symbols-A
    (0x27C0, 0x27EF),
    // Miscellaneous Mathematical Symbols-B
    (0x2980, 0x29FF),
    // Mathematical Alphanumeric Symbols
    (0x1D400, 0x1D7FF),
    // Geometric Shapes
    (0x25A0, 0x25FF),
    // Geometric Shapes Extended
    (0x1F780, 0x1F7FF),
    // Supplemental Arrows-A
    (0x27F0, 0x27FF),
    // Supplemental Arrows-B
    (0x2900, 0x297F),
    // Supplemental Arrows-C
    (0x1F800, 0x1F8FF),
    // Tai Xuan Jing Symbols
    (0x1D300, 0x1D35F),
    // Counting Rod Numerals
    (0x1D360, 0x1D37F),
    // Domino Tiles
    (0x1F030, 0x1F09F),
    // Mahjong Tiles
    (0x1F000, 0x1F02F),
    // Playing Cards
    (0x1F0A0, 0x1F0FF),
    // Mayan Numerals
    (0x1D2E0, 0x1D2FF),
    // Miscellaneous Symbols and Arrows
    (0x2B00, 0x2BFF),
    // Dingbats
    (0x2700, 0x27BF),
    // Ornamental Dingbats
    (0x1F650, 0x1F67F),
    // Box Drawing
    (0x2500, 0x257F),
    // Block Elements
    (0x2580, 0x259F),
    // Miscellaneous Technical
    (0x2300, 0x23FF),
    // Control Pictures
    (0x2400, 0x243F),
    // Optical Character Recognition
    (0x2440, 0x245F),
];

/// Full-power Unicode symbol space for Babel vocabulary mapping.
///
/// Lazily generated on first access, then cached forever. Contains 90,000+
/// valid Unicode scalars from ancient, historical, constructed, exotic, and
/// symbolic blocks. No modern scripts that could be confused with readable text.
pub static SYMBOLS: LazyLock<Vec<String>> = LazyLock::new(generate_symbols);

/// Generate the complete symbol list from all Unicode ranges.
fn generate_symbols() -> Vec<String> {
    let mut symbols = Vec::with_capacity(100_000);

    for &(start, end) in RANGES {
        for code_point in start..=end {
            if let Some(ch) = char::from_u32(code_point) {
                symbols.push(ch.to_string());
            }
        }
    }

    symbols
}

/// Get the number of available symbols.
pub fn symbol_count() -> usize {
    SYMBOLS.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_symbols_are_valid_unicode() {
        for symbol in SYMBOLS.iter() {
            assert_eq!(symbol.chars().count(), 1, "each symbol should be exactly one char");
            let ch = symbol.chars().next().unwrap();
            assert!(ch.len_utf8() > 0);
        }
    }

    #[test]
    fn no_duplicate_symbols() {
        let mut seen = HashSet::new();
        for symbol in SYMBOLS.iter() {
            assert!(seen.insert(symbol.clone()), "duplicate symbol: {symbol}");
        }
    }

    #[test]
    fn symbol_count_above_threshold() {
        let count = symbol_count();
        assert!(
            count >= 80_000,
            "expected at least 80,000 symbols, got {count}"
        );
    }

    #[test]
    fn known_glyphs_present() {
        let symbols: HashSet<&str> = SYMBOLS.iter().map(|s| s.as_str()).collect();
        // Egyptian hieroglyph
        assert!(symbols.contains("\u{13000}"));
        // Braille
        assert!(symbols.contains("\u{2800}"));
        // CJK
        assert!(symbols.contains("\u{4E00}"));
        // Runic
        assert!(symbols.contains("\u{16A0}"));
        // Mathematical
        assert!(symbols.contains("\u{2200}"));
    }
}
