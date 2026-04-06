//! Form digit helpers — typed constructors and parsers for interactive form elements.
//!
//! Form metadata is stored in Digit properties as `Value` types.
//! Used by Studio Interactive in Throne for building forms.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use crate::helpers::{check_type, prop_bool, prop_str, prop_str_opt};
use crate::schema::{DigitSchema, PropertyType};
use x::Value;

const DOMAIN: &str = "form";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The kind of input field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// Single-line plain text.
    Text,
    /// Numeric input.
    Number,
    /// Email address with basic validation.
    Email,
    /// Date picker.
    Date,
    /// Masked password input.
    Password,
    /// Multi-line text area.
    Multiline,
}

impl InputType {
    fn to_str(&self) -> &'static str {
        match self {
            InputType::Text => "text",
            InputType::Number => "number",
            InputType::Email => "email",
            InputType::Date => "date",
            InputType::Password => "password",
            InputType::Multiline => "multiline",
        }
    }

    fn from_str_value(s: &str) -> Result<Self, IdeasError> {
        match s {
            "text" => Ok(InputType::Text),
            "number" => Ok(InputType::Number),
            "email" => Ok(InputType::Email),
            "date" => Ok(InputType::Date),
            "password" => Ok(InputType::Password),
            "multiline" => Ok(InputType::Multiline),
            other => Err(IdeasError::FormParsing(format!(
                "unknown input type: {other}"
            ))),
        }
    }
}

/// Metadata for an input field digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputFieldMeta {
    pub input_type: InputType,
    pub label: String,
    pub placeholder: Option<String>,
    pub required: bool,
    /// Regex validation pattern.
    pub pattern: Option<String>,
}

/// Metadata for a checkbox digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckboxMeta {
    pub label: String,
    pub checked: bool,
}

/// Metadata for a radio button digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadioMeta {
    pub label: String,
    pub group: String,
    pub value: String,
}

/// Metadata for a toggle switch digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToggleMeta {
    pub label: String,
    pub on: bool,
}

/// Metadata for a dropdown/select digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DropdownMeta {
    pub label: String,
    pub options: Vec<String>,
    pub selected: Option<String>,
}

/// Metadata for a submit button digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitMeta {
    pub label: String,
    pub action_ref: Option<String>,
}

/// Metadata for a form container digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormMeta {
    pub name: String,
    pub submit_handler_ref: Option<String>,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Create an input field digit.
pub fn input_field_digit(meta: &InputFieldMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.input".into(), Value::Null, author.into())?;
    digit = digit.with_property(
        "input_type".into(),
        Value::String(meta.input_type.to_str().into()),
        author,
    );
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    if let Some(ref ph) = meta.placeholder {
        digit = digit.with_property("placeholder".into(), Value::String(ph.clone()), author);
    }
    digit = digit.with_property("required".into(), Value::Bool(meta.required), author);
    if let Some(ref pat) = meta.pattern {
        digit = digit.with_property("pattern".into(), Value::String(pat.clone()), author);
    }
    Ok(digit)
}

/// Create a checkbox digit.
pub fn checkbox_digit(meta: &CheckboxMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.checkbox".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    digit = digit.with_property("checked".into(), Value::Bool(meta.checked), author);
    Ok(digit)
}

/// Create a radio button digit.
pub fn radio_digit(meta: &RadioMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.radio".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    digit = digit.with_property("group".into(), Value::String(meta.group.clone()), author);
    digit = digit.with_property("value".into(), Value::String(meta.value.clone()), author);
    Ok(digit)
}

/// Create a toggle switch digit.
pub fn toggle_digit(meta: &ToggleMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.toggle".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    digit = digit.with_property("on".into(), Value::Bool(meta.on), author);
    Ok(digit)
}

/// Create a dropdown digit.
pub fn dropdown_digit(meta: &DropdownMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.dropdown".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    let options_value = Value::Array(
        meta.options
            .iter()
            .map(|o| Value::String(o.clone()))
            .collect(),
    );
    digit = digit.with_property("options".into(), options_value, author);
    if let Some(ref sel) = meta.selected {
        digit = digit.with_property("selected".into(), Value::String(sel.clone()), author);
    }
    Ok(digit)
}

