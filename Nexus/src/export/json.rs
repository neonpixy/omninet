//! JSON exporter -- serializes Ideas digits to a JSON array.
//!
//! The simplest exporter: takes all non-tombstoned digits and serializes them
//! as a pretty-printed JSON array via `serde_json`.

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as a JSON array (.json).
///
/// Non-tombstoned digits are serialized as a pretty-printed JSON array.
/// This is the simplest exporter -- a direct serialization with no
/// transformation.
///
/// # Example
///
/// ```ignore
/// let exporter = JsonExporter;
/// let config = ExportConfig::new(ExportFormat::Json);
/// let output = exporter.export(&digits, None, &config)?;
/// let json = String::from_utf8(output.data).unwrap();
/// ```
pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn id(&self) -> &str {
        "json"
    }

    fn display_name(&self) -> &str {
        "JSON Exporter"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Json]
    }

    fn export(
        &self,
        digits: &[Digit],
        _root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let live: Vec<&Digit> = digits.iter().filter(|d| !d.is_deleted()).collect();
        let data = serde_json::to_vec_pretty(&live)?;

        Ok(ExportOutput::new(data, "export.json", "application/json"))
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
    fn exports_empty_as_empty_array() {
        let config = ExportConfig::new(ExportFormat::Json);
        let output = JsonExporter.export(&[], None, &config).unwrap();
        let json = String::from_utf8(output.data).unwrap();
        assert_eq!(json.trim(), "[]");
    }

    #[test]
    fn exports_digits_as_json_array() {
        let d1 = paragraph_digit(
            &ParagraphMeta {
                text: "Hello".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let d2 = heading_digit(
            &HeadingMeta {
                level: 1,
                text: "Title".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();

        let config = ExportConfig::new(ExportFormat::Json);
        let output = JsonExporter.export(&[d1, d2], None, &config).unwrap();
        let json = String::from_utf8(output.data).unwrap();

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn skips_tombstoned_digits() {
        let live = make_digit("text")
            .with_content(Value::String("visible".into()), "test");
        let dead = make_digit("text")
            .with_content(Value::String("hidden".into()), "test")
            .deleted("test");

        let config = ExportConfig::new(ExportFormat::Json);
        let output = JsonExporter
            .export(&[live, dead], None, &config)
            .unwrap();
        let json = String::from_utf8(output.data).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn output_is_pretty_printed() {
        let digit = make_digit("text")
            .with_content(Value::String("hello".into()), "test");
        let config = ExportConfig::new(ExportFormat::Json);
        let output = JsonExporter.export(&[digit], None, &config).unwrap();
        let json = String::from_utf8(output.data).unwrap();

        // Pretty-printed JSON contains newlines and indentation
        assert!(json.contains('\n'));
        assert!(json.contains("  "));
    }

    #[test]
    fn round_trip_through_serde() {
        let original = paragraph_digit(
            &ParagraphMeta {
                text: "Round trip test".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();

        let config = ExportConfig::new(ExportFormat::Json);
        let output = JsonExporter
            .export(std::slice::from_ref(&original), None, &config)
            .unwrap();
        let json = String::from_utf8(output.data).unwrap();

        let decoded: Vec<Digit> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].id(), original.id());
        assert_eq!(decoded[0].digit_type(), original.digit_type());
    }

    #[test]
    fn metadata() {
        assert_eq!(JsonExporter.id(), "json");
        assert_eq!(JsonExporter.display_name(), "JSON Exporter");
        assert_eq!(JsonExporter.supported_formats(), &[ExportFormat::Json]);
    }

    #[test]
    fn output_metadata() {
        let config = ExportConfig::new(ExportFormat::Json);
        let output = JsonExporter.export(&[], None, &config).unwrap();
        assert_eq!(output.filename, "export.json");
        assert_eq!(output.mime_type, "application/json");
    }
}
