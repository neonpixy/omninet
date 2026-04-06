//! Sheet digit helpers — typed constructors and parsers for spreadsheet content.
//!
//! Sheet metadata is stored in Digit properties as `Value` types.
//! These helpers provide ergonomic creation and parsing of sheet and cell
//! digits used by the Abacus program in Throne.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use crate::helpers::{check_type, prop_str};
use crate::schema::{DigitSchema, PropertyType};
use x::Value;

const DOMAIN: &str = "sheet";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The data type of a cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CellType {
    /// Plain text content.
    Text,
    /// A numeric value.
    Number,
    /// A date/time value.
    Date,
    /// A true/false value.
    Boolean,
    /// A computed formula (starts with `=`).
    Formula,
    /// A reference to another cell or sheet.
    Reference,
    /// Rich text with formatting.
    Rich,
}

impl CellType {
    fn to_str(&self) -> &'static str {
        match self {
            CellType::Text => "text",
            CellType::Number => "number",
            CellType::Date => "date",
            CellType::Boolean => "boolean",
            CellType::Formula => "formula",
            CellType::Reference => "reference",
            CellType::Rich => "rich",
        }
    }

    fn from_str_value(s: &str) -> Result<Self, IdeasError> {
        match s {
            "text" => Ok(CellType::Text),
            "number" => Ok(CellType::Number),
            "date" => Ok(CellType::Date),
            "boolean" => Ok(CellType::Boolean),
            "formula" => Ok(CellType::Formula),
            "reference" => Ok(CellType::Reference),
            "rich" => Ok(CellType::Rich),
            other => Err(IdeasError::SheetParsing(format!(
                "unknown cell type: {other}"
            ))),
        }
    }
}

/// A cell address (e.g., Sheet1!A1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellAddress {
    /// Optional sheet name for cross-sheet references.
    pub sheet: Option<String>,
    /// Column identifier (e.g., "A", "AB").
    pub column: String,
    /// Row number (1-based).
    pub row: u32,
}

impl CellAddress {
    fn to_string_repr(&self) -> String {
        match &self.sheet {
            Some(s) => format!("{s}!{}{}", self.column, self.row),
            None => format!("{}{}", self.column, self.row),
        }
    }

    fn from_string_repr(s: &str) -> Result<Self, IdeasError> {
        let (sheet, rest) = if let Some(idx) = s.find('!') {
            (Some(s[..idx].to_string()), &s[idx + 1..])
        } else {
            (None, s)
        };

        // Split column letters from row digits
        let col_end = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
        if col_end == 0 || col_end == rest.len() {
            return Err(IdeasError::SheetParsing(format!(
                "invalid cell address: {s}"
            )));
        }
        let column = rest[..col_end].to_string();
        let row: u32 = rest[col_end..]
            .parse()
            .map_err(|_| IdeasError::SheetParsing(format!("invalid row in address: {s}")))?;

        Ok(CellAddress { sheet, column, row })
    }
}

/// A range of cells.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRange {
    pub start: CellAddress,
    pub end: CellAddress,
}

/// How a sheet is displayed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Traditional spreadsheet grid.
    Grid,
    /// Card-based kanban board.
    Kanban,
    /// Calendar view for date-based data.
    Calendar,
    /// Image-centric gallery view.
    Gallery,
}

impl ViewMode {
    fn to_str(&self) -> &'static str {
        match self {
            ViewMode::Grid => "grid",
            ViewMode::Kanban => "kanban",
            ViewMode::Calendar => "calendar",
            ViewMode::Gallery => "gallery",
        }
    }

    fn from_str_value(s: &str) -> Result<Self, IdeasError> {
        match s {
            "grid" => Ok(ViewMode::Grid),
            "kanban" => Ok(ViewMode::Kanban),
            "calendar" => Ok(ViewMode::Calendar),
            "gallery" => Ok(ViewMode::Gallery),
            other => Err(IdeasError::SheetParsing(format!(
                "unknown view mode: {other}"
            ))),
        }
    }
}

/// Definition of a column in a sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub cell_type: CellType,
    pub required: bool,
    pub unique: bool,
}

/// Metadata for a sheet digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheetMeta {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub default_view: ViewMode,
}

/// Metadata for a cell digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellMeta {
    pub address: CellAddress,
    pub cell_type: CellType,
    /// Raw value as string. Formulas start with `=`.
    pub value: String,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Create a sheet digit from metadata.
