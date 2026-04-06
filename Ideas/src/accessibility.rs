//! Accessibility metadata for Digits.
//!
//! Stores accessibility information in Digit properties with the `a11y_` prefix.
//! This is a cross-cutting concern: any Digit can carry accessibility metadata
//! regardless of its type.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::helpers::{prop_int_opt, prop_str_opt};
use x::Value;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The semantic role of a UI element for assistive technology.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccessibilityRole {
    /// A clickable button.
    Button,
    /// A navigational link.
    Link,
    /// A visual image.
    Image,
    /// A section heading (pair with `heading_level`).
    Heading,
    /// An ordered or unordered list container.
    List,
    /// An item within a list.
    ListItem,
    /// A text input field.
    TextField,
    /// A checkbox toggle.
    Checkbox,
    /// A value slider.
    Slider,
    /// A tab within a tab group.
    Tab,
    /// A data table.
    Table,
    /// A cell within a table.
    Cell,
    /// A form container.
    Form,
    /// A navigation landmark.
    Navigation,
    /// An alert or notification.
    Alert,
    /// A modal dialog.
    Dialog,
    /// A custom role identified by name.
    Custom(String),
}

impl AccessibilityRole {
    fn to_property_value(&self) -> String {
        match self {
            AccessibilityRole::Custom(s) => format!("custom:{s}"),
            other => serde_json::to_string(other)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        }
    }

    fn from_property_value(s: &str) -> Self {
        if let Some(custom) = s.strip_prefix("custom:") {
            return AccessibilityRole::Custom(custom.to_string());
        }
        serde_json::from_str(&format!("\"{s}\"")).unwrap_or(AccessibilityRole::Custom(s.to_string()))
    }
}

/// How a live region announces changes to assistive technology.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LiveRegion {
    /// Announce changes when convenient (does not interrupt current speech).
    Polite,
    /// Announce changes immediately (interrupts current speech).
    Assertive,
    /// Do not announce changes.
    Off,
}

/// Accessibility metadata attached to a Digit via properties.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibilityMetadata {
    /// The semantic role of this element.
    pub role: AccessibilityRole,
    /// Human-readable label for assistive technology.
    pub label: String,
    /// Current value (e.g., slider position, text field content).
    pub value: Option<String>,
    /// Usage hint for assistive technology.
    pub hint: Option<String>,
    /// Heading level (1-6) if role is Heading.
    pub heading_level: Option<u8>,
    /// Language code (BCP 47) for this element.
    pub language: Option<String>,
    /// Tab/focus order index.
    pub focus_order: Option<i32>,
    /// Live region announcement behavior.
    pub live_region: Option<LiveRegion>,
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

/// Attach accessibility metadata to a digit, returning a new digit with
/// `a11y_`-prefixed properties set.
pub fn with_accessibility(digit: Digit, meta: &AccessibilityMetadata, author: &str) -> Digit {
    let mut d = digit;
    d = d.with_property(
        "a11y_role".into(),
        Value::String(meta.role.to_property_value()),
        author,
    );
    d = d.with_property(
        "a11y_label".into(),
        Value::String(meta.label.clone()),
        author,
    );
    if let Some(ref v) = meta.value {
        d = d.with_property("a11y_value".into(), Value::String(v.clone()), author);
    }
    if let Some(ref h) = meta.hint {
        d = d.with_property("a11y_hint".into(), Value::String(h.clone()), author);
    }
    if let Some(level) = meta.heading_level {
        d = d.with_property("a11y_heading_level".into(), Value::Int(level as i64), author);
    }
    if let Some(ref lang) = meta.language {
        d = d.with_property("a11y_language".into(), Value::String(lang.clone()), author);
    }
    if let Some(order) = meta.focus_order {
        d = d.with_property("a11y_focus_order".into(), Value::Int(order as i64), author);
    }
    if let Some(ref lr) = meta.live_region {
        let lr_str = serde_json::to_string(lr)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        d = d.with_property("a11y_live_region".into(), Value::String(lr_str), author);
    }
    d
}

