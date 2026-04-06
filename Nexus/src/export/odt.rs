//! ODT exporter -- converts Ideas rich text digits to an ODF text document.
//!
//! Produces a minimal valid ODT (Open Document Format) file by building
//! the required ZIP structure with `mimetype`, `META-INF/manifest.xml`,
//! and `content.xml`. Maps text digits to ODF paragraph/heading elements.

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

/// Exports Ideas digits as an ODT (LibreOffice Writer) document.
///
/// Creates a minimal but valid ODT file that opens in LibreOffice Writer,
/// OpenOffice, and other ODF-compatible editors. Rich text digits are
/// mapped to their ODF equivalents.
///
/// # Example
///
/// ```ignore
/// let exporter = OdtExporter;
/// let config = ExportConfig::new(ExportFormat::Odt);
/// let output = exporter.export(&digits, None, &config)?;
/// ```
pub struct OdtExporter;

impl Exporter for OdtExporter {
    fn id(&self) -> &str {
        "nexus.odt"
    }

    fn display_name(&self) -> &str {
        "ODF Text Document"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Odt]
    }

    fn export(
        &self,
        digits: &[Digit],
        root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let ordered = order_digits(digits, root_id);

        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);

        // mimetype must be the first file, stored uncompressed (ODF spec requirement)
        let stored = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("mimetype", stored).map_err(zip_err)?;
        zip.write_all(b"application/vnd.oasis.opendocument.text").map_err(zip_err)?;

        // META-INF/manifest.xml
        zip.start_file("META-INF/manifest.xml", deflated).map_err(zip_err)?;
        zip.write_all(manifest_xml().as_bytes()).map_err(zip_err)?;

        // content.xml
        let content = build_content_xml(&ordered);
        zip.start_file("content.xml", deflated).map_err(zip_err)?;
        zip.write_all(content.as_bytes()).map_err(zip_err)?;

        let result = zip.finish().map_err(zip_err)?;

        Ok(ExportOutput::new(
            result.into_inner(),
            "export.odt",
            ExportFormat::Odt.mime_type(),
        ))
    }
}

fn zip_err(e: impl std::fmt::Display) -> NexusError {
    NexusError::ExportFailed(format!("ODT zip error: {e}"))
}

fn manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="application/vnd.oasis.opendocument.text"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#.to_string()
}

fn build_content_xml(digits: &[&Digit]) -> String {
    let mut buf = Vec::new();
    let mut writer = Writer::new_with_indent(Cursor::new(&mut buf), b' ', 2);

    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));

    // <office:document-content>
    let mut root = BytesStart::new("office:document-content");
    root.push_attribute(("xmlns:office", "urn:oasis:names:tc:opendocument:xmlns:office:1.0"));
    root.push_attribute(("xmlns:text", "urn:oasis:names:tc:opendocument:xmlns:text:1.0"));
    root.push_attribute(("xmlns:fo", "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"));
    root.push_attribute(("office:version", "1.2"));
    let _ = writer.write_event(Event::Start(root));

    // <office:body><office:text>
    let _ = writer.write_event(Event::Start(BytesStart::new("office:body")));
    let _ = writer.write_event(Event::Start(BytesStart::new("office:text")));

    for digit in digits {
        if digit.is_deleted() {
            continue;
        }
        emit_digit_odf(&mut writer, digit);
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("office:text")));
    let _ = writer.write_event(Event::End(BytesEnd::new("office:body")));
    let _ = writer.write_event(Event::End(BytesEnd::new("office:document-content")));

    String::from_utf8(buf).unwrap_or_default()
}

fn emit_digit_odf(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    match digit.digit_type() {
        "text.heading" => emit_heading(writer, digit),
        "text.paragraph" => emit_paragraph(writer, digit),
        "text.list" => emit_list(writer, digit),
        "text.blockquote" => emit_blockquote(writer, digit),
        "text.callout" => emit_callout(writer, digit),
        "text.code" => emit_code_block(writer, digit),
        "text.footnote" => emit_footnote(writer, digit),
        "text.citation" => emit_citation(writer, digit),
        _ => emit_generic(writer, digit),
    }
}

