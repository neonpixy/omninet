//! Plain text exporter -- extracts all text content from Ideas digits.
//!
//! Walks each digit and calls `Digit::extract_text()`, concatenating results
//! with newlines. No formatting, no markup -- just the text.

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as plain text (.txt).
///
/// Extracts all text content from each digit using `Digit::extract_text()`.
/// Results are separated by newlines. No formatting is applied.
///
/// # Example
///
/// ```ignore
/// let exporter = TxtExporter;
/// let config = ExportConfig::new(ExportFormat::Txt);
/// let output = exporter.export(&digits, None, &config)?;
/// let text = String::from_utf8(output.data).unwrap();
/// ```
pub struct TxtExporter;

impl Exporter for TxtExporter {
    fn id(&self) -> &str {
        "txt"
    }

    fn display_name(&self) -> &str {
        "Plain Text Exporter"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Txt]
    }

    fn export(
        &self,
        digits: &[Digit],
        _root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let mut lines = Vec::new();

        for digit in digits {
            if digit.is_deleted() {
                continue;
            }
            let text = digit.extract_text();
            if !text.is_empty() {
                lines.push(text);
            }
        }

        let mut output = lines.join("\n");
        if !output.is_empty() {
            output.push('\n');
        }

        Ok(ExportOutput::new(
            output.into_bytes(),
            "export.txt",
            "text/plain",
        ))
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
    fn exports_text_content() {
        let d1 = paragraph_digit(
            &ParagraphMeta {
                text: "First paragraph".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let d2 = paragraph_digit(
            &ParagraphMeta {
                text: "Second paragraph".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();

        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[d1, d2], None, &config).unwrap();
        let text = String::from_utf8(output.data).unwrap();

        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn exports_heading_text() {
        let digit = heading_digit(
            &HeadingMeta {
                level: 1,
                text: "Title Text".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();

        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[digit], None, &config).unwrap();
        let text = String::from_utf8(output.data).unwrap();

        assert!(text.contains("Title Text"));
    }

    #[test]
    fn exports_content_field() {
        let digit = make_digit("text")
            .with_content(Value::String("content value".into()), "test");

        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[digit], None, &config).unwrap();
        let text = String::from_utf8(output.data).unwrap();

        assert!(text.contains("content value"));
    }

    #[test]
    fn skips_tombstoned_digits() {
        let live = paragraph_digit(
            &ParagraphMeta {
                text: "visible".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let dead = paragraph_digit(
            &ParagraphMeta {
                text: "hidden".into(),
                spans: None,
            },
            "test",
        )
        .unwrap()
        .deleted("test");

        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[live, dead], None, &config).unwrap();
        let text = String::from_utf8(output.data).unwrap();

        assert!(text.contains("visible"));
        assert!(!text.contains("hidden"));
    }

    #[test]
    fn skips_digits_with_no_text() {
        let digit = make_digit("divider");
        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[digit], None, &config).unwrap();
        let text = String::from_utf8(output.data).unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn empty_digits_produces_empty_output() {
        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[], None, &config).unwrap();
        assert!(output.data.is_empty());
    }

    #[test]
    fn metadata() {
        assert_eq!(TxtExporter.id(), "txt");
        assert_eq!(TxtExporter.display_name(), "Plain Text Exporter");
        assert_eq!(TxtExporter.supported_formats(), &[ExportFormat::Txt]);
    }

    #[test]
    fn output_metadata() {
        let digit = make_digit("text")
            .with_content(Value::String("hi".into()), "test");
        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[digit], None, &config).unwrap();
        assert_eq!(output.filename, "export.txt");
        assert_eq!(output.mime_type, "text/plain");
    }

    #[test]
    fn lines_separated_by_newlines() {
        let d1 = make_digit("text")
            .with_content(Value::String("line one".into()), "test");
        let d2 = make_digit("text")
            .with_content(Value::String("line two".into()), "test");

        let config = ExportConfig::new(ExportFormat::Txt);
        let output = TxtExporter.export(&[d1, d2], None, &config).unwrap();
        let text = String::from_utf8(output.data).unwrap();

        assert!(text.contains("line one\nline two"));
    }
}
