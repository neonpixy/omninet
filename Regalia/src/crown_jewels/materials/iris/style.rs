use serde::{Deserialize, Serialize};

use crate::crown_jewels::material::Material;
use crate::crown_jewels::materials::iris::delta::IrisStyleDelta;
use crate::crown_jewels::materials::iris::dimple::IrisDimple;

/// Thin-film interference material. Simulates iridescent surfaces like
/// nacre, oil slicks, beetle shells, and soap bubbles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrisStyle {
    // -- Film stack --
    /// Number of superimposed films (1-6).
    pub layer_count: u32,
    /// Optical thickness (0.3-3.0).
    pub base_thickness: f64,
    /// Inter-layer variation (0.0-1.0).
    pub thickness_spread: f64,
    /// Global thickness multiplier (0.5-3.0).
    pub thickness_scale: f64,

    // -- Dimples --
    /// Light scatter points on the surface (0-16).
    pub dimples: Vec<IrisDimple>,

    // -- Appearance --
    /// Color saturation (0-1).
    pub intensity: f64,
    /// Brightness multiplier (0-1).
    pub brightness: f64,
    /// Vignette fade at edges (0-1).
    pub edge_fade: f64,
    /// Backdrop lens strength (0-1).
    pub refraction: f64,
    /// Chromatic aberration (0-1).
    pub dispersion: f64,
    /// Spectral drift animation enabled.
    pub animated: bool,
    /// Animation speed (0-3).
    pub shift_speed: f64,
    /// Overall opacity (0-1).
    pub opacity: f64,

    // -- Gleam tracking --
    /// Dimple position shift from light tracking (0-1).
    pub gleam_influence: f64,
    /// Max shift distance (0-0.5).
    pub gleam_radius: f64,
}

impl Default for IrisStyle {
    fn default() -> Self {
        Self::nacre()
    }
}

impl IrisStyle {
    /// Pearlescent, subtle, 3 layers.
    pub fn nacre() -> Self {
        Self {
            layer_count: 3,
            base_thickness: 1.0,
            thickness_spread: 0.6,
            thickness_scale: 1.0,
            dimples: vec![IrisDimple::default()],
            intensity: 0.85,
            brightness: 0.9,
            edge_fade: 0.15,
            refraction: 0.3,
            dispersion: 0.2,
            animated: true,
            shift_speed: 0.3,
            opacity: 1.0,
            gleam_influence: 0.5,
            gleam_radius: 0.2,
        }
    }

    /// Liquid, wide bands, 2 layers.
    pub fn oil_slick() -> Self {
        Self {
            layer_count: 2,
            base_thickness: 1.5,
            thickness_spread: 0.8,
            thickness_scale: 1.2,
            dimples: vec![IrisDimple::default()],
            intensity: 0.9,
            brightness: 0.85,
            edge_fade: 0.1,
            refraction: 0.25,
            dispersion: 0.25,
            animated: true,
            shift_speed: 0.2,
            opacity: 1.0,
            gleam_influence: 0.6,
            gleam_radius: 0.25,
        }
    }

    /// Tight, dense banding, 4 layers.
    pub fn beetle() -> Self {
        Self {
            layer_count: 4,
            base_thickness: 0.7,
            thickness_spread: 0.3,
            thickness_scale: 0.8,
            dimples: vec![IrisDimple::default()],
            intensity: 0.95,
            brightness: 0.8,
            edge_fade: 0.2,
            refraction: 0.15,
            dispersion: 0.1,
            animated: true,
            shift_speed: 0.15,
            opacity: 1.0,
            gleam_influence: 0.3,
            gleam_radius: 0.15,
        }
    }

    /// Soft, transparent, 1 layer.
    pub fn bubble() -> Self {
        Self {
            layer_count: 1,
            base_thickness: 1.2,
            thickness_spread: 0.0,
            thickness_scale: 1.0,
            dimples: vec![],
            intensity: 0.7,
            brightness: 0.95,
            edge_fade: 0.05,
            refraction: 0.4,
            dispersion: 0.3,
            animated: true,
            shift_speed: 0.4,
            opacity: 0.8,
            gleam_influence: 0.7,
            gleam_radius: 0.3,
        }
    }

