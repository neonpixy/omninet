//! DOCX exporter -- converts Ideas rich text digits to a Word document.
//!
//! Produces a minimal valid DOCX (Office Open XML) file by building the
//! required ZIP structure with `[Content_Types].xml`, `_rels/.rels`,
//! `word/_rels/document.xml.rels`, and `word/document.xml`. Maps text
//! digits to OOXML paragraph/run elements.

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

/// Exports Ideas digits as a DOCX (Word) document.
///
/// Creates a minimal but valid DOCX file that opens in Microsoft Word,
/// LibreOffice Writer, and Google Docs. Rich text digits (headings,
/// paragraphs, lists, blockquotes, code) are mapped to their OOXML
/// equivalents.
///
/// # Example
///
/// ```ignore
/// let exporter = DocxExporter;
/// let config = ExportConfig::new(ExportFormat::Docx);
/// let output = exporter.export(&digits, None, &config)?;
/// std::fs::write("document.docx", &output.data)?;
/// ```
pub struct DocxExporter;

impl Exporter for DocxExporter {
    fn id(&self) -> &str {
        "nexus.docx"
    }

    fn display_name(&self) -> &str {
        "Word Document"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Docx]
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
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // [Content_Types].xml
        zip.start_file("[Content_Types].xml", options)
            .map_err(zip_err)?;
        zip.write_all(content_types_xml().as_bytes())
            .map_err(zip_err)?;

        // _rels/.rels
        zip.start_file("_rels/.rels", options)
            .map_err(zip_err)?;
        zip.write_all(rels_xml().as_bytes())
            .map_err(zip_err)?;

        // word/_rels/document.xml.rels
        zip.start_file("word/_rels/document.xml.rels", options)
            .map_err(zip_err)?;
        zip.write_all(document_rels_xml().as_bytes())
            .map_err(zip_err)?;

        // word/document.xml
        let doc_xml = build_document_xml(&ordered);
        zip.start_file("word/document.xml", options)
            .map_err(zip_err)?;
        zip.write_all(doc_xml.as_bytes())
            .map_err(zip_err)?;

        let result = zip.finish().map_err(zip_err)?;

        Ok(ExportOutput::new(
            result.into_inner(),
            "export.docx",
            ExportFormat::Docx.mime_type(),
        ))
    }
}

fn zip_err(e: impl std::fmt::Display) -> NexusError {
    NexusError::ExportFailed(format!("DOCX zip error: {e}"))
}

fn content_types_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#.to_string()
}

fn rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#.to_string()
}

fn document_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#.to_string()
}

/// Build the word/document.xml with content from digits.
fn build_document_xml(digits: &[&Digit]) -> String {
    let mut buf = Vec::new();
    let mut writer = Writer::new_with_indent(Cursor::new(&mut buf), b' ', 2);

    // XML declaration
    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), Some("yes"))));

    // <w:document>
    let mut doc_start = BytesStart::new("w:document");
    doc_start.push_attribute(("xmlns:w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"));
    doc_start.push_attribute(("xmlns:r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"));
    let _ = writer.write_event(Event::Start(doc_start));

    // <w:body>
    let _ = writer.write_event(Event::Start(BytesStart::new("w:body")));

    for digit in digits {
        if digit.is_deleted() {
            continue;
        }
        emit_digit_xml(&mut writer, digit);
    }

    // </w:body>
    let _ = writer.write_event(Event::End(BytesEnd::new("w:body")));
    // </w:document>
    let _ = writer.write_event(Event::End(BytesEnd::new("w:document")));

    String::from_utf8(buf).unwrap_or_default()
}

/// Emit a single digit as OOXML paragraph(s).
fn emit_digit_xml(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
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
    let style_name = format!("Heading{level}");

    // <w:p>
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    // <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
    let _ = writer.write_event(Event::Start(BytesStart::new("w:pPr")));
    let mut style = BytesStart::new("w:pStyle");
    style.push_attribute(("w:val", style_name.as_str()));
    let _ = writer.write_event(Event::Empty(style));
    let _ = writer.write_event(Event::End(BytesEnd::new("w:pPr")));
    // <w:r><w:t>text</w:t></w:r>
    write_run(writer, &text, false);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
}

fn emit_paragraph(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    write_run(writer, &text, false);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
}

fn emit_list(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let style = digit
        .properties
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("unordered");
    let items = extract_items(digit);

    for (i, item) in items.iter().enumerate() {
        let prefix = match style {
            "ordered" => format!("{}. ", i + 1),
            "checklist" => "[ ] ".to_string(),
            _ => "- ".to_string(),
        };
        let text = format!("{prefix}{item}");
        let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
        // Indent for list items
        let _ = writer.write_event(Event::Start(BytesStart::new("w:pPr")));
        let _ = writer.write_event(Event::Start(BytesStart::new("w:ind")));
        let mut ind = BytesStart::new("w:ind");
        ind.push_attribute(("w:left", "720"));
        let _ = writer.write_event(Event::Empty(ind));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:ind")));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:pPr")));
        write_run(writer, &text, false);
        let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
    }
}

