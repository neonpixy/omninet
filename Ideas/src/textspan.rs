//! Inline text formatting via TextSpan — structured spans on richtext digits.
//!
//! Design: `spans: [{text, attributes}]` + plain `text` for search/accessibility.
//! All Office programs share this model (Quill, Tome, Courier, etc.).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use x::Value;

/// Inline formatting attributes for a span of text.
///
/// All fields are optional — a span with no attributes set is plain text.
/// Known attributes have typed fields; unknown attributes go in `extra`
/// for forward-compatible extensibility.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextAttribute {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Catch-all for future or custom attributes.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl TextAttribute {
    /// Returns true if all known attributes are None and extra is empty (plain text).
    pub fn is_plain(&self) -> bool {
        self.bold.is_none()
            && self.italic.is_none()
            && self.underline.is_none()
            && self.strikethrough.is_none()
            && self.code.is_none()
            && self.link.is_none()
            && self.color.is_none()
            && self.font.is_none()
            && self.font_size.is_none()
            && self.font_weight.is_none()
            && self.language.is_none()
            && self.extra.is_empty()
    }
}

/// A span of text with inline formatting attributes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    #[serde(default)]
    pub attributes: TextAttribute,
}

impl TextSpan {
    /// Create a plain text span with no formatting.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            attributes: TextAttribute::default(),
        }
    }

    /// Create a span with specific attributes.
    pub fn with_attributes(text: impl Into<String>, attributes: TextAttribute) -> Self {
        Self {
            text: text.into(),
            attributes,
        }
    }
}

/// Concatenate all span texts into a single plain text string for search/accessibility.
pub fn plain_text_from_spans(spans: &[TextSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect::<String>()
}

/// Serialize spans to a `Value::String` for storage in Digit properties.
pub fn spans_to_value(spans: &[TextSpan]) -> Value {
    match serde_json::to_string(spans) {
        Ok(json) => Value::String(json),
        Err(_) => Value::Null,
    }
}

/// Deserialize spans from a `Value::String` property.
pub fn spans_from_value(value: &Value) -> Option<Vec<TextSpan>> {
    value
        .as_str()
        .and_then(|json| serde_json::from_str(json).ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_span() {
        let span = TextSpan::plain("hello");
        assert_eq!(span.text, "hello");
        assert!(span.attributes.is_plain());
    }

    #[test]
    fn bold_span() {
        let span = TextSpan::with_attributes(
            "bold text",
            TextAttribute {
                bold: Some(true),
                ..Default::default()
            },
        );
        assert!(!span.attributes.is_plain());
        assert_eq!(span.attributes.bold, Some(true));
    }

    #[test]
    fn mixed_formatting() {
        let span = TextSpan::with_attributes(
            "fancy",
            TextAttribute {
                bold: Some(true),
                italic: Some(true),
                color: Some("#ff0000".into()),
                ..Default::default()
            },
        );
        assert_eq!(span.attributes.bold, Some(true));
        assert_eq!(span.attributes.italic, Some(true));
        assert_eq!(span.attributes.color.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn link_span() {
        let span = TextSpan::with_attributes(
            "click here",
            TextAttribute {
                link: Some("net://example.idea".into()),
                underline: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(span.attributes.link.as_deref(), Some("net://example.idea"));
    }

    #[test]
    fn plain_text_extraction() {
        let spans = vec![
            TextSpan::plain("Hello "),
            TextSpan::with_attributes(
                "world",
                TextAttribute {
                    bold: Some(true),
                    ..Default::default()
                },
            ),
            TextSpan::plain("!"),
        ];
        assert_eq!(plain_text_from_spans(&spans), "Hello world!");
    }

    #[test]
    fn empty_spans() {
        let spans: Vec<TextSpan> = vec![];
        assert_eq!(plain_text_from_spans(&spans), "");
    }

    #[test]
    fn serde_round_trip() {
        let spans = vec![
            TextSpan::plain("normal "),
            TextSpan::with_attributes(
                "bold",
                TextAttribute {
                    bold: Some(true),
                    ..Default::default()
                },
            ),
            TextSpan::with_attributes(
                " link",
                TextAttribute {
                    link: Some("https://example.com".into()),
                    ..Default::default()
                },
            ),
        ];
        let json = serde_json::to_string(&spans).unwrap();
        let rt: Vec<TextSpan> = serde_json::from_str(&json).unwrap();
        assert_eq!(rt, spans);
    }

    #[test]
    fn value_round_trip() {
        let spans = vec![
            TextSpan::plain("one "),
            TextSpan::with_attributes(
                "two",
                TextAttribute {
                    italic: Some(true),
                    font_size: Some(14),
                    ..Default::default()
                },
            ),
        ];
        let value = spans_to_value(&spans);
        let recovered = spans_from_value(&value).unwrap();
        assert_eq!(recovered, spans);
    }

    #[test]
    fn null_value_returns_none() {
        assert!(spans_from_value(&Value::Null).is_none());
    }

    #[test]
    fn invalid_json_returns_none() {
        assert!(spans_from_value(&Value::String("not json".into())).is_none());
    }

    #[test]
    fn extra_attributes_preserved() {
        let json = r#"[{"text":"hi","attributes":{"bold":true,"custom_attr":"special"}}]"#;
        let spans: Vec<TextSpan> = serde_json::from_str(json).unwrap();
        assert_eq!(spans[0].attributes.bold, Some(true));
        assert_eq!(
            spans[0].attributes.extra.get("custom_attr"),
            Some(&serde_json::Value::String("special".into()))
        );
        // Round-trip preserves extra
        let out = serde_json::to_string(&spans).unwrap();
        let rt: Vec<TextSpan> = serde_json::from_str(&out).unwrap();
        assert_eq!(rt[0].attributes.extra.get("custom_attr"), spans[0].attributes.extra.get("custom_attr"));
    }

    #[test]
    fn minimal_json_deserializes() {
        // Old data without spans should not break
        let json = r#"[{"text":"hello"}]"#;
        let spans: Vec<TextSpan> = serde_json::from_str(json).unwrap();
        assert_eq!(spans[0].text, "hello");
        assert!(spans[0].attributes.is_plain());
    }

    #[test]
    fn all_attributes() {
        let attr = TextAttribute {
            bold: Some(true),
            italic: Some(true),
            underline: Some(true),
            strikethrough: Some(true),
            code: Some(true),
            link: Some("net://doc.idea".into()),
            color: Some("#333".into()),
            font: Some("monospace".into()),
            font_size: Some(16),
            font_weight: Some("semibold".into()),
            language: Some("en-US".into()),
            extra: HashMap::new(),
        };
        assert!(!attr.is_plain());
        let span = TextSpan::with_attributes("styled", attr.clone());
        let json = serde_json::to_string(&span).unwrap();
        let rt: TextSpan = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.attributes, attr);
    }
}
