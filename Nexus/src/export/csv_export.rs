//! CSV exporter -- converts Ideas sheet digits to CSV text.
//!
//! Finds `data.sheet` and `data.cell` digits, extracts column headers from the
//! sheet definition, and emits rows of cell values. If no sheet digit is found,
//! falls back to a simple text extraction of all digits.

use std::collections::HashMap;
use std::fmt::Write;

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas sheet digits as CSV (.csv).
///
/// Looks for `data.sheet` digits to define column structure and `data.cell`
/// digits for cell values. Falls back to a simple text dump if no sheet
/// structure is found.
///
/// # Example
///
/// ```ignore
/// let exporter = CsvExporter;
/// let config = ExportConfig::new(ExportFormat::Csv);
/// let output = exporter.export(&digits, None, &config)?;
/// let csv = String::from_utf8(output.data).unwrap();
/// ```
pub struct CsvExporter;

impl Exporter for CsvExporter {
    fn id(&self) -> &str {
        "csv"
    }

    fn display_name(&self) -> &str {
        "CSV Exporter"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Csv]
    }

    fn export(
        &self,
        digits: &[Digit],
        _root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let live_digits: Vec<&Digit> = digits.iter().filter(|d| !d.is_deleted()).collect();

        // Try structured sheet export first
        if let Some(output) = try_sheet_export(&live_digits) {
            return Ok(ExportOutput::new(
                output.into_bytes(),
                "export.csv",
                "text/csv",
            ));
        }

        // Fallback: extract text from all digits into a single-column CSV
        let output = fallback_text_export(&live_digits);
        Ok(ExportOutput::new(
            output.into_bytes(),
            "export.csv",
            "text/csv",
        ))
    }
}

/// Attempt a structured CSV export from sheet/cell digits.
fn try_sheet_export(digits: &[&Digit]) -> Option<String> {
    // Find the first sheet digit
    let sheet_digit = digits
        .iter()
        .find(|d| d.digit_type() == "data.sheet")?;

    let sheet_meta = ideas::sheet::parse_sheet_meta(sheet_digit).ok()?;

    // Collect cell digits
    let cells: Vec<ideas::CellMeta> = digits
        .iter()
        .filter(|d| d.digit_type() == "data.cell")
        .filter_map(|d| ideas::sheet::parse_cell_meta(d).ok())
        .collect();

    if sheet_meta.columns.is_empty() {
        return None;
    }

    // Build column name -> index mapping
    let col_names: Vec<&str> = sheet_meta.columns.iter().map(|c| c.name.as_str()).collect();

    // Group cells by row
    let mut row_map: HashMap<u32, HashMap<String, String>> = HashMap::new();
    for cell in &cells {
        let row_cells = row_map.entry(cell.address.row).or_default();
        row_cells.insert(cell.address.column.clone(), cell.value.clone());
    }

    let mut output = String::new();

    // Header row
    let _ = writeln!(
        output,
        "{}",
        col_names
            .iter()
            .map(|n| csv_escape(n))
            .collect::<Vec<_>>()
            .join(",")
    );

    // Data rows (sorted by row number)
    let mut rows: Vec<u32> = row_map.keys().copied().collect();
    rows.sort();

    // Build a mapping from column letter to column index
    // We generate column letters A, B, C, ... matching the column order
    for row_num in rows {
        let row_cells = &row_map[&row_num];
        let values: Vec<String> = (0..col_names.len())
            .map(|i| {
                let col_letter = column_letter(i);
                row_cells
                    .get(&col_letter)
                    .map(|v| csv_escape(v))
                    .unwrap_or_default()
            })
            .collect();
        let _ = writeln!(output, "{}", values.join(","));
    }

    Some(output)
}

/// Fallback: export all digit text as a single-column CSV.
fn fallback_text_export(digits: &[&Digit]) -> String {
    let mut output = String::from("content\n");
    for digit in digits {
        let text = digit.extract_text();
        if !text.is_empty() {
            let _ = writeln!(output, "{}", csv_escape(&text));
        }
    }
    output
}

