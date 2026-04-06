use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::aura::{
    Arch, Aura, Crest, Gradient, ImageStyle, Impulse, Inscription, MotionPreference, Span,
    UmbraScale,
};

/// Appearance mode. Extensible via custom string values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Aspect(pub String);

impl Aspect {
    /// Light appearance mode.
    pub fn light() -> Self {
        Self("light".into())
    }
    /// Dark appearance mode.
    pub fn dark() -> Self {
        Self("dark".into())
    }
    /// User-defined appearance mode (e.g., "high-contrast").
    pub fn custom(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    /// The name of this appearance mode.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for Aspect {
    fn default() -> Self {
        Self::light()
    }
}

/// Complete theme: name + tokens + appearance mode.
///
/// Serializes to `.excalibur` theme files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reign {
    pub name: String,
    pub aura: Aura,
    pub aspect: Aspect,
}

impl Reign {
    /// Create a new theme with the given name, token set, and appearance mode.
    pub fn new(name: impl Into<String>, aura: Aura, aspect: Aspect) -> Self {
        Self {
            name: name.into(),
            aura,
            aspect,
        }
    }

    /// Resolved color palette for the active appearance.
    pub fn crest(&self) -> &Crest {
        self.aura.crest(&self.aspect)
    }

    /// Shortcut to spacing tokens.
    pub fn span(&self) -> &Span {
        &self.aura.span
    }

    /// Shortcut to typography tokens.
    pub fn inscription(&self) -> &Inscription {
        &self.aura.inscription
    }

    /// Shortcut to corner radii tokens.
    pub fn arch(&self) -> &Arch {
        &self.aura.arch
    }

    /// Shortcut to shadow tokens.
    pub fn umbra(&self) -> &UmbraScale {
        &self.aura.umbra
    }

    /// Shortcut to animation preset.
    pub fn impulse(&self) -> &Impulse {
        &self.aura.impulse
    }

    /// Shortcut to gradient dictionary.
    pub fn gradients(&self) -> &HashMap<String, Gradient> {
        &self.aura.gradients
    }

    /// Shortcut to image style dictionary.
    pub fn image_styles(&self) -> &HashMap<String, ImageStyle> {
        &self.aura.image_styles
    }

    /// Shortcut to motion preference.
    pub fn motion_preference(&self) -> &MotionPreference {
        &self.aura.motion_preference
    }

    /// Shortcut to minimum touch target size.
    pub fn minimum_touch_target(&self) -> f64 {
        self.aura.minimum_touch_target
    }

    /// Shortcut to minimum font size.
    pub fn minimum_font_size(&self) -> f64 {
        self.aura.minimum_font_size
    }
}

impl Default for Reign {
    fn default() -> Self {
        Self::new("Default", Aura::default(), Aspect::light())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aura::Ember;

    #[test]
    fn default_reign() {
        let r = Reign::default();
        assert_eq!(r.name, "Default");
        assert_eq!(r.aspect, Aspect::light());
    }

    #[test]
    fn crest_resolves_by_aspect() {
        let light = Reign::new("Light", Aura::default(), Aspect::light());
        let dark = Reign::new("Dark", Aura::default(), Aspect::dark());
        // Light mode primary = black, dark mode primary = white
        assert_eq!(light.crest().primary, Ember::BLACK);
        assert_eq!(dark.crest().primary, Ember::WHITE);
    }

    #[test]
    fn shortcuts() {
        let r = Reign::default();
        assert_eq!(r.span().md, 16.0);
        assert_eq!(r.inscription().body.size, 16.0);
        assert_eq!(r.arch().md, 12.0);
        assert!(r.umbra().subtle.radius > 0.0);
        assert_eq!(r.impulse(), &Impulse::smooth());
    }

    #[test]
    fn custom_aspect() {
        let r = Reign::new("High Contrast", Aura::default(), Aspect::custom("high-contrast"));
        assert_eq!(r.aspect.name(), "high-contrast");
        // Unknown aspect defaults to light crest
        assert_eq!(r.crest().primary, Ember::BLACK);
    }

    #[test]
    fn serde_roundtrip() {
        let r = Reign::default();
        let json = serde_json::to_string(&r).unwrap();
        let decoded: Reign = serde_json::from_str(&json).unwrap();
        assert_eq!(r, decoded);
    }

    #[test]
    fn excalibur_theme_file() {
        let r = Reign::new("Ocean", Aura::default(), Aspect::dark());
        let json = serde_json::to_string_pretty(&r).unwrap();
        assert!(json.contains("Ocean"));
        // Can be read back
        let decoded: Reign = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, "Ocean");
    }
}
