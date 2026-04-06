//! DOCX importer — parse Word documents into rich text digits using `zip` + `roxmltree`.

use std::io::{Cursor, Read};

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;
use ideas::richtext::{HeadingMeta, ParagraphMeta};
use x::Value;

/// Imports DOCX (Word) documents into Ideas rich text digits.
///
/// Approach:
/// - Opens the DOCX as a ZIP archive.
/// - Reads `word/document.xml`.
/// - Parses OOXML paragraph elements (`w:p`) and their style properties.
/// - Maps paragraph styles to heading levels (Heading1-Heading6).
/// - Regular paragraphs become `text.paragraph` digits.
///
/// Limitations:
/// - Tables, images, and complex formatting are not yet extracted.
/// - Only text content from `w:t` elements is captured.
#[derive(Debug)]
pub struct DocxImporter;

impl Importer for DocxImporter {
    fn id(&self) -> &str {
        "nexus.docx.import"
    }

    fn display_name(&self) -> &str {
        "Word (DOCX)"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"]
    }

    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let author = &config.author;
        let mut digits: Vec<Digit> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Create document container.
        let root = Digit::new("document".into(), Value::Null, author.clone())
            .map_err(|e| NexusError::ImportFailed(e.to_string()))?;
        let root_id = root.id();
        let mut root_digit = root;

        // Open ZIP.
        let cursor = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| NexusError::ImportFailed(format!("failed to open DOCX archive: {e}")))?;

        // Read document.xml.
        let xml_content = {
            let mut file = archive
                .by_name("word/document.xml")
                .map_err(|e| NexusError::ImportFailed(format!("missing document.xml: {e}")))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| NexusError::ImportFailed(format!("failed to read document.xml: {e}")))?;
            content
        };

        // Parse XML.
        let doc = roxmltree::Document::parse(&xml_content)
            .map_err(|e| NexusError::ImportFailed(format!("invalid XML in document.xml: {e}")))?;

        // Find all w:p (paragraph) elements.
        for node in doc.descendants() {
            if node.tag_name().name() != "p" {
                continue;
            }
            // Check if this is in the wordprocessingml namespace.
            if !is_word_ns(node.tag_name().namespace()) {
                continue;
            }

            // Extract text from all w:t descendants.
            let text = extract_paragraph_text(&node);
            if text.is_empty() {
                continue;
            }

            // Check paragraph style for heading level.
            let heading_level = extract_heading_level(&node);

            if let Some(level) = heading_level {
                let meta = HeadingMeta { level, text, spans: None };
                match ideas::richtext::heading_digit(&meta, author) {
                    Ok(digit) => {
                        root_digit = root_digit.with_child(digit.id(), author);
                        digits.push(digit);
                    }
                    Err(e) => warnings.push(format!("Failed to create heading: {e}")),
                }
            } else {
                let meta = ParagraphMeta { text, spans: None };
                match ideas::richtext::paragraph_digit(&meta, author) {
                    Ok(digit) => {
                        root_digit = root_digit.with_child(digit.id(), author);
                        digits.push(digit);
                    }
                    Err(e) => warnings.push(format!("Failed to create paragraph: {e}")),
                }
            }
        }

        digits.insert(0, root_digit);

        let mut output = ImportOutput::new(digits, Some(root_id));
        output.warnings = warnings;
        Ok(output)
    }
}

/// Check if a namespace URI belongs to the Word processing ML namespace.
fn is_word_ns(ns: Option<&str>) -> bool {
    match ns {
        Some(uri) => uri.contains("wordprocessingml"),
        None => true, // Accept unnamespaced elements too.
    }
}

/// Extract all text content from `w:t` elements within a paragraph node.
fn extract_paragraph_text(para: &roxmltree::Node) -> String {
    let mut text = String::new();
    for descendant in para.descendants() {
        if descendant.tag_name().name() == "t" {
            if let Some(t) = descendant.text() {
                text.push_str(t);
            }
        }
    }
    text
}

/// Check paragraph properties for a heading style.
///
/// OOXML stores heading level in `w:pPr/w:pStyle[@w:val="HeadingN"]` or
/// `w:pPr/w:pStyle[@w:val="heading N"]` or numeric outline level.
fn extract_heading_level(para: &roxmltree::Node) -> Option<u8> {
    for child in para.children() {
        if child.tag_name().name() != "pPr" {
            continue;
        }
        for prop in child.children() {
            if prop.tag_name().name() == "pStyle" {
                // Check w:val attribute.
                if let Some(val) = prop.attribute("val").or_else(|| {
                    prop.attributes()
                        .into_iter()
                        .find(|a| a.name() == "val")
                        .map(|a| a.value())
                }) {
                    return parse_heading_style(val);
                }
            }
            // Also check outlineLvl for explicit outline level.
            if prop.tag_name().name() == "outlineLvl" {
                if let Some(val) = prop.attribute("val").or_else(|| {
                    prop.attributes()
                        .into_iter()
                        .find(|a| a.name() == "val")
                        .map(|a| a.value())
                }) {
                    if let Ok(level) = val.parse::<u8>() {
                        let clamped = (level + 1).min(6);
                        if clamped >= 1 {
                            return Some(clamped);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Parse a paragraph style string to extract a heading level.
///
/// Handles common conventions:
/// - "Heading1", "Heading2", ..., "Heading6"
/// - "heading 1", "heading 2", ...
/// - "Title" -> H1
fn parse_heading_style(style: &str) -> Option<u8> {
    let lower = style.to_lowercase();

    // "headingN" or "heading N"
    if let Some(rest) = lower.strip_prefix("heading") {
        let trimmed = rest.trim();
        if let Ok(level) = trimmed.parse::<u8>() {
            if (1..=6).contains(&level) {
                return Some(level);
            }
        }
    }

    // "Title" maps to H1.
    if lower == "title" {
        return Some(1);
    }

    // "Subtitle" maps to H2.
    if lower == "subtitle" {
        return Some(2);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_heading_style_heading1() {
        assert_eq!(parse_heading_style("Heading1"), Some(1));
        assert_eq!(parse_heading_style("Heading2"), Some(2));
        assert_eq!(parse_heading_style("Heading6"), Some(6));
    }

    #[test]
    fn parse_heading_style_heading_space() {
        assert_eq!(parse_heading_style("heading 1"), Some(1));
        assert_eq!(parse_heading_style("heading 3"), Some(3));
    }

    #[test]
    fn parse_heading_style_title() {
        assert_eq!(parse_heading_style("Title"), Some(1));
        assert_eq!(parse_heading_style("Subtitle"), Some(2));
    }

    #[test]
    fn parse_heading_style_unknown() {
        assert_eq!(parse_heading_style("BodyText"), None);
        assert_eq!(parse_heading_style("Normal"), None);
    }

    #[test]
    fn invalid_docx_bytes() {
        let config = ImportConfig::new("cpub1test");
        let result = DocxImporter.import(b"not a zip", &config);
        assert!(result.is_err());
    }

    #[test]
    fn is_word_ns_checks() {
        assert!(is_word_ns(Some(
            "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
        )));
        assert!(is_word_ns(None));
        assert!(!is_word_ns(Some("http://example.com/other")));
    }

    #[test]
    fn heading_level_out_of_range() {
        assert_eq!(parse_heading_style("Heading0"), None);
        assert_eq!(parse_heading_style("Heading7"), None);
    }
}
