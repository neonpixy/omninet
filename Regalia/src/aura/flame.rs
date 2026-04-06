use serde::{Deserialize, Serialize};

use super::Ember;

/// Three-level color ramp: shade (darker), base (primary), tint (lighter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Flame {
    pub shade: Ember,
    pub base: Ember,
    pub tint: Ember,
}

impl Flame {
    /// Create a flame from explicit shade, base, and tint colors.
    pub fn new(shade: Ember, base: Ember, tint: Ember) -> Self {
        Self { shade, base, tint }
    }

    /// Auto-generate shade (30% darker) and tint (30% lighter) from base.
    pub fn from_base(base: Ember) -> Self {
        Self {
            shade: base.darken(0.3),
            base,
            tint: base.lighten(0.3),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_base_generates_ramp() {
        let f = Flame::from_base(Ember::rgb(0.5, 0.5, 0.5));
        assert!(f.shade.red < f.base.red);
        assert!(f.tint.red > f.base.red);
    }

    #[test]
    fn serde_roundtrip() {
        // Use hex-aligned values to avoid quantization drift
        let f = Flame::new(
            Ember::from_hex("#003366").unwrap(),
            Ember::from_hex("#007AFF").unwrap(),
            Ember::from_hex("#66BBFF").unwrap(),
        );
        let json = serde_json::to_string(&f).unwrap();
        let decoded: Flame = serde_json::from_str(&json).unwrap();
        assert_eq!(f, decoded);
    }
}
