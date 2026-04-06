//! Interactive element digit helpers — buttons, navigation, accordions, tabs.
//!
//! Interactive metadata is stored in Digit properties as `Value` types.
//! These elements are used across programs for user interaction.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use crate::helpers::{check_type, prop_bool, prop_int, prop_str, prop_str_opt};
use crate::schema::{DigitSchema, PropertyType};
use x::Value;

const DOMAIN: &str = "interactive";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Visual style of a button.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonStyle {
    /// Main call-to-action style.
    Primary,
    /// Supporting action style.
    Secondary,
    /// Subtle/low-emphasis style.
    Tertiary,
    /// Destructive or warning action style.
    Danger,
    /// A custom style identified by name.
    Custom(String),
}

impl ButtonStyle {
    fn to_property_value(&self) -> String {
        match self {
            ButtonStyle::Custom(s) => format!("custom:{s}"),
            other => serde_json::to_string(other)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        }
    }

    fn from_property_value(s: &str) -> Self {
        if let Some(custom) = s.strip_prefix("custom:") {
            return ButtonStyle::Custom(custom.to_string());
        }
        serde_json::from_str(&format!("\"{s}\"")).unwrap_or(ButtonStyle::Custom(s.to_string()))
    }
}

/// Metadata for a button digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ButtonMeta {
    pub label: String,
    pub action_ref: Option<String>,
    pub style: ButtonStyle,
}

/// Metadata for a navigation link digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavLinkMeta {
    pub label: String,
    /// .idea bond reference to the target.
    pub target_ref: String,
}

/// Metadata for an accordion digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccordionMeta {
    pub title: String,
    pub expanded: bool,
}

/// Metadata for a tab group digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabGroupMeta {
    pub tabs: Vec<String>,
    pub active_index: u32,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Create a button digit.
