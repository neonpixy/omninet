use serde::{Deserialize, Serialize};

use super::value::FormulaValue;

/// A reference to a cell in a spreadsheet.
///
/// This is Lingo's own type — independent of Ideas' CellAddress — to keep the
/// Lingo crate free of Ideas dependencies.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormulaCellRef {
    /// Optional sheet name (e.g., "Sheet1" in "Sheet1!A1").
    pub sheet: Option<String>,
    /// Column letters (e.g., "A", "AB", "ZZ").
    pub column: String,
    /// 1-based row number.
    pub row: u32,
    /// Whether the column is absolute (prefixed with $).
    pub abs_column: bool,
    /// Whether the row is absolute (prefixed with $).
    pub abs_row: bool,
}

impl FormulaCellRef {
    /// Convert column letters to a 0-based column index ("A" -> 0, "B" -> 1, "Z" -> 25, "AA" -> 26).
    pub fn column_index(&self) -> u32 {
        let mut index: u32 = 0;
        for byte in self.column.as_bytes() {
            let c = byte.to_ascii_uppercase();
            index = index * 26 + (c - b'A') as u32 + 1;
        }
        index.saturating_sub(1)
    }

    /// Create a FormulaCellRef from column index (0-based) and row (1-based).
    pub fn from_indices(col_index: u32, row: u32) -> Self {
        Self {
            sheet: None,
            column: column_index_to_letters(col_index),
            row,
            abs_column: false,
            abs_row: false,
        }
    }

    /// Produce a canonical string like "A1", "$A$1", or "Sheet1!$B$2".
    pub fn to_string_repr(&self) -> String {
        let mut s = String::new();
        if let Some(sheet) = &self.sheet {
            s.push_str(sheet);
            s.push('!');
        }
        if self.abs_column {
            s.push('$');
        }
        s.push_str(&self.column);
        if self.abs_row {
            s.push('$');
        }
        s.push_str(&self.row.to_string());
        s
    }
}

impl std::fmt::Display for FormulaCellRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_repr())
    }
}

/// Convert a 0-based column index to column letters ("A", "B", ..., "Z", "AA", ...).
fn column_index_to_letters(mut index: u32) -> String {
    let mut letters = String::new();
    loop {
        letters.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    letters
}

/// The abstract syntax tree for a parsed formula.
#[derive(Clone, Debug, PartialEq)]
pub enum FormulaNode {
    /// A literal value (number, text, bool).
    Literal(FormulaValue),
    /// A single cell reference.
    CellRef(FormulaCellRef),
    /// A range of cells (e.g., A1:B5).
    Range {
        start: FormulaCellRef,
        end: FormulaCellRef,
    },
    /// A binary operation (e.g., A1 + B1).
    BinaryOp {
        op: BinaryOp,
        left: Box<FormulaNode>,
        right: Box<FormulaNode>,
    },
    /// A unary operation (e.g., -A1, 50%).
    UnaryOp {
        op: UnaryOp,
        operand: Box<FormulaNode>,
    },
    /// A function call (e.g., SUM(A1:A10)).
    FunctionCall {
        name: String,
        args: Vec<FormulaNode>,
    },
    /// A parenthesized expression (preserved for display fidelity).
    Parenthesized(Box<FormulaNode>),
}

/// Binary operators with their semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    /// Addition (+).
    Add,
    /// Subtraction (-).
    Sub,
    /// Multiplication (*).
    Mul,
    /// Division (/).
    Div,
    /// Modulo (%).
    Mod,
    /// Exponentiation (^), right-associative.
    Pow,
    /// Equality (=).
    Eq,
    /// Inequality (<>).
    Ne,
    /// Less than (<).
    Lt,
    /// Less than or equal (<=).
    Le,
    /// Greater than (>).
    Gt,
    /// Greater than or equal (>=).
    Ge,
    /// String concatenation (&).
    Concat,
}

/// Unary operators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// Negation (-x).
    Neg,
    /// Percentage (x% = x/100).
    Percent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_index_simple() {
        let cell = FormulaCellRef {
            sheet: None,
            column: "A".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.column_index(), 0);

        let cell = FormulaCellRef {
            sheet: None,
            column: "B".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.column_index(), 1);

        let cell = FormulaCellRef {
            sheet: None,
            column: "Z".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.column_index(), 25);
    }

    #[test]
    fn column_index_multi_letter() {
        let cell = FormulaCellRef {
            sheet: None,
            column: "AA".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.column_index(), 26);

        let cell = FormulaCellRef {
            sheet: None,
            column: "AZ".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.column_index(), 51);

        let cell = FormulaCellRef {
            sheet: None,
            column: "BA".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.column_index(), 52);
    }

    #[test]
    fn from_indices_round_trip() {
        for i in 0..100 {
            let cell = FormulaCellRef::from_indices(i, 1);
            assert_eq!(cell.column_index(), i);
        }
    }

    #[test]
    fn column_index_to_letters_cases() {
        assert_eq!(column_index_to_letters(0), "A");
        assert_eq!(column_index_to_letters(25), "Z");
        assert_eq!(column_index_to_letters(26), "AA");
        assert_eq!(column_index_to_letters(51), "AZ");
        assert_eq!(column_index_to_letters(52), "BA");
        assert_eq!(column_index_to_letters(701), "ZZ");
    }

    #[test]
    fn to_string_repr_variations() {
        let cell = FormulaCellRef {
            sheet: None,
            column: "A".into(),
            row: 1,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.to_string_repr(), "A1");

        let cell = FormulaCellRef {
            sheet: None,
            column: "A".into(),
            row: 1,
            abs_column: true,
            abs_row: true,
        };
        assert_eq!(cell.to_string_repr(), "$A$1");

        let cell = FormulaCellRef {
            sheet: Some("Sheet1".into()),
            column: "B".into(),
            row: 2,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(cell.to_string_repr(), "Sheet1!B2");

        let cell = FormulaCellRef {
            sheet: Some("Data".into()),
            column: "C".into(),
            row: 5,
            abs_column: true,
            abs_row: true,
        };
        assert_eq!(cell.to_string_repr(), "Data!$C$5");
    }

    #[test]
    fn display_trait() {
        let cell = FormulaCellRef {
            sheet: None,
            column: "AB".into(),
            row: 99,
            abs_column: false,
            abs_row: false,
        };
        assert_eq!(format!("{}", cell), "AB99");
    }

    #[test]
    fn cell_ref_serialization() {
        let cell = FormulaCellRef {
            sheet: Some("Sheet2".into()),
            column: "D".into(),
            row: 10,
            abs_column: true,
            abs_row: false,
        };
        let json = serde_json::to_string(&cell).unwrap();
        let back: FormulaCellRef = serde_json::from_str(&json).unwrap();
        assert_eq!(cell, back);
    }
}
