//! Markdown exporter -- converts Ideas digits to Markdown text.
//!
//! Walks the digit list and maps each type to its Markdown equivalent.
//! Unknown digit types are emitted as HTML comments so nothing is silently lost.

use std::collections::HashMap;
use std::fmt::Write;

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as Markdown (.md).
///
/// Supports headings, paragraphs, lists, blockquotes, callouts, code blocks,
/// images, links, dividers, buttons, and footnotes. Unknown types become
/// HTML comments.
///
/// # Example
///
/// ```ignore
/// let exporter = MarkdownExporter;
/// let config = ExportConfig::new(ExportFormat::Markdown);
/// let output = exporter.export(&digits, None, &config)?;
/// let markdown = String::from_utf8(output.data).unwrap();
/// ```
pub struct MarkdownExporter;

impl Exporter for MarkdownExporter {
    fn id(&self) -> &str {
        "markdown"
    }

    fn display_name(&self) -> &str {
        "Markdown Exporter"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Markdown]
    }

    fn export(
        &self,
        digits: &[Digit],
        root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let ordered = order_digits(digits, root_id);
        let mut output = String::new();

        for digit in &ordered {
            if digit.is_deleted() {
                continue;
            }
            emit_digit(&mut output, digit);
        }

        // Trim trailing whitespace but keep one trailing newline
        let trimmed = output.trim_end().to_string() + "\n";

        Ok(ExportOutput::new(
            trimmed.into_bytes(),
            "export.md",
            "text/markdown",
        ))
    }
}

/// Emit the Markdown representation of a single digit.
fn emit_digit(out: &mut String, digit: &Digit) {
    match digit.digit_type() {
        "text.heading" => emit_heading(out, digit),
        "text.paragraph" => emit_paragraph(out, digit),
        "text.list" => emit_list(out, digit),
        "text.blockquote" => emit_blockquote(out, digit),
        "text.callout" => emit_callout(out, digit),
        "text.code" => emit_code_block(out, digit),
        "text.footnote" => emit_footnote(out, digit),
        "text.citation" => emit_citation(out, digit),
        "media.image" => emit_image(out, digit),
        "link" => emit_link(out, digit),
        "divider" => emit_divider(out),
        "interactive.button" => emit_button(out, digit),
        "interactive.nav-link" => emit_nav_link(out, digit),
        _ => emit_unknown(out, digit),
    }
}

fn emit_heading(out: &mut String, digit: &Digit) {
    let level = digit
        .properties
        .get("level")
        .and_then(|v| v.as_int())
        .unwrap_or(1)
        .clamp(1, 6) as usize;
    let text = prop_text(digit);
    let hashes = "#".repeat(level);
    let _ = writeln!(out, "{hashes} {text}");
    out.push('\n');
}

fn emit_paragraph(out: &mut String, digit: &Digit) {
    let text = prop_text(digit);
    let _ = writeln!(out, "{text}");
    out.push('\n');
}

fn emit_list(out: &mut String, digit: &Digit) {
    let style = digit
        .properties
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("unordered");

    let items: Vec<String> = digit
        .properties
        .get("items")
        .and_then(|v| {
            if let x::Value::Array(arr) = v {
                Some(
                    arr.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect(),
                )
            } else {
                None
            }
        })
        .unwrap_or_default();

    for (i, item) in items.iter().enumerate() {
        match style {
            "ordered" => {
                let _ = writeln!(out, "{}. {item}", i + 1);
            }
            "checklist" => {
                // Checklist items may contain [ ] or [x] prefix
                if item.starts_with("[x]") || item.starts_with("[ ]") {
                    let _ = writeln!(out, "- {item}");
                } else {
                    let _ = writeln!(out, "- [ ] {item}");
                }
            }
            _ => {
                let _ = writeln!(out, "- {item}");
            }
        }
    }
    out.push('\n');
}

fn emit_blockquote(out: &mut String, digit: &Digit) {
    let text = prop_text(digit);
    for line in text.lines() {
        let _ = writeln!(out, "> {line}");
    }
    if let Some(attr) = digit.properties.get("attribution").and_then(|v| v.as_str()) {
        let _ = writeln!(out, ">");
        let _ = writeln!(out, "> -- {attr}");
    }
    out.push('\n');
}

fn emit_callout(out: &mut String, digit: &Digit) {
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

    let _ = writeln!(out, "> **{label}:** {text}");
    out.push('\n');
}

fn emit_code_block(out: &mut String, digit: &Digit) {
    let code = digit
        .properties
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let language = digit
        .properties
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let _ = writeln!(out, "```{language}");
    let _ = writeln!(out, "{code}");
    out.push_str("```\n\n");
}

fn emit_footnote(out: &mut String, digit: &Digit) {
    let marker = digit
        .properties
        .get("marker")
        .and_then(|v| v.as_str())
        .unwrap_or("*");
    let text = prop_text(digit);
    let _ = writeln!(out, "[^{marker}]: {text}");
    out.push('\n');
}

fn emit_citation(out: &mut String, digit: &Digit) {
    let source = digit
        .properties
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let url = digit.properties.get("url").and_then(|v| v.as_str());
    let author = digit.properties.get("author").and_then(|v| v.as_str());

    let mut parts = Vec::new();
    if let Some(a) = author {
        parts.push(a.to_string());
    }
    parts.push(match url {
        Some(u) => format!("[{source}]({u})"),
        None => source.to_string(),
    });

    let _ = writeln!(out, "> {}", parts.join(", "));
    out.push('\n');
}

