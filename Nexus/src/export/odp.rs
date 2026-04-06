//! ODP exporter -- converts Ideas slide digits to an ODF presentation.
//!
//! Produces a minimal valid ODP (Open Document Format) file by building
//! the required ZIP structure with `mimetype`, `META-INF/manifest.xml`,
//! and `content.xml`. Maps slide digits to ODF `<draw:page>` elements.

use std::collections::HashMap;
use std::io::{Cursor, Write};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as an ODP (LibreOffice Impress) presentation.
///
/// Creates a minimal but valid ODP file. Slide digits
/// (`presentation.slide`) become `<draw:page>` elements with text frames.
/// Non-slide text digits are collected into a fallback slide.
///
/// # Example
///
/// ```ignore
/// let exporter = OdpExporter;
/// let config = ExportConfig::new(ExportFormat::Odp);
/// let output = exporter.export(&digits, None, &config)?;
/// ```
pub struct OdpExporter;

impl Exporter for OdpExporter {
    fn id(&self) -> &str {
        "nexus.odp"
    }

    fn display_name(&self) -> &str {
        "ODF Presentation"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Odp]
    }

    fn export(
        &self,
        digits: &[Digit],
        root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let ordered = order_digits(digits, root_id);

        // Separate slide digits from text content
        let mut slides: Vec<SlideData> = Vec::new();
        let mut text_lines: Vec<String> = Vec::new();

        for digit in &ordered {
            if digit.is_deleted() {
                continue;
            }
            if digit.digit_type() == "presentation.slide" {
                let title = digit
                    .properties
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled Slide")
                    .to_string();
                let body = digit
                    .properties
                    .get("speaker_notes")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                slides.push(SlideData { title, body });
            } else {
                let text = prop_text(digit);
                if !text.is_empty() {
                    text_lines.push(text);
                }
            }
        }

        // Fallback slide for text content
        if slides.is_empty() && !text_lines.is_empty() {
            slides.push(SlideData {
                title: "Content".to_string(),
                body: text_lines.join("\n"),
            });
        }

        // Ensure at least one slide
        if slides.is_empty() {
            slides.push(SlideData {
                title: "Untitled".to_string(),
                body: String::new(),
            });
        }

        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);

        let stored = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // mimetype (uncompressed)
        zip.start_file("mimetype", stored).map_err(zip_err)?;
        zip.write_all(b"application/vnd.oasis.opendocument.presentation")
            .map_err(zip_err)?;

        // META-INF/manifest.xml
        zip.start_file("META-INF/manifest.xml", deflated).map_err(zip_err)?;
        zip.write_all(manifest_xml().as_bytes()).map_err(zip_err)?;

        // content.xml
        let content = build_content_xml(&slides);
        zip.start_file("content.xml", deflated).map_err(zip_err)?;
        zip.write_all(content.as_bytes()).map_err(zip_err)?;

        let result = zip.finish().map_err(zip_err)?;

        Ok(ExportOutput::new(
            result.into_inner(),
            "export.odp",
            ExportFormat::Odp.mime_type(),
        ))
    }
}

struct SlideData {
    title: String,
    body: String,
}

fn zip_err(e: impl std::fmt::Display) -> NexusError {
    NexusError::ExportFailed(format!("ODP zip error: {e}"))
}

fn manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="application/vnd.oasis.opendocument.presentation"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#.to_string()
}

fn build_content_xml(slides: &[SlideData]) -> String {
    let mut buf = Vec::new();
    let mut writer = Writer::new_with_indent(Cursor::new(&mut buf), b' ', 2);

    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));

    let mut root = BytesStart::new("office:document-content");
    root.push_attribute(("xmlns:office", "urn:oasis:names:tc:opendocument:xmlns:office:1.0"));
    root.push_attribute(("xmlns:text", "urn:oasis:names:tc:opendocument:xmlns:text:1.0"));
    root.push_attribute(("xmlns:draw", "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"));
    root.push_attribute(("xmlns:presentation", "urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"));
    root.push_attribute(("xmlns:svg", "urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0"));
    root.push_attribute(("xmlns:fo", "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"));
    root.push_attribute(("office:version", "1.2"));
    let _ = writer.write_event(Event::Start(root));

    let _ = writer.write_event(Event::Start(BytesStart::new("office:body")));
    let _ = writer.write_event(Event::Start(BytesStart::new("office:presentation")));

    for (i, slide) in slides.iter().enumerate() {
        emit_slide(&mut writer, slide, i + 1);
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("office:presentation")));
    let _ = writer.write_event(Event::End(BytesEnd::new("office:body")));
    let _ = writer.write_event(Event::End(BytesEnd::new("office:document-content")));

    String::from_utf8(buf).unwrap_or_default()
}

