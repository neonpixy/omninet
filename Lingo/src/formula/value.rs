use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Spreadsheet error variants, modeled after Excel/Sheets conventions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FormulaErrorKind {
    /// #REF! — invalid cell reference
    Ref,
    /// #DIV/0! — division by zero
    Div0,
    /// #VALUE! — wrong value type
    Value,
    /// #NAME? — unknown function or name
    Name,
    /// #N/A — value not available
    Na,
    /// #NUM! — invalid numeric value
    Num,
    /// #CIRCULAR! — circular reference detected
    Circular,
}

impl fmt::Display for FormulaErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormulaErrorKind::Ref => write!(f, "#REF!"),
            FormulaErrorKind::Div0 => write!(f, "#DIV/0!"),
            FormulaErrorKind::Value => write!(f, "#VALUE!"),
            FormulaErrorKind::Name => write!(f, "#NAME?"),
            FormulaErrorKind::Na => write!(f, "#N/A"),
            FormulaErrorKind::Num => write!(f, "#NUM!"),
            FormulaErrorKind::Circular => write!(f, "#CIRCULAR!"),
        }
    }
}

/// A value that a formula cell can hold.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FormulaValue {
    /// A numeric value (integer or floating-point).
    Number(f64),
    /// A text string.
    Text(String),
    /// A boolean value (TRUE or FALSE).
    Bool(bool),
    /// A date-time value.
    Date(DateTime<Utc>),
    /// A spreadsheet error (e.g., #REF!, #DIV/0!).
    Error(FormulaErrorKind),
    /// An empty cell with no value.
    Empty,
}

impl FormulaValue {
    /// Try to extract a numeric value. Bools coerce (true=1, false=0).
    pub fn as_number(&self) -> Option<f64> {
        match self {
            FormulaValue::Number(n) => Some(*n),
            FormulaValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// Try to extract a text value.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FormulaValue::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Try to extract a boolean value. Numbers coerce (0=false, else true).
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FormulaValue::Bool(b) => Some(*b),
            FormulaValue::Number(n) => Some(*n != 0.0),
            _ => None,
        }
    }

    /// Returns true if this value is an error.
    pub fn is_error(&self) -> bool {
        matches!(self, FormulaValue::Error(_))
    }

    /// Returns true if this value is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, FormulaValue::Empty)
    }

    /// Convert to a human-readable display string.
    pub fn to_display_string(&self) -> String {
        match self {
            FormulaValue::Number(n) => {
                // Display integers without decimal point
                if n.fract() == 0.0 && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            FormulaValue::Text(s) => s.clone(),
            FormulaValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            FormulaValue::Date(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            FormulaValue::Error(kind) => kind.to_string(),
            FormulaValue::Empty => String::new(),
        }
    }
}

// Custom PartialEq that handles f64 comparison (NaN != NaN is fine for our purposes).
impl PartialEq for FormulaValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FormulaValue::Number(a), FormulaValue::Number(b)) => a == b,
            (FormulaValue::Text(a), FormulaValue::Text(b)) => a == b,
            (FormulaValue::Bool(a), FormulaValue::Bool(b)) => a == b,
            (FormulaValue::Date(a), FormulaValue::Date(b)) => a == b,
            (FormulaValue::Error(a), FormulaValue::Error(b)) => a == b,
            (FormulaValue::Empty, FormulaValue::Empty) => true,
            _ => false,
        }
    }
}

impl fmt::Display for FormulaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_as_number() {
        let v = FormulaValue::Number(42.0);
        assert_eq!(v.as_number(), Some(42.0));
    }

    #[test]
    fn bool_coerces_to_number() {
        assert_eq!(FormulaValue::Bool(true).as_number(), Some(1.0));
        assert_eq!(FormulaValue::Bool(false).as_number(), Some(0.0));
    }

    #[test]
    fn text_does_not_coerce_to_number() {
        assert_eq!(FormulaValue::Text("hello".into()).as_number(), None);
    }

    #[test]
    fn number_coerces_to_bool() {
        assert_eq!(FormulaValue::Number(0.0).as_bool(), Some(false));
        assert_eq!(FormulaValue::Number(1.0).as_bool(), Some(true));
        assert_eq!(FormulaValue::Number(-5.0).as_bool(), Some(true));
    }

    #[test]
    fn text_extraction() {
        let v = FormulaValue::Text("hello".into());
        assert_eq!(v.as_text(), Some("hello"));
        assert_eq!(FormulaValue::Number(1.0).as_text(), None);
    }

    #[test]
    fn error_checks() {
        assert!(FormulaValue::Error(FormulaErrorKind::Div0).is_error());
        assert!(!FormulaValue::Number(1.0).is_error());
    }

    #[test]
    fn empty_checks() {
        assert!(FormulaValue::Empty.is_empty());
        assert!(!FormulaValue::Number(0.0).is_empty());
    }

    #[test]
    fn display_strings() {
        assert_eq!(FormulaValue::Number(42.0).to_display_string(), "42");
        assert_eq!(FormulaValue::Number(3.15).to_display_string(), "3.15");
        assert_eq!(FormulaValue::Bool(true).to_display_string(), "TRUE");
        assert_eq!(FormulaValue::Bool(false).to_display_string(), "FALSE");
        assert_eq!(
            FormulaValue::Text("hi".into()).to_display_string(),
            "hi"
        );
        assert_eq!(FormulaValue::Empty.to_display_string(), "");
        assert_eq!(
            FormulaValue::Error(FormulaErrorKind::Ref).to_display_string(),
            "#REF!"
        );
        assert_eq!(
            FormulaValue::Error(FormulaErrorKind::Div0).to_display_string(),
            "#DIV/0!"
        );
        assert_eq!(
            FormulaValue::Error(FormulaErrorKind::Name).to_display_string(),
            "#NAME?"
        );
    }

    #[test]
    fn error_kind_display() {
        assert_eq!(FormulaErrorKind::Na.to_string(), "#N/A");
        assert_eq!(FormulaErrorKind::Num.to_string(), "#NUM!");
        assert_eq!(FormulaErrorKind::Circular.to_string(), "#CIRCULAR!");
        assert_eq!(FormulaErrorKind::Value.to_string(), "#VALUE!");
    }

    #[test]
    fn value_equality() {
        assert_eq!(FormulaValue::Number(1.0), FormulaValue::Number(1.0));
        assert_ne!(FormulaValue::Number(1.0), FormulaValue::Number(2.0));
        assert_ne!(FormulaValue::Number(1.0), FormulaValue::Text("1".into()));
        assert_eq!(FormulaValue::Empty, FormulaValue::Empty);
    }

    #[test]
    fn serialization_round_trip() {
        let values = vec![
            FormulaValue::Number(42.5),
            FormulaValue::Text("hello".into()),
            FormulaValue::Bool(true),
            FormulaValue::Error(FormulaErrorKind::Div0),
            FormulaValue::Empty,
        ];
        for v in &values {
            let json = serde_json::to_string(v).unwrap();
            let back: FormulaValue = serde_json::from_str(&json).unwrap();
            assert_eq!(*v, back);
        }
    }
}