fn emit_image(out: &mut String, digit: &Digit) {
    let alt = digit
        .properties
        .get("alt")
        .and_then(|v| v.as_str())
        .unwrap_or("Image");
    let hash = digit
        .properties
        .get("hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let _ = writeln!(out, "![{alt}]({hash})");
    out.push('\n');
}

fn emit_link(out: &mut String, digit: &Digit) {
    let text = digit
        .content
        .as_str()
        .or_else(|| digit.properties.get("text").and_then(|v| v.as_str()))
        .or_else(|| digit.properties.get("label").and_then(|v| v.as_str()))
        .unwrap_or("Link");
    let url = digit
        .properties
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("#");
    let _ = writeln!(out, "[{text}]({url})");
    out.push('\n');
}

fn emit_divider(out: &mut String) {
    out.push_str("---\n\n");
}

fn emit_button(out: &mut String, digit: &Digit) {
    let label = digit
        .properties
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or("Button");
    let _ = writeln!(out, "[Button: {label}]");
    out.push('\n');
}

fn emit_nav_link(out: &mut String, digit: &Digit) {
    let label = digit
        .properties
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or("Link");
    let target = digit
        .properties
        .get("target_ref")
        .and_then(|v| v.as_str())
        .unwrap_or("#");
    let _ = writeln!(out, "[{label}]({target})");
    out.push('\n');
}

fn emit_unknown(out: &mut String, digit: &Digit) {
    let dtype = digit.digit_type();
    let _ = writeln!(out, "<!-- unknown: {dtype} -->");
    out.push('\n');
}

/// Extract the primary text from a digit (checks "text" property, then content).
fn prop_text(digit: &Digit) -> String {
    digit
        .properties
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| digit.content.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

/// Order digits by following the parent-child tree from root_id.
/// Falls back to input order if no root or no children structure.
fn order_digits(digits: &[Digit], root_id: Option<Uuid>) -> Vec<&Digit> {
    if digits.is_empty() {
        return Vec::new();
    }

    let index: HashMap<Uuid, &Digit> = digits.iter().map(|d| (d.id(), d)).collect();

    if let Some(rid) = root_id {
        if let Some(&root) = index.get(&rid) {
            let mut ordered = Vec::new();
            walk_tree(root, &index, &mut ordered);
            // Add any orphans not visited
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

    // No root: return in input order
    digits.iter().collect()
}

/// Depth-first walk of the digit tree.
fn walk_tree<'a>(
    digit: &'a Digit,
    index: &HashMap<Uuid, &'a Digit>,
    out: &mut Vec<&'a Digit>,
) {
    out.push(digit);
    if let Some(children) = &digit.children {
        for child_id in children {
            if let Some(&child) = index.get(child_id) {
                walk_tree(child, index, out);
            }
        }
    }
}

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
    fn exports_heading() {
        let digit = heading_digit(
            &HeadingMeta {
                level: 2,
                text: "Hello World".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("## Hello World"));
    }

    #[test]
    fn exports_paragraph() {
        let digit = paragraph_digit(
            &ParagraphMeta {
                text: "Some text here.".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("Some text here."));
    }

    #[test]
    fn exports_unordered_list() {
        let digit = list_digit(
            &ListMeta {
                style: ListStyle::Unordered,
                items: vec!["Alpha".into(), "Beta".into()],
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("- Alpha"));
        assert!(md.contains("- Beta"));
    }

    #[test]
    fn exports_ordered_list() {
        let digit = list_digit(
            &ListMeta {
                style: ListStyle::Ordered,
                items: vec!["First".into(), "Second".into()],
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("1. First"));
        assert!(md.contains("2. Second"));
    }

    #[test]
    fn exports_blockquote() {
        let digit = blockquote_digit(
            &BlockquoteMeta {
                text: "A wise saying".into(),
                attribution: Some("Author".into()),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("> A wise saying"));
        assert!(md.contains("> -- Author"));
    }

    #[test]
    fn exports_callout() {
        let digit = callout_digit(
            &CalloutMeta {
                text: "Be careful!".into(),
                style: "warning".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("> **Warning:** Be careful!"));
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
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
        assert!(md.contains("```"));
    }

    #[test]
    fn exports_divider() {
        let digit = make_digit("divider");
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("---"));
    }

    #[test]
    fn exports_button() {
        let digit = make_digit("interactive.button")
            .with_property("label".into(), Value::String("Click Me".into()), "test")
            .with_property("style".into(), Value::String("primary".into()), "test");
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("[Button: Click Me]"));
    }

    #[test]
    fn exports_unknown_type_as_comment() {
        let digit = make_digit("some.custom.type");
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[digit], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("<!-- unknown: some.custom.type -->"));
    }

    #[test]
    fn skips_tombstoned_digits() {
        let digit = paragraph_digit(
            &ParagraphMeta {
                text: "visible".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let deleted = paragraph_digit(
            &ParagraphMeta {
                text: "deleted".into(),
                spans: None,
            },
            "test",
        )
        .unwrap()
        .deleted("test");

        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter
            .export(&[digit, deleted], None, &config)
            .unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert!(md.contains("visible"));
        assert!(!md.contains("deleted"));
    }

    #[test]
    fn metadata() {
        assert_eq!(MarkdownExporter.id(), "markdown");
        assert_eq!(MarkdownExporter.display_name(), "Markdown Exporter");
        assert_eq!(
            MarkdownExporter.supported_formats(),
            &[ExportFormat::Markdown]
        );
    }

    #[test]
    fn empty_digits_produces_empty_output() {
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[], None, &config).unwrap();
        let md = String::from_utf8(output.data).unwrap();
        assert_eq!(md, "\n");
    }

    #[test]
    fn output_metadata() {
        let config = ExportConfig::new(ExportFormat::Markdown);
        let output = MarkdownExporter.export(&[], None, &config).unwrap();
        assert_eq!(output.filename, "export.md");
        assert_eq!(output.mime_type, "text/markdown");
    }
}
