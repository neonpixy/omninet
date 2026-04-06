use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::RegaliaError;

/// Atomic RGBA color. The smallest unit of color in Regalia.
///
/// All channels are 0.0–1.0. Serializes as a hex string ("#RRGGBB" or "#RRGGBBAA").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ember {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Ember {
    /// Create a new Ember with RGBA channels, each clamped to 0.0-1.0.
    pub fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Self {
        Self {
            red: red.clamp(0.0, 1.0),
            green: green.clamp(0.0, 1.0),
            blue: blue.clamp(0.0, 1.0),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    /// Create an opaque Ember (alpha = 1.0) from RGB channels.
    pub fn rgb(red: f64, green: f64, blue: f64) -> Self {
        Self::new(red, green, blue, 1.0)
    }

    /// Parse a hex color string: "#RRGGBB" or "#RRGGBBAA".
    pub fn from_hex(hex: &str) -> Result<Self, RegaliaError> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r =
                    u8::from_str_radix(&hex[0..2], 16).map_err(|_| bad_hex(hex))?;
                let g =
                    u8::from_str_radix(&hex[2..4], 16).map_err(|_| bad_hex(hex))?;
                let b =
                    u8::from_str_radix(&hex[4..6], 16).map_err(|_| bad_hex(hex))?;
                Ok(Self::new(
                    r as f64 / 255.0,
                    g as f64 / 255.0,
                    b as f64 / 255.0,
                    1.0,
                ))
            }
            8 => {
                let r =
                    u8::from_str_radix(&hex[0..2], 16).map_err(|_| bad_hex(hex))?;
                let g =
                    u8::from_str_radix(&hex[2..4], 16).map_err(|_| bad_hex(hex))?;
                let b =
                    u8::from_str_radix(&hex[4..6], 16).map_err(|_| bad_hex(hex))?;
                let a =
                    u8::from_str_radix(&hex[6..8], 16).map_err(|_| bad_hex(hex))?;
                Ok(Self::new(
                    r as f64 / 255.0,
                    g as f64 / 255.0,
                    b as f64 / 255.0,
                    a as f64 / 255.0,
                ))
            }
            _ => Err(bad_hex(hex)),
        }
    }

    /// Returns "#RRGGBB" or "#RRGGBBAA" if alpha < 1.
    pub fn to_hex(&self) -> String {
        let r = (self.red * 255.0).round() as u8;
        let g = (self.green * 255.0).round() as u8;
        let b = (self.blue * 255.0).round() as u8;
        if (self.alpha - 1.0).abs() < f64::EPSILON {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            let a = (self.alpha * 255.0).round() as u8;
            format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }

    /// Lighten by a fraction (0.0–1.0). Moves channels toward 1.0.
    pub fn lighten(&self, amount: f64) -> Self {
        let a = amount.clamp(0.0, 1.0);
        Self::new(
            self.red + (1.0 - self.red) * a,
            self.green + (1.0 - self.green) * a,
            self.blue + (1.0 - self.blue) * a,
            self.alpha,
        )
    }

    /// Darken by a fraction (0.0–1.0). Moves channels toward 0.0.
    pub fn darken(&self, amount: f64) -> Self {
        let a = amount.clamp(0.0, 1.0);
        Self::new(
            self.red * (1.0 - a),
            self.green * (1.0 - a),
            self.blue * (1.0 - a),
            self.alpha,
        )
    }

    // Presets
    pub const BLACK: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
    pub const WHITE: Self = Self {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    pub const CLEAR: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };
}

fn bad_hex(hex: &str) -> RegaliaError {
    RegaliaError::InvalidHex(hex.to_string())
}

impl std::hash::Hash for Ember {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the hex representation for stable hashing
        self.to_hex().hash(state);
    }
}

impl Eq for Ember {}

impl Default for Ember {
    fn default() -> Self {
        Self::BLACK
    }
}

// Serialize as hex string
impl Serialize for Ember {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Ember {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clamps_values() {
        let e = Ember::new(1.5, -0.5, 0.5, 2.0);
        assert_eq!(e.red, 1.0);
        assert_eq!(e.green, 0.0);
        assert_eq!(e.blue, 0.5);
        assert_eq!(e.alpha, 1.0);
    }

    #[test]
    fn hex_roundtrip_rgb() {
        let e = Ember::from_hex("#FF8040").unwrap();
        assert_eq!(e.to_hex(), "#FF8040");
    }

    #[test]
    fn hex_roundtrip_rgba() {
        let e = Ember::from_hex("#FF804080").unwrap();
        assert_eq!(e.to_hex(), "#FF804080");
    }

    #[test]
    fn hex_without_hash() {
        let e = Ember::from_hex("FF0000").unwrap();
        assert!((e.red - 1.0).abs() < 0.01);
    }

    #[test]
    fn hex_invalid() {
        assert!(Ember::from_hex("xyz").is_err());
        assert!(Ember::from_hex("#GG0000").is_err());
        assert!(Ember::from_hex("#12345").is_err());
    }

    #[test]
    fn lighten_darken() {
        let base = Ember::rgb(0.5, 0.5, 0.5);
        let lighter = base.lighten(0.5);
        assert!(lighter.red > base.red);
        let darker = base.darken(0.5);
        assert!(darker.red < base.red);
    }

    #[test]
    fn serde_roundtrip() {
        let e = Ember::from_hex("#007AFF").unwrap();
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, "\"#007AFF\"");
        let decoded: Ember = serde_json::from_str(&json).unwrap();
        assert_eq!(e, decoded);
    }

    #[test]
    fn presets() {
        assert_eq!(Ember::BLACK.to_hex(), "#000000");
        assert_eq!(Ember::WHITE.to_hex(), "#FFFFFF");
        assert_eq!(Ember::CLEAR.alpha, 0.0);
    }

    #[test]
    fn hash_and_eq() {
        use std::collections::HashSet;
        let a = Ember::from_hex("#FF0000").unwrap();
        let b = Ember::from_hex("#FF0000").unwrap();
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