    /// Maximum spectral impact, saturated.
    pub fn vivid() -> Self {
        Self {
            layer_count: 3,
            base_thickness: 1.0,
            thickness_spread: 0.7,
            thickness_scale: 1.5,
            dimples: vec![IrisDimple::default()],
            intensity: 1.0,
            brightness: 1.0,
            edge_fade: 0.0,
            refraction: 0.35,
            dispersion: 0.3,
            animated: true,
            shift_speed: 0.5,
            opacity: 1.0,
            gleam_influence: 0.8,
            gleam_radius: 0.3,
        }
    }
}

impl Material for IrisStyle {
    type Delta = IrisStyleDelta;

    fn applying(&self, delta: &IrisStyleDelta) -> Self {
        let mut s = self.clone();
        if let Some(d) = delta.base_thickness_delta {
            s.base_thickness = (s.base_thickness + d).clamp(0.3, 3.0);
        }
        if let Some(d) = delta.thickness_spread_delta {
            s.thickness_spread = (s.thickness_spread + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.thickness_scale_delta {
            s.thickness_scale = (s.thickness_scale + d).clamp(0.5, 3.0);
        }
        if let Some(d) = delta.intensity_delta {
            s.intensity = (s.intensity + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.brightness_delta {
            s.brightness = (s.brightness + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.edge_fade_delta {
            s.edge_fade = (s.edge_fade + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.refraction_delta {
            s.refraction = (s.refraction + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.dispersion_delta {
            s.dispersion = (s.dispersion + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.shift_speed_delta {
            s.shift_speed = (s.shift_speed + d).clamp(0.0, 3.0);
        }
        if let Some(d) = delta.opacity_delta {
            s.opacity = (s.opacity + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.gleam_influence_delta {
            s.gleam_influence = (s.gleam_influence + d).clamp(0.0, 1.0);
        }
        if let Some(d) = delta.gleam_radius_delta {
            s.gleam_radius = (s.gleam_radius + d).clamp(0.0, 0.5);
        }
        s
    }

    fn kind() -> &'static str {
        "iris"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nacre_defaults() {
        let s = IrisStyle::nacre();
        assert_eq!(s.layer_count, 3);
        assert!((s.base_thickness - 1.0).abs() < 1e-10);
        assert!((s.intensity - 0.85).abs() < 1e-10);
        assert_eq!(s.dimples.len(), 1);
    }

    #[test]
    fn oil_slick_preset() {
        let s = IrisStyle::oil_slick();
        assert_eq!(s.layer_count, 2);
        assert!((s.base_thickness - 1.5).abs() < 1e-10);
    }

    #[test]
    fn beetle_preset() {
        let s = IrisStyle::beetle();
        assert_eq!(s.layer_count, 4);
        assert!((s.base_thickness - 0.7).abs() < 1e-10);
    }

    #[test]
    fn bubble_preset() {
        let s = IrisStyle::bubble();
        assert_eq!(s.layer_count, 1);
        assert!(s.dimples.is_empty());
        assert!((s.opacity - 0.8).abs() < 1e-10);
    }

    #[test]
    fn vivid_preset() {
        let s = IrisStyle::vivid();
        assert!((s.intensity - 1.0).abs() < 1e-10);
        assert!((s.thickness_scale - 1.5).abs() < 1e-10);
    }

    #[test]
    fn applying_delta_clamps() {
        let s = IrisStyle::nacre();
        let d = IrisStyleDelta {
            base_thickness_delta: Some(10.0), // should clamp to 3.0
            gleam_radius_delta: Some(1.0),    // should clamp to 0.5
            ..Default::default()
        };
        let result = s.applying(&d);
        assert!((result.base_thickness - 3.0).abs() < 1e-10);
        assert!((result.gleam_radius - 0.5).abs() < 1e-10);
    }

    #[test]
    fn serde_roundtrip() {
        let s = IrisStyle::oil_slick();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: IrisStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(s.layer_count, decoded.layer_count);
        assert!((s.base_thickness - decoded.base_thickness).abs() < 1e-10);
    }

    #[test]
    fn material_kind() {
        assert_eq!(IrisStyle::kind(), "iris");
    }
}
