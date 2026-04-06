//! SVG exporter -- converts Ideas digits to SVG XML.
//!
//! Hand-generates valid SVG markup by mapping each digit type to SVG
//! elements. Text digits become `<text>` elements, media digits become
//! labeled `<rect>` placeholders, and layout is a simple vertical stack.
//! No external SVG library is needed.

use std::collections::HashMap;
use std::fmt::Write;

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Default SVG width.
const DEFAULT_WIDTH: f64 = 800.0;
/// Default SVG height.
const DEFAULT_HEIGHT: f64 = 600.0;
/// Left margin for text elements.
const MARGIN_X: f64 = 20.0;
/// Starting Y offset.
const START_Y: f64 = 40.0;
/// Line spacing between elements.
const LINE_SPACING: f64 = 24.0;
/// Heading font size multiplier.
const HEADING_SCALE: f64 = 1.5;

/// Exports Ideas digits as an SVG image.
///
/// Maps text-based digits (headings, paragraphs, lists, etc.) to SVG
/// `<text>` elements arranged in a vertical flow. Non-text digits are
/// rendered as labeled rectangle placeholders.
///
/// # Example
///
/// ```ignore
/// let exporter = SvgExporter;
/// let config = ExportConfig::new(ExportFormat::Svg);
/// let output = exporter.export(&digits, None, &config)?;
/// let svg = String::from_utf8(output.data).unwrap();
/// ```
pub struct SvgExporter;

impl Exporter for SvgExporter {
    fn id(&self) -> &str {
        "nexus.svg"
    }

    fn display_name(&self) -> &str {
        "SVG Image"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Svg]
    }

    fn export(
        &self,
        digits: &[Digit],
        root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let (width, height) = svg_dimensions(config);
        let ordered = order_digits(digits, root_id);

        let mut svg = String::new();
        let _ = writeln!(
            svg,
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        );
        let _ = writeln!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#,
        );
        // White background
        let _ = writeln!(
            svg,
            r#"  <rect width="{width}" height="{height}" fill="white"/>"#,
        );

        let mut y = START_Y;

        for digit in &ordered {
            if digit.is_deleted() {
                continue;
            }
            y = emit_digit(&mut svg, digit, y, width);
        }

        svg.push_str("</svg>\n");

        Ok(ExportOutput::new(
            svg.into_bytes(),
            "export.svg",
            "image/svg+xml",
        ))
    }
}

/// Emit a single digit as SVG elements. Returns the new Y position.
fn emit_digit(svg: &mut String, digit: &Digit, y: f64, width: f64) -> f64 {
    match digit.digit_type() {
        "text.heading" => emit_heading(svg, digit, y),
        "text.paragraph" => emit_paragraph(svg, digit, y),
        "text.list" => emit_list(svg, digit, y),
        "text.blockquote" => emit_blockquote(svg, digit, y),
        "text.callout" => emit_callout(svg, digit, y),
        "text.code" => emit_code_block(svg, digit, y),
        "text.footnote" => emit_footnote(svg, digit, y),
        "text.citation" => emit_citation(svg, digit, y),
        _ => emit_placeholder(svg, digit, y, width),
    }
}

fn emit_heading(svg: &mut String, digit: &Digit, y: f64) -> f64 {
    let level = digit
        .properties
        .get("level")
        .and_then(|v| v.as_int())
        .unwrap_or(1)
        .clamp(1, 6) as f64;
    let text = prop_text(digit);
    let font_size = 16.0 * HEADING_SCALE / (1.0 + (level - 1.0) * 0.15);
    let escaped = escape_xml(&text);
    let _ = writeln!(
        svg,
        r#"  <text x="{MARGIN_X}" y="{y}" font-size="{font_size:.1}" font-weight="bold" font-family="sans-serif">{escaped}</text>"#,
    );
    y + LINE_SPACING * 1.5
}

fn emit_paragraph(svg: &mut String, digit: &Digit, y: f64) -> f64 {
    let text = prop_text(digit);
    let escaped = escape_xml(&text);
    let _ = writeln!(
        svg,
        r#"  <text x="{MARGIN_X}" y="{y}" font-size="14" font-family="sans-serif">{escaped}</text>"#,
    );
    y + LINE_SPACING
}

fn emit_list(svg: &mut String, digit: &Digit, mut y: f64) -> f64 {
    let style = digit
        .properties
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("unordered");
    let items = extract_items(digit);

    for (i, item) in items.iter().enumerate() {
        let prefix = match style {
            "ordered" => format!("{}. ", i + 1),
            "checklist" => "\u{2610} ".to_string(),
            _ => "\u{2022} ".to_string(),
        };
        let escaped = escape_xml(&format!("{prefix}{item}"));
        let _ = writeln!(
            svg,
            r#"  <text x="{}" y="{y}" font-size="14" font-family="sans-serif">{escaped}</text>"#,
            MARGIN_X + 15.0,
        );
        y += LINE_SPACING;
    }
    y + LINE_SPACING * 0.5
}

fn emit_blockquote(svg: &mut String, digit: &Digit, y: f64) -> f64 {
    let text = prop_text(digit);
    let escaped = escape_xml(&text);
    let x = MARGIN_X + 10.0;
    // Draw a vertical quote bar
    let _ = writeln!(
        svg,
        r##"  <line x1="{MARGIN_X}" y1="{}" x2="{MARGIN_X}" y2="{}" stroke="#999" stroke-width="3"/>"##,
        y - 14.0,
        y + 4.0,
    );
    let _ = writeln!(
        svg,
        r#"  <text x="{x}" y="{y}" font-size="14" font-style="italic" font-family="serif">{escaped}</text>"#,
    );
    let mut next_y = y + LINE_SPACING;
    if let Some(attr) = digit.properties.get("attribution").and_then(|v| v.as_str()) {
        let escaped_attr = escape_xml(&format!("-- {attr}"));
        let _ = writeln!(
            svg,
            r#"  <text x="{x}" y="{next_y}" font-size="12" font-family="serif">{escaped_attr}</text>"#,
        );
        next_y += LINE_SPACING;
    }
    next_y + LINE_SPACING * 0.5
}

