//! ODS exporter -- converts Ideas sheet/cell digits to an ODF spreadsheet.
//!
//! Produces a minimal valid ODS (Open Document Format) file by building
//! the required ZIP structure with `mimetype`, `META-INF/manifest.xml`,
//! and `content.xml`. Maps sheet and cell digits to ODF table elements.

use std::collections::HashMap;
use std::io::{Cursor, Write};

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;

/// Maximum rows to pre-allocate in the grid for cell placement.
const MAX_ROWS: usize = 1000;
/// Maximum columns to pre-allocate.
const MAX_COLS: usize = 26;

/// Exports Ideas digits as an ODS (LibreOffice Calc) spreadsheet.
///
/// Sheet digits (`data.sheet`) become named tables. Cell digits
/// (`data.cell`) are placed at their specified addresses. Text-only
/// digits go into a fallback "Content" table.
///
/// # Example
///
/// ```ignore
/// let exporter = OdsExporter;
/// let config = ExportConfig::new(ExportFormat::Ods);
/// let output = exporter.export(&digits, None, &config)?;
/// ```
pub struct OdsExporter;

impl Exporter for OdsExporter {
    fn id(&self) -> &str {
        "nexus.ods"
    }

    fn display_name(&self) -> &str {
        "ODF Spreadsheet"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Ods]
    }

    fn export(
        &self,
        digits: &[Digit],
        _root_id: Option<Uuid>,
        _config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        // Separate digits by type
        let mut sheet_names: Vec<String> = Vec::new();
        let mut cells_by_sheet: HashMap<String, Vec<CellData>> = HashMap::new();
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
                    if let Some(cell) = parse_cell(digit) {
                        cells_by_sheet
                            .entry(cell.sheet.clone())
                            .or_default()
                            .push(cell);
                    }
                }
                _ => {
                    let text = extract_text(digit);
                    if !text.is_empty() {
                        text_rows.push(text);
                    }
                }
            }
        }

        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);

        let stored = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // mimetype (uncompressed, per ODF spec)
        zip.start_file("mimetype", stored).map_err(zip_err)?;
        zip.write_all(b"application/vnd.oasis.opendocument.spreadsheet").map_err(zip_err)?;

        // META-INF/manifest.xml
        zip.start_file("META-INF/manifest.xml", deflated).map_err(zip_err)?;
        zip.write_all(manifest_xml().as_bytes()).map_err(zip_err)?;

        // content.xml
        let content = build_content_xml(&sheet_names, &cells_by_sheet, &text_rows);
        zip.start_file("content.xml", deflated).map_err(zip_err)?;
        zip.write_all(content.as_bytes()).map_err(zip_err)?;

        let result = zip.finish().map_err(zip_err)?;

        Ok(ExportOutput::new(
            result.into_inner(),
            "export.ods",
            ExportFormat::Ods.mime_type(),
        ))
    }
}

struct CellData {
    sheet: String,
    col: usize,
    row: usize,
    value: String,
    cell_type: String,
}

fn parse_cell(digit: &Digit) -> Option<CellData> {
    let addr_str = digit
        .properties
        .get("address")
        .and_then(|v| v.as_str())?;
    let (sheet, cell_part) = if let Some(idx) = addr_str.find('!') {
        (addr_str[..idx].to_string(), &addr_str[idx + 1..])
    } else {
        ("Sheet1".to_string(), addr_str)
    };
    let (col, row) = parse_cell_address(cell_part);
    let value = digit
        .properties
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cell_type = digit
        .properties
        .get("cell_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();
    Some(CellData {
        sheet,
        col,
        row,
        value,
        cell_type,
    })
}

fn parse_cell_address(addr: &str) -> (usize, usize) {
    let col_end = addr
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(addr.len());
    let col_str = &addr[..col_end];
    let row_str = &addr[col_end..];

    let mut col: usize = 0;
    for c in col_str.chars() {
        col = col * 26 + (c.to_ascii_uppercase() as usize - b'A' as usize + 1);
    }
    let col = col.saturating_sub(1);
    let row: usize = row_str.parse::<usize>().unwrap_or(1).saturating_sub(1);
    (col, row)
}

fn zip_err(e: impl std::fmt::Display) -> NexusError {
    NexusError::ExportFailed(format!("ODS zip error: {e}"))
}

fn manifest_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#.to_string()
}

