//! Rich text block digit helpers — typed constructors and parsers for document content.
//!
//! Rich text metadata is stored in Digit properties as `Value` types.
//! Used by the Quill program in Throne for document editing.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use crate::helpers::{check_type, prop_int, prop_str, prop_str_opt};
use crate::schema::{DigitSchema, PropertyType};
use crate::textspan::{self, TextSpan};
use x::Value;

const DOMAIN: &str = "richtext";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Metadata for a heading digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadingMeta {
    /// Heading level (1-6).
    pub level: u8,
    pub text: String,
    /// Optional inline formatting spans. When present, `text` is the plain-text
    /// concatenation of all span texts (kept in sync for search/accessibility).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<TextSpan>>,
}

/// Metadata for a paragraph digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParagraphMeta {
    pub text: String,
    /// Optional inline formatting spans. When present, `text` is the plain-text
    /// concatenation of all span texts (kept in sync for search/accessibility).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<TextSpan>>,
}

/// List ordering style.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListStyle {
    /// Numbered list (1, 2, 3...).
    Ordered,
    /// Bulleted list.
    Unordered,
    /// Checkbox list with check/uncheck state.
    Checklist,
}

impl ListStyle {
    fn to_str(&self) -> &'static str {
        match self {
            ListStyle::Ordered => "ordered",
            ListStyle::Unordered => "unordered",
            ListStyle::Checklist => "checklist",
        }
    }

    fn from_str_value(s: &str) -> Result<Self, IdeasError> {
        match s {
            "ordered" => Ok(ListStyle::Ordered),
            "unordered" => Ok(ListStyle::Unordered),
            "checklist" => Ok(ListStyle::Checklist),
            other => Err(IdeasError::RichTextParsing(format!(
                "unknown list style: {other}"
            ))),
        }
    }
}

/// Metadata for a list digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListMeta {
    pub style: ListStyle,
    pub items: Vec<String>,
}

/// Metadata for a blockquote digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockquoteMeta {
    pub text: String,
    pub attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<TextSpan>>,
}

/// Metadata for a callout digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalloutMeta {
    pub text: String,
    /// Style hint: "info", "warning", "error", "success", etc.
    pub style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<TextSpan>>,
}

/// Metadata for a code block digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeBlockMeta {
    pub code: String,
    pub language: Option<String>,
}

/// Metadata for a footnote digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FootnoteMeta {
    pub marker: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<TextSpan>>,
}

/// Metadata for a citation digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CitationMeta {
    pub source: String,
    pub url: Option<String>,
    pub author: Option<String>,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Create a heading digit.
pub fn heading_digit(meta: &HeadingMeta, author: &str) -> Result<Digit, IdeasError> {
    if meta.level == 0 || meta.level > 6 {
        return Err(IdeasError::RichTextParsing(format!(
            "heading level must be 1-6, got {}",
            meta.level
        )));
    }
    let mut digit = Digit::new("text.heading".into(), Value::Null, author.into())?;
    digit = digit.with_property("level".into(), Value::Int(meta.level as i64), author);
    digit = digit.with_property("text".into(), Value::String(meta.text.clone()), author);
    if let Some(ref spans) = meta.spans {
        digit = digit.with_property("spans".into(), textspan::spans_to_value(spans), author);
    }
    Ok(digit)
}

/// Create a paragraph digit.
pub fn paragraph_digit(meta: &ParagraphMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.paragraph".into(), Value::Null, author.into())?;
    digit = digit.with_property("text".into(), Value::String(meta.text.clone()), author);
    if let Some(ref spans) = meta.spans {
        digit = digit.with_property("spans".into(), textspan::spans_to_value(spans), author);
    }
    Ok(digit)
}

/// Create a list digit.
pub fn list_digit(meta: &ListMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.list".into(), Value::Null, author.into())?;
    digit = digit.with_property(
        "style".into(),
        Value::String(meta.style.to_str().into()),
        author,
    );
    let items_value = Value::Array(
        meta.items
            .iter()
            .map(|i| Value::String(i.clone()))
            .collect(),
    );
    digit = digit.with_property("items".into(), items_value, author);
    Ok(digit)
}

/// Create a blockquote digit.
pub fn blockquote_digit(meta: &BlockquoteMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.blockquote".into(), Value::Null, author.into())?;
    digit = digit.with_property("text".into(), Value::String(meta.text.clone()), author);
    if let Some(ref attr) = meta.attribution {
        digit = digit.with_property("attribution".into(), Value::String(attr.clone()), author);
    }
    if let Some(ref spans) = meta.spans {
        digit = digit.with_property("spans".into(), textspan::spans_to_value(spans), author);
    }
    Ok(digit)
}

