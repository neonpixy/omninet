use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Font weight. Extensible via custom string values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlyphWeight(pub String);

impl GlyphWeight {
    pub const ULTRA_LIGHT: Self = Self(String::new()); // see Default impls below
    pub const THIN: Self = Self(String::new());
    pub const LIGHT: Self = Self(String::new());
    pub const REGULAR: Self = Self(String::new());
    pub const MEDIUM: Self = Self(String::new());
    pub const SEMIBOLD: Self = Self(String::new());
    pub const BOLD: Self = Self(String::new());
    pub const HEAVY: Self = Self(String::new());
    pub const BLACK: Self = Self(String::new());

    /// Ultra-light weight (100).
    pub fn ultra_light() -> Self {
        Self("ultraLight".into())
    }
    /// Thin weight (200).
    pub fn thin() -> Self {
        Self("thin".into())
    }
    /// Light weight (300).
    pub fn light() -> Self {
        Self("light".into())
    }
    /// Regular weight (400), the default.
    pub fn regular() -> Self {
        Self("regular".into())
    }
    /// Medium weight (500).
    pub fn medium() -> Self {
        Self("medium".into())
    }
    /// Semibold weight (600).
    pub fn semibold() -> Self {
        Self("semibold".into())
    }
    /// Bold weight (700).
    pub fn bold() -> Self {
        Self("bold".into())
    }
    /// Heavy weight (800).
    pub fn heavy() -> Self {
        Self("heavy".into())
    }
    /// Black weight (900).
    pub fn black() -> Self {
        Self("black".into())
    }
    /// User-defined font weight.
    pub fn custom(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl Default for GlyphWeight {
    fn default() -> Self {
        Self::regular()
    }
}

/// Single typography level: family, weight, size, optional line height and letter spacing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Glyph {
    /// Font family ("system" = platform default)
    pub family: String,
    pub weight: GlyphWeight,
    pub size: f64,
    pub line_height: Option<f64>,
    pub letter_spacing: Option<f64>,
}

impl Glyph {
    /// Create a new glyph with the given font family, weight, and size.
    pub fn new(
        family: impl Into<String>,
        weight: GlyphWeight,
        size: f64,
    ) -> Self {
        Self {
            family: family.into(),
            weight,
            size,
            line_height: None,
            letter_spacing: None,
        }
    }

    /// Set the line height for this glyph.
    pub fn with_line_height(mut self, lh: f64) -> Self {
        self.line_height = Some(lh);
        self
    }

    /// Set the letter spacing (tracking) for this glyph.
    pub fn with_letter_spacing(mut self, ls: f64) -> Self {
        self.letter_spacing = Some(ls);
        self
    }
}

impl Default for Glyph {
    fn default() -> Self {
        Self::new("system", GlyphWeight::regular(), 16.0)
    }
}

/// Typography scale with 6 named levels + custom extensibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inscription {
    pub display: Glyph,
    pub title: Glyph,
    pub headline: Glyph,
    pub body: Glyph,
    pub caption: Glyph,
    pub mono: Glyph,
    pub custom: HashMap<String, Glyph>,
}

impl Inscription {
    /// Look up a custom typography level by name.
    pub fn get_custom(&self, name: &str) -> Option<&Glyph> {
        self.custom.get(name)
    }
}

impl Default for Inscription {
    fn default() -> Self {
        Self {
            display: Glyph::new("system", GlyphWeight::bold(), 34.0),
            title: Glyph::new("system", GlyphWeight::semibold(), 28.0),
            headline: Glyph::new("system", GlyphWeight::semibold(), 20.0),
            body: Glyph::new("system", GlyphWeight::regular(), 16.0),
            caption: Glyph::new("system", GlyphWeight::regular(), 12.0),
            mono: Glyph::new("monospace", GlyphWeight::regular(), 14.0),
            custom: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_weight_equality() {
        assert_eq!(GlyphWeight::bold(), GlyphWeight::bold());
        assert_ne!(GlyphWeight::bold(), GlyphWeight::regular());
    }

    #[test]
    fn glyph_builder() {
        let g = Glyph::new("Helvetica", GlyphWeight::bold(), 24.0)
            .with_line_height(28.0)
            .with_letter_spacing(-0.5);
        assert_eq!(g.family, "Helvetica");
        assert_eq!(g.line_height, Some(28.0));
        assert_eq!(g.letter_spacing, Some(-0.5));
    }

    #[test]
    fn default_inscription_scale() {
        let i = Inscription::default();
        assert_eq!(i.display.size, 34.0);
        assert_eq!(i.body.size, 16.0);
        assert_eq!(i.caption.size, 12.0);
        assert_eq!(i.mono.family, "monospace");
    }

    #[test]
    fn custom_inscription() {
        let mut i = Inscription::default();
        i.custom.insert(
            "subtitle".into(),
            Glyph::new("system", GlyphWeight::medium(), 18.0),
        );
        assert!(i.get_custom("subtitle").is_some());
        assert_eq!(i.get_custom("subtitle").unwrap().size, 18.0);
    }

    #[test]
    fn serde_roundtrip() {
        let i = Inscription::default();
        let json = serde_json::to_string(&i).unwrap();
        let decoded: Inscription = serde_json::from_str(&json).unwrap();
        assert_eq!(i, decoded);
    }
}