fn build_content_xml(
    sheet_names: &[String],
    cells_by_sheet: &HashMap<String, Vec<CellData>>,
    text_rows: &[String],
) -> String {
    let mut buf = Vec::new();
    let mut writer = Writer::new_with_indent(Cursor::new(&mut buf), b' ', 2);

    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));

    let mut root = BytesStart::new("office:document-content");
    root.push_attribute(("xmlns:office", "urn:oasis:names:tc:opendocument:xmlns:office:1.0"));
    root.push_attribute(("xmlns:text", "urn:oasis:names:tc:opendocument:xmlns:text:1.0"));
    root.push_attribute(("xmlns:table", "urn:oasis:names:tc:opendocument:xmlns:table:1.0"));
    root.push_attribute(("xmlns:fo", "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"));
    root.push_attribute(("office:version", "1.2"));
    let _ = writer.write_event(Event::Start(root));

    let _ = writer.write_event(Event::Start(BytesStart::new("office:body")));
    let _ = writer.write_event(Event::Start(BytesStart::new("office:spreadsheet")));

    // Emit named sheets
    for name in sheet_names {
        emit_table(&mut writer, name, cells_by_sheet.get(name));
    }

    // Emit tables for cells with no explicit sheet digit
    for (sheet_name, cells) in cells_by_sheet {
        if sheet_names.contains(sheet_name) {
            continue;
        }
        emit_table(&mut writer, sheet_name, Some(cells));
    }

    // Text-only fallback
    if !text_rows.is_empty() {
        emit_text_table(&mut writer, text_rows);
    }

    // Ensure at least one table
    if sheet_names.is_empty() && cells_by_sheet.is_empty() && text_rows.is_empty() {
        emit_table(&mut writer, "Sheet1", None);
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("office:spreadsheet")));
    let _ = writer.write_event(Event::End(BytesEnd::new("office:body")));
    let _ = writer.write_event(Event::End(BytesEnd::new("office:document-content")));

    String::from_utf8(buf).unwrap_or_default()
}

fn emit_table(
    writer: &mut Writer<Cursor<&mut Vec<u8>>>,
    name: &str,
    cells: Option<&Vec<CellData>>,
) {
    let mut table = BytesStart::new("table:table");
    table.push_attribute(("table:name", name));
    let _ = writer.write_event(Event::Start(table));

    if let Some(cells) = cells {
        // Find grid bounds
        let max_row = cells.iter().map(|c| c.row).max().unwrap_or(0).min(MAX_ROWS - 1);
        let max_col = cells.iter().map(|c| c.col).max().unwrap_or(0).min(MAX_COLS - 1);

        // Columns
        for _ in 0..=max_col {
            let _ = writer.write_event(Event::Empty(BytesStart::new("table:table-column")));
        }

        // Rows
        for row_idx in 0..=max_row {
            let _ = writer.write_event(Event::Start(BytesStart::new("table:table-row")));
            for col_idx in 0..=max_col {
                if let Some(cell) = cells.iter().find(|c| c.row == row_idx && c.col == col_idx) {
                    emit_cell(writer, cell);
                } else {
                    let _ = writer.write_event(Event::Empty(BytesStart::new("table:table-cell")));
                }
            }
            let _ = writer.write_event(Event::End(BytesEnd::new("table:table-row")));
        }
    } else {
        // Empty table with one column and one empty row
        let _ = writer.write_event(Event::Empty(BytesStart::new("table:table-column")));
        let _ = writer.write_event(Event::Start(BytesStart::new("table:table-row")));
        let _ = writer.write_event(Event::Empty(BytesStart::new("table:table-cell")));
        let _ = writer.write_event(Event::End(BytesEnd::new("table:table-row")));
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("table:table")));
}

