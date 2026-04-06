use regex::Regex;
use std::sync::LazyLock;

use crate::error::IdeasError;

static TYPE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9.\-]*$").expect("invalid regex"));

static PROPERTY_KEY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").expect("invalid regex"));

/// Validates a digit type string against the allowed pattern.
///
/// Digit types must start with a lowercase letter and contain only lowercase
/// letters, digits, dots, and hyphens. Max 64 characters.
pub fn validate_digit_type(type_str: &str) -> Result<(), IdeasError> {
    if type_str.len() > 64 {
        return Err(IdeasError::TypeTooLong(type_str.to_string(), 64));
    }
    if !TYPE_PATTERN.is_match(type_str) {
        return Err(IdeasError::InvalidDigitType(
            type_str.to_string(),
            "must start with lowercase letter, contain only lowercase letters, digits, dots, and hyphens".to_string(),
        ));
    }
    Ok(())
}

/// Validates a digit property key against the allowed pattern.
///
/// Property keys must start with a letter and contain only letters, digits,
/// underscores, and hyphens. Max 64 characters.
pub fn validate_property_key(key: &str) -> Result<(), IdeasError> {
    if key.len() > 64 {
        return Err(IdeasError::PropertyKeyTooLong(key.to_string(), 64));
    }
    if !PROPERTY_KEY_PATTERN.is_match(key) {
        return Err(IdeasError::InvalidPropertyKey(
            key.to_string(),
            "must start with letter, contain only letters, digits, underscores, and hyphens".to_string(),
        ));
    }
    Ok(())
}

/// Validates a local bond path is absolute and contains no directory traversal.
///
/// Local bond paths must start with `/` and must not contain `../` sequences.
pub fn validate_local_bond_path(path: &str) -> Result<(), IdeasError> {
    if !path.starts_with('/') {
        return Err(IdeasError::RelativePath(path.to_string()));
    }
    if path.contains("../") || path.contains("/..") {
        return Err(IdeasError::PathTraversal(path.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_digit_types() {
        assert!(validate_digit_type("text").is_ok());
        assert!(validate_digit_type("image.png").is_ok());
        assert!(validate_digit_type("code-block").is_ok());
        assert!(validate_digit_type("a").is_ok());
        assert!(validate_digit_type("text2").is_ok());
    }

    #[test]
    fn invalid_digit_types() {
        assert!(validate_digit_type("Text").is_err());
        assert!(validate_digit_type("1text").is_err());
        assert!(validate_digit_type("").is_err());
        assert!(validate_digit_type("text spaces").is_err());
        assert!(validate_digit_type(&"a".repeat(65)).is_err());
    }

    #[test]
    fn valid_property_keys() {
        assert!(validate_property_key("font").is_ok());
        assert!(validate_property_key("fontSize").is_ok());
        assert!(validate_property_key("line_height").is_ok());
        assert!(validate_property_key("A1").is_ok());
        assert!(validate_property_key("font-size").is_ok());
        assert!(validate_property_key("thumbnail-hash").is_ok());
        assert!(validate_property_key("relay-url").is_ok());
    }

    #[test]
    fn invalid_property_keys() {
        assert!(validate_property_key("1font").is_err());
        assert!(validate_property_key("").is_err());
        assert!(validate_property_key(&"a".repeat(65)).is_err());
    }

    #[test]
    fn valid_local_bond_paths() {
        assert!(validate_local_bond_path("/Users/test/doc.idea").is_ok());
        assert!(validate_local_bond_path("/").is_ok());
    }

    #[test]
    fn invalid_local_bond_paths() {
        assert!(validate_local_bond_path("relative/path").is_err());
        assert!(validate_local_bond_path("/path/../escape").is_err());
        assert!(validate_local_bond_path("/path/..").is_err());
    }
}