pub fn button_digit(meta: &ButtonMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("interactive.button".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    if let Some(ref action) = meta.action_ref {
        digit = digit.with_property("action_ref".into(), Value::String(action.clone()), author);
    }
    digit = digit.with_property(
        "style".into(),
        Value::String(meta.style.to_property_value()),
        author,
    );
    Ok(digit)
}

/// Create a navigation link digit.
pub fn nav_link_digit(meta: &NavLinkMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("interactive.nav-link".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    digit = digit.with_property(
        "target_ref".into(),
        Value::String(meta.target_ref.clone()),
        author,
    );
    Ok(digit)
}

/// Create an accordion digit.
pub fn accordion_digit(meta: &AccordionMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("interactive.accordion".into(), Value::Null, author.into())?;
    digit = digit.with_property("title".into(), Value::String(meta.title.clone()), author);
    digit = digit.with_property("expanded".into(), Value::Bool(meta.expanded), author);
    Ok(digit)
}

/// Create a tab group digit.
pub fn tab_group_digit(meta: &TabGroupMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("interactive.tab-group".into(), Value::Null, author.into())?;
    let tabs_value = Value::Array(
        meta.tabs
            .iter()
            .map(|t| Value::String(t.clone()))
            .collect(),
    );
    digit = digit.with_property("tabs".into(), tabs_value, author);
    digit = digit.with_property(
        "active_index".into(),
        Value::Int(meta.active_index as i64),
        author,
    );
    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse button metadata from a digit.
pub fn parse_button_meta(digit: &Digit) -> Result<ButtonMeta, IdeasError> {
    check_type(digit, "interactive.button", DOMAIN)?;
    let style_str = prop_str(digit, "style", DOMAIN)?;
    Ok(ButtonMeta {
        label: prop_str(digit, "label", DOMAIN)?,
        action_ref: prop_str_opt(digit, "action_ref"),
        style: ButtonStyle::from_property_value(&style_str),
    })
}

/// Parse navigation link metadata from a digit.
pub fn parse_nav_link_meta(digit: &Digit) -> Result<NavLinkMeta, IdeasError> {
    check_type(digit, "interactive.nav-link", DOMAIN)?;
    Ok(NavLinkMeta {
        label: prop_str(digit, "label", DOMAIN)?,
        target_ref: prop_str(digit, "target_ref", DOMAIN)?,
    })
}

/// Parse accordion metadata from a digit.
pub fn parse_accordion_meta(digit: &Digit) -> Result<AccordionMeta, IdeasError> {
    check_type(digit, "interactive.accordion", DOMAIN)?;
    Ok(AccordionMeta {
        title: prop_str(digit, "title", DOMAIN)?,
        expanded: prop_bool(digit, "expanded", DOMAIN)?,
    })
}

/// Parse tab group metadata from a digit.
pub fn parse_tab_group_meta(digit: &Digit) -> Result<TabGroupMeta, IdeasError> {
    check_type(digit, "interactive.tab-group", DOMAIN)?;
    let tabs = digit
        .properties
        .get("tabs")
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
        .ok_or_else(|| IdeasError::InteractiveParsing("missing property: tabs".into()))?;
    let active_index = prop_int(digit, "active_index", DOMAIN)? as u32;

    Ok(TabGroupMeta {
        tabs,
        active_index,
    })
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

/// Schema for `interactive.button` digits.
pub fn button_schema() -> DigitSchema {
    DigitSchema::new("interactive.button".into())
        .with_required("label", PropertyType::String)
        .with_required("style", PropertyType::String)
        .with_optional("action_ref", PropertyType::String)
        .with_description("Interactive button element")
}

/// Schema for `interactive.nav-link` digits.
pub fn nav_link_schema() -> DigitSchema {
    DigitSchema::new("interactive.nav-link".into())
        .with_required("label", PropertyType::String)
        .with_required("target_ref", PropertyType::String)
        .with_description("Navigation link to another .idea")
}

/// Schema for `interactive.accordion` digits.
pub fn accordion_schema() -> DigitSchema {
    DigitSchema::new("interactive.accordion".into())
        .with_required("title", PropertyType::String)
        .with_required("expanded", PropertyType::Bool)
        .with_description("Collapsible accordion section")
}

/// Schema for `interactive.tab-group` digits.
pub fn tab_group_schema() -> DigitSchema {
    DigitSchema::new("interactive.tab-group".into())
        .with_required("tabs", PropertyType::Array)
        .with_required("active_index", PropertyType::Int)
        .with_description("Tab group with multiple tabs")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_round_trip() {
        let meta = ButtonMeta {
            label: "Save".into(),
            action_ref: Some("save-handler".into()),
            style: ButtonStyle::Primary,
        };
        let digit = button_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "interactive.button");

        let parsed = parse_button_meta(&digit).unwrap();
        assert_eq!(parsed.label, "Save");
        assert_eq!(parsed.action_ref.as_deref(), Some("save-handler"));
        assert_eq!(parsed.style, ButtonStyle::Primary);
    }

    #[test]
    fn button_no_action() {
        let meta = ButtonMeta {
            label: "Cancel".into(),
            action_ref: None,
            style: ButtonStyle::Secondary,
        };
        let digit = button_digit(&meta, "alice").unwrap();
        let parsed = parse_button_meta(&digit).unwrap();
        assert!(parsed.action_ref.is_none());
        assert_eq!(parsed.style, ButtonStyle::Secondary);
    }

    #[test]
    fn button_danger_style() {
        let meta = ButtonMeta {
            label: "Delete".into(),
            action_ref: None,
            style: ButtonStyle::Danger,
        };
        let digit = button_digit(&meta, "alice").unwrap();
        let parsed = parse_button_meta(&digit).unwrap();
        assert_eq!(parsed.style, ButtonStyle::Danger);
    }

    #[test]
    fn button_custom_style() {
        let meta = ButtonMeta {
            label: "Special".into(),
            action_ref: None,
            style: ButtonStyle::Custom("glow-green".into()),
        };
        let digit = button_digit(&meta, "alice").unwrap();
        let parsed = parse_button_meta(&digit).unwrap();
        assert_eq!(parsed.style, ButtonStyle::Custom("glow-green".into()));
    }

    #[test]
    fn nav_link_round_trip() {
        let meta = NavLinkMeta {
            label: "Go to homepage".into(),
            target_ref: "/home.idea".into(),
        };
        let digit = nav_link_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "interactive.nav-link");

        let parsed = parse_nav_link_meta(&digit).unwrap();
        assert_eq!(parsed.label, "Go to homepage");
        assert_eq!(parsed.target_ref, "/home.idea");
    }

    #[test]
    fn accordion_round_trip() {
        let meta = AccordionMeta {
            title: "FAQ Section".into(),
            expanded: false,
        };
        let digit = accordion_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "interactive.accordion");

        let parsed = parse_accordion_meta(&digit).unwrap();
        assert_eq!(parsed.title, "FAQ Section");
        assert!(!parsed.expanded);
    }

    #[test]
    fn accordion_expanded() {
        let meta = AccordionMeta {
            title: "Details".into(),
            expanded: true,
        };
        let digit = accordion_digit(&meta, "alice").unwrap();
        let parsed = parse_accordion_meta(&digit).unwrap();
        assert!(parsed.expanded);
    }

    #[test]
    fn tab_group_round_trip() {
        let meta = TabGroupMeta {
            tabs: vec!["Overview".into(), "Details".into(), "Reviews".into()],
            active_index: 1,
        };
        let digit = tab_group_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "interactive.tab-group");

        let parsed = parse_tab_group_meta(&digit).unwrap();
        assert_eq!(parsed.tabs, vec!["Overview", "Details", "Reviews"]);
        assert_eq!(parsed.active_index, 1);
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_button_meta(&digit).is_err());
        assert!(parse_nav_link_meta(&digit).is_err());
        assert!(parse_accordion_meta(&digit).is_err());
        assert!(parse_tab_group_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit =
            Digit::new("interactive.button".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_button_meta(&digit).is_err());

        let digit =
            Digit::new("interactive.nav-link".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_nav_link_meta(&digit).is_err());

        let digit =
            Digit::new("interactive.accordion".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_accordion_meta(&digit).is_err());

        let digit =
            Digit::new("interactive.tab-group".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_tab_group_meta(&digit).is_err());
    }

    #[test]
    fn all_button_styles() {
        for style in [
            ButtonStyle::Primary,
            ButtonStyle::Secondary,
            ButtonStyle::Tertiary,
            ButtonStyle::Danger,
        ] {
            let s = style.to_property_value();
            let rt = ButtonStyle::from_property_value(&s);
            assert_eq!(rt, style);
        }
    }

    #[test]
    fn schema_validates_button() {
        let schema = button_schema();
        let meta = ButtonMeta {
            label: "OK".into(),
            action_ref: None,
            style: ButtonStyle::Primary,
        };
        let digit = button_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn schema_validates_nav_link() {
        let schema = nav_link_schema();
        let meta = NavLinkMeta {
            label: "Home".into(),
            target_ref: "/index.idea".into(),
        };
        let digit = nav_link_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn serde_round_trip() {
        let meta = ButtonMeta {
            label: "Click".into(),
            action_ref: None,
            style: ButtonStyle::Tertiary,
        };
        let digit = button_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_button_meta(&rt).unwrap();
        assert_eq!(parsed.style, ButtonStyle::Tertiary);
    }
}
