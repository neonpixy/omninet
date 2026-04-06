use serde::{Deserialize, Serialize};

/// A span within the formula source text.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Create a span covering bytes `start..end` in the formula source.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Errors that can occur during formula parsing and evaluation.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize, PartialEq)]
pub enum FormulaError {
    /// The parser encountered a token it did not expect at the given position.
    #[error("unexpected token at {span:?}: {message}")]
    UnexpectedToken { span: Span, message: String },

    /// A string literal was opened with `"` but never closed.
    #[error("unterminated string at {span:?}")]
    UnterminatedString { span: Span },

    /// A cell reference like "$A$0" or "!!" could not be parsed.
    #[error("invalid cell reference: {reference}")]
    InvalidCellRef { reference: String },

    /// A function name was used that is not registered in the function registry.
    #[error("unknown function: {name}")]
    UnknownFunction { name: String },

    /// A function was called with the wrong number of arguments.
    #[error("wrong argument count for {name}: expected {expected}, got {got}")]
    ArgumentCount {
        name: String,
        expected: usize,
        got: usize,
    },

    /// An operand or argument has the wrong type for the operation.
    #[error("type error: {message}")]
    TypeError { message: String },

    /// A division or modulo by zero was attempted.
    #[error("division by zero")]
    DivisionByZero,

    /// A cell's formula chain forms a cycle back to itself.
    #[error("circular reference detected")]
    CircularReference,

    /// A general error during formula evaluation.
    #[error("evaluation error: {message}")]
    EvaluationError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_creation() {
        let span = Span::new(0, 5);
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 5);
    }

    #[test]
    fn error_display() {
        let err = FormulaError::DivisionByZero;
        assert_eq!(err.to_string(), "division by zero");

        let err = FormulaError::UnknownFunction {
            name: "VLOOKUP".into(),
        };
        assert!(err.to_string().contains("VLOOKUP"));

        let err = FormulaError::ArgumentCount {
            name: "IF".into(),
            expected: 3,
            got: 1,
        };
        assert!(err.to_string().contains("IF"));
        assert!(err.to_string().contains("3"));
        assert!(err.to_string().contains("1"));
    }

    #[test]
    fn error_clone_and_eq() {
        let err1 = FormulaError::CircularReference;
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn error_serialization_round_trip() {
        let err = FormulaError::UnexpectedToken {
            span: Span::new(3, 7),
            message: "expected operator".into(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: FormulaError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }
}