/// Create a callout digit.
pub fn callout_digit(meta: &CalloutMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.callout".into(), Value::Null, author.into())?;
    digit = digit.with_property("text".into(), Value::String(meta.text.clone()), author);
    digit = digit.with_property("style".into(), Value::String(meta.style.clone()), author);
    if let Some(ref spans) = meta.spans {
        digit = digit.with_property("spans".into(), textspan::spans_to_value(spans), author);
    }
    Ok(digit)
}

/// Create a code block digit.
pub fn code_block_digit(meta: &CodeBlockMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.code".into(), Value::Null, author.into())?;
    digit = digit.with_property("code".into(), Value::String(meta.code.clone()), author);
    if let Some(ref lang) = meta.language {
        digit = digit.with_property("language".into(), Value::String(lang.clone()), author);
    }
    Ok(digit)
}

/// Create a footnote digit.
pub fn footnote_digit(meta: &FootnoteMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.footnote".into(), Value::Null, author.into())?;
    digit = digit.with_property("marker".into(), Value::String(meta.marker.clone()), author);
    digit = digit.with_property("text".into(), Value::String(meta.text.clone()), author);
    if let Some(ref spans) = meta.spans {
        digit = digit.with_property("spans".into(), textspan::spans_to_value(spans), author);
    }
    Ok(digit)
}

