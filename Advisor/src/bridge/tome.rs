//! Tome bridge — translates tome.* SkillCalls into Magic Actions or DirectResults.

use ideas::Digit;
use magic::Action;
use uuid::Uuid;
use x::Value;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Tome note-taking skills.
pub struct TomeBridge;

impl ActionBridge for TomeBridge {
    fn program_prefix(&self) -> &str {
        "tome"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "tome.createNote" => translate_create_note(call),
            "tome.append" => translate_append(call),
            "tome.tag" => translate_tag(call),
            "tome.searchNotes" => translate_search_notes(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_create_note(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let title = call.get_string("title")?;
    let content = call.get_string_opt("content");
    let tags_json = call.get_string_opt("tags");

    let author = "advisor";
    let content_value = content.map(Value::String).unwrap_or(Value::Null);
    let mut digit = Digit::new("document".into(), content_value, author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;
    digit = digit.with_property("title".into(), Value::String(title), author);
    digit = digit.with_property("kind".into(), Value::String("note".into()), author);

    if let Some(ref tags_str) = tags_json {
        let tags: Vec<String> = serde_json::from_str(tags_str).map_err(|e| {
            AdvisorError::InvalidSkillParameters {
                id: call.skill_id.clone(),
                reason: format!("invalid tags JSON: {e}"),
            }
        })?;
        let tags_value = Value::Array(tags.into_iter().map(Value::String).collect());
        digit = digit.with_property("tags".into(), tags_value, author);
    }

    Ok(BridgeOutput::Action(Action::insert(digit, None)))
}

fn translate_append(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let note_id = parse_uuid(&call.get_string("note_id")?)?;
    let content = call.get_string("content")?;

    // Appending creates a new child text digit under the note.
    let author = "advisor";
    let digit = Digit::new("text".into(), Value::String(content), author.into())
        .map_err(|e| AdvisorError::SkillFailed {
            id: call.skill_id.clone(),
            reason: e.to_string(),
        })?;

    Ok(BridgeOutput::Action(Action::insert(
        digit,
        Some(note_id),
    )))
}

fn translate_tag(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let note_id = parse_uuid(&call.get_string("note_id")?)?;
    let tags_json = call.get_string("tags")?;
    let _remove_tags = call.get_string_opt("remove_tags");

    let tags: Vec<String> =
        serde_json::from_str(&tags_json).map_err(|e| AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid tags JSON: {e}"),
        })?;
    let tags_value = Value::Array(tags.into_iter().map(Value::String).collect());

    Ok(BridgeOutput::Action(Action::update(
        note_id,
        "tags",
        Value::Null,
        tags_value,
    )))
}

/// Translate a `tome.searchNotes` skill call into a DirectResult.
///
/// # Parameters
///
/// - `query` (required, string): The search query text.
/// - `tags` (optional, string): JSON array of tag strings to filter by.
/// - `limit` (optional, number): Maximum results to return (default 20).
/// - `semantic` (optional, string): `"true"` to request semantic search via
///   CognitiveStore embeddings. Falls back to keyword search when the
///   EmbeddingProvider is unavailable.
///
/// # Search workflow
///
/// Search is a read-only operation that doesn't mutate Ideas/Magic state, so it
/// returns a `DirectResult` rather than an `Action`. The actual search execution
/// happens in the orchestrator pipeline:
///
/// 1. Advisor bridge extracts and validates parameters (this function).
/// 2. Orchestrator receives the DirectResult with structured `data` fields.
/// 3. Orchestrator queries Vault (keyword) and/or CognitiveStore (semantic).
/// 4. Results are returned as JSON array of `{ id, title, relevance, snippet }`.
///
/// The `search_mode` data field tells the orchestrator which path to take:
/// - `"keyword"` — substring match in Vault.
/// - `"semantic"` — embedding similarity via CognitiveStore.semantic_search().
/// - `"hybrid"` — both, merged and deduplicated by relevance.
fn translate_search_notes(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let query = call.get_string("query")?;
    let tags = call.get_string_opt("tags");
    let limit = call.get_number_opt("limit").unwrap_or(20.0) as usize;
    let semantic = call
        .get_string_opt("semantic")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    // Validate tags JSON if provided.
    if let Some(ref tags_str) = tags {
        let _: Vec<String> =
            serde_json::from_str(tags_str).map_err(|e| AdvisorError::InvalidSkillParameters {
                id: call.skill_id.clone(),
                reason: format!("invalid tags JSON: {e}"),
            })?;
    }

    let search_mode = if semantic { "semantic" } else { "keyword" };

    let mut result = SkillResult::success(format!("Search requested: {query}"))
        .with_data("query", query)
        .with_data("limit", limit.to_string())
        .with_data("search_mode", search_mode.to_string())
        .with_data("action", "search".to_string());

    if let Some(ref t) = tags {
        result = result.with_data("tags_json", t.clone());
    }

    Ok(BridgeOutput::DirectResult(result))
}

// ── Helpers ─────────────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AdvisorError> {
    Uuid::parse_str(s).map_err(|e| AdvisorError::InvalidSkillParameters {
        id: "tome".into(),
        reason: format!("invalid UUID: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_note_produces_insert_action() {
        let call = SkillCall::new("c1", "tome.createNote")
            .with_argument("title", serde_json::Value::String("My Note".into()));

        let result = TomeBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::Action(Action::InsertDigit { digit, parent_id, .. }) => {
                assert_eq!(digit.digit_type(), "document");
                assert!(parent_id.is_none());
            }
            _ => panic!("expected InsertDigit action"),
        }
    }

    #[test]
    fn append_produces_insert_action() {
        let note_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "tome.append")
            .with_argument("note_id", serde_json::Value::String(note_id.to_string()))
            .with_argument("content", serde_json::Value::String("More text".into()));

        let result = TomeBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::Action(Action::InsertDigit { parent_id, .. }) => {
                assert_eq!(parent_id, Some(note_id));
            }
            _ => panic!("expected InsertDigit action"),
        }
    }

    #[test]
    fn tag_produces_update_action() {
        let note_id = Uuid::new_v4();
        let call = SkillCall::new("c1", "tome.tag")
            .with_argument("note_id", serde_json::Value::String(note_id.to_string()))
            .with_argument("tags", serde_json::Value::String(r#"["todo","important"]"#.into()));

        let result = TomeBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::Action(Action::UpdateDigit { .. })));
    }

    #[test]
    fn search_notes_produces_direct_result() {
        let call = SkillCall::new("c1", "tome.searchNotes")
            .with_argument("query", serde_json::Value::String("meeting notes".into()));

        let result = TomeBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn search_notes_keyword_mode_by_default() {
        let call = SkillCall::new("c1", "tome.searchNotes")
            .with_argument("query", serde_json::Value::String("design".into()));

        let result = TomeBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref r) => {
                assert_eq!(r.data.get("search_mode").unwrap(), "keyword");
                assert_eq!(r.data.get("query").unwrap(), "design");
            }
            _ => panic!("expected DirectResult"),
        }
    }

