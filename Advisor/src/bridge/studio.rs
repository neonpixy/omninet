//! Studio bridge — translates studio.* SkillCalls into Magic Actions.

use ideas::Digit;
use magic::Action;
use uuid::Uuid;
use x::Value;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Studio design skills.
pub struct StudioBridge;

impl ActionBridge for StudioBridge {
    fn program_prefix(&self) -> &str {
        "studio"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "studio.createFrame" => translate_create_frame(call),
            "studio.addShape" => translate_add_shape(call),
            "studio.setFill" => translate_set_fill(call),
            "studio.setText" => translate_set_text(call),
            "studio.applyComponentStyle" => translate_apply_component_style(call),
            "studio.connectToDataSource" => translate_connect_to_data_source(call),
            "studio.exportAs" => translate_export_as(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_create_frame(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let x = call.get_number("x")?;
    let y = call.get_number("y")?;
    let width = call.get_number("width")?;
    let height = call.get_number("height")?;
    let name = call.get_string_opt("name");
    let parent_id = parse_optional_uuid(call.get_string_opt("parent_id"))?;

    // Author comes from context — use a placeholder that the caller fills in.
    let author = "advisor";
    let mut digit = Digit::new("container".into(), Value::Null, author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("x".into(), Value::Double(x), author);
    digit = digit.with_property("y".into(), Value::Double(y), author);
    digit = digit.with_property("width".into(), Value::Double(width), author);
    digit = digit.with_property("height".into(), Value::Double(height), author);
    if let Some(ref n) = name {
        digit = digit.with_property("name".into(), Value::String(n.clone()), author);
    }

    Ok(BridgeOutput::Action(Action::insert(digit, parent_id)))
}

fn translate_add_shape(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let shape_type = call.get_string("shape_type")?;
    let x = call.get_number("x")?;
    let y = call.get_number("y")?;
    let width = call.get_number("width")?;
    let height = call.get_number("height")?;
    let parent_id = parse_optional_uuid(call.get_string_opt("parent_id"))?;

    let author = "advisor";
    let mut digit = Digit::new("container".into(), Value::String(shape_type.clone()), author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("shape_type".into(), Value::String(shape_type), author);
    digit = digit.with_property("x".into(), Value::Double(x), author);
    digit = digit.with_property("y".into(), Value::Double(y), author);
    digit = digit.with_property("width".into(), Value::Double(width), author);
    digit = digit.with_property("height".into(), Value::Double(height), author);

    Ok(BridgeOutput::Action(Action::insert(digit, parent_id)))
}

fn translate_set_fill(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let digit_id = parse_uuid(&call.get_string("digit_id")?)?;
    let color = call.get_string("color")?;
    let opacity = call.get_number_opt("opacity");

    let new_value = if let Some(op) = opacity {
        // Store as a structured fill value with color and opacity.
        let fill_json = serde_json::json!({
            "color": color,
            "opacity": op,
        });
        Value::String(fill_json.to_string())
    } else {
        Value::String(color)
    };

    Ok(BridgeOutput::Action(Action::update(
        digit_id,
        "fill",
        Value::Null,
        new_value,
    )))
}

fn translate_set_text(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let x = call.get_number("x")?;
    let y = call.get_number("y")?;
    let text = call.get_string("text")?;
    let parent_id = parse_optional_uuid(call.get_string_opt("parent_id"))?;

    let author = "advisor";
    let mut digit = Digit::new("text".into(), Value::String(text), author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("x".into(), Value::Double(x), author);
    digit = digit.with_property("y".into(), Value::Double(y), author);

    if let Some(size) = call.get_number_opt("font_size") {
        digit = digit.with_property("font_size".into(), Value::Double(size), author);
    }
    if let Some(ref family) = call.get_string_opt("font_family") {
        digit = digit.with_property("font_family".into(), Value::String(family.clone()), author);
    }

    Ok(BridgeOutput::Action(Action::insert(digit, parent_id)))
}

fn translate_apply_component_style(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let digit_id = parse_uuid(&call.get_string("digit_id")?)?;
    let style_name = call.get_string("style_name")?;

    Ok(BridgeOutput::Action(Action::update(
        digit_id,
        "component_style",
        Value::Null,
        Value::String(style_name),
    )))
}

fn translate_connect_to_data_source(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let digit_id = parse_uuid(&call.get_string("digit_id")?)?;
    let source_ref = call.get_string("source_ref")?;
    let source_path = call.get_string("source_path")?;

    let binding_json = serde_json::json!({
        "source_ref": source_ref,
        "source_path": source_path,
        "live": call.get_number_opt("live").map(|v| v != 0.0).unwrap_or(true),
    });

    Ok(BridgeOutput::Action(Action::update(
        digit_id,
        "binding_source",
        Value::Null,
        Value::String(binding_json.to_string()),
    )))
}

fn translate_export_as(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let format = call.get_string("format")?;
    let digit_id = call.get_string_opt("digit_id");
    let scale = call.get_number_opt("scale").unwrap_or(1.0);

    // Export is a read-only operation — return DirectResult.
    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Export requested: format={format}, digit={}, scale={scale}",
            digit_id.as_deref().unwrap_or("canvas")
        ))
        .with_data("format", format)
        .with_data("scale", scale.to_string()),
    ))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AdvisorError> {
    Uuid::parse_str(s).map_err(|e| AdvisorError::InvalidSkillParameters {
        id: "studio".into(),
        reason: format!("invalid UUID: {e}"),
    })
}

fn parse_optional_uuid(s: Option<String>) -> Result<Option<Uuid>, AdvisorError> {
    s.map(|v| parse_uuid(&v)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_frame_produces_insert_action() {
        let call = SkillCall::new("c1", "studio.createFrame")
            .with_argument("x", serde_json::json!(100.0))
            .with_argument("y", serde_json::json!(200.0))
            .with_argument("width", serde_json::json!(300.0))
            .with_argument("height", serde_json::json!(400.0));

        let result = StudioBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::InsertDigit { .. })));
    }

    #[test]
    fn set_fill_produces_update_action() {
        let digit_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "studio.setFill")
            .with_argument("digit_id", serde_json::Value::String(digit_id.to_string()))
            .with_argument("color", serde_json::Value::String("#FF0000".into()));

        let result = StudioBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::UpdateDigit { .. })));
    }

    #[test]
    fn export_as_produces_direct_result() {
        let call = SkillCall::new("c1", "studio.exportAs")
            .with_argument("format", serde_json::Value::String("png".into()));

        let result = StudioBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn unknown_studio_skill_fails() {
        let call = SkillCall::new("c1", "studio.unknownAction");
        let result = StudioBridge.translate(&call);
        assert!(result.is_err());
    }
}
