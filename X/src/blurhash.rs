//! Blurhash — compact image placeholders.
//!
//! Encodes an image into a 20-30 character string representing a blurry
//! placeholder. Decodes the string back to RGBA pixels. Pure math — no
//! platform or image library dependency.
//!
//! Reference: <https://blurha.sh>

use std::f64::consts::PI;

// -- Base83 --

const BASE83_CHARS: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

fn base83_encode(mut value: u32, length: usize, buf: &mut String) {
    for i in (0..length).rev() {
        let divisor = 83u32.pow(i as u32);
        let digit = (value / divisor) % 83;
        buf.push(BASE83_CHARS[digit as usize] as char);
        value -= digit * divisor;
    }
}

fn base83_decode(s: &str) -> u32 {
    let mut value: u32 = 0;
    for byte in s.bytes() {
        let digit = BASE83_CHARS
            .iter()
            .position(|&c| c == byte)
            .unwrap_or(0) as u32;
        value = value * 83 + digit;
    }
    value
}

// -- sRGB ↔ Linear --

fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f64) -> f64 {
    let v = value.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92 * 255.0 + 0.5
    } else {
        (1.055 * v.powf(1.0 / 2.4) - 0.055) * 255.0 + 0.5
    }
}

fn sign_pow(value: f64, exp: f64) -> f64 {
    value.abs().powf(exp).copysign(value)
}

// -- Encode --

