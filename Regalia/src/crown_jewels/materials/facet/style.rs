use serde::{Deserialize, Serialize};

use crate::aura::Ember;
use crate::crown_jewels::material::Material;
use crate::crown_jewels::materials::facet::appearance::FacetAppearance;
use crate::crown_jewels::materials::facet::delta::FacetStyleDelta;
use crate::crown_jewels::materials::facet::light_source::LightSource;
use crate::crown_jewels::materials::facet::variant::FacetVariant;

/// Glass material style. Controls all visual properties of a liquid glass
/// surface: blur (frost), refraction, chromatic aberration (dispersion),
/// lighting, tint, and more.
///
/// All shader properties are in the 0-1 range unless noted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FacetStyle {
    /// How the light direction is determined.
    pub light_source: LightSource,
    /// Glass material variant (regular or clear).
    pub variant: FacetVariant,
    /// Optional tint color (platform-independent RGBA).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tint: Option<Ember>,
    /// Tint blending amount (0-1).
    pub tint_opacity: f64,
    /// Corner radius override. None = inferred from clipping shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,

    // -- Shader properties (0-1) --
    /// Background bending through the glass.
    pub refraction: f64,
    /// Background blur/frosting amount.
    pub frost: f64,
    /// Chromatic color separation at edges.
    pub dispersion: f64,
    /// Magnification zoom effect.
    pub depth: f64,
    /// Radial barrel distortion.
    pub splay: f64,
    /// Light angle (0-1 maps to 0-360 degrees).
    pub light_rotation: f64,
    /// Rim light intensity.
    pub light_intensity: f64,
    /// Light gradient falloff (0=sharp, 1=soft).
    pub light_banding: f64,
    /// How far refraction extends inward from edges.
    pub edge_width: f64,

    // -- Adaptive properties --
    /// Adaptive tinting by backdrop luminance.
    pub resonance: bool,
    /// Inner light/shadow from backdrop.
    pub luminance: bool,
    /// World-space brilliance light source positions (x, y). Max 4.
    pub brilliance_sources: Vec<(f32, f32)>,
    /// Foreground color mode.
    pub appearance: FacetAppearance,
    /// Periodic backdrop refresh for animated content.
    pub dynamic_backdrop: bool,
}

impl Default for FacetStyle {
    fn default() -> Self {
        Self::REGULAR
    }
}

impl FacetStyle {
    /// Standard glass with default parameters.
    pub const REGULAR: Self = Self {
        light_source: LightSource::Fixed,
        variant: FacetVariant(String::new()), // set in regular()
        tint: None,
        tint_opacity: 0.0,
        corner_radius: None,
        refraction: 0.35,
        frost: 0.5,
        dispersion: 0.15,
        depth: 0.0,
        splay: 0.0,
        light_rotation: 0.6,
        light_intensity: 0.6,
        light_banding: 0.5,
        edge_width: 0.05,
        resonance: false,
        luminance: false,
        brilliance_sources: Vec::new(),
        appearance: FacetAppearance(String::new()), // set in regular()
        dynamic_backdrop: false,
    };

    /// Standard glass preset.
    pub fn regular() -> Self {
        Self {
            variant: FacetVariant::regular(),
            appearance: FacetAppearance::base(),
            ..Self::REGULAR
        }
    }

    /// Clear/transparent glass with reduced blur and refraction.
    pub fn clear() -> Self {
        Self {
            variant: FacetVariant::clear(),
            refraction: 0.2,
            frost: 0.3,
            ..Self::regular()
        }
    }

    /// Subtle glass — barely-there frosting and gentle refraction.
    pub fn subtle() -> Self {
        Self {
            refraction: 0.15,
            frost: 0.3,
            dispersion: 0.08,
            light_intensity: 0.4,
            ..Self::regular()
        }
    }

    /// Heavy frost — strong blur, visible refraction.
    pub fn frosted() -> Self {
        Self {
            refraction: 0.4,
            frost: 0.85,
            dispersion: 0.2,
            light_intensity: 0.7,
            ..Self::regular()
        }
    }

    /// Whether the renderer needs a per-pixel luminance mask.
    pub fn needs_luminance_mask(&self) -> bool {
        self.appearance != FacetAppearance::base()
    }
}

impl Material for FacetStyle {
    type Delta = FacetStyleDelta;

