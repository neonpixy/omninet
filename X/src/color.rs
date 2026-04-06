//! RGBA color with HSL/HSB conversions, WCAG accessibility, and blend modes.
//!
//! Ported from Swiftlight's CASColor.swift and Regalia's color_math.rs.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur when parsing colors.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ColorError {
    /// The hex string is not a valid color format.
    #[error("invalid hex color: {0}")]
    InvalidHex(String),
}

/// Blend modes for compositing two colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    /// Source replaces destination entirely.
    Normal,
    /// Darkens by multiplying channels -- useful for shadows and tinting.
    Multiply,
    /// Lightens by inverting, multiplying, and inverting again -- useful for highlights.
    Screen,
    /// Combines Multiply and Screen based on the base color's lightness.
    Overlay,
}

/// An RGBA color with channels in the 0.0-1.0 range.
///
/// Serde format: `{"r": 0.5, "g": 0.3, "b": 0.8, "a": 1.0}`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    // -- Preset constants --

    /// Pure black (#000000).
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    /// Pure white (#FFFFFF).
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    /// Fully transparent black (alpha 0).
    pub const CLEAR: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    /// Pure red (#FF0000).
    pub const RED: Self = Self { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    /// Pure green (#00FF00).
    pub const GREEN: Self = Self { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    /// Pure blue (#0000FF).
    pub const BLUE: Self = Self { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    /// Yellow (#FFFF00).
    pub const YELLOW: Self = Self { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    /// Cyan (#00FFFF).
    pub const CYAN: Self = Self { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    /// Magenta (#FF00FF).
    pub const MAGENTA: Self = Self { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    /// Orange (#FF8000).
    pub const ORANGE: Self = Self { r: 1.0, g: 0.5, b: 0.0, a: 1.0 };
    /// Purple (#800080).
    pub const PURPLE: Self = Self { r: 0.5, g: 0.0, b: 0.5, a: 1.0 };
    /// 50% gray (#808080).
    pub const GRAY: Self = Self { r: 0.5, g: 0.5, b: 0.5, a: 1.0 };

    // -- Constructors --

    /// Creates a color from RGBA values, clamping each to 0.0-1.0.
    #[inline]
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }

    /// Creates a color from RGB values with alpha 1.0.
    #[inline]
    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Creates a color from 0-255 byte values.
    #[inline]
    pub fn from_bytes(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: f64::from(r) / 255.0,
            g: f64::from(g) / 255.0,
            b: f64::from(b) / 255.0,
            a: f64::from(a) / 255.0,
        }
    }

    /// Creates a color from HSL values.
    ///
    /// - `h`: Hue in degrees (0-360)
    /// - `s`: Saturation (0-1)
    /// - `l`: Lightness (0-1)
    pub fn from_hsl(h: f64, s: f64, l: f64) -> Self {
        let (r, g, b) = hsl_to_rgb(h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        Self { r, g, b, a: 1.0 }
    }

    /// Creates a color from HSB/HSV values.
    ///
    /// - `h`: Hue in degrees (0-360)
    /// - `s`: Saturation (0-1)
    /// - `b`: Brightness/Value (0-1)
    pub fn from_hsb(h: f64, s: f64, b: f64) -> Self {
        let (r, g, bl) = hsb_to_rgb(h, s.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
        Self {
            r,
            g,
            b: bl,
            a: 1.0,
        }
    }

    /// Creates a color from a hex string.
    ///
    /// Supports `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` (with or without `#`).
    pub fn from_hex(hex: &str) -> Result<Self, ColorError> {
        let hex = hex.trim().trim_start_matches('#');

        let parse_err = || ColorError::InvalidHex(hex.to_string());

        // Validate hex characters
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(parse_err());
        }

        let value =
            u64::from_str_radix(hex, 16).map_err(|_| parse_err())?;

        match hex.len() {
            3 => {
                // RGB
                let r = ((value & 0xF00) >> 8) as f64 / 15.0;
                let g = ((value & 0x0F0) >> 4) as f64 / 15.0;
                let b = (value & 0x00F) as f64 / 15.0;
                Ok(Self { r, g, b, a: 1.0 })
            }
            4 => {
                // RGBA
                let r = ((value & 0xF000) >> 12) as f64 / 15.0;
                let g = ((value & 0x0F00) >> 8) as f64 / 15.0;
                let b = ((value & 0x00F0) >> 4) as f64 / 15.0;
                let a = (value & 0x000F) as f64 / 15.0;
                Ok(Self { r, g, b, a })
            }
            6 => {
                // RRGGBB
                let r = ((value & 0xFF0000) >> 16) as f64 / 255.0;
                let g = ((value & 0x00FF00) >> 8) as f64 / 255.0;
                let b = (value & 0x0000FF) as f64 / 255.0;
                Ok(Self { r, g, b, a: 1.0 })
            }
            8 => {
                // RRGGBBAA
                let r = ((value & 0xFF00_0000) >> 24) as f64 / 255.0;
                let g = ((value & 0x00FF_0000) >> 16) as f64 / 255.0;
                let b = ((value & 0x0000_FF00) >> 8) as f64 / 255.0;
                let a = (value & 0x0000_00FF) as f64 / 255.0;
                Ok(Self { r, g, b, a })
            }
            _ => Err(parse_err()),
        }
    }

    /// Creates a grayscale color with equal RGB channels.
    #[inline]
    pub fn grayscale(value: f64) -> Self {
        let v = value.clamp(0.0, 1.0);
        Self {
            r: v,
            g: v,
            b: v,
            a: 1.0,
        }
    }

    // -- HSL accessors --

    /// The hue component in degrees (0-360).
    pub fn hue(&self) -> f64 {
        let (h, _, _) = rgb_to_hsl(self.r, self.g, self.b);
        h
    }

    /// The saturation component (0-1) in HSL.
    pub fn saturation_hsl(&self) -> f64 {
        let (_, s, _) = rgb_to_hsl(self.r, self.g, self.b);
        s
    }

    /// The lightness component (0-1).
    pub fn lightness(&self) -> f64 {
        let (_, _, l) = rgb_to_hsl(self.r, self.g, self.b);
        l
    }

    // -- HSB accessors --

    /// The saturation component (0-1) in HSB/HSV.
    pub fn saturation_hsb(&self) -> f64 {
        let (_, s, _) = rgb_to_hsb(self.r, self.g, self.b);
        s
    }

    /// The brightness/value component (0-1) in HSB/HSV.
    pub fn brightness(&self) -> f64 {
        let (_, _, b) = rgb_to_hsb(self.r, self.g, self.b);
        b
    }

    // -- WCAG --

    /// Relative luminance per WCAG 2.1 formula.
    pub fn luminance(&self) -> f64 {
        fn linearize(c: f64) -> f64 {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linearize(self.r) + 0.7152 * linearize(self.g) + 0.0722 * linearize(self.b)
    }

    /// WCAG contrast ratio with another color (>= 1.0).
    pub fn contrast_ratio(&self, other: &Color) -> f64 {
        let l1 = self.luminance();
        let l2 = other.luminance();
        let lighter = l1.max(l2);
        let darker = l1.min(l2);
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Whether the contrast ratio meets WCAG AA for normal text (>= 4.5).
    pub fn meets_aa(&self, other: &Color) -> bool {
        self.contrast_ratio(other) >= 4.5
    }

    /// Whether the contrast ratio meets WCAG AA for large text (>= 3.0).
    pub fn meets_aa_large(&self, other: &Color) -> bool {
        self.contrast_ratio(other) >= 3.0
    }

    /// Whether the contrast ratio meets WCAG AAA (>= 7.0).
    pub fn meets_aaa(&self, other: &Color) -> bool {
        self.contrast_ratio(other) >= 7.0
    }

    // -- Operations --

    /// Returns a copy with a different alpha.
    #[inline]
    pub fn with_alpha(&self, a: f64) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: a.clamp(0.0, 1.0),
        }
    }

    /// Returns a lighter version via HSL.
    pub fn lightened(&self, amount: f64) -> Self {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        let (r, g, b) = hsl_to_rgb(h, s, (l + amount).min(1.0));
        Self {
            r,
            g,
            b,
            a: self.a,
        }
    }

    /// Returns a darker version via HSL.
    pub fn darkened(&self, amount: f64) -> Self {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        let (r, g, b) = hsl_to_rgb(h, s, (l - amount).max(0.0));
        Self {
            r,
            g,
            b,
            a: self.a,
        }
    }

    /// Returns a more saturated version via HSL.
    pub fn saturated(&self, amount: f64) -> Self {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        let (r, g, b) = hsl_to_rgb(h, (s + amount).min(1.0), l);
        Self {
            r,
            g,
            b,
            a: self.a,
        }
    }

    /// Returns a less saturated version via HSL.
    pub fn desaturated(&self, amount: f64) -> Self {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        let (r, g, b) = hsl_to_rgb(h, (s - amount).max(0.0), l);
        Self {
            r,
            g,
            b,
            a: self.a,
        }
    }

    /// Returns the inverted color (preserves alpha).
    pub fn inverted(&self) -> Self {
        Self {
            r: 1.0 - self.r,
            g: 1.0 - self.g,
            b: 1.0 - self.b,
            a: self.a,
        }
    }

    /// Returns the complementary color (opposite on the color wheel).
    pub fn complementary(&self) -> Self {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        let new_h = (h + 180.0) % 360.0;
        let (r, g, b) = hsl_to_rgb(new_h, s, l);
        Self {
            r,
            g,
            b,
            a: self.a,
        }
    }

    /// Whether this is a light color (luminance > 0.5).
    pub fn is_light(&self) -> bool {
        self.luminance() > 0.5
    }

    /// Whether this is a dark color (luminance <= 0.5).
    pub fn is_dark(&self) -> bool {
        !self.is_light()
    }

    // -- Mixing --

    /// Linearly interpolates between this color and another.
    pub fn lerp(&self, other: &Color, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Blends this color with another using the specified blend mode.
    pub fn blend(&self, other: &Color, mode: BlendMode) -> Self {
        match mode {
            BlendMode::Normal => *other,
            BlendMode::Multiply => Self {
                r: self.r * other.r,
                g: self.g * other.g,
                b: self.b * other.b,
                a: self.a * other.a,
            },
            BlendMode::Screen => Self {
                r: 1.0 - (1.0 - self.r) * (1.0 - other.r),
                g: 1.0 - (1.0 - self.g) * (1.0 - other.g),
                b: 1.0 - (1.0 - self.b) * (1.0 - other.b),
                a: 1.0 - (1.0 - self.a) * (1.0 - other.a),
            },
            BlendMode::Overlay => {
                fn overlay_channel(base: f64, blend: f64) -> f64 {
                    if base < 0.5 {
                        2.0 * base * blend
                    } else {
                        1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
                    }
                }
                Self {
                    r: overlay_channel(self.r, other.r),
                    g: overlay_channel(self.g, other.g),
                    b: overlay_channel(self.b, other.b),
                    a: overlay_channel(self.a, other.a),
                }
            }
        }
    }

    // -- Output --

    /// Converts to a hex string (`#RRGGBB` or `#RRGGBBAA` if alpha != 1.0).
    pub fn to_hex(&self) -> String {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;

        if (self.a - 1.0).abs() < 1e-6 {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            let a = (self.a * 255.0).round() as u8;
            format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }

    /// Converts from sRGB to linear RGB.
    pub fn to_linear(&self) -> (f64, f64, f64) {
        fn srgb_to_linear(c: f64) -> f64 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        (
            srgb_to_linear(self.r),
            srgb_to_linear(self.g),
            srgb_to_linear(self.b),
        )
    }

    /// Creates a color from linear RGB values (converts to sRGB).
    pub fn from_linear(r: f64, g: f64, b: f64) -> Self {
        fn linear_to_srgb(c: f64) -> f64 {
            if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            }
        }
        Self::rgb(linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if (self.a - 1.0).abs() < 1e-6 {
            write!(f, "Color({})", self.to_hex())
        } else {
            write!(f, "Color({}, a: {:.2})", self.to_hex(), self.a)
        }
    }
}

// ---------------------------------------------------------------------------
// Private HSL/HSB helpers
// ---------------------------------------------------------------------------

fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 1e-10 {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < 1e-10 {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < 1e-10 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s.abs() < 1e-10 {
        return (l, l, l);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let h_norm = h / 360.0;

    let r = hue_to_rgb(p, q, h_norm + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h_norm);
    let b = hue_to_rgb(p, q, h_norm - 1.0 / 3.0);

    (r, g, b)
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn rgb_to_hsb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let brightness = max;

    if (max - min).abs() < 1e-10 {
        return (0.0, 0.0, brightness);
    }

    let d = max - min;
    let s = d / max;

    let h = if (max - r).abs() < 1e-10 {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < 1e-10 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    (h * 60.0, s, brightness)
}

fn hsb_to_rgb(h: f64, s: f64, b: f64) -> (f64, f64, f64) {
    if s.abs() < 1e-10 {
        return (b, b, b);
    }

    let h = (h % 360.0) / 60.0;
    let i = h.floor() as i32;
    let f = h - h.floor();
    let p = b * (1.0 - s);
    let q = b * (1.0 - s * f);
    let t = b * (1.0 - s * (1.0 - f));

    match i % 6 {
        0 => (b, t, p),
        1 => (q, b, p),
        2 => (p, b, t),
        3 => (p, q, b),
        4 => (t, p, b),
        _ => (b, p, q),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-6;

    // -- Construction --

    #[test]
    fn test_new_clamps() {
        let c = Color::new(1.5, -0.5, 0.5, 2.0);
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!((c.g - 0.0).abs() < EPSILON);
        assert!((c.b - 0.5).abs() < EPSILON);
        assert!((c.a - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_rgb() {
        let c = Color::rgb(0.5, 0.6, 0.7);
        assert!((c.a - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_bytes() {
        let c = Color::from_bytes(255, 0, 127, 255);
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!((c.g - 0.0).abs() < EPSILON);
        assert!((c.b - 127.0 / 255.0).abs() < EPSILON);
        assert!((c.a - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_grayscale() {
        let c = Color::grayscale(0.5);
        assert!((c.r - 0.5).abs() < EPSILON);
        assert!((c.g - 0.5).abs() < EPSILON);
        assert!((c.b - 0.5).abs() < EPSILON);
    }

    // -- HSL --

    #[test]
    fn test_from_hsl_red() {
        let c = Color::from_hsl(0.0, 1.0, 0.5);
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!(c.g.abs() < EPSILON);
        assert!(c.b.abs() < EPSILON);
    }

    #[test]
    fn test_hsl_roundtrip() {
        let c = Color::rgb(0.8, 0.3, 0.5);
        let h = c.hue();
        let s = c.saturation_hsl();
        let l = c.lightness();
        let c2 = Color::from_hsl(h, s, l);
        assert!((c.r - c2.r).abs() < EPSILON);
        assert!((c.g - c2.g).abs() < EPSILON);
        assert!((c.b - c2.b).abs() < EPSILON);
    }

    #[test]
    fn test_hsl_gray() {
        let c = Color::grayscale(0.5);
        assert!((c.saturation_hsl() - 0.0).abs() < EPSILON);
    }

    // -- HSB --

    #[test]
    fn test_from_hsb_red() {
        let c = Color::from_hsb(0.0, 1.0, 1.0);
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!(c.g.abs() < EPSILON);
        assert!(c.b.abs() < EPSILON);
    }

    #[test]
    fn test_hsb_roundtrip() {
        let c = Color::rgb(0.6, 0.2, 0.9);
        let h = c.hue();
        let s = c.saturation_hsb();
        let b = c.brightness();
        let c2 = Color::from_hsb(h, s, b);
        assert!((c.r - c2.r).abs() < EPSILON);
        assert!((c.g - c2.g).abs() < EPSILON);
        assert!((c.b - c2.b).abs() < EPSILON);
    }

    // -- Hex --

    #[test]
    fn test_from_hex_rrggbb() {
        let c = Color::from_hex("#FF8800").unwrap();
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!((c.g - 0x88 as f64 / 255.0).abs() < EPSILON);
        assert!(c.b.abs() < EPSILON);
        assert!((c.a - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_hex_rrggbbaa() {
        let c = Color::from_hex("#FF000080").unwrap();
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!((c.a - 0x80 as f64 / 255.0).abs() < 0.01);
    }

    #[test]
    fn test_from_hex_rgb() {
        let c = Color::from_hex("#F00").unwrap();
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!(c.g.abs() < EPSILON);
        assert!(c.b.abs() < EPSILON);
    }

    #[test]
    fn test_from_hex_rgba() {
        let c = Color::from_hex("#F008").unwrap();
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!((c.a - 8.0 / 15.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_hex_no_hash() {
        let c = Color::from_hex("FF0000").unwrap();
        assert!((c.r - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_hex_invalid() {
        assert!(Color::from_hex("#GGG").is_err());
        assert!(Color::from_hex("#12345").is_err());
        assert!(Color::from_hex("").is_err());
    }

    #[test]
    fn test_to_hex() {
        assert_eq!(Color::RED.to_hex(), "#FF0000");
        assert_eq!(Color::GREEN.to_hex(), "#00FF00");
        assert_eq!(Color::BLUE.to_hex(), "#0000FF");
        assert_eq!(Color::BLACK.to_hex(), "#000000");
        assert_eq!(Color::WHITE.to_hex(), "#FFFFFF");
    }

    #[test]
    fn test_to_hex_with_alpha() {
        let c = Color::new(1.0, 0.0, 0.0, 0.5);
        let hex = c.to_hex();
        assert!(hex.starts_with("#FF0000"));
        assert_eq!(hex.len(), 9); // #RRGGBBAA
    }

    // -- WCAG --

    #[test]
    fn test_luminance_black() {
        assert!(Color::BLACK.luminance().abs() < 1e-10);
    }

    #[test]
    fn test_luminance_white() {
        assert!((Color::WHITE.luminance() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_contrast_ratio_black_white() {
        let ratio = Color::BLACK.contrast_ratio(&Color::WHITE);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn test_contrast_ratio_same() {
        let c = Color::GRAY;
        let ratio = c.contrast_ratio(&c);
        assert!((ratio - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_meets_aa() {
        assert!(Color::BLACK.meets_aa(&Color::WHITE));
        assert!(!Color::GRAY.meets_aa(&Color::WHITE));
    }

    #[test]
    fn test_meets_aa_large() {
        assert!(Color::BLACK.meets_aa_large(&Color::WHITE));
    }

    #[test]
    fn test_meets_aaa() {
        assert!(Color::BLACK.meets_aaa(&Color::WHITE));
    }

    // -- Operations --

    #[test]
    fn test_with_alpha() {
        let c = Color::RED.with_alpha(0.5);
        assert!((c.r - 1.0).abs() < EPSILON);
        assert!((c.a - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_lightened() {
        let c = Color::from_hsl(0.0, 1.0, 0.3);
        let lighter = c.lightened(0.2);
        assert!(lighter.lightness() > c.lightness());
    }

    #[test]
    fn test_darkened() {
        let c = Color::from_hsl(0.0, 1.0, 0.7);
        let darker = c.darkened(0.2);
        assert!(darker.lightness() < c.lightness());
    }

    #[test]
    fn test_saturated() {
        let c = Color::from_hsl(0.0, 0.5, 0.5);
        let sat = c.saturated(0.2);
        assert!(sat.saturation_hsl() > c.saturation_hsl());
    }

    #[test]
    fn test_desaturated() {
        let c = Color::from_hsl(0.0, 0.5, 0.5);
        let desat = c.desaturated(0.2);
        assert!(desat.saturation_hsl() < c.saturation_hsl());
    }

    #[test]
    fn test_inverted() {
        let c = Color::RED.inverted();
        assert!((c.r - 0.0).abs() < EPSILON);
        assert!((c.g - 1.0).abs() < EPSILON);
        assert!((c.b - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_complementary() {
        let c = Color::RED.complementary();
        // Complement of red should be cyan
        assert!(c.r < 0.1);
        assert!(c.g > 0.9);
        assert!(c.b > 0.9);
    }

    #[test]
    fn test_is_light_dark() {
        assert!(Color::WHITE.is_light());
        assert!(Color::BLACK.is_dark());
    }

    // -- Mixing --

    #[test]
    fn test_lerp() {
        let a = Color::BLACK;
        let b = Color::WHITE;
        let mid = a.lerp(&b, 0.5);
        assert!((mid.r - 0.5).abs() < EPSILON);
        assert!((mid.g - 0.5).abs() < EPSILON);
        assert!((mid.b - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_lerp_clamped() {
        let a = Color::BLACK;
        let b = Color::WHITE;
        let over = a.lerp(&b, 2.0);
        assert!((over.r - 1.0).abs() < EPSILON);
        let under = a.lerp(&b, -1.0);
        assert!((under.r - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_blend_normal() {
        let result = Color::RED.blend(&Color::BLUE, BlendMode::Normal);
        assert_eq!(result, Color::BLUE);
    }

    #[test]
    fn test_blend_multiply() {
        let result = Color::WHITE.blend(&Color::RED, BlendMode::Multiply);
        assert!((result.r - 1.0).abs() < EPSILON);
        assert!(result.g.abs() < EPSILON);
        assert!(result.b.abs() < EPSILON);
    }

    #[test]
    fn test_blend_screen() {
        let result = Color::BLACK.blend(&Color::RED, BlendMode::Screen);
        assert!((result.r - 1.0).abs() < EPSILON);
        assert!(result.g.abs() < EPSILON);
    }

    #[test]
    fn test_blend_overlay() {
        let base = Color::grayscale(0.25);
        let blend = Color::WHITE;
        let result = base.blend(&blend, BlendMode::Overlay);
        // For dark base (<0.5), overlay = 2*base*blend
        assert!((result.r - 0.5).abs() < EPSILON);
    }

    // -- Linear color space --

    #[test]
    fn test_to_linear_roundtrip() {
        let c = Color::rgb(0.5, 0.3, 0.8);
        let (lr, lg, lb) = c.to_linear();
        let c2 = Color::from_linear(lr, lg, lb);
        assert!((c.r - c2.r).abs() < EPSILON);
        assert!((c.g - c2.g).abs() < EPSILON);
        assert!((c.b - c2.b).abs() < EPSILON);
    }

    #[test]
    fn test_to_linear_black_white() {
        let (r, g, b) = Color::BLACK.to_linear();
        assert!(r.abs() < EPSILON);
        assert!(g.abs() < EPSILON);
        assert!(b.abs() < EPSILON);

        let (r, g, b) = Color::WHITE.to_linear();
        assert!((r - 1.0).abs() < EPSILON);
        assert!((g - 1.0).abs() < EPSILON);
        assert!((b - 1.0).abs() < EPSILON);
    }

    // -- Presets --

    #[test]
    fn test_preset_colors() {
        assert_eq!(Color::CLEAR.a, 0.0);
        assert_eq!(Color::BLACK, Color::rgb(0.0, 0.0, 0.0));
        assert_eq!(Color::WHITE, Color::rgb(1.0, 1.0, 1.0));
        assert_eq!(Color::RED, Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(Color::GREEN, Color::rgb(0.0, 1.0, 0.0));
        assert_eq!(Color::BLUE, Color::rgb(0.0, 0.0, 1.0));
    }

    // -- Serde --

    #[test]
    fn test_serde_roundtrip() {
        let c = Color::new(0.5, 0.3, 0.8, 0.9);
        let json = serde_json::to_string(&c).unwrap();
        let c2: Color = serde_json::from_str(&json).unwrap();
        assert!((c.r - c2.r).abs() < EPSILON);
        assert!((c.g - c2.g).abs() < EPSILON);
        assert!((c.b - c2.b).abs() < EPSILON);
        assert!((c.a - c2.a).abs() < EPSILON);
    }

    #[test]
    fn test_serde_format() {
        let c = Color::new(0.5, 0.3, 0.8, 1.0);
        let json = serde_json::to_string(&c).unwrap();
        // Should contain "r", "g", "b", "a" keys (NOT hex)
        assert!(json.contains("\"r\""));
        assert!(json.contains("\"g\""));
        assert!(json.contains("\"b\""));
        assert!(json.contains("\"a\""));
    }

    // -- Display --

    #[test]
    fn test_display() {
        let c = Color::RED;
        let s = format!("{c}");
        assert!(s.contains("#FF0000"));
    }

    // -- BlendMode serde --

    #[test]
    fn test_blend_mode_serde() {
        let m = BlendMode::Overlay;
        let json = serde_json::to_string(&m).unwrap();
        let m2: BlendMode = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }
}
