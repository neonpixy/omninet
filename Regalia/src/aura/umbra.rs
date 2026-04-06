use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Single shadow token.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Umbra {
    pub radius: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    /// Shadow opacity 0.0–1.0.
    pub opacity: f64,
}

impl Umbra {
    /// Create a shadow with the given blur radius, offset, and opacity.
    pub fn new(radius: f64, offset_x: f64, offset_y: f64, opacity: f64) -> Self {
        Self {
            radius,
            offset_x,
            offset_y,
            opacity: opacity.clamp(0.0, 1.0),
        }
    }
}

impl Default for Umbra {
    fn default() -> Self {
        Self::new(4.0, 0.0, 2.0, 0.15)
    }
}

/// Shadow scale with 4 named levels + custom extensibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UmbraScale {
    pub subtle: Umbra,
    pub medium: Umbra,
    pub elevated: Umbra,
    pub floating: Umbra,
    pub custom: HashMap<String, Umbra>,
}

impl UmbraScale {
    /// Look up a custom shadow level by name.
    pub fn get_custom(&self, name: &str) -> Option<&Umbra> {
        self.custom.get(name)
    }
}

impl Default for UmbraScale {
    fn default() -> Self {
        Self {
            subtle: Umbra::new(2.0, 0.0, 1.0, 0.08),
            medium: Umbra::new(6.0, 0.0, 3.0, 0.12),
            elevated: Umbra::new(12.0, 0.0, 6.0, 0.18),
            floating: Umbra::new(24.0, 0.0, 12.0, 0.25),
            custom: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scale() {
        let s = UmbraScale::default();
        assert!(s.subtle.radius < s.medium.radius);
        assert!(s.medium.radius < s.elevated.radius);
        assert!(s.elevated.radius < s.floating.radius);
    }

    #[test]
    fn opacity_clamped() {
        let u = Umbra::new(4.0, 0.0, 2.0, 1.5);
        assert_eq!(u.opacity, 1.0);
    }

    #[test]
    fn serde_roundtrip() {
        let s = UmbraScale::default();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: UmbraScale = serde_json::from_str(&json).unwrap();
        assert_eq!(s, decoded);
    }
}
