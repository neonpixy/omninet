use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The result of a successful export operation.
#[derive(Debug, Clone)]
pub struct ExportOutput {
    /// Raw bytes of the exported file.
    pub data: Vec<u8>,
    /// Suggested filename (e.g., "document.pdf").
    pub filename: String,
    /// MIME type of the output (e.g., "application/pdf").
    pub mime_type: String,
}

impl ExportOutput {
    /// Create a new export output.
    pub fn new(
        data: Vec<u8>,
        filename: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            data,
            filename: filename.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Size of the exported data in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// The result of a successful import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOutput {
    /// Digits produced from the imported data.
    pub digits: Vec<ideas::Digit>,
    /// The root digit ID, if one was identified (e.g., a document container).
    pub root_digit_id: Option<Uuid>,
    /// Non-fatal warnings encountered during import (e.g., unsupported
    /// features that were skipped).
    pub warnings: Vec<String>,
}

impl ImportOutput {
    /// Create a new import output with no warnings.
    pub fn new(digits: Vec<ideas::Digit>, root_digit_id: Option<Uuid>) -> Self {
        Self {
            digits,
            root_digit_id,
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the output.
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Number of digits produced.
    pub fn digit_count(&self) -> usize {
        self.digits.len()
    }
}

/// The result of a successful protocol bridge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResult {
    /// Whether the bridge operation succeeded.
    pub success: bool,
    /// Protocol-specific response data.
    pub response: serde_json::Value,
    /// Human-readable summary of what happened.
    pub summary: String,
}

impl BridgeResult {
    /// Create a successful bridge result.
    pub fn ok(summary: impl Into<String>, response: serde_json::Value) -> Self {
        Self {
            success: true,
            response,
            summary: summary.into(),
        }
    }

    /// Create a failed bridge result (for non-fatal failures that still
    /// produce a result rather than an error).
    pub fn failed(summary: impl Into<String>) -> Self {
        Self {
            success: false,
            response: serde_json::Value::Null,
            summary: summary.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_output_size() {
        let output = ExportOutput::new(vec![0u8; 1024], "test.pdf", "application/pdf");
        assert_eq!(output.size(), 1024);
        assert_eq!(output.filename, "test.pdf");
        assert_eq!(output.mime_type, "application/pdf");
    }

    #[test]
    fn import_output_with_warnings() {
        let output = ImportOutput::new(vec![], None)
            .with_warning("Unsupported font: Comic Sans")
            .with_warning("Image resolution too low");

        assert_eq!(output.digit_count(), 0);
        assert!(output.root_digit_id.is_none());
        assert_eq!(output.warnings.len(), 2);
    }

    #[test]
    fn bridge_result_ok() {
        let result = BridgeResult::ok("Email sent", serde_json::json!({"message_id": "abc123"}));
        assert!(result.success);
        assert_eq!(result.summary, "Email sent");
    }

    #[test]
    fn bridge_result_failed() {
        let result = BridgeResult::failed("SMTP server unreachable");
        assert!(!result.success);
        assert_eq!(result.response, serde_json::Value::Null);
    }

    #[test]
    fn bridge_result_serde_round_trip() {
        let result = BridgeResult::ok("done", serde_json::json!({"status": 200}));
        let json = serde_json::to_string(&result).unwrap();
        let decoded: BridgeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.success, result.success);
        assert_eq!(decoded.summary, result.summary);
    }
}
