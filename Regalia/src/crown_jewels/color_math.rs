/// Convert sRGB to HSL. Returns (h, s, l) where h is in 0-360 degrees.
pub fn rgba_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
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

/// Convert HSL to sRGB. h is in 0-360 degrees.
pub fn hsl_to_rgba(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
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

/// Convert sRGB to HSB/HSV. Returns (h, s, b) where h is in 0-360 degrees.
pub fn rgba_to_hsb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
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

/// Convert HSB/HSV to sRGB. h is in 0-360 degrees.
pub fn hsb_to_rgba(h: f64, s: f64, b: f64) -> (f64, f64, f64) {
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

/// WCAG 2.1 relative luminance from linear sRGB.
pub fn relative_luminance(r: f64, g: f64, b: f64) -> f64 {
    let linearize = |c: f64| -> f64 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG contrast ratio between two luminance values.
/// Returns a value >= 1.0 (lighter / darker).
pub fn contrast_ratio(lum1: f64, lum2: f64) -> f64 {
    let lighter = lum1.max(lum2);
    let darker = lum1.min(lum2);
    (lighter + 0.05) / (darker + 0.05)
}

/// Pre-multiply RGBA for GPU submission.
pub fn premultiply(r: f64, g: f64, b: f64, a: f64) -> (f64, f64, f64, f64) {
    (r * a, g * a, b * a, a)
}

/// Schlick Fresnel approximation per RGB wavelength channel.
/// Returns spectral weights (r, g, b) for thin-film interference.
pub fn fresnel_spectral_weights(ior: f64, cos_theta: f64) -> (f64, f64, f64) {
    let r0 = ((ior - 1.0) / (ior + 1.0)).powi(2);
    let fresnel = r0 + (1.0 - r0) * (1.0 - cos_theta).powi(5);

    // Slight wavelength-dependent variation (red < green < blue).
    let r_weight = fresnel * 0.95;
    let g_weight = fresnel;
    let b_weight = fresnel * 1.05;

    (r_weight.clamp(0.0, 1.0), g_weight.clamp(0.0, 1.0), b_weight.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_roundtrip() {
        let (r, g, b) = (0.8, 0.3, 0.5);
        let (h, s, l) = rgba_to_hsl(r, g, b);
        let (r2, g2, b2) = hsl_to_rgba(h, s, l);
        assert!((r - r2).abs() < 1e-6);
        assert!((g - g2).abs() < 1e-6);
        assert!((b - b2).abs() < 1e-6);
    }

    #[test]
    fn hsb_roundtrip() {
        let (r, g, b) = (0.6, 0.2, 0.9);
        let (h, s, v) = rgba_to_hsb(r, g, b);
        let (r2, g2, b2) = hsb_to_rgba(h, s, v);
        assert!((r - r2).abs() < 1e-6);
        assert!((g - g2).abs() < 1e-6);
        assert!((b - b2).abs() < 1e-6);
    }

    #[test]
    fn black_luminance_zero() {
        let l = relative_luminance(0.0, 0.0, 0.0);
        assert!(l.abs() < 1e-10);
    }

    #[test]
    fn white_luminance_one() {
        let l = relative_luminance(1.0, 1.0, 1.0);
        assert!((l - 1.0).abs() < 1e-6);
    }

    #[test]
    fn contrast_ratio_black_white() {
        let l_white = relative_luminance(1.0, 1.0, 1.0);
        let l_black = relative_luminance(0.0, 0.0, 0.0);
        let ratio = contrast_ratio(l_white, l_black);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn contrast_ratio_same_color() {
        let l = relative_luminance(0.5, 0.5, 0.5);
        let ratio = contrast_ratio(l, l);
        assert!((ratio - 1.0).abs() < 1e-6);
    }

    #[test]
    fn premultiply_opaque() {
        let (r, g, b, a) = premultiply(0.8, 0.6, 0.4, 1.0);
        assert!((r - 0.8).abs() < 1e-10);
        assert!((g - 0.6).abs() < 1e-10);
        assert!((b - 0.4).abs() < 1e-10);
        assert!((a - 1.0).abs() < 1e-10);
    }

    #[test]
    fn premultiply_half_alpha() {
        let (r, g, b, a) = premultiply(1.0, 1.0, 1.0, 0.5);
        assert!((r - 0.5).abs() < 1e-10);
        assert!((g - 0.5).abs() < 1e-10);
        assert!((b - 0.5).abs() < 1e-10);
        assert!((a - 0.5).abs() < 1e-10);
    }

    #[test]
    fn fresnel_normal_incidence() {
        // At normal incidence (cos_theta = 1), Fresnel should be close to R0.
        let (r, g, b) = fresnel_spectral_weights(1.5, 1.0);
        let r0 = ((1.5_f64 - 1.0) / (1.5 + 1.0)).powi(2); // 0.04
        assert!((r - r0 * 0.95).abs() < 1e-6);
        assert!((g - r0).abs() < 1e-6);
        assert!((b - r0 * 1.05).abs() < 1e-6);
    }
}
