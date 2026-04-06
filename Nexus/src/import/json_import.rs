//! JSON importer — deserialize .idea JSON back into digits (native round-trip).

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;

/// Imports JSON as a `Vec<Digit>` — native .idea round-trip.
///
/// The JSON must be an array of serialized Digit objects. This is the
/// simplest importer: just `serde_json::from_slice`. The author from
/// config is not applied because the digits already have their original
/// authors.
#[derive(Debug)]
pub struct JsonImporter;

impl Importer for JsonImporter {
    fn id(&self) -> &str {
        "nexus.json.import"
    }

    fn display_name(&self) -> &str {
        "JSON (.idea)"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["application/json"]
    }

    fn import(
        &self,
        data: &[u8],
        _config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let digits: Vec<Digit> = serde_json::from_slice(data)
            .map_err(|e| NexusError::ImportFailed(format!("invalid JSON digit array: {e}")))?;

        // If there are digits, use the first one's id as root (convention).
        let root_id = digits.first().map(|d| d.id());

        Ok(ImportOutput::new(digits, root_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x::Value;

    #[test]
    fn import_empty_array() {
        let config = ImportConfig::new("cpub1test");
        let output = JsonImporter.import(b"[]", &config).unwrap();
        assert!(output.digits.is_empty());
        assert!(output.root_digit_id.is_none());
    }

    #[test]
    fn import_single_digit() {
        let digit = Digit::new("text".into(), Value::from("Hello"), "cpub1alice".into()).unwrap();
        let json = serde_json::to_vec(&vec![&digit]).unwrap();
        let config = ImportConfig::new("cpub1test");
        let output = JsonImporter.import(&json, &config).unwrap();
        assert_eq!(output.digits.len(), 1);
        assert_eq!(output.digits[0].id(), digit.id());
        assert_eq!(output.root_digit_id, Some(digit.id()));
    }

    #[test]
    fn import_multiple_digits() {
        let d1 = Digit::new("text".into(), Value::from("one"), "alice".into()).unwrap();
        let d2 = Digit::new("text".into(), Value::from("two"), "bob".into()).unwrap();
        let json = serde_json::to_vec(&vec![&d1, &d2]).unwrap();
        let config = ImportConfig::new("cpub1test");
        let output = JsonImporter.import(&json, &config).unwrap();
        assert_eq!(output.digits.len(), 2);
        assert_eq!(output.root_digit_id, Some(d1.id()));
    }

    #[test]
    fn import_invalid_json() {
        let config = ImportConfig::new("cpub1test");
        let result = JsonImporter.import(b"not json", &config);
        assert!(result.is_err());
    }

    #[test]
    fn import_preserves_original_author() {
        let digit =
            Digit::new("text".into(), Value::from("x"), "cpub1original".into()).unwrap();
        let json = serde_json::to_vec(&vec![&digit]).unwrap();
        let config = ImportConfig::new("cpub1different");
        let output = JsonImporter.import(&json, &config).unwrap();
        // The digit retains its original author, not the config author.
        assert_eq!(output.digits[0].author(), "cpub1original");
    }
}
