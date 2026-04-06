//! JPEG exporter -- creates a placeholder JPEG image from Ideas digits.
//!
//! Produces a white-background image at the configured dimensions (or
//! 800x600 default). Real rasterization of digit layouts requires the
//! platform GPU pipeline and is a Phase 7 concern.

use image::{ImageBuffer, Rgb, RgbImage};
use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Default placeholder width in pixels.
const DEFAULT_WIDTH: u32 = 800;
/// Default placeholder height in pixels.
const DEFAULT_HEIGHT: u32 = 600;

/// Exports Ideas digits as a JPEG image.
///
/// Creates a placeholder white image. The actual rasterization of digit
/// layouts requires Magic + Divinity's rendering pipeline, which is a
/// Phase 7 concern. This exporter exists so the JPG format works end-to-end
/// through the Nexus pipeline.
///
/// # Example
///
/// ```ignore
/// let exporter = JpgExporter;
/// let config = ExportConfig::new(ExportFormat::Jpg)
///     .with_page_size(1920.0, 1080.0);
/// let output = exporter.export(&digits, None, &config)?;
/// ```
pub struct JpgExporter;

impl Exporter for JpgExporter {
    fn id(&self) -> &str {
        "nexus.jpg"
    }

    fn display_name(&self) -> &str {
        "JPEG Image"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Jpg]
    }

    fn export(
        &self,
        _digits: &[Digit],
        _root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let (width, height) = image_dimensions(config);
        // JPEG does not support alpha, so use RGB
        let img: RgbImage = ImageBuffer::from_pixel(width, height, Rgb([255, 255, 255]));

        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Jpeg)
            .map_err(|e| NexusError::ExportFailed(format!("failed to encode JPEG: {e}")))?;

        Ok(ExportOutput::new(
            buf.into_inner(),
            "export.jpg",
            "image/jpeg",
        ))
    }
}

/// Derive image dimensions from config or use defaults.
fn image_dimensions(config: &ExportConfig) -> (u32, u32) {
    config
        .page_size
        .map(|(w, h)| (w.max(1.0) as u32, h.max(1.0) as u32))
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExportConfig;

    #[test]
    fn metadata() {
        assert_eq!(JpgExporter.id(), "nexus.jpg");
        assert_eq!(JpgExporter.display_name(), "JPEG Image");
        assert_eq!(JpgExporter.supported_formats(), &[ExportFormat::Jpg]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Jpg);
        let output = JpgExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.jpg");
        assert_eq!(output.mime_type, "image/jpeg");
        // JPEG magic number: 0xFF 0xD8
        assert_eq!(&output.data[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn custom_dimensions() {
        let config = ExportConfig::new(ExportFormat::Jpg)
            .with_page_size(320.0, 240.0);
        let output = JpgExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }
}
