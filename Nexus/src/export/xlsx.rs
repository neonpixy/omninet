//! XLSX exporter -- converts Ideas sheet/cell digits to an Excel workbook via `rust_xlsxwriter`.
//!
//! Finds `data.sheet` and `data.cell` digits, maps them to workbook sheets and
//! cells. Non-sheet digits (text, headings, etc.) are placed in a default
//! "Content" worksheet as plain text rows.

use std::collections::HashMap;

use rust_xlsxwriter::Workbook;
use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Exports Ideas digits as an Excel workbook (.xlsx).
///
/// Sheet digits (`data.sheet`) become named worksheets. Cell digits
/// (`data.cell`) are placed at their specified addresses. Any non-sheet
/// text digits are collected into a fallback "Content" sheet.
///
/// # Example
///
/// ```ignore
/// let exporter = XlsxExporter;
/// let config = ExportConfig::new(ExportFormat::Xlsx);
/// let output = exporter.export(&digits, None, &config)?;
/// std::fs::write("workbook.xlsx", &output.data)?;
/// ```
pub struct XlsxExporter;

impl Exporter for XlsxExporter {
    fn id(&self) -> &str {
        "nexus.xlsx"
    }

    fn display_name(&self) -> &str {
        "Excel Workbook"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Xlsx]
    }

    fn export(
        &self,
        digits: &[Digit],
        _root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let mut workbook = Workbook::new();

        // Separate digits by type
        let mut sheet_names: Vec<String> = Vec::new();
        let mut cells_by_sheet: HashMap<String, Vec<&Digit>> = HashMap::new();
        let mut text_rows: Vec<String> = Vec::new();

        for digit in digits {
            if digit.is_deleted() {
                continue;
            }
            match digit.digit_type() {
                "data.sheet" => {
                    let name = digit
                        .properties
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Sheet")
                        .to_string();
                    if !sheet_names.contains(&name) {
                        sheet_names.push(name);
                    }
                }
                "data.cell" => {
                    let addr_str = digit
                        .properties
                        .get("address")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let sheet_name = if let Some(idx) = addr_str.find('!') {
                        addr_str[..idx].to_string()
                    } else {
                        "Sheet1".to_string()
                    };
                    cells_by_sheet
                        .entry(sheet_name)
                        .or_default()
                        .push(digit);
                }
                _ => {
                    let text = extract_text(digit);
                    if !text.is_empty() {
                        text_rows.push(text);
                    }
                }
            }
        }

        // Create sheets from sheet digits
        for name in &sheet_names {
            let worksheet = workbook
                .add_worksheet()
                .set_name(name)
                .map_err(|e| NexusError::ExportFailed(format!("failed to set sheet name: {e}")))?;

            if let Some(cells) = cells_by_sheet.get(name) {
                for cell_digit in cells {
                    write_cell(worksheet, cell_digit);
                }
            }
        }

        // Create sheets for cells that reference sheets not explicitly defined
        for (sheet_name, cells) in &cells_by_sheet {
            if sheet_names.contains(sheet_name) {
                continue;
            }
            let worksheet = workbook
                .add_worksheet()
                .set_name(sheet_name)
                .map_err(|e| NexusError::ExportFailed(format!("failed to set sheet name: {e}")))?;

            for cell_digit in cells {
                write_cell(worksheet, cell_digit);
            }
        }

        // If there are text rows but no sheets yet, create a "Content" sheet
        if !text_rows.is_empty() {
            let worksheet = workbook
                .add_worksheet()
                .set_name("Content")
                .map_err(|e| NexusError::ExportFailed(format!("failed to set sheet name: {e}")))?;

            for (row, text) in text_rows.iter().enumerate() {
                let _ = worksheet.write_string(row as u32, 0, text);
            }
        }

        // If no content at all, ensure at least one empty sheet
        if sheet_names.is_empty() && cells_by_sheet.is_empty() && text_rows.is_empty() {
            let _ = workbook
                .add_worksheet()
                .set_name("Sheet1")
                .map_err(|e| NexusError::ExportFailed(format!("failed to set sheet name: {e}")))?;
        }

        let xlsx_bytes = workbook
            .save_to_buffer()
            .map_err(|e| NexusError::ExportFailed(format!("failed to save XLSX: {e}")))?;

        Ok(ExportOutput::new(
            xlsx_bytes,
            "export.xlsx",
            ExportFormat::Xlsx.mime_type(),
        ))
    }
}