/// Create a citation digit.
pub fn citation_digit(meta: &CitationMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("text.citation".into(), Value::Null, author.into())?;
    digit = digit.with_property("source".into(), Value::String(meta.source.clone()), author);
    if let Some(ref url) = meta.url {
        digit = digit.with_property("url".into(), Value::String(url.clone()), author);
    }
    if let Some(ref auth) = meta.author {
        digit = digit.with_property("author".into(), Value::String(auth.clone()), author);
    }
    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse heading metadata from a digit.
pub fn parse_heading_meta(digit: &Digit) -> Result<HeadingMeta, IdeasError> {
    check_type(digit, "text.heading", DOMAIN)?;
    let level = prop_int(digit, "level", DOMAIN)? as u8;
    if level == 0 || level > 6 {
        return Err(IdeasError::RichTextParsing(format!(
            "heading level must be 1-6, got {level}"
        )));
    }
    let spans = digit
        .properties
        .get("spans")
        .and_then(textspan::spans_from_value);
    Ok(HeadingMeta {
        level,
        text: prop_str(digit, "text", DOMAIN)?,
        spans,
    })
}

/// Parse paragraph metadata from a digit.
pub fn parse_paragraph_meta(digit: &Digit) -> Result<ParagraphMeta, IdeasError> {
    check_type(digit, "text.paragraph", DOMAIN)?;
    let spans = digit
        .properties
        .get("spans")
        .and_then(textspan::spans_from_value);
    Ok(ParagraphMeta {
        text: prop_str(digit, "text", DOMAIN)?,
        spans,
    })
}

/// Parse list metadata from a digit.
pub fn parse_list_meta(digit: &Digit) -> Result<ListMeta, IdeasError> {
    check_type(digit, "text.list", DOMAIN)?;
    let style_str = prop_str(digit, "style", DOMAIN)?;
    let style = ListStyle::from_str_value(&style_str)?;
    let items = digit
        .properties
        .get("items")
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
        .ok_or_else(|| IdeasError::RichTextParsing("missing property: items".into()))?;
    Ok(ListMeta { style, items })
}

/// Parse blockquote metadata from a digit.
pub fn parse_blockquote_meta(digit: &Digit) -> Result<BlockquoteMeta, IdeasError> {
    check_type(digit, "text.blockquote", DOMAIN)?;
    let spans = digit
        .properties
        .get("spans")
        .and_then(textspan::spans_from_value);
    Ok(BlockquoteMeta {
        text: prop_str(digit, "text", DOMAIN)?,
        attribution: prop_str_opt(digit, "attribution"),
        spans,
    })
}

/// Parse callout metadata from a digit.
pub fn parse_callout_meta(digit: &Digit) -> Result<CalloutMeta, IdeasError> {
    check_type(digit, "text.callout", DOMAIN)?;
    let spans = digit
        .properties
        .get("spans")
        .and_then(textspan::spans_from_value);
    Ok(CalloutMeta {
        text: prop_str(digit, "text", DOMAIN)?,
        style: prop_str(digit, "style", DOMAIN)?,
        spans,
    })
}

/// Parse code block metadata from a digit.
pub fn parse_code_block_meta(digit: &Digit) -> Result<CodeBlockMeta, IdeasError> {
    check_type(digit, "text.code", DOMAIN)?;
    Ok(CodeBlockMeta {
        code: prop_str(digit, "code", DOMAIN)?,
        language: prop_str_opt(digit, "language"),
    })
}

/// Parse footnote metadata from a digit.
pub fn parse_footnote_meta(digit: &Digit) -> Result<FootnoteMeta, IdeasError> {
    check_type(digit, "text.footnote", DOMAIN)?;
    let spans = digit
        .properties
        .get("spans")
        .and_then(textspan::spans_from_value);
    Ok(FootnoteMeta {
        marker: prop_str(digit, "marker", DOMAIN)?,
        text: prop_str(digit, "text", DOMAIN)?,
        spans,
    })
}

/// Parse citation metadata from a digit.
pub fn parse_citation_meta(digit: &Digit) -> Result<CitationMeta, IdeasError> {
    check_type(digit, "text.citation", DOMAIN)?;
    Ok(CitationMeta {
        source: prop_str(digit, "source", DOMAIN)?,
        url: prop_str_opt(digit, "url"),
        author: prop_str_opt(digit, "author"),
    })
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

/// Schema for `text.heading` digits.
pub fn heading_schema() -> DigitSchema {
    DigitSchema::new("text.heading".into())
        .with_required("level", PropertyType::Int)
        .with_required("text", PropertyType::String)
        .with_description("Rich text heading (level 1-6)")
}

/// Schema for `text.paragraph` digits.
pub fn paragraph_schema() -> DigitSchema {
    DigitSchema::new("text.paragraph".into())
        .with_required("text", PropertyType::String)
        .with_description("Rich text paragraph")
}

/// Schema for `text.list` digits.
pub fn list_schema() -> DigitSchema {
    DigitSchema::new("text.list".into())
        .with_required("style", PropertyType::String)
        .with_required("items", PropertyType::Array)
        .with_description("Rich text list (ordered, unordered, or checklist)")
}

/// Schema for `text.blockquote` digits.
pub fn blockquote_schema() -> DigitSchema {
    DigitSchema::new("text.blockquote".into())
        .with_required("text", PropertyType::String)
        .with_optional("attribution", PropertyType::String)
        .with_description("Rich text blockquote")
}

/// Schema for `text.callout` digits.
pub fn callout_schema() -> DigitSchema {
    DigitSchema::new("text.callout".into())
        .with_required("text", PropertyType::String)
        .with_required("style", PropertyType::String)
        .with_description("Rich text callout (info, warning, etc.)")
}

/// Schema for `text.code` digits.
pub fn code_block_schema() -> DigitSchema {
    DigitSchema::new("text.code".into())
        .with_required("code", PropertyType::String)
        .with_optional("language", PropertyType::String)
        .with_description("Code block with optional language hint")
}

/// Schema for `text.footnote` digits.
pub fn footnote_schema() -> DigitSchema {
    DigitSchema::new("text.footnote".into())
        .with_required("marker", PropertyType::String)
        .with_required("text", PropertyType::String)
        .with_description("Footnote reference and content")
}

/// Schema for `text.citation` digits.
pub fn citation_schema() -> DigitSchema {
    DigitSchema::new("text.citation".into())
        .with_required("source", PropertyType::String)
        .with_optional("url", PropertyType::String)
        .with_optional("author", PropertyType::String)
        .with_description("Citation with source, optional URL and author")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_round_trip() {
        let meta = HeadingMeta {
            level: 2,
            text: "Chapter One".into(),
            spans: None,
        };
        let digit = heading_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.heading");

        let parsed = parse_heading_meta(&digit).unwrap();
        assert_eq!(parsed.level, 2);
        assert_eq!(parsed.text, "Chapter One");
        assert!(parsed.spans.is_none());
    }

    #[test]
    fn heading_invalid_level_zero() {
        let meta = HeadingMeta {
            level: 0,
            text: "Bad".into(),
            spans: None,
        };
        assert!(heading_digit(&meta, "alice").is_err());
    }

    #[test]
    fn heading_invalid_level_seven() {
        let meta = HeadingMeta {
            level: 7,
            text: "Bad".into(),
            spans: None,
        };
        assert!(heading_digit(&meta, "alice").is_err());
    }

    #[test]
    fn heading_all_levels() {
        for level in 1..=6u8 {
            let meta = HeadingMeta {
                level,
                text: format!("H{level}"),
                spans: None,
            };
            let digit = heading_digit(&meta, "alice").unwrap();
            let parsed = parse_heading_meta(&digit).unwrap();
            assert_eq!(parsed.level, level);
        }
    }

    #[test]
    fn paragraph_round_trip() {
        let meta = ParagraphMeta {
            text: "Hello, world!".into(),
            spans: None,
        };
        let digit = paragraph_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.paragraph");

        let parsed = parse_paragraph_meta(&digit).unwrap();
        assert_eq!(parsed.text, "Hello, world!");
        assert!(parsed.spans.is_none());
    }

    #[test]
    fn list_round_trip() {
        let meta = ListMeta {
            style: ListStyle::Ordered,
            items: vec!["First".into(), "Second".into(), "Third".into()],
        };
        let digit = list_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.list");

        let parsed = parse_list_meta(&digit).unwrap();
        assert_eq!(parsed.style, ListStyle::Ordered);
        assert_eq!(parsed.items, vec!["First", "Second", "Third"]);
    }

    #[test]
    fn list_checklist() {
        let meta = ListMeta {
            style: ListStyle::Checklist,
            items: vec!["[ ] Todo".into(), "[x] Done".into()],
        };
        let digit = list_digit(&meta, "alice").unwrap();
        let parsed = parse_list_meta(&digit).unwrap();
        assert_eq!(parsed.style, ListStyle::Checklist);
    }

    #[test]
    fn blockquote_round_trip() {
        let meta = BlockquoteMeta {
            text: "To be or not to be".into(),
            attribution: Some("Shakespeare".into()),
            spans: None,
        };
        let digit = blockquote_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.blockquote");

        let parsed = parse_blockquote_meta(&digit).unwrap();
        assert_eq!(parsed.text, "To be or not to be");
        assert_eq!(parsed.attribution.as_deref(), Some("Shakespeare"));
    }

    #[test]
    fn blockquote_no_attribution() {
        let meta = BlockquoteMeta {
            text: "Anonymous wisdom".into(),
            attribution: None,
            spans: None,
        };
        let digit = blockquote_digit(&meta, "alice").unwrap();
        let parsed = parse_blockquote_meta(&digit).unwrap();
        assert!(parsed.attribution.is_none());
    }

    #[test]
    fn callout_round_trip() {
        let meta = CalloutMeta {
            text: "Be careful!".into(),
            style: "warning".into(),
            spans: None,
        };
        let digit = callout_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.callout");

        let parsed = parse_callout_meta(&digit).unwrap();
        assert_eq!(parsed.text, "Be careful!");
        assert_eq!(parsed.style, "warning");
    }

    #[test]
    fn code_block_round_trip() {
        let meta = CodeBlockMeta {
            code: "fn main() {}".into(),
            language: Some("rust".into()),
        };
        let digit = code_block_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.code");

        let parsed = parse_code_block_meta(&digit).unwrap();
        assert_eq!(parsed.code, "fn main() {}");
        assert_eq!(parsed.language.as_deref(), Some("rust"));
    }

    #[test]
    fn code_block_no_language() {
        let meta = CodeBlockMeta {
            code: "echo hello".into(),
            language: None,
        };
        let digit = code_block_digit(&meta, "alice").unwrap();
        let parsed = parse_code_block_meta(&digit).unwrap();
        assert!(parsed.language.is_none());
    }

    #[test]
    fn footnote_round_trip() {
        let meta = FootnoteMeta {
            marker: "1".into(),
            text: "See reference on page 42".into(),
            spans: None,
        };
        let digit = footnote_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.footnote");

        let parsed = parse_footnote_meta(&digit).unwrap();
        assert_eq!(parsed.marker, "1");
        assert_eq!(parsed.text, "See reference on page 42");
    }

    #[test]
    fn citation_round_trip() {
        let meta = CitationMeta {
            source: "The Sovereign Individual".into(),
            url: Some("https://example.com/book".into()),
            author: Some("Davidson & Rees-Mogg".into()),
        };
        let digit = citation_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "text.citation");

        let parsed = parse_citation_meta(&digit).unwrap();
        assert_eq!(parsed.source, "The Sovereign Individual");
        assert_eq!(parsed.url.as_deref(), Some("https://example.com/book"));
        assert_eq!(parsed.author.as_deref(), Some("Davidson & Rees-Mogg"));
    }

    #[test]
    fn citation_minimal() {
        let meta = CitationMeta {
            source: "RFC 7159".into(),
            url: None,
            author: None,
        };
        let digit = citation_digit(&meta, "alice").unwrap();
        let parsed = parse_citation_meta(&digit).unwrap();
        assert!(parsed.url.is_none());
        assert!(parsed.author.is_none());
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_heading_meta(&digit).is_err());
        assert!(parse_paragraph_meta(&digit).is_err());
        assert!(parse_list_meta(&digit).is_err());
        assert!(parse_blockquote_meta(&digit).is_err());
        assert!(parse_callout_meta(&digit).is_err());
        assert!(parse_code_block_meta(&digit).is_err());
        assert!(parse_footnote_meta(&digit).is_err());
        assert!(parse_citation_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit = Digit::new("text.heading".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_heading_meta(&digit).is_err());

        let digit = Digit::new("text.paragraph".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_paragraph_meta(&digit).is_err());

        let digit = Digit::new("text.list".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_list_meta(&digit).is_err());
    }

    #[test]
    fn all_list_styles() {
        for (style, name) in [
            (ListStyle::Ordered, "ordered"),
            (ListStyle::Unordered, "unordered"),
            (ListStyle::Checklist, "checklist"),
        ] {
            assert_eq!(style.to_str(), name);
            assert_eq!(ListStyle::from_str_value(name).unwrap(), style);
        }
    }

    #[test]
    fn invalid_list_style() {
        assert!(ListStyle::from_str_value("unknown").is_err());
    }

    #[test]
    fn paragraph_with_spans_round_trip() {
        use crate::textspan::{TextAttribute, TextSpan};
        let spans = vec![
            TextSpan::plain("This is "),
            TextSpan::with_attributes(
                "bold",
                TextAttribute {
                    bold: Some(true),
                    ..Default::default()
                },
            ),
            TextSpan::plain(" and "),
            TextSpan::with_attributes(
                "italic",
                TextAttribute {
                    italic: Some(true),
                    ..Default::default()
                },
            ),
            TextSpan::plain(" text."),
        ];
        let meta = ParagraphMeta {
            text: "This is bold and italic text.".into(),
            spans: Some(spans.clone()),
        };
        let digit = paragraph_digit(&meta, "alice").unwrap();
        let parsed = parse_paragraph_meta(&digit).unwrap();
        assert_eq!(parsed.text, "This is bold and italic text.");
        let parsed_spans = parsed.spans.unwrap();
        assert_eq!(parsed_spans.len(), 5);
        assert_eq!(parsed_spans[1].attributes.bold, Some(true));
        assert_eq!(parsed_spans[3].attributes.italic, Some(true));
    }

    #[test]
    fn heading_with_spans_round_trip() {
        use crate::textspan::{TextAttribute, TextSpan};
        let spans = vec![
            TextSpan::plain("Chapter "),
            TextSpan::with_attributes(
                "One",
                TextAttribute {
                    color: Some("#c00".into()),
                    ..Default::default()
                },
            ),
        ];
        let meta = HeadingMeta {
            level: 1,
            text: "Chapter One".into(),
            spans: Some(spans),
        };
        let digit = heading_digit(&meta, "alice").unwrap();
        let parsed = parse_heading_meta(&digit).unwrap();
        assert!(parsed.spans.is_some());
        assert_eq!(parsed.spans.unwrap().len(), 2);
    }

    #[test]
    fn old_digit_without_spans_parses() {
        // Simulate an old digit that was created before spans existed
        let digit = Digit::new("text.paragraph".into(), Value::Null, "alice".into()).unwrap();
        let digit = digit.with_property("text".into(), Value::String("old content".into()), "alice");
        // No "spans" property set
        let parsed = parse_paragraph_meta(&digit).unwrap();
        assert_eq!(parsed.text, "old content");
        assert!(parsed.spans.is_none());
    }

    #[test]
    fn schema_validates_heading() {
        let schema = heading_schema();
        let meta = HeadingMeta {
            level: 1,
            text: "Title".into(),
            spans: None,
        };
        let digit = heading_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn schema_validates_paragraph() {
        let schema = paragraph_schema();
        let meta = ParagraphMeta {
            text: "Hello".into(),
            spans: None,
        };
        let digit = paragraph_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn serde_round_trip() {
        let meta = HeadingMeta {
            level: 3,
            text: "Section".into(),
            spans: None,
        };
        let digit = heading_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_heading_meta(&rt).unwrap();
        assert_eq!(parsed.level, 3);
    }
}
