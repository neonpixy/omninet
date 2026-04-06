//! Media utilities — image metadata extraction and blurhash generation.
//!
//! When a user drops an image into a note, Hall can extract dimensions,
//! format, and generate a compact blurhash placeholder string. The blurhash
//! uses X's pure-math implementation (no platform dependencies).

use serde::{Deserialize, Serialize};

use crate::error::HallError;

/// Extracted metadata from an image file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// MIME type (e.g. `"image/png"`, `"image/jpeg"`).
    pub mime: String,
    /// Size of the raw input data in bytes.
    pub size: u64,
    /// Blurhash placeholder string (4x3 components). `None` if generation failed.
    pub blurhash: Option<String>,
}

/// Extract image metadata from raw bytes.
///
/// Decodes the image to get dimensions and format, then generates a blurhash
/// placeholder using X's pure-math blurhash implementation.
///
/// # Errors
///
/// Returns `HallError::CorruptedAsset` if the image cannot be decoded or
/// its format cannot be determined.
pub fn extract_image_metadata(data: &[u8]) -> Result<ImageMetadata, HallError> {
    let format = image::guess_format(data).map_err(|e| HallError::CorruptedAsset {
        hash: String::new(),
        reason: format!("unable to detect image format: {e}"),
    })?;

    let decoded = image::load_from_memory(data).map_err(|e| HallError::CorruptedAsset {
        hash: String::new(),
        reason: format!("unable to decode image: {e}"),
    })?;

    let width = decoded.width();
    let height = decoded.height();
    let mime = format_to_mime(format);
    let size = data.len() as u64;

    // Convert to RGBA8 for blurhash encoding.
    let rgba = decoded.to_rgba8();
    let pixels = rgba.as_raw();
    let blurhash = x::blurhash::encode(pixels, width, height, 4, 3);

    Ok(ImageMetadata {
        width,
        height,
        mime,
        size,
        blurhash,
    })
}

/// Map an `image::ImageFormat` to a MIME type string.
fn format_to_mime(format: image::ImageFormat) -> String {
    match format {
        image::ImageFormat::Png => "image/png".into(),
        image::ImageFormat::Jpeg => "image/jpeg".into(),
        image::ImageFormat::Gif => "image/gif".into(),
        image::ImageFormat::WebP => "image/webp".into(),
        // Fallback for any format we didn't explicitly enable but the
        // crate can still identify from magic bytes.
        other => format!("image/{}", format!("{other:?}").to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an in-memory PNG from a solid-color rectangle.
    fn make_png(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(width, height, |_, _| {
            image::Rgba([r, g, b, 255])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    /// Create an in-memory JPEG from a solid-color rectangle.
    fn make_jpeg(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        // JPEG doesn't support RGBA — use RGB.
        let img = image::RgbImage::from_fn(width, height, |_, _| {
            image::Rgb([r, g, b])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Jpeg).unwrap();
        buf.into_inner()
    }

    #[test]
    fn test_extract_png_metadata() {
        let data = make_png(120, 80, 50, 100, 200);
        let meta = extract_image_metadata(&data).unwrap();

        assert_eq!(meta.width, 120);
        assert_eq!(meta.height, 80);
        assert_eq!(meta.mime, "image/png");
        assert_eq!(meta.size, data.len() as u64);
    }

    #[test]
    fn test_extract_jpeg_metadata() {
        let data = make_jpeg(200, 150, 255, 128, 0);
        let meta = extract_image_metadata(&data).unwrap();

        assert_eq!(meta.width, 200);
        assert_eq!(meta.height, 150);
        assert_eq!(meta.mime, "image/jpeg");
        assert_eq!(meta.size, data.len() as u64);
    }

    #[test]
    fn test_blurhash_is_generated() {
        let data = make_png(64, 64, 100, 150, 200);
        let meta = extract_image_metadata(&data).unwrap();

        assert!(meta.blurhash.is_some(), "blurhash should be generated for a valid image");
        let hash = meta.blurhash.unwrap();
        assert!(
            x::blurhash::is_valid(&hash),
            "generated blurhash should be valid: {hash}"
        );
        // 4x3 components: expected length = 4 + 2 * (4*3) = 28
        assert_eq!(
            x::blurhash::components(&hash),
            Some((4, 3)),
            "should use 4x3 components"
        );
    }

    #[test]
    fn test_size_matches_input_length() {
        let data = make_png(10, 10, 0, 0, 0);
        let meta = extract_image_metadata(&data).unwrap();
        assert_eq!(meta.size, data.len() as u64);
    }

    #[test]
    fn test_corrupt_data_returns_error() {
        let garbage = b"this is definitely not an image file";
        let result = extract_image_metadata(garbage);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("corrupted asset"),
            "error should be CorruptedAsset, got: {msg}"
        );
    }

    #[test]
    fn test_empty_data_returns_error() {
        let result = extract_image_metadata(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_png_returns_error() {
        let data = make_png(32, 32, 128, 128, 128);
        // Keep only the first 20 bytes (valid PNG header prefix but truncated).
        let truncated = &data[..20];
        let result = extract_image_metadata(truncated);
        assert!(result.is_err());
    }

    #[test]
    fn test_metadata_serialization_round_trip() {
        let data = make_png(16, 16, 200, 100, 50);
        let meta = extract_image_metadata(&data).unwrap();

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ImageMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, deserialized);
    }

    #[test]
    fn test_format_to_mime_coverage() {
        assert_eq!(format_to_mime(image::ImageFormat::Png), "image/png");
        assert_eq!(format_to_mime(image::ImageFormat::Jpeg), "image/jpeg");
        assert_eq!(format_to_mime(image::ImageFormat::Gif), "image/gif");
        assert_eq!(format_to_mime(image::ImageFormat::WebP), "image/webp");
    }
}
