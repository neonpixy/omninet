use serde::{Deserialize, Serialize};

use crate::crown_jewels::material::MaterialDelta;

/// Additive property delta for IrisStyle stylesheet cascading.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IrisStyleDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_thickness_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness_spread_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness_scale_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intensity_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_fade_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refraction_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispersion_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift_speed_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gleam_influence_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gleam_radius_delta: Option<f64>,
}

impl MaterialDelta for IrisStyleDelta {
    fn is_identity(&self) -> bool {
        self.base_thickness_delta.is_none()
            && self.thickness_spread_delta.is_none()
            && self.thickness_scale_delta.is_none()
            && self.intensity_delta.is_none()
            && self.brightness_delta.is_none()
            && self.edge_fade_delta.is_none()
            && self.refraction_delta.is_none()
            && self.dispersion_delta.is_none()
            && self.shift_speed_delta.is_none()
            && self.opacity_delta.is_none()
            && self.gleam_influence_delta.is_none()
            && self.gleam_radius_delta.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity() {
        let d = IrisStyleDelta::default();
        assert!(d.is_identity());
    }

    #[test]
    fn not_identity_when_set() {
        let d = IrisStyleDelta {
            intensity_delta: Some(0.1),
            ..Default::default()
        };
        assert!(!d.is_identity());
    }

    #[test]
    fn serde_roundtrip() {
        let d = IrisStyleDelta {
            base_thickness_delta: Some(0.3),
            opacity_delta: Some(-0.1),
            ..Default::default()
        };
        let json = serde_json::to_string(&d).unwrap();
        let decoded: IrisStyleDelta = serde_json::from_str(&json).unwrap();
        assert_eq!(d, decoded);
    }
}