/// Extract accessibility metadata from a digit's properties.
/// Returns `None` if no `a11y_role` property is present.
pub fn accessibility_metadata(digit: &Digit) -> Option<AccessibilityMetadata> {
    let role_str = prop_str_opt(digit, "a11y_role")?;
    let label = prop_str_opt(digit, "a11y_label")?;

    let role = AccessibilityRole::from_property_value(&role_str);
    let value = prop_str_opt(digit, "a11y_value");
    let hint = prop_str_opt(digit, "a11y_hint");
    let heading_level = prop_int_opt(digit, "a11y_heading_level").map(|v| v as u8);
    let language = prop_str_opt(digit, "a11y_language");
    let focus_order = prop_int_opt(digit, "a11y_focus_order").map(|v| v as i32);
    let live_region = prop_str_opt(digit, "a11y_live_region")
        .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok());

    Some(AccessibilityMetadata {
        role,
        label,
        value,
        hint,
        heading_level,
        language,
        focus_order,
        live_region,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta() -> AccessibilityMetadata {
        AccessibilityMetadata {
            role: AccessibilityRole::Button,
            label: "Submit form".into(),
            value: Some("enabled".into()),
            hint: Some("Double-tap to submit".into()),
            heading_level: None,
            language: Some("en-US".into()),
            focus_order: Some(3),
            live_region: Some(LiveRegion::Polite),
        }
    }

    #[test]
    fn round_trip() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_accessibility(digit, &test_meta(), "alice");
        let parsed = accessibility_metadata(&digit).unwrap();

        assert_eq!(parsed.role, AccessibilityRole::Button);
        assert_eq!(parsed.label, "Submit form");
        assert_eq!(parsed.value.as_deref(), Some("enabled"));
        assert_eq!(parsed.hint.as_deref(), Some("Double-tap to submit"));
        assert!(parsed.heading_level.is_none());
        assert_eq!(parsed.language.as_deref(), Some("en-US"));
        assert_eq!(parsed.focus_order, Some(3));
        assert_eq!(parsed.live_region, Some(LiveRegion::Polite));
    }

    #[test]
    fn heading_with_level() {
        let meta = AccessibilityMetadata {
            role: AccessibilityRole::Heading,
            label: "Chapter 1".into(),
            value: None,
            hint: None,
            heading_level: Some(2),
            language: None,
            focus_order: None,
            live_region: None,
        };
        let digit = Digit::new("text.heading".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_accessibility(digit, &meta, "alice");
        let parsed = accessibility_metadata(&digit).unwrap();

        assert_eq!(parsed.role, AccessibilityRole::Heading);
        assert_eq!(parsed.heading_level, Some(2));
    }

    #[test]
    fn custom_role() {
        let meta = AccessibilityMetadata {
            role: AccessibilityRole::Custom("carousel".into()),
            label: "Image carousel".into(),
            value: None,
            hint: None,
            heading_level: None,
            language: None,
            focus_order: None,
            live_region: None,
        };
        let digit = Digit::new("container".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_accessibility(digit, &meta, "alice");
        let parsed = accessibility_metadata(&digit).unwrap();

        assert_eq!(parsed.role, AccessibilityRole::Custom("carousel".into()));
    }

    #[test]
    fn minimal_metadata() {
        let meta = AccessibilityMetadata {
            role: AccessibilityRole::Image,
            label: "Sunset photo".into(),
            value: None,
            hint: None,
            heading_level: None,
            language: None,
            focus_order: None,
            live_region: None,
        };
        let digit = Digit::new("media.image".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_accessibility(digit, &meta, "alice");
        let parsed = accessibility_metadata(&digit).unwrap();

        assert_eq!(parsed.role, AccessibilityRole::Image);
        assert_eq!(parsed.label, "Sunset photo");
        assert!(parsed.value.is_none());
        assert!(parsed.hint.is_none());
    }

    #[test]
    fn no_a11y_returns_none() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(accessibility_metadata(&digit).is_none());
    }

    #[test]
    fn live_region_assertive() {
        let meta = AccessibilityMetadata {
            role: AccessibilityRole::Alert,
            label: "Error message".into(),
            value: None,
            hint: None,
            heading_level: None,
            language: None,
            focus_order: None,
            live_region: Some(LiveRegion::Assertive),
        };
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        let digit = with_accessibility(digit, &meta, "alice");
        let parsed = accessibility_metadata(&digit).unwrap();
        assert_eq!(parsed.live_region, Some(LiveRegion::Assertive));
    }

    #[test]
    fn all_roles_roundtrip() {
        let roles = vec![
            AccessibilityRole::Button,
            AccessibilityRole::Link,
            AccessibilityRole::Image,
            AccessibilityRole::Heading,
            AccessibilityRole::List,
            AccessibilityRole::ListItem,
            AccessibilityRole::TextField,
            AccessibilityRole::Checkbox,
            AccessibilityRole::Slider,
            AccessibilityRole::Tab,
            AccessibilityRole::Table,
            AccessibilityRole::Cell,
            AccessibilityRole::Form,
            AccessibilityRole::Navigation,
            AccessibilityRole::Alert,
            AccessibilityRole::Dialog,
            AccessibilityRole::Custom("widget".into()),
        ];
        for role in roles {
            let meta = AccessibilityMetadata {
                role: role.clone(),
                label: "test".into(),
                value: None,
                hint: None,
                heading_level: None,
                language: None,
                focus_order: None,
                live_region: None,
            };
            let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
            let digit = with_accessibility(digit, &meta, "alice");
            let parsed = accessibility_metadata(&digit).unwrap();
            assert_eq!(parsed.role, role);
        }
    }

    #[test]
    fn serde_round_trip() {
        let meta = test_meta();
        let json = serde_json::to_string(&meta).unwrap();
        let rt: AccessibilityMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.role, AccessibilityRole::Button);
        assert_eq!(rt.label, "Submit form");
    }
}
