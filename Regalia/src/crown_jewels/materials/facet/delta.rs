use serde::{Deserialize, Serialize};

use crate::crown_jewels::material::MaterialDelta;

/// Additive property delta for FacetStyle stylesheet cascading.
/// Each non-nil field is added to the corresponding FacetStyle property,
/// clamped to valid ranges.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FacetStyleDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frost_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refraction_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispersion_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub splay_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_rotation_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_intensity_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_banding_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_width_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tint_opacity_delta: Option<f64>,
}

impl MaterialDelta for FacetStyleDelta {
    fn is_identity(&self) -> bool {
        self.frost_delta.is_none()
            && self.refraction_delta.is_none()
            && self.dispersion_delta.is_none()
            && self.depth_delta.is_none()
            && self.splay_delta.is_none()
            && self.light_rotation_delta.is_none()
            && self.light_intensity_delta.is_none()
            && self.light_banding_delta.is_none()
            && self.edge_width_delta.is_none()
            && self.tint_opacity_delta.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_delta() {
        let d = FacetStyleDelta::default();
        assert!(d.is_identity());
    }

    #[test]
    fn is_identity_false_when_any_set() {
        let d = FacetStyleDelta {
            frost_delta: Some(0.1),
            ..Default::default()
        };
        assert!(!d.is_identity());
    }

    #[test]
    fn serde_roundtrip() {
        let d = FacetStyleDelta {
            frost_delta: Some(0.15),
            refraction_delta: Some(-0.1),
            ..Default::default()
        };
        let json = serde_json::to_string(&d).unwrap();
        let decoded: FacetStyleDelta = serde_json::from_str(&json).unwrap();
        assert_eq!(d, decoded);
    }

    #[test]
    fn serde_skips_none_fields() {
        let d = FacetStyleDelta {
            frost_delta: Some(0.2),
            ..Default::default()
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("frost_delta"));
        assert!(!json.contains("refraction_delta"));
    }
}