    fn applying(&self, delta: &FacetStyleDelta) -> Self {
        let mut s = self.clone();
        if let Some(d) = delta.frost_delta {
            s.frost = (s.frost + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.refraction_delta {
            s.refraction = (s.refraction + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.dispersion_delta {
            s.dispersion = (s.dispersion + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.depth_delta {
            s.depth = (s.depth + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.splay_delta {
            s.splay = (s.splay + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.light_rotation_delta {
            s.light_rotation = (s.light_rotation + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.light_intensity_delta {
            s.light_intensity = (s.light_intensity + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.light_banding_delta {
            s.light_banding = (s.light_banding + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.edge_width_delta {
            s.edge_width = (s.edge_width + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.tint_opacity_delta {
            s.tint_opacity = (s.tint_opacity + d).clamp(0.0, 1.0);
        }
        s
    }

    fn kind() -> &'static str {
        "facet"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let s = FacetStyle::regular();
        assert_eq!(s.light_source, LightSource::Fixed);
        assert_eq!(s.variant, FacetVariant::regular());
        assert!(s.tint.is_none());
        assert!((s.tint_opacity).abs() < 1e-10);
        assert!(s.corner_radius.is_none());
        assert!((s.refraction - 0.35).abs() < 1e-10);
        assert!((s.frost - 0.5).abs() < 1e-10);
        assert!((s.dispersion - 0.15).abs() < 1e-10);
        assert!((s.depth).abs() < 1e-10);
        assert!((s.splay).abs() < 1e-10);
        assert!((s.light_rotation - 0.6).abs() < 1e-10);
        assert!((s.light_intensity - 0.6).abs() < 1e-10);
        assert!((s.light_banding - 0.5).abs() < 1e-10);
        assert!((s.edge_width - 0.05).abs() < 1e-10);
        assert!(!s.resonance);
        assert!(!s.luminance);
        assert!(s.brilliance_sources.is_empty());
        assert_eq!(s.appearance, FacetAppearance::base());
        assert!(!s.dynamic_backdrop);
    }

    #[test]
    fn clear_preset() {
        let s = FacetStyle::clear();
        assert_eq!(s.variant, FacetVariant::clear());
        assert!((s.refraction - 0.2).abs() < 1e-10);
        assert!((s.frost - 0.3).abs() < 1e-10);
    }

    #[test]
    fn subtle_preset() {
        let s = FacetStyle::subtle();
        assert!((s.refraction - 0.15).abs() < 1e-10);
        assert!((s.frost - 0.3).abs() < 1e-10);
        assert!((s.dispersion - 0.08).abs() < 1e-10);
        assert!((s.light_intensity - 0.4).abs() < 1e-10);
    }

    #[test]
    fn frosted_preset() {
        let s = FacetStyle::frosted();
        assert!((s.refraction - 0.4).abs() < 1e-10);
        assert!((s.frost - 0.85).abs() < 1e-10);
        assert!((s.dispersion - 0.2).abs() < 1e-10);
        assert!((s.light_intensity - 0.7).abs() < 1e-10);
    }

    #[test]
    fn applying_delta() {
        let s = FacetStyle::regular();
        let d = FacetStyleDelta {
            frost_delta: Some(0.3),
            refraction_delta: Some(-0.1),
            ..Default::default()
        };
        let result = s.applying(&d);
        assert!((result.frost - 0.8).abs() < 1e-10);
        assert!((result.refraction - 0.25).abs() < 1e-10);
        // Unchanged fields stay the same.
        assert!((result.dispersion - 0.15).abs() < 1e-10);
    }

    #[test]
    fn applying_delta_clamps() {
        let s = FacetStyle::regular();
        let d = FacetStyleDelta {
            frost_delta: Some(2.0), // would push to 2.5, should clamp to 1.0
            refraction_delta: Some(-1.0), // would push to -0.65, should clamp to 0.0
            ..Default::default()
        };
        let result = s.applying(&d);
        assert!((result.frost - 1.0).abs() < 1e-10);
        assert!(result.refraction.abs() < 1e-10);
    }

    #[test]
    fn needs_luminance_mask() {
        let mut s = FacetStyle::regular();
        assert!(!s.needs_luminance_mask());
        s.appearance = FacetAppearance::dark();
        assert!(s.needs_luminance_mask());
    }

    #[test]
    fn serde_roundtrip() {
        let s = FacetStyle::frosted();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: FacetStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(s.frost, decoded.frost);
        assert_eq!(s.refraction, decoded.refraction);
        assert_eq!(s.variant, decoded.variant);
    }

    #[test]
    fn material_kind() {
        assert_eq!(FacetStyle::kind(), "facet");
    }
}
