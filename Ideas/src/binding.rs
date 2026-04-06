//! Data binding helpers — cross-file DataSource bindings.
//!
//! Data bindings connect a digit to a data source in another .idea file,
//! enabling live or snapshot data flows between documents.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::helpers::{prop_bool, prop_str, prop_str_opt};
use x::Value;

const DOMAIN: &str = "binding";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A data binding connecting a digit to a data source in another .idea file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataBinding {
    /// .idea file reference for the source.
    pub source_ref: String,
    /// Path within the source (e.g., "sheet.column_name" or "digit.property").
    pub source_path: String,
    /// Optional transform expression.
    pub transform: Option<String>,
    /// If true, subscribe via Equipment for live updates. If false, static snapshot.
    pub live: bool,
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

/// Attach a data binding to a digit, returning a new digit with
/// `binding_`-prefixed properties set.
pub fn with_data_binding(digit: Digit, binding: &DataBinding, author: &str) -> Digit {
    let mut d = digit;
    d = d.with_property(
        "binding_source_ref".into(),
        Value::String(binding.source_ref.clone()),
        author,
    );
    d = d.with_property(
        "binding_source_path".into(),
        Value::String(binding.source_path.clone()),
        author,
    );
    if let Some(ref t) = binding.transform {
        d = d.with_property(
            "binding_transform".into(),
            Value::String(t.clone()),
            author,
        );
    }
    d = d.with_property("binding_live".into(), Value::Bool(binding.live), author);
    d
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Extract a data binding from a digit's properties.
/// Returns `None` if no binding properties are present.
pub fn parse_data_binding(digit: &Digit) -> Option<DataBinding> {
    let source_ref = prop_str(digit, "binding_source_ref", DOMAIN).ok()?;
    let source_path = prop_str(digit, "binding_source_path", DOMAIN).ok()?;
    let transform = prop_str_opt(digit, "binding_transform");
    let live = prop_bool(digit, "binding_live", DOMAIN).ok()?;

    Some(DataBinding {
        source_ref,
        source_path,
        transform,
        live,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_binding() -> DataBinding {
        DataBinding {
            source_ref: "/data/sales.idea".into(),
            source_path: "sheet.revenue".into(),
            transform: Some("sum()".into()),
            live: true,
        }
    }

    #[test]
    fn round_trip() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_data_binding(digit, &test_binding(), "alice");

        let parsed = parse_data_binding(&digit).unwrap();
        assert_eq!(parsed.source_ref, "/data/sales.idea");
        assert_eq!(parsed.source_path, "sheet.revenue");
        assert_eq!(parsed.transform.as_deref(), Some("sum()"));
        assert!(parsed.live);
    }

    #[test]
    fn no_transform() {
        let binding = DataBinding {
            source_ref: "/data/users.idea".into(),
            source_path: "digit.name".into(),
            transform: None,
            live: false,
        };
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_data_binding(digit, &binding, "alice");

        let parsed = parse_data_binding(&digit).unwrap();
        assert_eq!(parsed.source_ref, "/data/users.idea");
        assert!(parsed.transform.is_none());
        assert!(!parsed.live);
    }

    #[test]
    fn no_binding_returns_none() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_data_binding(&digit).is_none());
    }

    #[test]
    fn binding_on_any_digit_type() {
        // Bindings work on any digit type — they're cross-cutting
        for digit_type in ["text", "data.cell", "media.image", "form.input"] {
            let digit = Digit::new(digit_type.into(), Value::Null, "alice".into()).unwrap();
            let digit = with_data_binding(digit, &test_binding(), "alice");
            let parsed = parse_data_binding(&digit).unwrap();
            assert_eq!(parsed.source_ref, "/data/sales.idea");
        }
    }

    #[test]
    fn serde_round_trip() {
        let binding = test_binding();
        let json = serde_json::to_string(&binding).unwrap();
        let rt: DataBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.source_ref, binding.source_ref);
        assert_eq!(rt.source_path, binding.source_path);
        assert!(rt.live);
    }

    #[test]
    fn digit_serde_round_trip_with_binding() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_data_binding(digit, &test_binding(), "alice");
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_data_binding(&rt).unwrap();
        assert_eq!(parsed.source_ref, "/data/sales.idea");
    }
}