pub fn sheet_digit(meta: &SheetMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("data.sheet".into(), Value::Null, author.into())?;
    digit = digit.with_property("name".into(), Value::String(meta.name.clone()), author);

    let columns_json = serde_json::to_string(&meta.columns)
        .map_err(|e| IdeasError::SheetParsing(format!("failed to serialize columns: {e}")))?;
    digit = digit.with_property("columns".into(), Value::String(columns_json), author);

    digit = digit.with_property(
        "default_view".into(),
        Value::String(meta.default_view.to_str().into()),
        author,
    );
    Ok(digit)
}

/// Create a cell digit from metadata.
pub fn cell_digit(meta: &CellMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("data.cell".into(), Value::Null, author.into())?;
    digit = digit.with_property(
        "address".into(),
        Value::String(meta.address.to_string_repr()),
        author,
    );
    digit = digit.with_property(
        "cell_type".into(),
        Value::String(meta.cell_type.to_str().into()),
        author,
    );
    digit = digit.with_property("value".into(), Value::String(meta.value.clone()), author);
    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse sheet metadata from a digit.
pub fn parse_sheet_meta(digit: &Digit) -> Result<SheetMeta, IdeasError> {
    check_type(digit, "data.sheet", DOMAIN)?;

    let name = prop_str(digit, "name", DOMAIN)?;
    let columns_json = prop_str(digit, "columns", DOMAIN)?;
    let columns: Vec<ColumnDef> = serde_json::from_str(&columns_json)
        .map_err(|e| IdeasError::SheetParsing(format!("invalid columns JSON: {e}")))?;
    let view_str = prop_str(digit, "default_view", DOMAIN)?;
    let default_view = ViewMode::from_str_value(&view_str)?;

    Ok(SheetMeta {
        name,
        columns,
        default_view,
    })
}

/// Parse cell metadata from a digit.
pub fn parse_cell_meta(digit: &Digit) -> Result<CellMeta, IdeasError> {
    check_type(digit, "data.cell", DOMAIN)?;

    let addr_str = prop_str(digit, "address", DOMAIN)?;
    let address = CellAddress::from_string_repr(&addr_str)?;
    let type_str = prop_str(digit, "cell_type", DOMAIN)?;
    let cell_type = CellType::from_str_value(&type_str)?;
    let value = prop_str(digit, "value", DOMAIN)?;

    Ok(CellMeta {
        address,
        cell_type,
        value,
    })
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

/// Schema for `data.sheet` digits.
pub fn sheet_schema() -> DigitSchema {
    DigitSchema::new("data.sheet".into())
        .with_required("name", PropertyType::String)
        .with_required("columns", PropertyType::String) // JSON-encoded
        .with_required("default_view", PropertyType::String)
        .with_description("Spreadsheet/database sheet metadata")
}

/// Schema for `data.cell` digits.
pub fn cell_schema() -> DigitSchema {
    DigitSchema::new("data.cell".into())
        .with_required("address", PropertyType::String)
        .with_required("cell_type", PropertyType::String)
        .with_required("value", PropertyType::String)
        .with_description("Individual cell within a sheet")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sheet_meta() -> SheetMeta {
        SheetMeta {
            name: "Inventory".into(),
            columns: vec![
                ColumnDef {
                    name: "Item".into(),
                    cell_type: CellType::Text,
                    required: true,
                    unique: false,
                },
                ColumnDef {
                    name: "Quantity".into(),
                    cell_type: CellType::Number,
                    required: true,
                    unique: false,
                },
                ColumnDef {
                    name: "Price".into(),
                    cell_type: CellType::Number,
                    required: false,
                    unique: false,
                },
            ],
            default_view: ViewMode::Grid,
        }
    }

    fn test_cell_meta() -> CellMeta {
        CellMeta {
            address: CellAddress {
                sheet: None,
                column: "B".into(),
                row: 3,
            },
            cell_type: CellType::Number,
            value: "42".into(),
        }
    }

    #[test]
    fn sheet_round_trip() {
        let meta = test_sheet_meta();
        let digit = sheet_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "data.sheet");

        let parsed = parse_sheet_meta(&digit).unwrap();
        assert_eq!(parsed.name, meta.name);
        assert_eq!(parsed.columns.len(), 3);
        assert_eq!(parsed.columns[0].name, "Item");
        assert_eq!(parsed.columns[0].cell_type, CellType::Text);
        assert!(parsed.columns[0].required);
        assert_eq!(parsed.default_view, ViewMode::Grid);
    }

    #[test]
    fn cell_round_trip() {
        let meta = test_cell_meta();
        let digit = cell_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "data.cell");

        let parsed = parse_cell_meta(&digit).unwrap();
        assert_eq!(parsed.address.column, "B");
        assert_eq!(parsed.address.row, 3);
        assert!(parsed.address.sheet.is_none());
        assert_eq!(parsed.cell_type, CellType::Number);
        assert_eq!(parsed.value, "42");
    }

    #[test]
    fn cell_with_sheet_reference() {
        let meta = CellMeta {
            address: CellAddress {
                sheet: Some("Revenue".into()),
                column: "AA".into(),
                row: 100,
            },
            cell_type: CellType::Formula,
            value: "=SUM(A1:A10)".into(),
        };
        let digit = cell_digit(&meta, "alice").unwrap();
        let parsed = parse_cell_meta(&digit).unwrap();
        assert_eq!(parsed.address.sheet.as_deref(), Some("Revenue"));
        assert_eq!(parsed.address.column, "AA");
        assert_eq!(parsed.address.row, 100);
        assert_eq!(parsed.cell_type, CellType::Formula);
        assert!(parsed.value.starts_with('='));
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_sheet_meta(&digit).is_err());
        assert!(parse_cell_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit = Digit::new("data.sheet".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_sheet_meta(&digit).is_err());

        let digit = Digit::new("data.cell".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_cell_meta(&digit).is_err());
    }

    #[test]
    fn all_view_modes() {
        for (mode, name) in [
            (ViewMode::Grid, "grid"),
            (ViewMode::Kanban, "kanban"),
            (ViewMode::Calendar, "calendar"),
            (ViewMode::Gallery, "gallery"),
        ] {
            assert_eq!(mode.to_str(), name);
            assert_eq!(ViewMode::from_str_value(name).unwrap(), mode);
        }
    }

    #[test]
    fn all_cell_types() {
        for (ct, name) in [
            (CellType::Text, "text"),
            (CellType::Number, "number"),
            (CellType::Date, "date"),
            (CellType::Boolean, "boolean"),
            (CellType::Formula, "formula"),
            (CellType::Reference, "reference"),
            (CellType::Rich, "rich"),
        ] {
            assert_eq!(ct.to_str(), name);
            assert_eq!(CellType::from_str_value(name).unwrap(), ct);
        }
    }

    #[test]
    fn serde_round_trip() {
        let meta = test_sheet_meta();
        let digit = sheet_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_sheet_meta(&rt).unwrap();
        assert_eq!(parsed.name, "Inventory");
    }

    #[test]
    fn schema_validates_sheet() {
        let schema = sheet_schema();
        let meta = test_sheet_meta();
        let digit = sheet_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn schema_validates_cell() {
        let schema = cell_schema();
        let meta = test_cell_meta();
        let digit = cell_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn unique_column_roundtrip() {
        let meta = SheetMeta {
            name: "Users".into(),
            columns: vec![ColumnDef {
                name: "Email".into(),
                cell_type: CellType::Text,
                required: true,
                unique: true,
            }],
            default_view: ViewMode::Grid,
        };
        let digit = sheet_digit(&meta, "alice").unwrap();
        let parsed = parse_sheet_meta(&digit).unwrap();
        assert!(parsed.columns[0].unique);
    }

    #[test]
    fn cell_boolean_type() {
        let meta = CellMeta {
            address: CellAddress {
                sheet: None,
                column: "C".into(),
                row: 1,
            },
            cell_type: CellType::Boolean,
            value: "true".into(),
        };
        let digit = cell_digit(&meta, "alice").unwrap();
        let parsed = parse_cell_meta(&digit).unwrap();
        assert_eq!(parsed.cell_type, CellType::Boolean);
        assert_eq!(parsed.value, "true");
    }

    #[test]
    fn invalid_cell_address() {
        assert!(CellAddress::from_string_repr("123").is_err());
        assert!(CellAddress::from_string_repr("").is_err());
        assert!(CellAddress::from_string_repr("ABC").is_err());
    }

    #[test]
    fn invalid_view_mode() {
        assert!(ViewMode::from_str_value("unknown").is_err());
    }

    #[test]
    fn invalid_cell_type() {
        assert!(CellType::from_str_value("unknown").is_err());
    }
}