/// Escape a value for CSV output (RFC 4180).
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Convert a 0-based column index to a column letter (A, B, ..., Z, AA, AB, ...).
fn column_letter(index: usize) -> String {
    let mut result = String::new();
    let mut n = index;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExportConfig;
    use ideas::sheet::*;
    use x::Value;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    fn test_sheet_digits() -> Vec<Digit> {
        let sheet = sheet_digit(
            &SheetMeta {
                name: "Inventory".into(),
                columns: vec![
                    ColumnDef {
                        name: "Item".into(),
                        cell_type: CellType::Text,
                        required: true,
                        unique: false,
                    },
                    ColumnDef {
                        name: "Qty".into(),
                        cell_type: CellType::Number,
                        required: true,
                        unique: false,
                    },
                ],
                default_view: ViewMode::Grid,
            },
            "test",
        )
        .unwrap();

        let cell_a1 = cell_digit(
            &CellMeta {
                address: CellAddress {
                    sheet: None,
                    column: "A".into(),
                    row: 1,
                },
                cell_type: CellType::Text,
                value: "Widget".into(),
            },
            "test",
        )
        .unwrap();

        let cell_b1 = cell_digit(
            &CellMeta {
                address: CellAddress {
                    sheet: None,
                    column: "B".into(),
                    row: 1,
                },
                cell_type: CellType::Number,
                value: "42".into(),
            },
            "test",
        )
        .unwrap();

        let cell_a2 = cell_digit(
            &CellMeta {
                address: CellAddress {
                    sheet: None,
                    column: "A".into(),
                    row: 2,
                },
                cell_type: CellType::Text,
                value: "Gadget".into(),
            },
            "test",
        )
        .unwrap();

        let cell_b2 = cell_digit(
            &CellMeta {
                address: CellAddress {
                    sheet: None,
                    column: "B".into(),
                    row: 2,
                },
                cell_type: CellType::Number,
                value: "17".into(),
            },
            "test",
        )
        .unwrap();

        vec![sheet, cell_a1, cell_b1, cell_a2, cell_b2]
    }

    #[test]
    fn exports_sheet_as_csv() {
        let digits = test_sheet_digits();
        let config = ExportConfig::new(ExportFormat::Csv);
        let output = CsvExporter.export(&digits, None, &config).unwrap();
        let csv = String::from_utf8(output.data).unwrap();

        assert!(csv.starts_with("Item,Qty"));
        assert!(csv.contains("Widget,42"));
        assert!(csv.contains("Gadget,17"));
    }

    #[test]
    fn fallback_to_text_export() {
        let digit = make_digit("text")
            .with_content(Value::String("hello world".into()), "test");
        let config = ExportConfig::new(ExportFormat::Csv);
        let output = CsvExporter.export(&[digit], None, &config).unwrap();
        let csv = String::from_utf8(output.data).unwrap();

        assert!(csv.starts_with("content"));
        assert!(csv.contains("hello world"));
    }

    #[test]
    fn csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn csv_escape_with_comma() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_escape_with_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_with_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn column_letter_conversion() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(1), "B");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(27), "AB");
    }

    #[test]
    fn skips_tombstoned_digits() {
        let digit = make_digit("text")
            .with_content(Value::String("visible".into()), "test");
        let deleted = make_digit("text")
            .with_content(Value::String("hidden".into()), "test")
            .deleted("test");

        let config = ExportConfig::new(ExportFormat::Csv);
        let output = CsvExporter
            .export(&[digit, deleted], None, &config)
            .unwrap();
        let csv = String::from_utf8(output.data).unwrap();
        assert!(csv.contains("visible"));
        assert!(!csv.contains("hidden"));
    }

    #[test]
    fn metadata() {
        assert_eq!(CsvExporter.id(), "csv");
        assert_eq!(CsvExporter.display_name(), "CSV Exporter");
        assert_eq!(CsvExporter.supported_formats(), &[ExportFormat::Csv]);
    }

    #[test]
    fn output_metadata() {
        let config = ExportConfig::new(ExportFormat::Csv);
        let output = CsvExporter.export(&[], None, &config).unwrap();
        assert_eq!(output.filename, "export.csv");
        assert_eq!(output.mime_type, "text/csv");
    }

    #[test]
    fn empty_digits_produces_header_only() {
        let config = ExportConfig::new(ExportFormat::Csv);
        let output = CsvExporter.export(&[], None, &config).unwrap();
        let csv = String::from_utf8(output.data).unwrap();
        assert!(csv.starts_with("content"));
    }
}
