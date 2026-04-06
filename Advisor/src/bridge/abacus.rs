//! Abacus bridge — translates abacus.* SkillCalls into Magic Actions or DirectResults.

use ideas::sheet::{self, CellMeta, CellType, ColumnDef, SheetMeta, ViewMode};
use ideas::Digit;
use magic::Action;
use uuid::Uuid;
use x::Value;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Abacus spreadsheet skills.
pub struct AbacusBridge;

impl ActionBridge for AbacusBridge {
    fn program_prefix(&self) -> &str {
        "abacus"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "abacus.createSheet" => translate_create_sheet(call),
            "abacus.addRow" => translate_add_row(call),
            "abacus.addColumn" => translate_add_column(call),
            "abacus.setCell" => translate_set_cell(call),
            "abacus.applyFormula" => translate_apply_formula(call),
            "abacus.createView" => translate_create_view(call),
            "abacus.filter" => translate_filter(call),
            "abacus.sort" => translate_sort(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_create_sheet(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let name = call.get_string("name")?;
    let columns_json = call.get_string("columns")?;
    let default_view_str = call
        .get_string_opt("default_view")
        .unwrap_or_else(|| "grid".to_string());
    let parent_id = parse_optional_uuid(call.get_string_opt("parent_id"))?;

    let columns: Vec<ColumnDef> = serde_json::from_str(&columns_json)
        .map_err(|e| AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid columns JSON: {e}"),
        })?;

    let default_view = parse_view_mode(&default_view_str)?;

    let meta = SheetMeta {
        name,
        columns,
        default_view,
    };

    let digit = sheet::sheet_digit(&meta, "advisor").map_err(|e| AdvisorError::SkillFailed {
        id: call.skill_id.clone(),
        reason: e.to_string(),
    })?;

    Ok(BridgeOutput::Action(Action::insert(digit, parent_id)))
}

fn translate_add_row(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let sheet_id = parse_uuid(&call.get_string("sheet_id")?)?;

    let author = "advisor";
    let mut digit = Digit::new("data.row".into(), Value::Null, author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;

    // If values are provided, store them as properties on the row digit.
    if let Some(values_json) = call.get_string_opt("values") {
        let values: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&values_json).map_err(|e| {
                AdvisorError::InvalidSkillParameters {
                    id: call.skill_id.clone(),
                    reason: format!("invalid values JSON: {e}"),
                }
            })?;
        for (key, val) in values {
            let value = json_to_value(&val);
            digit = digit.with_property(key, value, author);
        }
    }

    Ok(BridgeOutput::Action(Action::insert(digit, Some(sheet_id))))
}

fn translate_add_column(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let sheet_id = parse_uuid(&call.get_string("sheet_id")?)?;
    let column_name = call.get_string("name")?;
    let cell_type = call.get_string("cell_type")?;

    // Adding a column is an update to the sheet's column definitions.
    let new_col = ColumnDef {
        name: column_name,
        cell_type: parse_cell_type(&cell_type)?,
        required: call
            .get_number_opt("required")
            .map(|v| v != 0.0)
            .unwrap_or(false),
        unique: call
            .get_number_opt("unique")
            .map(|v| v != 0.0)
            .unwrap_or(false),
    };

    let col_json = serde_json::to_string(&new_col).map_err(AdvisorError::from)?;

    Ok(BridgeOutput::Action(Action::update(
        sheet_id,
        "add_column",
        Value::Null,
        Value::String(col_json),
    )))
}

fn translate_set_cell(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let sheet_id = parse_uuid(&call.get_string("sheet_id")?)?;
    let address = call.get_string("address")?;
    let value = call.get_string("value")?;
    let cell_type_str = call
        .get_string_opt("cell_type")
        .unwrap_or_else(|| {
            if value.starts_with('=') {
                "formula".to_string()
            } else {
                "text".to_string()
            }
        });

    let cell_type = parse_cell_type(&cell_type_str)?;
    let cell_address = ideas::sheet::CellAddress {
        sheet: None,
        column: address
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect(),
        row: address
            .chars()
            .skip_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .parse()
            .map_err(|_| AdvisorError::InvalidSkillParameters {
                id: call.skill_id.clone(),
                reason: format!("invalid cell address: {address}"),
            })?,
    };

    let meta = CellMeta {
        address: cell_address,
        cell_type,
        value,
    };

    let digit =
        sheet::cell_digit(&meta, "advisor").map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(sheet_id),
    )))
}

