use serde::{Deserialize, Serialize};

/// How the glass light direction is determined.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LightSource {
    /// Static light at fixed rotation and intensity values.
    #[default]
    Fixed,

    /// Light tracks cursor/gaze position with distance-based falloff.
    Cursor {
        /// Distance in points where intensity fades to base_intensity.
        /// `None` = angle-only tracking (no distance falloff).
        #[serde(skip_serializing_if = "Option::is_none")]
        falloff_radius: Option<f64>,

        /// Minimum intensity when cursor/gaze is far away (0-1).
        base_intensity: f64,
    },
}


impl LightSource {
    /// Cursor-tracking light with default parameters.
    pub fn cursor() -> Self {
        Self::Cursor {
            falloff_radius: Some(300.0),
            base_intensity: 0.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fixed() {
        assert_eq!(LightSource::default(), LightSource::Fixed);
    }

    #[test]
    fn cursor_defaults() {
        let c = LightSource::cursor();
        match c {
            LightSource::Cursor {
                falloff_radius,
                base_intensity,
            } => {
                assert_eq!(falloff_radius, Some(300.0));
                assert!((base_intensity - 0.3).abs() < 1e-10);
            }
            _ => panic!("expected Cursor"),
        }
    }

    #[test]
    fn serde_roundtrip_fixed() {
        let ls = LightSource::Fixed;
        let json = serde_json::to_string(&ls).unwrap();
        let decoded: LightSource = serde_json::from_str(&json).unwrap();
        assert_eq!(ls, decoded);
    }

    #[test]
    fn serde_roundtrip_cursor() {
        let ls = LightSource::cursor();
        let json = serde_json::to_string(&ls).unwrap();
        let decoded: LightSource = serde_json::from_str(&json).unwrap();
        assert_eq!(ls, decoded);
    }
}
