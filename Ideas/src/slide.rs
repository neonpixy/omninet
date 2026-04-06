//! Slide digit helpers — typed constructors and parsers for presentation content.
//!
//! Slide metadata is stored in Digit properties as `Value` types.
//! Used by the Podium program in Throne for presentations.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use crate::helpers::{check_type, prop_int, prop_str, prop_str_opt};
use crate::schema::{DigitSchema, PropertyType};
use x::Value;

const DOMAIN: &str = "slide";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Transition effect between slides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionType {
    /// Cross-fade between slides.
    Fade,
    /// The new slide slides in from the side.
    Slide,
    /// The new slide pushes the old one off.
    Push,
    /// A dissolve/blend transition.
    Dissolve,
    /// A custom transition identified by name.
    Custom(String),
}

impl TransitionType {
    fn to_property_value(&self) -> String {
        match self {
            TransitionType::Custom(s) => format!("custom:{s}"),
            other => serde_json::to_string(other)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        }
    }

    fn from_property_value(s: &str) -> Result<Self, IdeasError> {
        if let Some(custom) = s.strip_prefix("custom:") {
            return Ok(TransitionType::Custom(custom.to_string()));
        }
        serde_json::from_str(&format!("\"{s}\""))
            .map_err(|e| IdeasError::SlideParsing(format!("invalid transition type: {e}")))
    }
}

/// Slide layout preset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlideLayout {
    /// Title slide (centered, large text).
    Title,
    /// Standard content layout.
    Content,
    /// Side-by-side two-column layout.
    TwoColumn,
    /// Empty canvas.
    Blank,
    /// A custom layout identified by name.
    Custom(String),
}

impl SlideLayout {
    fn to_property_value(&self) -> String {
        match self {
            SlideLayout::Custom(s) => format!("custom:{s}"),
            SlideLayout::TwoColumn => "twocolumn".to_string(),
            other => serde_json::to_string(other)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        }
    }

    fn from_property_value(s: &str) -> Result<Self, IdeasError> {
        if let Some(custom) = s.strip_prefix("custom:") {
            return Ok(SlideLayout::Custom(custom.to_string()));
        }
        match s {
            "twocolumn" => Ok(SlideLayout::TwoColumn),
            _ => serde_json::from_str(&format!("\"{s}\""))
                .map_err(|e| IdeasError::SlideParsing(format!("invalid layout: {e}"))),
        }
    }
}

/// Metadata for a slide digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlideMeta {
    /// Optional slide title.
    pub title: Option<String>,
    /// Speaker notes for this slide.
    pub speaker_notes: Option<String>,
    /// Transition effect to this slide.
    pub transition: Option<TransitionType>,
    /// Layout preset.
    pub layout: SlideLayout,
    /// Order index in the presentation.
    pub order: u32,
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