/// Encode RGBA pixels into a blurhash string.
///
/// - `pixels`: RGBA pixel data (4 bytes per pixel, row-major)
/// - `width`, `height`: image dimensions
/// - `components_x`, `components_y`: number of DCT components (1–9 each, typical 4×3)
///
/// Returns the blurhash string (typically 20-30 characters).
pub fn encode(
    pixels: &[u8],
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
) -> Option<String> {
    if !(1..=9).contains(&components_x) || !(1..=9).contains(&components_y) {
        return None;
    }
    if pixels.len() < (width * height * 4) as usize {
        return None;
    }

    let w = width as usize;
    let h = height as usize;
    let cx = components_x as usize;
    let cy = components_y as usize;

    // Compute DCT factors.
    let mut factors: Vec<[f64; 3]> = Vec::with_capacity(cx * cy);

    for j in 0..cy {
        for i in 0..cx {
            let mut r = 0.0_f64;
            let mut g = 0.0_f64;
            let mut b = 0.0_f64;
            let normalisation = if i == 0 && j == 0 { 1.0 } else { 2.0 };

            for y in 0..h {
                for x in 0..w {
                    let basis = (PI * i as f64 * x as f64 / w as f64).cos()
                        * (PI * j as f64 * y as f64 / h as f64).cos();
                    let idx = (y * w + x) * 4;
                    r += basis * srgb_to_linear(pixels[idx] as f64 / 255.0);
                    g += basis * srgb_to_linear(pixels[idx + 1] as f64 / 255.0);
                    b += basis * srgb_to_linear(pixels[idx + 2] as f64 / 255.0);
                }
            }

            let scale = normalisation / (w * h) as f64;
            factors.push([r * scale, g * scale, b * scale]);
        }
    }

    // Encode.
    let mut hash = String::with_capacity(4 + 2 * cx * cy);

    // Size flag (1 digit).
    let size_flag = (components_x - 1) + (components_y - 1) * 9;
    base83_encode(size_flag, 1, &mut hash);

    // Quantised max AC value (1 digit).
    let max_ac = if factors.len() > 1 {
        factors[1..]
            .iter()
            .flat_map(|f| f.iter())
            .fold(0.0_f64, |acc, &v| acc.max(v.abs()))
    } else {
        0.0
    };
    let quantised_max = ((max_ac * 166.0 - 0.5).floor() as i32).clamp(0, 82) as u32;
    base83_encode(quantised_max, 1, &mut hash);

    let real_max = (quantised_max as f64 + 1.0) / 166.0;

    // DC value (4 digits).
    let dc = &factors[0];
    let dc_value = ((linear_to_srgb(dc[0]) as u32) << 16)
        | ((linear_to_srgb(dc[1]) as u32) << 8)
        | (linear_to_srgb(dc[2]) as u32);
    base83_encode(dc_value, 4, &mut hash);

    // AC values (2 digits each).
    for factor in &factors[1..] {
        let r = (sign_pow(factor[0] / real_max, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u32;
        let g = (sign_pow(factor[1] / real_max, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u32;
        let b = (sign_pow(factor[2] / real_max, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u32;
        base83_encode(r * 19 * 19 + g * 19 + b, 2, &mut hash);
    }

    Some(hash)
}

// -- Decode --

/// Decode a blurhash string into RGBA pixels.
///
/// - `hash`: the blurhash string
/// - `width`, `height`: desired output dimensions
///
/// Returns RGBA pixel data (4 bytes per pixel, row-major), or `None` if invalid.
pub fn decode(hash: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    if hash.len() < 6 {
        return None;
    }

    let size_flag = base83_decode(&hash[0..1]);
    let cx = (size_flag % 9 + 1) as usize;
    let cy = (size_flag / 9 + 1) as usize;

    let expected_len = 4 + 2 * cx * cy;
    if hash.len() != expected_len {
        return None;
    }

    let quantised_max = base83_decode(&hash[1..2]);
    let real_max = (quantised_max as f64 + 1.0) / 166.0;

    // Decode DC.
    let dc_value = base83_decode(&hash[2..6]);
    let dc = [
        srgb_to_linear(((dc_value >> 16) & 255) as f64 / 255.0),
        srgb_to_linear(((dc_value >> 8) & 255) as f64 / 255.0),
        srgb_to_linear((dc_value & 255) as f64 / 255.0),
    ];

    // Decode AC.
    let mut colors: Vec<[f64; 3]> = Vec::with_capacity(cx * cy);
    colors.push(dc);

    for i in 1..(cx * cy) {
        let start = 4 + 2 * (i - 1);
        let ac_value = base83_decode(&hash[start..start + 2]);
        let r = ac_value / (19 * 19);
        let g = (ac_value / 19) % 19;
        let b = ac_value % 19;
        colors.push([
            sign_pow((r as f64 - 9.0) / 9.0, 2.0) * real_max,
            sign_pow((g as f64 - 9.0) / 9.0, 2.0) * real_max,
            sign_pow((b as f64 - 9.0) / 9.0, 2.0) * real_max,
        ]);
    }

    // Render pixels.
    let w = width as usize;
    let h = height as usize;
    let mut pixels = vec![0u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0_f64;
            let mut g = 0.0_f64;
            let mut b = 0.0_f64;

            for j in 0..cy {
                for i in 0..cx {
                    let basis = (PI * i as f64 * x as f64 / w as f64).cos()
                        * (PI * j as f64 * y as f64 / h as f64).cos();
                    let color = &colors[j * cx + i];
                    r += color[0] * basis;
                    g += color[1] * basis;
                    b += color[2] * basis;
                }
            }

            let idx = (y * w + x) * 4;
            pixels[idx] = linear_to_srgb(r).clamp(0.0, 255.0) as u8;
            pixels[idx + 1] = linear_to_srgb(g).clamp(0.0, 255.0) as u8;
            pixels[idx + 2] = linear_to_srgb(b).clamp(0.0, 255.0) as u8;
            pixels[idx + 3] = 255; // Full alpha.
        }
    }

    Some(pixels)
}

/// Number of components encoded in a blurhash string.
pub fn components(hash: &str) -> Option<(u32, u32)> {
    if hash.is_empty() {
        return None;
    }
    let size_flag = base83_decode(&hash[0..1]);
    let cx = size_flag % 9 + 1;
    let cy = size_flag / 9 + 1;
    Some((cx, cy))
}

/// Validate a blurhash string (correct length for its component count).
pub fn is_valid(hash: &str) -> bool {
    if hash.len() < 6 {
        return false;
    }
    let Some((cx, cy)) = components(hash) else {
        return false;
    };
    let expected = 4 + 2 * (cx * cy) as usize;
    hash.len() == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base83_encode_decode_round_trip() {
        for value in [0, 1, 42, 82, 83, 6888, 83 * 83 - 1] {
            let mut buf = String::new();
            base83_encode(value, 2, &mut buf);
            let decoded = base83_decode(&buf);
            assert_eq!(decoded, value, "failed for {value}");
        }
    }

    #[test]
    fn srgb_linear_round_trip() {
        for i in 0..=255 {
            let srgb = i as f64 / 255.0;
            let linear = srgb_to_linear(srgb);
            let back = linear_to_srgb(linear) as u8;
            assert!((back as i16 - i as i16).abs() <= 1, "failed for {i}: got {back}");
        }
    }

    #[test]
    fn encode_solid_red() {
        let w = 4;
        let h = 4;
        let mut pixels = vec![0u8; w * h * 4];
        for i in 0..(w * h) {
            pixels[i * 4] = 255; // R
            pixels[i * 4 + 1] = 0; // G
            pixels[i * 4 + 2] = 0; // B
            pixels[i * 4 + 3] = 255; // A
        }
        let hash = encode(&pixels, w as u32, h as u32, 4, 3).unwrap();
        assert!(is_valid(&hash));
        assert_eq!(components(&hash), Some((4, 3)));
    }

    #[test]
    fn encode_decode_round_trip() {
        let w = 8;
        let h = 8;
        let mut pixels = vec![0u8; w * h * 4];
        // Gradient: red→blue across width.
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                pixels[idx] = (x * 255 / (w - 1)) as u8;
                pixels[idx + 1] = 50;
                pixels[idx + 2] = (255 - x * 255 / (w - 1)) as u8;
                pixels[idx + 3] = 255;
            }
        }

        let hash = encode(&pixels, w as u32, h as u32, 4, 3).unwrap();
        assert!(is_valid(&hash));

        // Decode to same size.
        let decoded = decode(&hash, w as u32, h as u32).unwrap();
        assert_eq!(decoded.len(), w * h * 4);

        // Decoded pixels should be valid RGBA with full alpha.
        for i in 0..(w * h) {
            assert_eq!(decoded[i * 4 + 3], 255, "alpha should always be 255");
        }
        // Pixels should have color (not all black or all white).
        let has_color = decoded.chunks(4).any(|px| px[0] != px[1] || px[1] != px[2]);
        assert!(has_color, "decoded image should have color variation");
    }

    #[test]
    fn decode_known_hash() {
        // 1x1 component = just a DC value (solid color).
        // Encode a solid white image to get a known hash.
        let pixels = vec![255u8; 4 * 4 * 4]; // 4x4 white
        let hash = encode(&pixels, 4, 4, 1, 1).unwrap();
        assert_eq!(components(&hash), Some((1, 1)));
        assert_eq!(hash.len(), 6); // 1 + 1 + 4 = 6

        let decoded = decode(&hash, 2, 2).unwrap();
        assert_eq!(decoded.len(), 2 * 2 * 4);
        // Should be close to white.
        assert!(decoded[0] > 250);
        assert!(decoded[1] > 250);
        assert!(decoded[2] > 250);
    }

    #[test]
    fn invalid_components_rejected() {
        let pixels = vec![0u8; 16];
        assert!(encode(&pixels, 2, 2, 0, 1).is_none());
        assert!(encode(&pixels, 2, 2, 10, 1).is_none());
        assert!(encode(&pixels, 2, 2, 1, 0).is_none());
        assert!(encode(&pixels, 2, 2, 1, 10).is_none());
    }

    #[test]
    fn insufficient_pixels_rejected() {
        let pixels = vec![0u8; 4]; // Only 1 pixel, need 4 (2x2).
        assert!(encode(&pixels, 2, 2, 1, 1).is_none());
    }

    #[test]
    fn is_valid_checks_length() {
        assert!(!is_valid(""));
        assert!(!is_valid("abc"));
        // Valid 1x1: 6 chars.
        let pixels = vec![128u8; 4 * 4 * 4];
        let hash = encode(&pixels, 4, 4, 1, 1).unwrap();
        assert!(is_valid(&hash));
        // Truncated is invalid.
        assert!(!is_valid(&hash[..5]));
    }

    #[test]
    fn decode_too_short_rejected() {
        assert!(decode("abc", 4, 4).is_none());
    }

    #[test]
    fn decode_wrong_length_rejected() {
        assert!(decode("00000000", 4, 4).is_none());
    }

    #[test]
    fn components_extracts_from_hash() {
        let pixels = vec![100u8; 8 * 6 * 4];
        let hash = encode(&pixels, 8, 6, 3, 2).unwrap();
        assert_eq!(components(&hash), Some((3, 2)));
    }
}