/// Write a single cell digit to a worksheet.
fn write_cell(worksheet: &mut rust_xlsxwriter::Worksheet, digit: &Digit) {
    let addr_str = digit
        .properties
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("A1");

    // Strip sheet prefix if present
    let cell_part = if let Some(idx) = addr_str.find('!') {
        &addr_str[idx + 1..]
    } else {
        addr_str
    };

    let (col, row) = parse_cell_address(cell_part);
    let value = digit
        .properties
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let cell_type = digit
        .properties
        .get("cell_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text");

    match cell_type {
        "number" => {
            if let Ok(num) = value.parse::<f64>() {
                let _ = worksheet.write_number(row, col, num);
            } else {
                let _ = worksheet.write_string(row, col, value);
            }
        }
        "boolean" => {
            let b = value == "true" || value == "1";
            let _ = worksheet.write_boolean(row, col, b);
        }
        "formula" => {
            // Formulas start with '=' -- strip it for rust_xlsxwriter
            let formula = value.strip_prefix('=').unwrap_or(value);
            let _ = worksheet.write_formula(row, col, formula);
        }
        _ => {
            let _ = worksheet.write_string(row, col, value);
        }
    }
}

/// Parse a cell address like "A1" or "AB100" into (column_index, row_index).
/// Returns 0-based indices.
fn parse_cell_address(addr: &str) -> (u16, u32) {
    let col_end = addr
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(addr.len());
    let col_str = &addr[..col_end];
    let row_str = &addr[col_end..];

    let mut col: u16 = 0;
    for c in col_str.chars() {
        col = col * 26 + (c.to_ascii_uppercase() as u16 - b'A' as u16 + 1);
    }
    // Convert from 1-based to 0-based
    let col = col.saturating_sub(1);

    let row: u32 = row_str.parse::<u32>().unwrap_or(1).saturating_sub(1);

    (col, row)
}

/// Extract text from any digit for the fallback "Content" sheet.
fn extract_text(digit: &Digit) -> String {
    digit
        .properties
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| digit.content.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExportConfig;
    use ideas::richtext::*;
    use ideas::sheet::*;

    #[test]
    fn metadata() {
        assert_eq!(XlsxExporter.id(), "nexus.xlsx");
        assert_eq!(XlsxExporter.display_name(), "Excel Workbook");
        assert_eq!(XlsxExporter.supported_formats(), &[ExportFormat::Xlsx]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Xlsx);
        let output = XlsxExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.xlsx");
    }

    #[test]
    fn exports_sheet_with_cells() {
        let sheet = sheet_digit(
            &SheetMeta {
                name: "Inventory".into(),
                columns: vec![ColumnDef {
                    name: "Item".into(),
                    cell_type: CellType::Text,
                    required: true,
                    unique: false,
                }],
                default_view: ViewMode::Grid,
            },
            "test",
        )
        .unwrap();
        let cell = cell_digit(
            &CellMeta {
                address: CellAddress {
                    sheet: Some("Inventory".into()),
                    column: "A".into(),
                    row: 1,
                },
                cell_type: CellType::Text,
                value: "Widget".into(),
            },
            "test",
        )
        .unwrap();

        let config = ExportConfig::new(ExportFormat::Xlsx);
        let output = XlsxExporter
            .export(&[sheet, cell], None, &config)
            .unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn exports_text_as_content_sheet() {
        let p = paragraph_digit(
            &ParagraphMeta {
                text: "Hello world".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Xlsx);
        let output = XlsxExporter.export(&[p], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn skips_tombstoned() {
        let deleted = paragraph_digit(
            &ParagraphMeta {
                text: "gone".into(),
                spans: None,
            },
            "test",
        )
        .unwrap()
        .deleted("test");
        let config = ExportConfig::new(ExportFormat::Xlsx);
        let output = XlsxExporter.export(&[deleted], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn parse_cell_address_a1() {
        let (col, row) = parse_cell_address("A1");
        assert_eq!(col, 0);
        assert_eq!(row, 0);
    }

    #[test]
    fn parse_cell_address_b3() {
        let (col, row) = parse_cell_address("B3");
        assert_eq!(col, 1);
        assert_eq!(row, 2);
    }

    #[test]
    fn parse_cell_address_aa100() {
        let (col, row) = parse_cell_address("AA100");
        assert_eq!(col, 26);
        assert_eq!(row, 99);
    }
}