/// Create a slide digit from metadata.
pub fn slide_digit(meta: &SlideMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("presentation.slide".into(), Value::Null, author.into())?;

    if let Some(ref title) = meta.title {
        digit = digit.with_property("title".into(), Value::String(title.clone()), author);
    }
    if let Some(ref notes) = meta.speaker_notes {
        digit = digit.with_property("speaker_notes".into(), Value::String(notes.clone()), author);
    }
    if let Some(ref transition) = meta.transition {
        digit = digit.with_property(
            "transition".into(),
            Value::String(transition.to_property_value()),
            author,
        );
    }
    digit = digit.with_property(
        "layout".into(),
        Value::String(meta.layout.to_property_value()),
        author,
    );
    digit = digit.with_property("order".into(), Value::Int(meta.order as i64), author);

    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse slide metadata from a digit.
pub fn parse_slide_meta(digit: &Digit) -> Result<SlideMeta, IdeasError> {
    check_type(digit, "presentation.slide", DOMAIN)?;

    let title = prop_str_opt(digit, "title");
    let speaker_notes = prop_str_opt(digit, "speaker_notes");
    let transition = prop_str_opt(digit, "transition")
        .map(|s| TransitionType::from_property_value(&s))
        .transpose()?;
    let layout_str = prop_str(digit, "layout", DOMAIN)?;
    let layout = SlideLayout::from_property_value(&layout_str)?;
    let order = prop_int(digit, "order", DOMAIN)? as u32;

    Ok(SlideMeta {
        title,
        speaker_notes,
        transition,
        layout,
        order,
    })
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// Schema for `presentation.slide` digits.
pub fn slide_schema() -> DigitSchema {
    DigitSchema::new("presentation.slide".into())
        .with_required("layout", PropertyType::String)
        .with_required("order", PropertyType::Int)
        .with_optional("title", PropertyType::String)
        .with_optional("speaker_notes", PropertyType::String)
        .with_optional("transition", PropertyType::String)
        .with_description("Presentation slide metadata")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_slide_meta() -> SlideMeta {
        SlideMeta {
            title: Some("Introduction".into()),
            speaker_notes: Some("Welcome everyone to this talk".into()),
            transition: Some(TransitionType::Fade),
            layout: SlideLayout::Title,
            order: 0,
        }
    }

    #[test]
    fn round_trip() {
        let meta = test_slide_meta();
        let digit = slide_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "presentation.slide");

        let parsed = parse_slide_meta(&digit).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Introduction"));
        assert_eq!(
            parsed.speaker_notes.as_deref(),
            Some("Welcome everyone to this talk")
        );
        assert_eq!(parsed.transition, Some(TransitionType::Fade));
        assert_eq!(parsed.layout, SlideLayout::Title);
        assert_eq!(parsed.order, 0);
    }

    #[test]
    fn minimal_slide() {
        let meta = SlideMeta {
            title: None,
            speaker_notes: None,
            transition: None,
            layout: SlideLayout::Blank,
            order: 5,
        };
        let digit = slide_digit(&meta, "alice").unwrap();
        let parsed = parse_slide_meta(&digit).unwrap();
        assert!(parsed.title.is_none());
        assert!(parsed.speaker_notes.is_none());
        assert!(parsed.transition.is_none());
        assert_eq!(parsed.layout, SlideLayout::Blank);
        assert_eq!(parsed.order, 5);
    }

    #[test]
    fn custom_transition() {
        let meta = SlideMeta {
            title: None,
            speaker_notes: None,
            transition: Some(TransitionType::Custom("wipe-left".into())),
            layout: SlideLayout::Content,
            order: 2,
        };
        let digit = slide_digit(&meta, "alice").unwrap();
        let parsed = parse_slide_meta(&digit).unwrap();
        assert_eq!(
            parsed.transition,
            Some(TransitionType::Custom("wipe-left".into()))
        );
    }

    #[test]
    fn custom_layout() {
        let meta = SlideMeta {
            title: None,
            speaker_notes: None,
            transition: None,
            layout: SlideLayout::Custom("hero-image".into()),
            order: 1,
        };
        let digit = slide_digit(&meta, "alice").unwrap();
        let parsed = parse_slide_meta(&digit).unwrap();
        assert_eq!(
            parsed.layout,
            SlideLayout::Custom("hero-image".into())
        );
    }

    #[test]
    fn two_column_layout() {
        let meta = SlideMeta {
            title: Some("Comparison".into()),
            speaker_notes: None,
            transition: Some(TransitionType::Push),
            layout: SlideLayout::TwoColumn,
            order: 3,
        };
        let digit = slide_digit(&meta, "alice").unwrap();
        let parsed = parse_slide_meta(&digit).unwrap();
        assert_eq!(parsed.layout, SlideLayout::TwoColumn);
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_slide_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit = Digit::new("presentation.slide".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_slide_meta(&digit).is_err());
    }

    #[test]
    fn all_transitions() {
        for tt in [
            TransitionType::Fade,
            TransitionType::Slide,
            TransitionType::Push,
            TransitionType::Dissolve,
        ] {
            let s = tt.to_property_value();
            let rt = TransitionType::from_property_value(&s).unwrap();
            assert_eq!(rt, tt);
        }
    }

    #[test]
    fn all_layouts() {
        for layout in [
            SlideLayout::Title,
            SlideLayout::Content,
            SlideLayout::TwoColumn,
            SlideLayout::Blank,
        ] {
            let s = layout.to_property_value();
            let rt = SlideLayout::from_property_value(&s).unwrap();
            assert_eq!(rt, layout);
        }
    }

    #[test]
    fn serde_round_trip() {
        let meta = test_slide_meta();
        let digit = slide_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_slide_meta(&rt).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Introduction"));
    }

    #[test]
    fn schema_validates() {
        let schema = slide_schema();
        let meta = test_slide_meta();
        let digit = slide_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }
}
