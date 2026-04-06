//! PNG exporter -- creates a placeholder PNG image from Ideas digits.
//!
//! Produces a white-background image at the configured dimensions (or
//! 800x600 default). Real rasterization of digit layouts requires the
//! platform GPU pipeline and is a Phase 7 concern.

use image::{ImageBuffer, Rgba, RgbaImage};
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

/// Exports Ideas digits as a PNG image.
///
/// Creates a placeholder white image. The actual rasterization of digit
/// layouts requires Magic + Divinity's rendering pipeline, which is a
/// Phase 7 concern. This exporter exists so the PNG format works end-to-end
/// through the Nexus pipeline.
///
/// # Example
///
/// ```ignore
/// let exporter = PngExporter;
/// let config = ExportConfig::new(ExportFormat::Png)
///     .with_page_size(1920.0, 1080.0);
/// let output = exporter.export(&digits, None, &config)?;
/// ```
pub struct PngExporter;

impl Exporter for PngExporter {
    fn id(&self) -> &str {
        "nexus.png"
    }

    fn display_name(&self) -> &str {
        "PNG Image"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Png]
    }

    fn export(
        &self,
        _digits: &[Digit],
        _root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let (width, height) = image_dimensions(config);
        let img: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));

        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| NexusError::ExportFailed(format!("failed to encode PNG: {e}")))?;

        Ok(ExportOutput::new(
            buf.into_inner(),
            "export.png",
            "image/png",
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
        assert_eq!(PngExporter.id(), "nexus.png");
        assert_eq!(PngExporter.display_name(), "PNG Image");
        assert_eq!(PngExporter.supported_formats(), &[ExportFormat::Png]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Png);
        let output = PngExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.png");
        assert_eq!(output.mime_type, "image/png");
        // PNG magic number
        assert_eq!(&output.data[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn custom_dimensions() {
        let config = ExportConfig::new(ExportFormat::Png)
            .with_page_size(100.0, 50.0);
        let output = PngExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn default_dimensions() {
        let (w, h) = image_dimensions(&ExportConfig::new(ExportFormat::Png));
        assert_eq!(w, DEFAULT_WIDTH);
        assert_eq!(h, DEFAULT_HEIGHT);
    }
}