/// Create a submit button digit.
pub fn submit_digit(meta: &SubmitMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.submit".into(), Value::Null, author.into())?;
    digit = digit.with_property("label".into(), Value::String(meta.label.clone()), author);
    if let Some(ref action) = meta.action_ref {
        digit = digit.with_property("action_ref".into(), Value::String(action.clone()), author);
    }
    Ok(digit)
}

/// Create a form container digit.
pub fn form_digit(meta: &FormMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("form.container".into(), Value::Null, author.into())?;
    digit = digit.with_property("name".into(), Value::String(meta.name.clone()), author);
    if let Some(ref handler) = meta.submit_handler_ref {
        digit = digit.with_property(
            "submit_handler_ref".into(),
            Value::String(handler.clone()),
            author,
        );
    }
    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse input field metadata from a digit.
pub fn parse_input_field_meta(digit: &Digit) -> Result<InputFieldMeta, IdeasError> {
    check_type(digit, "form.input", DOMAIN)?;
    let type_str = prop_str(digit, "input_type", DOMAIN)?;
    Ok(InputFieldMeta {
        input_type: InputType::from_str_value(&type_str)?,
        label: prop_str(digit, "label", DOMAIN)?,
        placeholder: prop_str_opt(digit, "placeholder"),
        required: prop_bool(digit, "required", DOMAIN)?,
        pattern: prop_str_opt(digit, "pattern"),
    })
}

/// Parse checkbox metadata from a digit.
pub fn parse_checkbox_meta(digit: &Digit) -> Result<CheckboxMeta, IdeasError> {
    check_type(digit, "form.checkbox", DOMAIN)?;
    Ok(CheckboxMeta {
        label: prop_str(digit, "label", DOMAIN)?,
        checked: prop_bool(digit, "checked", DOMAIN)?,
    })
}

/// Parse radio button metadata from a digit.
pub fn parse_radio_meta(digit: &Digit) -> Result<RadioMeta, IdeasError> {
    check_type(digit, "form.radio", DOMAIN)?;
    Ok(RadioMeta {
        label: prop_str(digit, "label", DOMAIN)?,
        group: prop_str(digit, "group", DOMAIN)?,
        value: prop_str(digit, "value", DOMAIN)?,
    })
}

/// Parse toggle metadata from a digit.
pub fn parse_toggle_meta(digit: &Digit) -> Result<ToggleMeta, IdeasError> {
    check_type(digit, "form.toggle", DOMAIN)?;
    Ok(ToggleMeta {
        label: prop_str(digit, "label", DOMAIN)?,
        on: prop_bool(digit, "on", DOMAIN)?,
    })
}

/// Parse dropdown metadata from a digit.
pub fn parse_dropdown_meta(digit: &Digit) -> Result<DropdownMeta, IdeasError> {
    check_type(digit, "form.dropdown", DOMAIN)?;
    let label = prop_str(digit, "label", DOMAIN)?;
    let options = digit
        .properties
        .get("options")
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
        .ok_or_else(|| IdeasError::FormParsing("missing property: options".into()))?;
    let selected = prop_str_opt(digit, "selected");

    Ok(DropdownMeta {
        label,
        options,
        selected,
    })
}

/// Parse submit button metadata from a digit.
pub fn parse_submit_meta(digit: &Digit) -> Result<SubmitMeta, IdeasError> {
    check_type(digit, "form.submit", DOMAIN)?;
    Ok(SubmitMeta {
        label: prop_str(digit, "label", DOMAIN)?,
        action_ref: prop_str_opt(digit, "action_ref"),
    })
}

/// Parse form container metadata from a digit.
pub fn parse_form_meta(digit: &Digit) -> Result<FormMeta, IdeasError> {
    check_type(digit, "form.container", DOMAIN)?;
    Ok(FormMeta {
        name: prop_str(digit, "name", DOMAIN)?,
        submit_handler_ref: prop_str_opt(digit, "submit_handler_ref"),
    })
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

/// Schema for `form.input` digits.
pub fn input_field_schema() -> DigitSchema {
    DigitSchema::new("form.input".into())
        .with_required("input_type", PropertyType::String)
        .with_required("label", PropertyType::String)
        .with_required("required", PropertyType::Bool)
        .with_optional("placeholder", PropertyType::String)
        .with_optional("pattern", PropertyType::String)
        .with_description("Form input field")
}

/// Schema for `form.checkbox` digits.
pub fn checkbox_schema() -> DigitSchema {
    DigitSchema::new("form.checkbox".into())
        .with_required("label", PropertyType::String)
        .with_required("checked", PropertyType::Bool)
        .with_description("Form checkbox")
}

/// Schema for `form.radio` digits.
pub fn radio_schema() -> DigitSchema {
    DigitSchema::new("form.radio".into())
        .with_required("label", PropertyType::String)
        .with_required("group", PropertyType::String)
        .with_required("value", PropertyType::String)
        .with_description("Form radio button")
}

/// Schema for `form.toggle` digits.
pub fn toggle_schema() -> DigitSchema {
    DigitSchema::new("form.toggle".into())
        .with_required("label", PropertyType::String)
        .with_required("on", PropertyType::Bool)
        .with_description("Form toggle switch")
}

/// Schema for `form.dropdown` digits.
pub fn dropdown_schema() -> DigitSchema {
    DigitSchema::new("form.dropdown".into())
        .with_required("label", PropertyType::String)
        .with_required("options", PropertyType::Array)
        .with_optional("selected", PropertyType::String)
        .with_description("Form dropdown select")
}

/// Schema for `form.submit` digits.
pub fn submit_schema() -> DigitSchema {
    DigitSchema::new("form.submit".into())
        .with_required("label", PropertyType::String)
        .with_optional("action_ref", PropertyType::String)
        .with_description("Form submit button")
}

/// Schema for `form.container` digits.
pub fn form_schema() -> DigitSchema {
    DigitSchema::new("form.container".into())
        .with_required("name", PropertyType::String)
        .with_optional("submit_handler_ref", PropertyType::String)
        .with_description("Form container")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_field_round_trip() {
        let meta = InputFieldMeta {
            input_type: InputType::Email,
            label: "Email Address".into(),
            placeholder: Some("you@example.com".into()),
            required: true,
            pattern: Some(r"^[^@]+@[^@]+\.[^@]+$".into()),
        };
        let digit = input_field_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.input");

        let parsed = parse_input_field_meta(&digit).unwrap();
        assert_eq!(parsed.input_type, InputType::Email);
        assert_eq!(parsed.label, "Email Address");
        assert_eq!(parsed.placeholder.as_deref(), Some("you@example.com"));
        assert!(parsed.required);
        assert!(parsed.pattern.is_some());
    }

    #[test]
    fn input_field_minimal() {
        let meta = InputFieldMeta {
            input_type: InputType::Text,
            label: "Name".into(),
            placeholder: None,
            required: false,
            pattern: None,
        };
        let digit = input_field_digit(&meta, "alice").unwrap();
        let parsed = parse_input_field_meta(&digit).unwrap();
        assert_eq!(parsed.input_type, InputType::Text);
        assert!(!parsed.required);
        assert!(parsed.placeholder.is_none());
        assert!(parsed.pattern.is_none());
    }

    #[test]
    fn checkbox_round_trip() {
        let meta = CheckboxMeta {
            label: "I agree to terms".into(),
            checked: false,
        };
        let digit = checkbox_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.checkbox");

        let parsed = parse_checkbox_meta(&digit).unwrap();
        assert_eq!(parsed.label, "I agree to terms");
        assert!(!parsed.checked);
    }

    #[test]
    fn radio_round_trip() {
        let meta = RadioMeta {
            label: "Option A".into(),
            group: "choices".into(),
            value: "a".into(),
        };
        let digit = radio_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.radio");

        let parsed = parse_radio_meta(&digit).unwrap();
        assert_eq!(parsed.label, "Option A");
        assert_eq!(parsed.group, "choices");
        assert_eq!(parsed.value, "a");
    }

    #[test]
    fn toggle_round_trip() {
        let meta = ToggleMeta {
            label: "Dark mode".into(),
            on: true,
        };
        let digit = toggle_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.toggle");

        let parsed = parse_toggle_meta(&digit).unwrap();
        assert_eq!(parsed.label, "Dark mode");
        assert!(parsed.on);
    }

    #[test]
    fn dropdown_round_trip() {
        let meta = DropdownMeta {
            label: "Country".into(),
            options: vec!["US".into(), "UK".into(), "DE".into()],
            selected: Some("US".into()),
        };
        let digit = dropdown_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.dropdown");

        let parsed = parse_dropdown_meta(&digit).unwrap();
        assert_eq!(parsed.label, "Country");
        assert_eq!(parsed.options, vec!["US", "UK", "DE"]);
        assert_eq!(parsed.selected.as_deref(), Some("US"));
    }

    #[test]
    fn dropdown_no_selection() {
        let meta = DropdownMeta {
            label: "Size".into(),
            options: vec!["S".into(), "M".into(), "L".into()],
            selected: None,
        };
        let digit = dropdown_digit(&meta, "alice").unwrap();
        let parsed = parse_dropdown_meta(&digit).unwrap();
        assert!(parsed.selected.is_none());
    }

    #[test]
    fn submit_round_trip() {
        let meta = SubmitMeta {
            label: "Send".into(),
            action_ref: Some("handler-123".into()),
        };
        let digit = submit_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.submit");

        let parsed = parse_submit_meta(&digit).unwrap();
        assert_eq!(parsed.label, "Send");
        assert_eq!(parsed.action_ref.as_deref(), Some("handler-123"));
    }

    #[test]
    fn submit_no_action() {
        let meta = SubmitMeta {
            label: "OK".into(),
            action_ref: None,
        };
        let digit = submit_digit(&meta, "alice").unwrap();
        let parsed = parse_submit_meta(&digit).unwrap();
        assert!(parsed.action_ref.is_none());
    }

    #[test]
    fn form_container_round_trip() {
        let meta = FormMeta {
            name: "Registration".into(),
            submit_handler_ref: Some("register-handler".into()),
        };
        let digit = form_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "form.container");

        let parsed = parse_form_meta(&digit).unwrap();
        assert_eq!(parsed.name, "Registration");
        assert_eq!(
            parsed.submit_handler_ref.as_deref(),
            Some("register-handler")
        );
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_input_field_meta(&digit).is_err());
        assert!(parse_checkbox_meta(&digit).is_err());
        assert!(parse_radio_meta(&digit).is_err());
        assert!(parse_toggle_meta(&digit).is_err());
        assert!(parse_dropdown_meta(&digit).is_err());
        assert!(parse_submit_meta(&digit).is_err());
        assert!(parse_form_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit = Digit::new("form.input".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_input_field_meta(&digit).is_err());

        let digit = Digit::new("form.checkbox".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_checkbox_meta(&digit).is_err());

        let digit = Digit::new("form.dropdown".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_dropdown_meta(&digit).is_err());
    }

    #[test]
    fn all_input_types() {
        for (it, name) in [
            (InputType::Text, "text"),
            (InputType::Number, "number"),
            (InputType::Email, "email"),
            (InputType::Date, "date"),
            (InputType::Password, "password"),
            (InputType::Multiline, "multiline"),
        ] {
            assert_eq!(it.to_str(), name);
            assert_eq!(InputType::from_str_value(name).unwrap(), it);
        }
    }

    #[test]
    fn invalid_input_type() {
        assert!(InputType::from_str_value("unknown").is_err());
    }

    #[test]
    fn schema_validates_input() {
        let schema = input_field_schema();
        let meta = InputFieldMeta {
            input_type: InputType::Text,
            label: "Name".into(),
            placeholder: None,
            required: true,
            pattern: None,
        };
        let digit = input_field_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn schema_validates_checkbox() {
        let schema = checkbox_schema();
        let meta = CheckboxMeta {
            label: "Accept".into(),
            checked: false,
        };
        let digit = checkbox_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn serde_round_trip() {
        let meta = InputFieldMeta {
            input_type: InputType::Password,
            label: "Password".into(),
            placeholder: None,
            required: true,
            pattern: None,
        };
        let digit = input_field_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_input_field_meta(&rt).unwrap();
        assert_eq!(parsed.input_type, InputType::Password);
    }
}
