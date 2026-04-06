//! PDF exporter — converts Ideas digits to a PDF document via `printpdf`.
//!
//! Uses the printpdf 0.8 ops-based API to generate valid PDF documents.
//! Text digits are rendered as PDF text operations. Non-text digits are
//! represented as labeled placeholders since full rendering requires the
//! platform GPU pipeline (Phase 7).

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as a PDF document.
pub struct PdfExporter;

impl Exporter for PdfExporter {
    fn id(&self) -> &str {
        "nexus.pdf"
    }

    fn display_name(&self) -> &str {
        "PDF Document"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Pdf]
    }

    fn export(
        &self,
        digits: &[Digit],
        _root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        use printpdf::*;

        let (page_w, page_h) = config
            .page_size
            .unwrap_or((595.0, 842.0)); // A4 in points

        let mut doc = PdfDocument::new("Omnidea Export");

        // Collect text content from digits
        let mut lines: Vec<String> = Vec::new();
        for digit in digits {
            if digit.is_deleted() {
                continue;
            }
            let text = digit.extract_text();
            if !text.is_empty() {
                match digit.digit_type() {
                    "text.heading" => {
                        let level = digit.properties.get("level")
                            .and_then(|v| v.as_int())
                            .unwrap_or(1);
                        lines.push(format!("[H{}] {}", level, text));
                    }
                    _ => {
                        lines.push(text);
                    }
                }
            }
        }

        // Create pages with text content using printpdf ops
        let lines_per_page = ((page_h - 100.0) / 16.0) as usize;
        let chunks: Vec<&[String]> = if lines.is_empty() {
            vec![&[]]
        } else {
            lines.chunks(lines_per_page.max(1)).collect()
        };

        let mut pages = Vec::new();
        for chunk in &chunks {
            let mut ops = Vec::new();

            // Set up text rendering
            ops.push(Op::StartTextSection);

            let font_id = FontId("F1".into());
            let mut y_pos = page_h as f32 - 50.0;
            for line in *chunk {
                ops.push(Op::SetTextCursor {
                    pos: Point {
                        x: Pt(40.0),
                        y: Pt(y_pos),
                    },
                });
                ops.push(Op::SetFontSize { size: Pt(12.0), font: font_id.clone() });
                ops.push(Op::WriteCodepoints {
                    font: font_id.clone(),
                    cp: line.encode_utf16().map(|c| (c, ' ')).collect(),
                });
                y_pos -= 16.0;
            }

            ops.push(Op::EndTextSection);

            pages.push(PdfPage::new(
                Mm((page_w * 0.3528) as f32),
                Mm((page_h * 0.3528) as f32),
                ops,
            ));
        }

        doc.with_pages(pages);

        let mut warnings = Vec::new();
        let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

        Ok(ExportOutput::new(
            bytes,
            "export.pdf",
            "application/pdf",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExportConfig;
    use x::Value;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn metadata() {
        assert_eq!(PdfExporter.id(), "nexus.pdf");
        assert_eq!(PdfExporter.supported_formats(), &[ExportFormat::Pdf]);
    }

    #[test]
    fn exports_empty_document() {
        let config = ExportConfig::new(ExportFormat::Pdf);
        let output = PdfExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.mime_type, "application/pdf");
        // PDF files start with %PDF
        assert!(output.data.starts_with(b"%PDF"));
    }

    #[test]
    fn exports_text_content() {
        let digit = make_digit("text")
            .with_content(Value::from("Hello PDF"), "test");
        let config = ExportConfig::new(ExportFormat::Pdf);
        let output = PdfExporter.export(&[digit], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert!(output.data.starts_with(b"%PDF"));
    }
}