fn emit_heading(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let level = digit
        .properties
        .get("level")
        .and_then(|v| v.as_int())
        .unwrap_or(1)
        .clamp(1, 6);
    let text = prop_text(digit);
    let level_str = level.to_string();

    let mut h = BytesStart::new("text:h");
    h.push_attribute(("text:outline-level", level_str.as_str()));
    let _ = writer.write_event(Event::Start(h));
    let _ = writer.write_event(Event::Text(BytesText::new(&text)));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:h")));
}

fn emit_paragraph(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&text)));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
}

fn emit_list(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let items = extract_items(digit);
    let _ = writer.write_event(Event::Start(BytesStart::new("text:list")));
    for item in &items {
        let _ = writer.write_event(Event::Start(BytesStart::new("text:list-item")));
        let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
        let _ = writer.write_event(Event::Text(BytesText::new(item)));
        let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
        let _ = writer.write_event(Event::End(BytesEnd::new("text:list-item")));
    }
    let _ = writer.write_event(Event::End(BytesEnd::new("text:list")));
}

fn emit_blockquote(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    // ODF blockquotes are just indented paragraphs
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&format!("\u{201C}{text}\u{201D}"))));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
    if let Some(attr) = digit.properties.get("attribution").and_then(|v| v.as_str()) {
        let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
        let _ = writer.write_event(Event::Text(BytesText::new(&format!("-- {attr}"))));
        let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
    }
}

fn emit_callout(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    let style = digit
        .properties
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("info");
    let label = match style {
        "warning" => "Warning",
        "error" => "Error",
        "success" => "Success",
        "tip" => "Tip",
        _ => "Note",
    };
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&format!("[{label}] {text}"))));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
}

fn emit_code_block(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let code = digit
        .properties
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    for line in code.lines() {
        let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
        let _ = writer.write_event(Event::Text(BytesText::new(line)));
        let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
    }
}

fn emit_footnote(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let marker = digit
        .properties
        .get("marker")
        .and_then(|v| v.as_str())
        .unwrap_or("*");
    let text = prop_text(digit);
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&format!("[{marker}] {text}"))));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
}

fn emit_citation(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let source = digit
        .properties
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let author = digit
        .properties
        .get("author")
        .and_then(|v| v.as_str());
    let content = match author {
        Some(a) => format!("{a}, \"{source}\""),
        None => format!("\"{source}\""),
    };
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&content)));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
}

fn emit_generic(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    let content = if text.is_empty() {
        format!("[{}]", digit.digit_type())
    } else {
        text
    };
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&content)));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
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

fn extract_items(digit: &Digit) -> Vec<String> {
    digit
        .properties
        .get("items")
        .and_then(|v| {
            if let x::Value::Array(arr) = v {
                Some(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                )
            } else {
                None
            }
        })
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
    use x::Value;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn metadata() {
        assert_eq!(OdtExporter.id(), "nexus.odt");
        assert_eq!(OdtExporter.display_name(), "ODF Text Document");
        assert_eq!(OdtExporter.supported_formats(), &[ExportFormat::Odt]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Odt);
        let output = OdtExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.odt");
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_heading_and_paragraph() {
        let h = heading_digit(
            &HeadingMeta {
                level: 2,
                text: "Chapter One".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let p = paragraph_digit(
            &ParagraphMeta {
                text: "Body text".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Odt);
        let output = OdtExporter.export(&[h, p], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn exports_list() {
        let digit = list_digit(
            &ListMeta {
                style: ListStyle::Unordered,
                items: vec!["First".into(), "Second".into()],
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Odt);
        let output = OdtExporter.export(&[digit], None, &config).unwrap();
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
        let config = ExportConfig::new(ExportFormat::Odt);
        let output = OdtExporter.export(&[deleted], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn handles_unknown_type() {
        let digit = make_digit("some.custom");
        let config = ExportConfig::new(ExportFormat::Odt);
        let output = OdtExporter.export(&[digit], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }
}
