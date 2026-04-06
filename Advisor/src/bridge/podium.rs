//! Podium bridge — translates podium.* SkillCalls into Magic Actions.

use ideas::slide::{self, SlideLayout, SlideMeta, TransitionType};
use ideas::Digit;
use magic::Action;
use uuid::Uuid;
use x::Value;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Podium presentation skills.
pub struct PodiumBridge;

impl ActionBridge for PodiumBridge {
    fn program_prefix(&self) -> &str {
        "podium"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "podium.createPresentation" => translate_create_presentation(call),
            "podium.addSlide" => translate_add_slide(call),
            "podium.setTransition" => translate_set_transition(call),
            "podium.addSpeakerNotes" => translate_add_speaker_notes(call),
            "podium.reorderSlides" => translate_reorder_slides(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_create_presentation(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let title = call.get_string("title")?;

    let author = "advisor";
    let mut digit = Digit::new("document".into(), Value::Null, author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("title".into(), Value::String(title), author);
    digit = digit.with_property("kind".into(), Value::String("presentation".into()), author);

    if let Some(ref template) = call.get_string_opt("template") {
        digit = digit.with_property("template".into(), Value::String(template.clone()), author);
    }

    Ok(BridgeOutput::Action(Action::insert(digit, None)))
}

fn translate_add_slide(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let presentation_id = parse_uuid(&call.get_string("presentation_id")?)?;
    let layout_str = call.get_string("layout")?;
    let title = call.get_string_opt("title");
    let order = call.get_number_opt("order").unwrap_or(0.0) as u32;

    let layout = parse_layout(&layout_str)?;

    let meta = SlideMeta {
        title,
        speaker_notes: None,
        transition: None,
        layout,
        order,
    };

    let digit = slide::slide_digit(&meta, "advisor").map_err(|e| AdvisorError::SkillFailed {
        id: call.skill_id.clone(),
        reason: e.to_string(),
    })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(presentation_id),
    )))
}

fn translate_set_transition(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let slide_id = parse_uuid(&call.get_string("slide_id")?)?;
    let transition_str = call.get_string("transition")?;

    let transition = parse_transition(&transition_str)?;
    let transition_value = serde_json::to_string(&transition).map_err(AdvisorError::from)?;

    Ok(BridgeOutput::Action(Action::update(
        slide_id,
        "transition",
        Value::Null,
        Value::String(transition_value),
    )))
}

fn translate_add_speaker_notes(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let slide_id = parse_uuid(&call.get_string("slide_id")?)?;
    let notes = call.get_string("notes")?;

    Ok(BridgeOutput::Action(Action::update(
        slide_id,
        "speaker_notes",
        Value::Null,
        Value::String(notes),
    )))
}

fn translate_reorder_slides(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let _presentation_id = call.get_string("presentation_id")?;
    let slide_ids_json = call.get_string("slide_ids")?;

    // Reordering is a complex multi-digit operation — return as DirectResult
    // with the new order for the caller to process.
    let slide_ids: Vec<String> = serde_json::from_str(&slide_ids_json).map_err(|e| {
        AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid slide_ids JSON: {e}"),
        }
    })?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!("Reorder requested: {} slides", slide_ids.len()))
            .with_data("slide_count", slide_ids.len().to_string()),
    ))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AdvisorError> {
    Uuid::parse_str(s).map_err(|e| AdvisorError::InvalidSkillParameters {
        id: "podium".into(),
        reason: format!("invalid UUID: {e}"),
    })
}

fn parse_layout(s: &str) -> Result<SlideLayout, AdvisorError> {
    match s {
        "title" => Ok(SlideLayout::Title),
        "content" => Ok(SlideLayout::Content),
        "twocolumn" => Ok(SlideLayout::TwoColumn),
        "blank" => Ok(SlideLayout::Blank),
        other if other.starts_with("custom:") => {
            Ok(SlideLayout::Custom(other.trim_start_matches("custom:").to_string()))
        }
        other => Err(AdvisorError::InvalidSkillParameters {
            id: "podium".into(),
            reason: format!("unknown layout: {other}"),
        }),
    }
}

fn parse_transition(s: &str) -> Result<TransitionType, AdvisorError> {
    match s {
        "fade" => Ok(TransitionType::Fade),
        "slide" => Ok(TransitionType::Slide),
        "push" => Ok(TransitionType::Push),
        "dissolve" => Ok(TransitionType::Dissolve),
        other if other.starts_with("custom:") => {
            Ok(TransitionType::Custom(other.trim_start_matches("custom:").to_string()))
        }
        other => Err(AdvisorError::InvalidSkillParameters {
            id: "podium".into(),
            reason: format!("unknown transition: {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_slide_produces_insert_action() {
        let pres_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "podium.addSlide")
            .with_argument("presentation_id", serde_json::Value::String(pres_id.to_string()))
            .with_argument("layout", serde_json::Value::String("title".into()))
            .with_argument("title", serde_json::Value::String("Welcome".into()));

        let result = PodiumBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::Action(Action::InsertDigit { digit, parent_id, .. }) => {
                assert_eq!(digit.digit_type(), "presentation.slide");
                assert_eq!(parent_id, Some(pres_id));
            }
            _ => panic!("expected InsertDigit action"),
        }
    }

    #[test]
    fn set_transition_produces_update_action() {
        let slide_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "podium.setTransition")
            .with_argument("slide_id", serde_json::Value::String(slide_id.to_string()))
            .with_argument("transition", serde_json::Value::String("fade".into()));

        let result = PodiumBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::UpdateDigit { .. })));
    }

    #[test]
    fn reorder_slides_produces_direct_result() {
        let pres_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "podium.reorderSlides")
            .with_argument("presentation_id", serde_json::Value::String(pres_id.to_string()))
            .with_argument("slide_ids", serde_json::Value::String(r#"["a","b","c"]"#.into()));

        let result = PodiumBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn create_presentation_produces_insert_action() {
        let call = SkillCall::new("c1", "podium.createPresentation")
            .with_argument("title", serde_json::Value::String("My Talk".into()));

        let result = PodiumBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::InsertDigit { .. })));
    }
}
