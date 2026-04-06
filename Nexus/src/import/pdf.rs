//! PDF importer — extract text from PDF documents using `lopdf`.
//!
//! PDF text extraction is inherently lossy. This importer does its best
//! to pull readable text from each page but makes no guarantees about
//! formatting, column order, or completeness.

use lopdf::Document;

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;
use ideas::richtext::ParagraphMeta;
use x::Value;

/// Best-effort PDF text importer.
///
/// - Extracts text content from each PDF page using lopdf's built-in
///   text extraction.
/// - Creates a `text.paragraph` digit for each page that contains text.
/// - Complex layouts, images, and vector graphics are not extracted.
/// - This is intentionally simple — full-fidelity PDF import would
///   require a far more sophisticated parser.
#[derive(Debug)]
pub struct PdfImporter;

impl Importer for PdfImporter {
    fn id(&self) -> &str {
        "nexus.pdf.import"
    }

    fn display_name(&self) -> &str {
        "PDF (text only)"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["application/pdf"]
    }

    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let author = &config.author;
        let mut digits: Vec<Digit> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let doc = Document::load_mem(data)
            .map_err(|e| NexusError::ImportFailed(format!("failed to load PDF: {e}")))?;

        // Create document container.
        let root = Digit::new("document".into(), Value::Null, author.clone())
            .map_err(|e| NexusError::ImportFailed(e.to_string()))?;
        let root_id = root.id();
        let mut root_digit = root;

        let pages = doc.get_pages();
        let mut page_numbers: Vec<u32> = pages.keys().copied().collect();
        page_numbers.sort();

        if page_numbers.is_empty() {
            warnings.push("PDF contains no pages".into());
        }

        for page_num in &page_numbers {
            // lopdf::Document::extract_text takes 1-based page numbers.
            let text = match doc.extract_text(&[*page_num]) {
                Ok(t) => t,
                Err(e) => {
                    warnings.push(format!(
                        "Could not extract text from page {page_num}: {e}"
                    ));
                    continue;
                }
            };

            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                warnings.push(format!("Page {page_num} contains no extractable text"));
                continue;
            }

            let meta = ParagraphMeta { text: trimmed, spans: None };
            match ideas::richtext::paragraph_digit(&meta, author) {
                Ok(digit) => {
                    root_digit = root_digit.with_child(digit.id(), author);
                    digits.push(digit);
                }
                Err(e) => {
                    warnings.push(format!(
                        "Failed to create paragraph for page {page_num}: {e}"
                    ));
                }
            }
        }

        if digits.is_empty() {
            warnings.push("No text content could be extracted from this PDF".into());
        }

        digits.insert(0, root_digit);

        let mut output = ImportOutput::new(digits, Some(root_id));
        output.warnings = warnings;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_traits() {
        let importer = PdfImporter;
        assert_eq!(importer.id(), "nexus.pdf.import");
        assert_eq!(importer.display_name(), "PDF (text only)");
        assert_eq!(importer.supported_mime_types(), &["application/pdf"]);
    }

    #[test]
    fn invalid_pdf_bytes() {
        let config = ImportConfig::new("cpub1test");
        let result = PdfImporter.import(b"not a pdf", &config);
        assert!(result.is_err());
    }
}
