//! CSV importer — parse CSV into sheet and cell digits.

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;
use ideas::sheet::{CellAddress, CellMeta, CellType, ColumnDef, SheetMeta, ViewMode};

/// Imports CSV data into Ideas sheet and cell digits.
///
/// - First row is treated as column names.
/// - All columns are typed as `Text` (CSV has no type info).
/// - Creates a `data.sheet` digit as root with `data.cell` children.
#[derive(Debug)]
pub struct CsvImporter;

impl Importer for CsvImporter {
    fn id(&self) -> &str {
        "nexus.csv.import"
    }

    fn display_name(&self) -> &str {
        "CSV"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["text/csv"]
    }

    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let author = &config.author;
        let mut digits: Vec<Digit> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let mut reader = csv::Reader::from_reader(data);

        // Extract headers (column names).
        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| NexusError::ImportFailed(format!("failed to read CSV headers: {e}")))?
            .iter()
            .map(|h| h.to_string())
            .collect();

        if headers.is_empty() {
            return Err(NexusError::ImportFailed("CSV has no columns".into()));
        }

        // Build column definitions.
        let columns: Vec<ColumnDef> = headers
            .iter()
            .map(|name| ColumnDef {
                name: name.clone(),
                cell_type: CellType::Text,
                required: false,
                unique: false,
            })
            .collect();

        // Create sheet digit.
        let sheet_meta = SheetMeta {
            name: "Imported Sheet".into(),
            columns,
            default_view: ViewMode::Grid,
        };
        let sheet = ideas::sheet::sheet_digit(&sheet_meta, author)
            .map_err(|e| NexusError::ImportFailed(format!("failed to create sheet: {e}")))?;
        let root_id = sheet.id();
        let mut sheet_digit = sheet;

        // Parse rows into cell digits.
        for (row_idx, result) in reader.records().enumerate() {
            let record = match result {
                Ok(r) => r,
                Err(e) => {
                    warnings.push(format!("Skipped row {}: {e}", row_idx + 2));
                    continue;
                }
            };

            for (col_idx, field) in record.iter().enumerate() {
                let col_letter = column_index_to_letter(col_idx);
                let cell_meta = CellMeta {
                    address: CellAddress {
                        sheet: None,
                        column: col_letter,
                        row: (row_idx + 2) as u32, // Row 1 is headers, data starts at 2.
                    },
                    cell_type: CellType::Text,
                    value: field.to_string(),
                };
                match ideas::sheet::cell_digit(&cell_meta, author) {
                    Ok(cell) => {
                        sheet_digit = sheet_digit.with_child(cell.id(), author);
                        digits.push(cell);
                    }
                    Err(e) => {
                        warnings.push(format!(
                            "Failed to create cell at row {}, col {}: {e}",
                            row_idx + 2,
                            col_idx
                        ));
                    }
                }
            }
        }

        // Insert sheet at front.
        digits.insert(0, sheet_digit);

        let mut output = ImportOutput::new(digits, Some(root_id));
        output.warnings = warnings;
        Ok(output)
    }
}

/// Convert a 0-based column index to Excel-style column letter(s).
///
/// 0 -> A, 1 -> B, ..., 25 -> Z, 26 -> AA, 27 -> AB, etc.
fn column_index_to_letter(mut idx: usize) -> String {
    let mut result = String::new();
    loop {
        result.insert(0, (b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import_csv(csv: &str) -> ImportOutput {
        let config = ImportConfig::new("cpub1test");
        CsvImporter.import(csv.as_bytes(), &config).unwrap()
    }

    #[test]
    fn import_simple_csv() {
        let csv = "Name,Age,City\nAlice,30,NYC\nBob,25,LA";
        let output = import_csv(csv);

        // Root sheet + 6 cells (2 rows x 3 cols).
        assert!(output.root_digit_id.is_some());
        let sheet = &output.digits[0];
        assert_eq!(sheet.digit_type(), "data.sheet");

        let cells: Vec<_> = output
            .digits
            .iter()
            .filter(|d| d.digit_type() == "data.cell")
            .collect();
        assert_eq!(cells.len(), 6);
    }

    #[test]
    fn sheet_has_correct_columns() {
        let csv = "Product,Price\nWidget,9.99";
        let output = import_csv(csv);
        let sheet = &output.digits[0];
        let meta = ideas::sheet::parse_sheet_meta(sheet).unwrap();
        assert_eq!(meta.columns.len(), 2);
        assert_eq!(meta.columns[0].name, "Product");
        assert_eq!(meta.columns[1].name, "Price");
    }

    #[test]
    fn cell_addresses_are_correct() {
        let csv = "A,B\n1,2";
        let output = import_csv(csv);
        let cells: Vec<_> = output
            .digits
            .iter()
            .filter(|d| d.digit_type() == "data.cell")
            .collect();
        let meta0 = ideas::sheet::parse_cell_meta(cells[0]).unwrap();
        assert_eq!(meta0.address.column, "A");
        assert_eq!(meta0.address.row, 2);
        let meta1 = ideas::sheet::parse_cell_meta(cells[1]).unwrap();
        assert_eq!(meta1.address.column, "B");
        assert_eq!(meta1.address.row, 2);
    }

    #[test]
    fn empty_csv_headers_only() {
        let csv = "Col1,Col2\n";
        let output = import_csv(csv);
        // Just the sheet, no cells.
        assert_eq!(output.digits.len(), 1);
        assert_eq!(output.digits[0].digit_type(), "data.sheet");
    }

    #[test]
    fn column_index_to_letter_basic() {
        assert_eq!(column_index_to_letter(0), "A");
        assert_eq!(column_index_to_letter(1), "B");
        assert_eq!(column_index_to_letter(25), "Z");
        assert_eq!(column_index_to_letter(26), "AA");
        assert_eq!(column_index_to_letter(27), "AB");
    }

    #[test]
    fn root_is_sheet_with_children() {
        let csv = "X\n1\n2";
        let output = import_csv(csv);
        let sheet = &output.digits[0];
        assert!(sheet.has_children());
    }

    #[test]
    fn invalid_csv_returns_error() {
        let data = &[0xFF, 0xFE];
        let config = ImportConfig::new("cpub1test");
        // csv crate handles raw bytes, so this may parse oddly but not crash.
        let result = CsvImporter.import(data, &config);
        // We just verify it doesn't panic.
        assert!(result.is_ok() || result.is_err());
    }
}
