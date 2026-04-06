use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{Arch, Crest, Gradient, ImageStyle, Impulse, Inscription, MotionPreference, Span, UmbraScale};
use crate::reign::Aspect;

/// The complete design token container. Serializes to `.excalibur` theme files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aura {
    pub light_crest: Crest,
    pub dark_crest: Crest,
    pub span: Span,
    pub inscription: Inscription,
    pub arch: Arch,
    pub umbra: UmbraScale,
    pub impulse: Impulse,
    pub gradients: HashMap<String, Gradient>,
    pub image_styles: HashMap<String, ImageStyle>,
    pub motion_preference: MotionPreference,
    /// Minimum touch target size in points (accessibility).
    pub minimum_touch_target: f64,
    /// Minimum font size in points (accessibility).
    pub minimum_font_size: f64,
}

impl Aura {
    /// Resolve the color palette for the given appearance mode.
    pub fn crest(&self, aspect: &Aspect) -> &Crest {
        if aspect == &Aspect::dark() {
            &self.dark_crest
        } else {
            &self.light_crest
        }
    }

    /// Access the gradient dictionary.
    pub fn gradients(&self) -> &HashMap<String, Gradient> {
        &self.gradients
    }

    /// Access the image style dictionary.
    pub fn image_styles(&self) -> &HashMap<String, ImageStyle> {
        &self.image_styles
    }
}

impl Default for Aura {
    fn default() -> Self {
        Self {
            light_crest: Crest::light_default(),
            dark_crest: Crest::dark_default(),
            span: Span::default(),
            inscription: Inscription::default(),
            arch: Arch::default(),
            umbra: UmbraScale::default(),
            impulse: Impulse::default(),
            gradients: HashMap::new(),
            image_styles: HashMap::new(),
            motion_preference: MotionPreference::default(),
            minimum_touch_target: 44.0,
            minimum_font_size: 12.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_crest_for_aspect() {
        let aura = Aura::default();
        let light = aura.crest(&Aspect::light());
        let dark = aura.crest(&Aspect::dark());
        // Light mode has black primary, dark mode has white primary
        assert_ne!(light.primary, dark.primary);
    }

    #[test]
    fn unknown_aspect_defaults_to_light() {
        let aura = Aura::default();
        let custom = Aspect::custom("high-contrast");
        let crest = aura.crest(&custom);
        // Unknown aspects get light mode
        assert_eq!(crest, &aura.light_crest);
    }

    #[test]
    fn serde_roundtrip() {
        let aura = Aura::default();
        let json = serde_json::to_string(&aura).unwrap();
        let decoded: Aura = serde_json::from_str(&json).unwrap();
        assert_eq!(aura, decoded);
    }
}