    #[test]
    fn search_notes_semantic_mode() {
        let call = SkillCall::new("c1", "tome.searchNotes")
            .with_argument("query", serde_json::Value::String("design".into()))
            .with_argument("semantic", serde_json::Value::String("true".into()));

        let result = TomeBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref r) => {
                assert_eq!(r.data.get("search_mode").unwrap(), "semantic");
            }
            _ => panic!("expected DirectResult"),
        }
    }

    #[test]
    fn search_notes_with_tags_and_limit() {
        let call = SkillCall::new("c1", "tome.searchNotes")
            .with_argument("query", serde_json::Value::String("notes".into()))
            .with_argument("tags", serde_json::Value::String(r#"["work","draft"]"#.into()))
            .with_argument("limit", serde_json::json!(5));

        let result = TomeBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref r) => {
                assert_eq!(r.data.get("limit").unwrap(), "5");
                assert!(r.data.contains_key("tags_json"));
            }
            _ => panic!("expected DirectResult"),
        }
    }

    #[test]
    fn search_notes_invalid_tags_json_fails() {
        let call = SkillCall::new("c1", "tome.searchNotes")
            .with_argument("query", serde_json::Value::String("notes".into()))
            .with_argument("tags", serde_json::Value::String("not valid json".into()));

        let result = TomeBridge.translate(&call);
        assert!(result.is_err());
    }
}
