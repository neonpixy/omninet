//! XLSX importer — parse Excel workbooks into sheet and cell digits using `calamine`.

use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use std::io::Cursor;

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;
use ideas::sheet::{CellAddress, CellMeta, CellType, ColumnDef, SheetMeta, ViewMode};

/// Imports XLSX (Excel) workbooks into Ideas sheet and cell digits.
///
/// - Each worksheet becomes a `data.sheet` digit.
/// - First row of each sheet is treated as column names.
/// - Cell types are inferred from calamine's `Data` variants.
/// - Supports multiple sheets; the first sheet is the root.
#[derive(Debug)]
pub struct XlsxImporter;

impl Importer for XlsxImporter {
    fn id(&self) -> &str {
        "nexus.xlsx.import"
    }

    fn display_name(&self) -> &str {
        "Excel (XLSX)"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"]
    }

    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let author = &config.author;
        let mut digits: Vec<Digit> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut first_sheet_id = None;
        // Track where to insert each sheet (before its cells).
        let mut sheet_insert_pos: usize = 0;

        let cursor = Cursor::new(data);
        let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
            .map_err(|e| NexusError::ImportFailed(format!("failed to open XLSX: {e}")))?;

        let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

        for sheet_name in &sheet_names {
            let range = match workbook.worksheet_range(sheet_name) {
                Ok(r) => r,
                Err(e) => {
                    warnings.push(format!("Skipped sheet '{sheet_name}': {e}"));
                    continue;
                }
            };

            let rows: Vec<Vec<Data>> = range.rows().map(|r| r.to_vec()).collect();
            if rows.is_empty() {
                warnings.push(format!("Sheet '{sheet_name}' is empty"));
                continue;
            }

            // First row = column headers.
            let headers: Vec<String> = rows[0].iter().map(data_to_string).collect();
            let columns: Vec<ColumnDef> = headers
                .iter()
                .map(|name| ColumnDef {
                    name: name.clone(),
                    cell_type: CellType::Text,
                    required: false,
                    unique: false,
                })
                .collect();

            let sheet_meta = SheetMeta {
                name: sheet_name.clone(),
                columns,
                default_view: ViewMode::Grid,
            };

            let sheet_digit_result = ideas::sheet::sheet_digit(&sheet_meta, author)
                .map_err(|e| NexusError::ImportFailed(format!("failed to create sheet: {e}")))?;
            let sheet_id = sheet_digit_result.id();
            let mut current_sheet = sheet_digit_result;

            if first_sheet_id.is_none() {
                first_sheet_id = Some(sheet_id);
            }

            // Data rows (skip header row).
            for (row_idx, row) in rows.iter().enumerate().skip(1) {
                for (col_idx, cell_data) in row.iter().enumerate() {
                    let col_letter = column_index_to_letter(col_idx);
                    let (cell_type, value_str) = classify_data(cell_data);
                    let cell_meta = CellMeta {
                        address: CellAddress {
                            sheet: Some(sheet_name.clone()),
                            column: col_letter,
                            row: (row_idx + 1) as u32,
                        },
                        cell_type,
                        value: value_str,
                    };
                    match ideas::sheet::cell_digit(&cell_meta, author) {
                        Ok(cell) => {
                            current_sheet = current_sheet.with_child(cell.id(), author);
                            digits.push(cell);
                        }
                        Err(e) => {
                            warnings.push(format!(
                                "Failed to create cell at {sheet_name}!{}{}: {e}",
                                column_index_to_letter(col_idx),
                                row_idx + 1
                            ));
                        }
                    }
                }
            }

            digits.insert(sheet_insert_pos, current_sheet);
            // Next sheet should come after this sheet and all its cells.
            sheet_insert_pos = digits.len();
        }

        let mut output = ImportOutput::new(digits, first_sheet_id);
        output.warnings = warnings;
        Ok(output)
    }
}

/// Convert calamine `Data` to a display string.
fn data_to_string(data: &Data) -> String {
    match data {
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR({e:?})"),
        Data::Empty => String::new(),
    }
}

/// Classify calamine `Data` into an Ideas `CellType` and string value.
fn classify_data(data: &Data) -> (CellType, String) {
    match data {
        Data::Int(i) => (CellType::Number, i.to_string()),
        Data::Float(f) => (CellType::Number, f.to_string()),
        Data::Bool(b) => (CellType::Boolean, b.to_string()),
        Data::DateTime(dt) => (CellType::Date, dt.to_string()),
        Data::DateTimeIso(s) => (CellType::Date, s.clone()),
        Data::String(s) => (CellType::Text, s.clone()),
        Data::DurationIso(s) => (CellType::Text, s.clone()),
        Data::Error(e) => (CellType::Text, format!("#ERR({e:?})")),
        Data::Empty => (CellType::Text, String::new()),
    }
}

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

    #[test]
    fn column_index_to_letter_works() {
        assert_eq!(column_index_to_letter(0), "A");
        assert_eq!(column_index_to_letter(25), "Z");
        assert_eq!(column_index_to_letter(26), "AA");
    }

    #[test]
    fn classify_data_int() {
        let (ct, v) = classify_data(&Data::Int(42));
        assert_eq!(ct, CellType::Number);
        assert_eq!(v, "42");
    }

    #[test]
    fn classify_data_float() {
        let (ct, v) = classify_data(&Data::Float(3.15));
        assert_eq!(ct, CellType::Number);
        assert!(v.starts_with("3.15"));
    }

    #[test]
    fn classify_data_bool() {
        let (ct, v) = classify_data(&Data::Bool(true));
        assert_eq!(ct, CellType::Boolean);
        assert_eq!(v, "true");
    }

    #[test]
    fn classify_data_string() {
        let (ct, v) = classify_data(&Data::String("hello".into()));
        assert_eq!(ct, CellType::Text);
        assert_eq!(v, "hello");
    }

    #[test]
    fn classify_data_empty() {
        let (ct, v) = classify_data(&Data::Empty);
        assert_eq!(ct, CellType::Text);
        assert!(v.is_empty());
    }

    #[test]
    fn data_to_string_variants() {
        assert_eq!(data_to_string(&Data::Int(5)), "5");
        assert_eq!(data_to_string(&Data::String("hi".into())), "hi");
        assert_eq!(data_to_string(&Data::Bool(false)), "false");
        assert!(data_to_string(&Data::Empty).is_empty());
    }

    #[test]
    fn invalid_xlsx_bytes() {
        let config = ImportConfig::new("cpub1test");
        let result = XlsxImporter.import(b"not an xlsx", &config);
        assert!(result.is_err());
    }
}