fn emit_blockquote(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    let _ = writer.write_event(Event::Start(BytesStart::new("w:pPr")));
    let mut ind = BytesStart::new("w:ind");
    ind.push_attribute(("w:left", "720"));
    let _ = writer.write_event(Event::Empty(ind));
    let _ = writer.write_event(Event::End(BytesEnd::new("w:pPr")));
    write_run(writer, &text, true);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
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
    let content = format!("[{label}] {text}");
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    write_run(writer, &content, false);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
}

fn emit_code_block(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let code = digit
        .properties
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    for line in code.lines() {
        let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
        // Use monospace-style run properties
        let _ = writer.write_event(Event::Start(BytesStart::new("w:r")));
        let _ = writer.write_event(Event::Start(BytesStart::new("w:rPr")));
        let mut font = BytesStart::new("w:rFonts");
        font.push_attribute(("w:ascii", "Courier New"));
        font.push_attribute(("w:hAnsi", "Courier New"));
        let _ = writer.write_event(Event::Empty(font));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:rPr")));
        let mut t = BytesStart::new("w:t");
        t.push_attribute(("xml:space", "preserve"));
        let _ = writer.write_event(Event::Start(t));
        let _ = writer.write_event(Event::Text(BytesText::new(line)));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:t")));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:r")));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
    }
}

fn emit_footnote(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let marker = digit
        .properties
        .get("marker")
        .and_then(|v| v.as_str())
        .unwrap_or("*");
    let text = prop_text(digit);
    let content = format!("[{marker}] {text}");
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    write_run(writer, &content, false);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
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
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    write_run(writer, &content, true);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
}

fn emit_generic(writer: &mut Writer<Cursor<&mut Vec<u8>>>, digit: &Digit) {
    let text = prop_text(digit);
    let content = if text.is_empty() {
        format!("[{}]", digit.digit_type())
    } else {
        text
    };
    let _ = writer.write_event(Event::Start(BytesStart::new("w:p")));
    write_run(writer, &content, false);
    let _ = writer.write_event(Event::End(BytesEnd::new("w:p")));
}

/// Write a <w:r><w:t>text</w:t></w:r> element, optionally italic.
fn write_run(writer: &mut Writer<Cursor<&mut Vec<u8>>>, text: &str, italic: bool) {
    let _ = writer.write_event(Event::Start(BytesStart::new("w:r")));
    if italic {
        let _ = writer.write_event(Event::Start(BytesStart::new("w:rPr")));
        let _ = writer.write_event(Event::Empty(BytesStart::new("w:i")));
        let _ = writer.write_event(Event::End(BytesEnd::new("w:rPr")));
    }
    let mut t = BytesStart::new("w:t");
    t.push_attribute(("xml:space", "preserve"));
    let _ = writer.write_event(Event::Start(t));
    let _ = writer.write_event(Event::Text(BytesText::new(text)));
    let _ = writer.write_event(Event::End(BytesEnd::new("w:t")));
    let _ = writer.write_event(Event::End(BytesEnd::new("w:r")));
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
        assert_eq!(DocxExporter.id(), "nexus.docx");
        assert_eq!(DocxExporter.display_name(), "Word Document");
        assert_eq!(DocxExporter.supported_formats(), &[ExportFormat::Docx]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Docx);
        let output = DocxExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.docx");
        // ZIP magic number: PK
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_heading_and_paragraph() {
        let h = heading_digit(
            &HeadingMeta {
                level: 1,
                text: "Title".into(),
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
        let config = ExportConfig::new(ExportFormat::Docx);
        let output = DocxExporter.export(&[h, p], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_code_block() {
        let digit = code_block_digit(
            &CodeBlockMeta {
                code: "fn main() {}".into(),
                language: Some("rust".into()),
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Docx);
        let output = DocxExporter.export(&[digit], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn exports_list() {
        let digit = list_digit(
            &ListMeta {
                style: ListStyle::Unordered,
                items: vec!["Alpha".into(), "Beta".into()],
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Docx);
        let output = DocxExporter.export(&[digit], None, &config).unwrap();
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
        let config = ExportConfig::new(ExportFormat::Docx);
        let output = DocxExporter.export(&[deleted], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn handles_unknown_digit_type() {
        let digit = make_digit("some.custom.type");
        let config = ExportConfig::new(ExportFormat::Docx);
        let output = DocxExporter.export(&[digit], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }
}