fn translate_apply_formula(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let sheet_id = parse_uuid(&call.get_string("sheet_id")?)?;
    let address = call.get_string("address")?;
    let formula = call.get_string("formula")?;

    // Applying a formula is just setting a cell with type=formula.
    let cell_address = ideas::sheet::CellAddress {
        sheet: None,
        column: address
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect(),
        row: address
            .chars()
            .skip_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .parse()
            .map_err(|_| AdvisorError::InvalidSkillParameters {
                id: call.skill_id.clone(),
                reason: format!("invalid cell address: {address}"),
            })?,
    };

    let meta = CellMeta {
        address: cell_address,
        cell_type: CellType::Formula,
        value: formula,
    };

    let digit =
        sheet::cell_digit(&meta, "advisor").map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(sheet_id),
    )))
}

fn translate_create_view(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let sheet_id = parse_uuid(&call.get_string("sheet_id")?)?;
    let view_mode = call.get_string("view_mode")?;

    // View creation is a metadata update on the sheet.
    Ok(BridgeOutput::Action(Action::update(
        sheet_id,
        "default_view",
        Value::Null,
        Value::String(view_mode),
    )))
}

fn translate_filter(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let _sheet_id = call.get_string("sheet_id")?;
    let column = call.get_string("column")?;
    let operator = call.get_string("operator")?;
    let value = call.get_string("value")?;

    // Filtering is a read-only view operation.
    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Filter applied: {column} {operator} {value}"
        ))
        .with_data("column", column)
        .with_data("operator", operator)
        .with_data("value", value),
    ))
}

fn translate_sort(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let _sheet_id = call.get_string("sheet_id")?;
    let column = call.get_string("column")?;
    let direction = call
        .get_string_opt("direction")
        .unwrap_or_else(|| "asc".to_string());

    // Sorting is a read-only view operation.
    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!("Sort applied: {column} {direction}"))
            .with_data("column", column)
            .with_data("direction", direction),
    ))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AdvisorError> {
    Uuid::parse_str(s).map_err(|e| AdvisorError::InvalidSkillParameters {
        id: "abacus".into(),
        reason: format!("invalid UUID: {e}"),
    })
}

fn parse_optional_uuid(s: Option<String>) -> Result<Option<Uuid>, AdvisorError> {
    s.map(|v| parse_uuid(&v)).transpose()
}

fn parse_view_mode(s: &str) -> Result<ViewMode, AdvisorError> {
    match s {
        "grid" => Ok(ViewMode::Grid),
        "kanban" => Ok(ViewMode::Kanban),
        "calendar" => Ok(ViewMode::Calendar),
        "gallery" => Ok(ViewMode::Gallery),
        other => Err(AdvisorError::InvalidSkillParameters {
            id: "abacus".into(),
            reason: format!("unknown view mode: {other}"),
        }),
    }
}

fn parse_cell_type(s: &str) -> Result<CellType, AdvisorError> {
    match s {
        "text" => Ok(CellType::Text),
        "number" => Ok(CellType::Number),
        "date" => Ok(CellType::Date),
        "boolean" => Ok(CellType::Boolean),
        "formula" => Ok(CellType::Formula),
        "reference" => Ok(CellType::Reference),
        "rich" => Ok(CellType::Rich),
        other => Err(AdvisorError::InvalidSkillParameters {
            id: "abacus".into(),
            reason: format!("unknown cell type: {other}"),
        }),
    }
}

fn json_to_value(val: &serde_json::Value) -> Value {
    match val {
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Double(f)
            } else {
                Value::String(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Null => Value::Null,
        other => Value::String(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sheet_produces_insert_action() {
        let call = SkillCall::new("c1", "abacus.createSheet")
            .with_argument("name", serde_json::Value::String("Inventory".into()))
            .with_argument(
                "columns",
                serde_json::Value::String(
                    r#"[{"name":"Item","cell_type":"text","required":true,"unique":false}]"#.into(),
                ),
            );

        let result = AbacusBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::InsertDigit { .. })));
    }

    #[test]
    fn set_cell_produces_insert_action() {
        let sheet_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "abacus.setCell")
            .with_argument("sheet_id", serde_json::Value::String(sheet_id.to_string()))
            .with_argument("address", serde_json::Value::String("A1".into()))
            .with_argument("value", serde_json::Value::String("Hello".into()));

        let result = AbacusBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::InsertDigit { .. })));
    }

    #[test]
    fn filter_produces_direct_result() {
        let sheet_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "abacus.filter")
            .with_argument("sheet_id", serde_json::Value::String(sheet_id.to_string()))
            .with_argument("column", serde_json::Value::String("Price".into()))
            .with_argument("operator", serde_json::Value::String("gt".into()))
            .with_argument("value", serde_json::Value::String("100".into()));

        let result = AbacusBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn sort_produces_direct_result() {
        let sheet_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "abacus.sort")
            .with_argument("sheet_id", serde_json::Value::String(sheet_id.to_string()))
            .with_argument("column", serde_json::Value::String("Name".into()));

        let result = AbacusBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }
}
