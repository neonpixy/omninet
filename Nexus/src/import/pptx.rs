//! PPTX importer — parse PowerPoint presentations into slide digits using `zip` + `roxmltree`.

use std::io::{Cursor, Read};

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;
use ideas::richtext::ParagraphMeta;
use ideas::slide::{SlideLayout, SlideMeta};
use x::Value;

/// Imports PPTX (PowerPoint) presentations into Ideas slide digits.
///
/// Approach:
/// - Opens the PPTX as a ZIP archive.
/// - Discovers slide XML files (`ppt/slides/slide1.xml`, etc.).
/// - Extracts text content from each slide's shape tree.
/// - Creates a `presentation.slide` digit per slide with text content
///   as `text.paragraph` children.
///
/// Limitations:
/// - Images, charts, and complex shapes are not extracted.
/// - Slide layouts and master slides are not interpreted.
/// - Only text from `a:t` elements is captured.
#[derive(Debug)]
pub struct PptxImporter;

impl Importer for PptxImporter {
    fn id(&self) -> &str {
        "nexus.pptx.import"
    }

    fn display_name(&self) -> &str {
        "PowerPoint (PPTX)"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["application/vnd.openxmlformats-officedocument.presentationml.presentation"]
    }

    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let author = &config.author;
        let mut digits: Vec<Digit> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Create a presentation container as root.
        let root = Digit::new("presentation".into(), Value::Null, author.clone())
            .map_err(|e| NexusError::ImportFailed(e.to_string()))?;
        let root_id = root.id();
        let mut root_digit = root;

        // Open ZIP.
        let cursor = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| NexusError::ImportFailed(format!("failed to open PPTX archive: {e}")))?;

        // Discover slide files. They follow the pattern ppt/slides/slideN.xml.
        let mut slide_paths: Vec<String> = archive
            .file_names()
            .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
            .map(|s| s.to_string())
            .collect();

        // Sort by slide number for correct ordering.
        slide_paths.sort_by(|a, b| {
            let num_a = extract_slide_number(a);
            let num_b = extract_slide_number(b);
            num_a.cmp(&num_b)
        });

        for (order, slide_path) in slide_paths.iter().enumerate() {
            let xml_content = {
                let mut file = match archive.by_name(slide_path) {
                    Ok(f) => f,
                    Err(e) => {
                        warnings.push(format!("Skipped {slide_path}: {e}"));
                        continue;
                    }
                };
                let mut content = String::new();
                if let Err(e) = file.read_to_string(&mut content) {
                    warnings.push(format!("Failed to read {slide_path}: {e}"));
                    continue;
                }
                content
            };

            let doc = match roxmltree::Document::parse(&xml_content) {
                Ok(d) => d,
                Err(e) => {
                    warnings.push(format!("Failed to parse {slide_path}: {e}"));
                    continue;
                }
            };

            // Extract all text paragraphs from the slide.
            let paragraphs = extract_slide_text(&doc);

            // Determine title: first non-empty paragraph, or None.
            let title = paragraphs.first().cloned();

            let slide_meta = SlideMeta {
                title: title.clone(),
                speaker_notes: None,
                transition: None,
                layout: SlideLayout::Content,
                order: order as u32,
            };

            let slide = match ideas::slide::slide_digit(&slide_meta, author) {
                Ok(d) => d,
                Err(e) => {
                    warnings.push(format!("Failed to create slide {}: {e}", order + 1));
                    continue;
                }
            };
            let slide_id = slide.id();
            let mut slide_digit = slide;

            // Create child paragraph digits for each text block.
            for (i, para_text) in paragraphs.iter().enumerate() {
                // Skip the title text as a child — it's already in slide metadata.
                if i == 0 && title.is_some() {
                    continue;
                }
                if para_text.is_empty() {
                    continue;
                }
                let meta = ParagraphMeta {
                    text: para_text.clone(),
                    spans: None,
                };
                match ideas::richtext::paragraph_digit(&meta, author) {
                    Ok(para) => {
                        slide_digit = slide_digit.with_child(para.id(), author);
                        digits.push(para);
                    }
                    Err(e) => {
                        warnings.push(format!("Failed to create paragraph in slide {}: {e}", order + 1));
                    }
                }
            }

            root_digit = root_digit.with_child(slide_id, author);
            digits.push(slide_digit);
        }

        digits.insert(0, root_digit);

        let mut output = ImportOutput::new(digits, Some(root_id));
        output.warnings = warnings;
        Ok(output)
    }
}

/// Extract the slide number from a path like "ppt/slides/slide3.xml" -> 3.
fn extract_slide_number(path: &str) -> u32 {
    let filename = path.rsplit('/').next().unwrap_or("");
    let num_str = filename
        .strip_prefix("slide")
        .and_then(|s| s.strip_suffix(".xml"))
        .unwrap_or("0");
    num_str.parse().unwrap_or(0)
}

/// Extract all text paragraphs from a slide XML document.
///
/// Walks all `a:p` elements and collects `a:t` text runs within each.
fn extract_slide_text(doc: &roxmltree::Document) -> Vec<String> {
    let mut paragraphs: Vec<String> = Vec::new();

    for node in doc.descendants() {
        // Look for <a:p> (DrawingML paragraph) elements.
        if node.tag_name().name() != "p" {
            continue;
        }
        // Check it's in a DrawingML namespace.
        if let Some(ns) = node.tag_name().namespace() {
            if !ns.contains("drawingml") {
                continue;
            }
        }

        let mut para_text = String::new();
        for child in node.descendants() {
            if child.tag_name().name() == "t" {
                if let Some(text) = child.text() {
                    para_text.push_str(text);
                }
            }
        }
        let trimmed = para_text.trim().to_string();
        if !trimmed.is_empty() {
            paragraphs.push(trimmed);
        }
    }

    paragraphs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_slide_number_basic() {
        assert_eq!(extract_slide_number("ppt/slides/slide1.xml"), 1);
        assert_eq!(extract_slide_number("ppt/slides/slide10.xml"), 10);
        assert_eq!(extract_slide_number("ppt/slides/slide2.xml"), 2);
    }

    #[test]
    fn extract_slide_number_invalid() {
        assert_eq!(extract_slide_number("ppt/slides/foo.xml"), 0);
        assert_eq!(extract_slide_number(""), 0);
    }

    #[test]
    fn invalid_pptx_bytes() {
        let config = ImportConfig::new("cpub1test");
        let result = PptxImporter.import(b"not a zip", &config);
        assert!(result.is_err());
    }

    #[test]
    fn import_traits() {
        let importer = PptxImporter;
        assert_eq!(importer.id(), "nexus.pptx.import");
        assert_eq!(importer.display_name(), "PowerPoint (PPTX)");
        assert_eq!(
            importer.supported_mime_types(),
            &["application/vnd.openxmlformats-officedocument.presentationml.presentation"]
        );
    }
}