fn emit_cell(writer: &mut Writer<Cursor<&mut Vec<u8>>>, cell: &CellData) {
    let mut tc = BytesStart::new("table:table-cell");
    match cell.cell_type.as_str() {
        "number" => {
            tc.push_attribute(("office:value-type", "float"));
            tc.push_attribute(("office:value", cell.value.as_str()));
        }
        "boolean" => {
            tc.push_attribute(("office:value-type", "boolean"));
            let bool_val = if cell.value == "true" || cell.value == "1" {
                "true"
            } else {
                "false"
            };
            tc.push_attribute(("office:boolean-value", bool_val));
        }
        _ => {
            tc.push_attribute(("office:value-type", "string"));
        }
    }
    let _ = writer.write_event(Event::Start(tc));
    let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
    let _ = writer.write_event(Event::Text(BytesText::new(&cell.value)));
    let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
    let _ = writer.write_event(Event::End(BytesEnd::new("table:table-cell")));
}

fn emit_text_table(writer: &mut Writer<Cursor<&mut Vec<u8>>>, text_rows: &[String]) {
    let mut table = BytesStart::new("table:table");
    table.push_attribute(("table:name", "Content"));
    let _ = writer.write_event(Event::Start(table));
    let _ = writer.write_event(Event::Empty(BytesStart::new("table:table-column")));

    for text in text_rows {
        let _ = writer.write_event(Event::Start(BytesStart::new("table:table-row")));
        let mut tc = BytesStart::new("table:table-cell");
        tc.push_attribute(("office:value-type", "string"));
        let _ = writer.write_event(Event::Start(tc));
        let _ = writer.write_event(Event::Start(BytesStart::new("text:p")));
        let _ = writer.write_event(Event::Text(BytesText::new(text)));
        let _ = writer.write_event(Event::End(BytesEnd::new("text:p")));
        let _ = writer.write_event(Event::End(BytesEnd::new("table:table-cell")));
        let _ = writer.write_event(Event::End(BytesEnd::new("table:table-row")));
    }

    let _ = writer.write_event(Event::End(BytesEnd::new("table:table")));
}

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
        assert_eq!(OdsExporter.id(), "nexus.ods");
        assert_eq!(OdsExporter.display_name(), "ODF Spreadsheet");
        assert_eq!(OdsExporter.supported_formats(), &[ExportFormat::Ods]);
    }

    #[test]
    fn exports_empty_digits() {
        let config = ExportConfig::new(ExportFormat::Ods);
        let output = OdsExporter.export(&[], None, &config).unwrap();
        assert!(!output.data.is_empty());
        assert_eq!(output.filename, "export.ods");
        assert_eq!(&output.data[..2], b"PK");
    }

    #[test]
    fn exports_sheet_with_cells() {
        let sheet = sheet_digit(
            &SheetMeta {
                name: "Data".into(),
                columns: vec![ColumnDef {
                    name: "Name".into(),
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
                    sheet: Some("Data".into()),
                    column: "A".into(),
                    row: 1,
                },
                cell_type: CellType::Text,
                value: "Hello".into(),
            },
            "test",
        )
        .unwrap();

        let config = ExportConfig::new(ExportFormat::Ods);
        let output = OdsExporter.export(&[sheet, cell], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn exports_text_as_content_table() {
        let p = paragraph_digit(
            &ParagraphMeta {
                text: "Hello".into(),
                spans: None,
            },
            "test",
        )
        .unwrap();
        let config = ExportConfig::new(ExportFormat::Ods);
        let output = OdsExporter.export(&[p], None, &config).unwrap();
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
        let config = ExportConfig::new(ExportFormat::Ods);
        let output = OdsExporter.export(&[deleted], None, &config).unwrap();
        assert!(!output.data.is_empty());
    }

    #[test]
    fn parse_cell_address_simple() {
        let (col, row) = parse_cell_address("A1");
        assert_eq!(col, 0);
        assert_eq!(row, 0);
    }

    #[test]
    fn parse_cell_address_multi_col() {
        let (col, row) = parse_cell_address("AB5");
        assert_eq!(col, 27);
        assert_eq!(row, 4);
    }
}
