//! Shared property extraction helpers for domain digit modules.
//!
//! These functions provide ergonomic access to Digit properties by key,
//! returning typed values or errors. Used by media.rs, sheet.rs, slide.rs,
//! and all other domain digit modules.

use crate::digit::Digit;
use crate::error::IdeasError;
use x::Value;

/// Extract a required string property from a digit.
pub fn prop_str(digit: &Digit, key: &str, domain: &str) -> Result<String, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| make_error(domain, &format!("missing property: {key}")))
}

/// Extract an optional string property from a digit.
pub fn prop_str_opt(digit: &Digit, key: &str) -> Option<String> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract a required integer property from a digit.
pub fn prop_int(digit: &Digit, key: &str, domain: &str) -> Result<i64, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_int())
        .ok_or_else(|| make_error(domain, &format!("missing property: {key}")))
}

/// Extract an optional integer property from a digit.
pub fn prop_int_opt(digit: &Digit, key: &str) -> Option<i64> {
    digit.properties.get(key).and_then(|v| v.as_int())
}

/// Extract a required double property from a digit.
pub fn prop_double(digit: &Digit, key: &str, domain: &str) -> Result<f64, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_double())
        .ok_or_else(|| make_error(domain, &format!("missing property: {key}")))
}

/// Extract an optional double property from a digit.
#[allow(dead_code)]
pub fn prop_double_opt(digit: &Digit, key: &str) -> Option<f64> {
    digit.properties.get(key).and_then(|v| v.as_double())
}

/// Extract a required boolean property from a digit.
pub fn prop_bool(digit: &Digit, key: &str, domain: &str) -> Result<bool, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| make_error(domain, &format!("missing property: {key}")))
}

/// Extract an optional boolean property from a digit.
pub fn prop_bool_opt(digit: &Digit, key: &str) -> Option<bool> {
    digit.properties.get(key).and_then(|v| v.as_bool())
}

/// Extract a string array property from a digit.
pub fn prop_str_array(digit: &Digit, key: &str, domain: &str) -> Result<Vec<String>, IdeasError> {
    digit
        .properties
        .get(key)
        .and_then(|v| {
            if let Value::Array(arr) = v {
                Some(
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect(),
                )
            } else {
                None
            }
        })
        .ok_or_else(|| make_error(domain, &format!("missing property: {key}")))
}

/// Check that a digit has the expected type, returning an error if not.
pub fn check_type(digit: &Digit, expected: &str, domain: &str) -> Result<(), IdeasError> {
    if digit.digit_type() != expected {
        return Err(make_error(
            domain,
            &format!("expected {expected}, got {}", digit.digit_type()),
        ));
    }
    Ok(())
}

/// Create an IdeasError for the given domain.
pub fn make_error(domain: &str, msg: &str) -> IdeasError {
    match domain {
        "sheet" => IdeasError::SheetParsing(msg.to_string()),
        "slide" => IdeasError::SlideParsing(msg.to_string()),
        "form" => IdeasError::FormParsing(msg.to_string()),
        "richtext" => IdeasError::RichTextParsing(msg.to_string()),
        "interactive" => IdeasError::InteractiveParsing(msg.to_string()),
        "commerce" => IdeasError::CommerceError(msg.to_string()),
        "accessibility" => IdeasError::AccessibilityError(msg.to_string()),
        "binding" => IdeasError::BindingError(msg.to_string()),
        _ => IdeasError::MediaParsing(msg.to_string()),
    }
}