fn emit_callout(svg: &mut String, digit: &Digit, y: f64) -> f64 {
    let text = prop_text(digit);
    let style = digit
        .properties
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("info");
    let (label, color) = match style {
        "warning" => ("Warning", "#e6a700"),
        "error" => ("Error", "#cc0000"),
        "success" => ("Success", "#008800"),
        "tip" => ("Tip", "#0066cc"),
        _ => ("Note", "#666666"),
    };
    let escaped = escape_xml(&format!("[{label}] {text}"));
    let _ = writeln!(
        svg,
        r#"  <text x="{MARGIN_X}" y="{y}" font-size="14" fill="{color}" font-family="sans-serif">{escaped}</text>"#,
    );
    y + LINE_SPACING
}

fn emit_code_block(svg: &mut String, digit: &Digit, mut y: f64) -> f64 {
    let code = digit
        .properties
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    for line in code.lines() {
        let escaped = escape_xml(line);
        let _ = writeln!(
            svg,
            r#"  <text x="{}" y="{y}" font-size="12" font-family="monospace">{escaped}</text>"#,
            MARGIN_X + 10.0,
        );
        y += LINE_SPACING * 0.8;
    }
    y + LINE_SPACING * 0.5
}

fn emit_footnote(svg: &mut String, digit: &Digit, y: f64) -> f64 {
    let marker = digit
        .properties
        .get("marker")
        .and_then(|v| v.as_str())
        .unwrap_or("*");
    let text = prop_text(digit);
    let escaped = escape_xml(&format!("[{marker}] {text}"));
    let _ = writeln!(
        svg,
        r#"  <text x="{MARGIN_X}" y="{y}" font-size="11" font-family="sans-serif">{escaped}</text>"#,
    );
    y + LINE_SPACING * 0.8
}

fn emit_citation(svg: &mut String, digit: &Digit, y: f64) -> f64 {
    let source = digit
        .properties
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let author = digit
        .properties
        .get("author")
        .and_then(|v| v.as_str());
    let label = match author {
        Some(a) => format!("{a}, \"{source}\""),
        None => format!("\"{source}\""),
    };
    let escaped = escape_xml(&label);
    let _ = writeln!(
        svg,
        r#"  <text x="{MARGIN_X}" y="{y}" font-size="12" font-style="italic" font-family="serif">{escaped}</text>"#,
    );
    y + LINE_SPACING
}

fn emit_placeholder(svg: &mut String, digit: &Digit, y: f64, width: f64) -> f64 {
    let dtype = digit.digit_type();
    let rect_width = width - MARGIN_X * 2.0;
    let rect_height = 30.0;
    let _ = writeln!(
        svg,
        r##"  <rect x="{MARGIN_X}" y="{}" width="{rect_width}" height="{rect_height}" fill="#f0f0f0" stroke="#ccc" rx="4"/>"##,
        y - 14.0,
    );
    let escaped = escape_xml(&format!("[{dtype}]"));
    let _ = writeln!(
        svg,
        r##"  <text x="{}" y="{}" font-size="12" fill="#999" font-family="sans-serif">{escaped}</text>"##,
        MARGIN_X + 8.0,
        y + 2.0,
    );
    y + rect_height + LINE_SPACING * 0.5
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn svg_dimensions(config: &ExportConfig) -> (f64, f64) {
    config
        .page_size
        .map(|(w, h)| (w.max(1.0), h.max(1.0)))
        .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT))
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

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
        assert_eq!(SvgExporter.id(), "nexus.svg");
        assert_eq!(SvgExporter.display_name(), "SVG Image");
        assert_eq!(SvgExporter.supported_formats(), &[ExportFormat::Svg]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert_eq!(output.filename, "export.svg");
        assert_eq!(output.mime_type, "image/svg+xml");
    }

    #[test]
    fn exports_heading() {
        let digit = heading_digit(
            &HeadingMeta {
                level: 1,
                text: "Title".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[digit], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(svg.contains("Title"));
        assert!(svg.contains("font-weight=\"bold\""));
    }

    #[test]
    fn exports_paragraph() {
        let digit = paragraph_digit(
            &ParagraphMeta {
                text: "Hello world".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[digit], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(svg.contains("Hello world"));
    }

    #[test]
    fn exports_list() {
        let digit = list_digit(
            &ListMeta {
                style: ListStyle::Ordered,
                items: vec!["A".into(), "B".into()],
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[digit], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(svg.contains("1. A"));
        assert!(svg.contains("2. B"));
    }

    #[test]
    fn exports_unknown_as_placeholder() {
        let digit = make_digit("media.image");
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[digit], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(svg.contains("[media.image]"));
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn escapes_xml_entities() {
        let escaped = escape_xml("<script>alert('xss')&\"</script>");
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&apos;"));
        assert!(escaped.contains("&quot;"));
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
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[deleted], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(!svg.contains("gone"));
    }

    #[test]
    fn valid_svg_structure() {
        let config = ExportConfig::new(ExportFormat::Svg);
        let output = SvgExporter.export(&[], None, &config).unwrap();
        let svg = String::from_utf8(output.data).unwrap();
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    }
}
