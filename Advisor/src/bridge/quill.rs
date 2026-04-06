//! Quill bridge — translates quill.* SkillCalls into Magic Actions.

use ideas::richtext::{self, HeadingMeta, ListMeta, ListStyle, ParagraphMeta};
use ideas::Digit;
use magic::Action;
use uuid::Uuid;
use x::Value;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Quill document editing skills.
pub struct QuillBridge;

impl ActionBridge for QuillBridge {
    fn program_prefix(&self) -> &str {
        "quill"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "quill.createDocument" => translate_create_document(call),
            "quill.addHeading" => translate_add_heading(call),
            "quill.addParagraph" => translate_add_paragraph(call),
            "quill.addList" => translate_add_list(call),
            "quill.addImage" => translate_add_image(call),
            "quill.setStyle" => translate_set_style(call),
            "quill.exportAs" => translate_export_as(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_create_document(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let title = call.get_string("title")?;

    let author = "advisor";
    let mut digit = Digit::new("document".into(), Value::Null, author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("title".into(), Value::String(title), author);

    if let Some(ref template) = call.get_string_opt("template") {
        digit = digit.with_property("template".into(), Value::String(template.clone()), author);
    }

    Ok(BridgeOutput::Action(Action::insert(digit, None)))
}

fn translate_add_heading(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let parent_id = parse_uuid(&call.get_string("parent_id")?)?;
    let level = call.get_number("level")? as u8;
    let text = call.get_string("text")?;

    let meta = HeadingMeta { level, text, spans: None };
    let digit = richtext::heading_digit(&meta, "advisor").map_err(|e| {
        AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        }
    })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(parent_id),
    )))
}

fn translate_add_paragraph(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let parent_id = parse_uuid(&call.get_string("parent_id")?)?;
    let text = call.get_string("text")?;

    let meta = ParagraphMeta { text, spans: None };
    let digit = richtext::paragraph_digit(&meta, "advisor").map_err(|e| {
        AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        }
    })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(parent_id),
    )))
}

fn translate_add_list(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let parent_id = parse_uuid(&call.get_string("parent_id")?)?;
    let style_str = call.get_string("style")?;
    let items_json = call.get_string("items")?;

    let style = parse_list_style(&style_str)?;
    let items: Vec<String> =
        serde_json::from_str(&items_json).map_err(|e| AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid items JSON: {e}"),
        })?;

    let meta = ListMeta { style, items };
    let digit = richtext::list_digit(&meta, "advisor").map_err(|e| {
        AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        }
    })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(parent_id),
    )))
}

fn translate_add_image(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let parent_id = parse_uuid(&call.get_string("parent_id")?)?;
    let image_ref = call.get_string("image_ref")?;

    let author = "advisor";
    let mut digit = Digit::new("image".into(), Value::String(image_ref.clone()), author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("image_ref".into(), Value::String(image_ref), author);

    if let Some(ref alt) = call.get_string_opt("alt_text") {
        digit = digit.with_property("alt_text".into(), Value::String(alt.clone()), author);
    }
    if let Some(ref caption) = call.get_string_opt("caption") {
        digit = digit.with_property("caption".into(), Value::String(caption.clone()), author);
    }

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(parent_id),
    )))
}

fn translate_set_style(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let digit_id = parse_uuid(&call.get_string("digit_id")?)?;
    let style = call.get_string("style")?;

    Ok(BridgeOutput::Action(Action::update(
        digit_id,
        "text_style",
        Value::Null,
        Value::String(style),
    )))
}

fn translate_export_as(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let document_id = call.get_string("document_id")?;
    let format = call.get_string("format")?;

    // Export is a read-only operation — return DirectResult.
    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Export requested: document={document_id}, format={format}"
        ))
        .with_data("document_id", document_id)
        .with_data("format", format),
    ))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AdvisorError> {
    Uuid::parse_str(s).map_err(|e| AdvisorError::InvalidSkillParameters {
        id: "quill".into(),
        reason: format!("invalid UUID: {e}"),
    })
}

fn parse_list_style(s: &str) -> Result<ListStyle, AdvisorError> {
    match s {
        "ordered" => Ok(ListStyle::Ordered),
        "unordered" => Ok(ListStyle::Unordered),
        "checklist" => Ok(ListStyle::Checklist),
        other => Err(AdvisorError::InvalidSkillParameters {
            id: "quill".into(),
            reason: format!("unknown list style: {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_heading_produces_insert_action() {
        let parent_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "quill.addHeading")
            .with_argument("parent_id", serde_json::Value::String(parent_id.to_string()))
            .with_argument("level", serde_json::json!(2))
            .with_argument("text", serde_json::Value::String("Chapter One".into()));

        let result = QuillBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::Action(Action::InsertDigit { digit, parent_id: pid, .. }) => {
                assert_eq!(digit.digit_type(), "text.heading");
                assert_eq!(pid, Some(parent_id));
            }
            _ => panic!("expected InsertDigit action"),
        }
    }

    #[test]
    fn add_paragraph_produces_insert_action() {
        let parent_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "quill.addParagraph")
            .with_argument("parent_id", serde_json::Value::String(parent_id.to_string()))
            .with_argument("text", serde_json::Value::String("Hello, world!".into()));

        let result = QuillBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::InsertDigit { .. })));
    }

    #[test]
    fn add_list_produces_insert_action() {
        let parent_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "quill.addList")
            .with_argument("parent_id", serde_json::Value::String(parent_id.to_string()))
            .with_argument("style", serde_json::Value::String("ordered".into()))
            .with_argument("items", serde_json::Value::String(r#"["First","Second"]"#.into()));

        let result = QuillBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::InsertDigit { .. })));
    }

    #[test]
    fn export_as_produces_direct_result() {
        let call = SkillCall::new("c1", "quill.exportAs")
            .with_argument("document_id", serde_json::Value::String(Uuid::new_v4().to_string()))
            .with_argument("format", serde_json::Value::String("pdf".into()));

        let result = QuillBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn invalid_heading_level_fails() {
        let parent_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "quill.addHeading")
            .with_argument("parent_id", serde_json::Value::String(parent_id.to_string()))
            .with_argument("level", serde_json::json!(0))
            .with_argument("text", serde_json::Value::String("Bad".into()));

        let result = QuillBridge.translate(&call);
        assert!(result.is_err());
    }
}
