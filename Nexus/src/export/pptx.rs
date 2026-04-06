//! PPTX exporter -- converts Ideas slide digits to a PowerPoint presentation.
//!
//! Produces a minimal valid PPTX (Office Open XML) file by building the
//! required ZIP structure with content types, relationships, and slide XML.
//! Each `presentation.slide` digit becomes a slide. Non-slide text digits
//! are collected into a single "Notes" slide.

use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::io::{Cursor, Write};

use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as a PPTX (PowerPoint) presentation.
///
/// Creates a minimal but valid PPTX file. Slide digits
/// (`presentation.slide`) become individual slides with title text boxes.
/// Non-slide text digits are collected into a fallback "Notes" slide.
///
/// # Example
///
/// ```ignore
/// let exporter = PptxExporter;
/// let config = ExportConfig::new(ExportFormat::Pptx);
/// let output = exporter.export(&digits, None, &config)?;
/// ```
pub struct PptxExporter;

impl Exporter for PptxExporter {
    fn id(&self) -> &str {
        "nexus.pptx"
    }

    fn display_name(&self) -> &str {
        "PowerPoint Presentation"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Pptx]
    }

    fn export(
        &self,
        digits: &[Digit],
        root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let ordered = order_digits(digits, root_id);

        // Separate slide digits from text content
        let mut slides: Vec<SlideContent> = Vec::new();
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
                slides.push(SlideContent { title, body });
            } else {
                let text = prop_text(digit);
                if !text.is_empty() {
                    text_lines.push(text);
                }
            }
        }

        // If there are text lines but no slides, create a content slide
        if slides.is_empty() && !text_lines.is_empty() {
            slides.push(SlideContent {
                title: "Content".to_string(),
                body: text_lines.join("\n"),
            });
        }

        // Ensure at least one slide
        if slides.is_empty() {
            slides.push(SlideContent {
                title: "Untitled".to_string(),
                body: String::new(),
            });
        }

        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // [Content_Types].xml
        zip.start_file("[Content_Types].xml", options).map_err(zip_err)?;
        zip.write_all(content_types_xml(slides.len()).as_bytes()).map_err(zip_err)?;

        // _rels/.rels
        zip.start_file("_rels/.rels", options).map_err(zip_err)?;
        zip.write_all(root_rels_xml().as_bytes()).map_err(zip_err)?;

        // ppt/presentation.xml
        zip.start_file("ppt/presentation.xml", options).map_err(zip_err)?;
        zip.write_all(presentation_xml(slides.len()).as_bytes()).map_err(zip_err)?;

        // ppt/_rels/presentation.xml.rels
        zip.start_file("ppt/_rels/presentation.xml.rels", options).map_err(zip_err)?;
        zip.write_all(presentation_rels_xml(slides.len()).as_bytes()).map_err(zip_err)?;

        // Each slide
        for (i, slide) in slides.iter().enumerate() {
            let path = format!("ppt/slides/slide{}.xml", i + 1);
            zip.start_file(&path, options).map_err(zip_err)?;
            zip.write_all(slide_xml(&slide.title, &slide.body).as_bytes()).map_err(zip_err)?;
        }

        let result = zip.finish().map_err(zip_err)?;

        Ok(ExportOutput::new(
            result.into_inner(),
            "export.pptx",
            ExportFormat::Pptx.mime_type(),
        ))
    }
}

struct SlideContent {
    title: String,
    body: String,
}

fn zip_err(e: impl std::fmt::Display) -> NexusError {
    NexusError::ExportFailed(format!("PPTX zip error: {e}"))
}

fn content_types_xml(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
"#,
    );
    for i in 1..=slide_count {
        let _ = writeln!(
            xml,
            r#"  <Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#,
        );
    }
    xml.push_str("</Types>");
    xml
}

fn root_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#.to_string()
}

fn presentation_xml(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:sldIdLst>
"#,
    );
    for i in 1..=slide_count {
        let _ = writeln!(
            xml,
            r#"    <p:sldId id="{}" r:id="rId{}"/>"#,
            255 + i,
            i,
        );
    }
    xml.push_str(
        r#"  </p:sldIdLst>
  <p:sldSz cx="9144000" cy="6858000"/>
</p:presentation>"#,
    );
    xml
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#,
    );
    for i in 1..=slide_count {
        let _ = writeln!(
            xml,
            r#"  <Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#,
        );
    }
    xml.push_str("</Relationships>");
    xml
}

fn slide_xml(title: &str, body: &str) -> String {
    let escaped_title = escape_xml(title);
    let escaped_body = escape_xml(body);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="274638"/>
            <a:ext cx="8229600" cy="1143000"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:p><a:r><a:t>{escaped_title}</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Body"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="457200" y="1600200"/>
            <a:ext cx="8229600" cy="4525963"/>
          </a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:p><a:r><a:t>{escaped_body}</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#,
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

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
        assert_eq!(PptxExporter.id(), "nexus.pptx");
        assert_eq!(PptxExporter.display_name(), "PowerPoint Presentation");
        assert_eq!(PptxExporter.supported_formats(), &[ExportFormat::Pptx]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Pptx);
        let output = PptxExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.pptx");
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_slide_digits() {
        let slide = slide_digit(
            &SlideMeta {
                title: Some("Welcome".into()),
                speaker_notes: Some("Greet the audience".into()),
                transition: None,
                layout: SlideLayout::Title,
                order: 0,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Pptx);
        let output = PptxExporter.export(&[slide], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_text_as_fallback_slide() {
        let p = paragraph_digit(
            &ParagraphMeta {
                text: "Some notes".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Pptx);
        let output = PptxExporter.export(&[p], None, &config).unwrap();
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
        let config = ExportConfig::new(ExportFormat::Pptx);
        let output = PptxExporter.export(&[deleted], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn multiple_slides() {
        let s1 = slide_digit(
            &SlideMeta {
                title: Some("Slide 1".into()),
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
                title: Some("Slide 2".into()),
                speaker_notes: None,
                transition: None,
                layout: SlideLayout::Content,
                order: 1,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Pptx);
        let output = PptxExporter.export(&[s1, s2], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }
}