fn emit_slide(writer: &mut Writer<Cursor<&mut Vec<u8>>>, slide: &SlideData, index: usize) {
    let page_name = format!("Slide{index}");
    let mut page = BytesStart::new("draw:page");
    page.push_attribute(("draw:name", page_name.as_str()));
    let _ = writer.write_event(Event::Start(page));

    // Title text frame
    let mut title_frame = BytesStart::new("draw:frame");
    title_frame.push_attribute(("svg:x", "2cm"));
    title_frame.push_attribute(("svg:y", "1cm"));
    title_frame.push_attribute(("svg:width", "22cm"));
    title_frame.push_attribute(("svg:height", "3cm"));
    title_frame.push_attribute(("presentation:class", "title"));
    let _ = writer.write_event(Event::Start(title_frame));
    let _ = writer.write_event(Event::Start(BytesStart::new("draw:text-box")));
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&slide.title)));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
    let _ = writer.write_event(Event::End(BytesEnd::new("draw:text-box")));
    let _ = writer.write_event(Event::End(BytesEnd::new("draw:frame")));

    // Body text frame (if body is non-empty)
    if !slide.body.is_empty() {
        let mut body_frame = BytesStart::new("draw:frame");
        body_frame.push_attribute(("svg:x", "2cm"));
        body_frame.push_attribute(("svg:y", "5cm"));
        body_frame.push_attribute(("svg:width", "22cm"));
        body_frame.push_attribute(("svg:height", "12cm"));
        body_frame.push_attribute(("presentation:class", "subtitle"));
        let _ = writer.write_event(Event::Start(body_frame));
        let _ = writer.write_event(Event::Start(BytesStart::new("draw:text-box")));
        for line in slide.body.lines() {
            let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
            let _ = writer.write_event(Event::Text(BytesText::new(line)));
            let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
        }
        let _ = writer.write_event(Event::End(BytesEnd::new("draw:text-box")));
        let _ = writer.write_event(Event::End(BytesEnd::new("draw:frame")));
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("draw:page")));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn prop_text(digit: &Digit) -> String {
    digit
        .properties
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| digit.content.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn order_digits(digits: &[Digit], root_id: Option<Uuid>) -> Vec<&Digit> {
    if digits.is_empty() {
        return Vec::new();
    }
    let index: HashMap<Uuid, &Digit> = digits.iter().map(|d| (d.id(), d)).collect();
    if let Some(rid) = root_id {
        if let Some(root) = index.get(&rid) {
            let mut ordered = Vec::new();
            walk_tree(root, &index, &mut ordered);
            let visited: std::collections::HashSet<Uuid> =
                ordered.iter().map(|d| d.id()).collect();
            for d in digits {
                if !visited.contains(&d.id()) {
                    ordered.push(d);
                }
            }
            return ordered;
        }
    }
    digits.iter().collect()
}

fn walk_tree<'a>(
    digit: &'a Digit,
    index: &HashMap<Uuid, &'a Digit>,
    out: &mut Vec<&'a Digit>,
) {
    out.push(digit);
    if let Some(children) = &digit.children {
        for child_id in children {
            if let Some(child) = index.get(child_id) {
                walk_tree(child, index, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExportConfig;
    use ideas::richtext::*;
    use ideas::slide::*;

    #[test]
    fn metadata() {
        assert_eq!(OdpExporter.id(), "nexus.odp");
        assert_eq!(OdpExporter.display_name(), "ODF Presentation");
        assert_eq!(OdpExporter.supported_formats(), &[ExportFormat::Odp]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Odp);
        let output = OdpExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.odp");
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_slide_digits() {
        let slide = slide_digit(
            &SlideMeta {
                title: Some("Welcome".into()),
                speaker_notes: Some("Hello audience".into()),
                transition: None,
                layout: SlideLayout::Title,
                order: 0,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Odp);
        let output = OdpExporter.export(&[slide], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn exports_text_as_fallback_slide() {
        let p = paragraph_digit(
            &ParagraphMeta {
                text: "Notes content".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Odp);
        let output = OdpExporter.export(&[p], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn skips_tombstoned() {
        let deleted = paragraph_digit(
            &ParagraphMeta {
                text: "gone".into(),
                spans: None,
            },
            "test",
        )
        .unwrap()
        .deleted("test");
        let config = ExportConfig::new(ExportFormat::Odp);
        let output = OdpExporter.export(&[deleted], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn multiple_slides() {
        let s1 = slide_digit(
            &SlideMeta {
                title: Some("One".into()),
                speaker_notes: None,
                transition: None,
                layout: SlideLayout::Title,
                order: 0,
            },
            "test",
        )
        .unwrap();
        let s2 = slide_digit(
            &SlideMeta {
                title: Some("Two".into()),
                speaker_notes: None,
                transition: None,
                layout: SlideLayout::Content,
                order: 1,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Odp);
        let output = OdpExporter.export(&[s1, s2], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }
}
