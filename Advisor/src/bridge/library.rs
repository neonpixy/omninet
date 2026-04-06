//! Library bridge — translates library.* SkillCalls into DirectResults.
//!
//! Library operations are metadata-only — they don't produce Magic digit
//! operations. All results are DirectResult for the caller to process.

use uuid::Uuid;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

use super::{ActionBridge, BridgeOutput};

/// Bridge for Library content management skills.
///
/// Library operations (organize, tag, publish, visibility, collections)
/// are metadata operations on .idea packages, not digit mutations.
pub struct LibraryBridge;

impl ActionBridge for LibraryBridge {
    fn program_prefix(&self) -> &str {
        "library"
    }

    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        match call.skill_id.as_str() {
            "library.organize" => translate_organize(call),
            "library.tag" => translate_tag(call),
            "library.publish" => translate_publish(call),
            "library.setVisibility" => translate_set_visibility(call),
            "library.createCollection" => translate_create_collection(call),
            _ => Err(AdvisorError::SkillNotFound(call.skill_id.clone())),
        }
    }
}

fn translate_organize(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let idea_id = call.get_string("idea_id")?;
    let destination = call.get_string("destination")?;

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Organize: move {idea_id} to {destination}"
        ))
        .with_data("idea_id", idea_id)
        .with_data("destination", destination)
        .with_data("action", "organize".to_string()),
    ))
}

fn translate_tag(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let idea_id = call.get_string("idea_id")?;
    let tags_json = call.get_string("tags")?;
    let remove_tags = call.get_string_opt("remove_tags");

    let tags: Vec<String> =
        serde_json::from_str(&tags_json).map_err(|e| AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid tags JSON: {e}"),
        })?;

    let mut result = SkillResult::success(format!(
        "Tagged {idea_id}: +{} tags",
        tags.len()
    ))
    .with_data("idea_id", idea_id)
    .with_data("tags_json", tags_json)
    .with_data("action", "tag".to_string());

    if let Some(ref rt) = remove_tags {
        result = result.with_data("remove_tags_json", rt.clone());
    }

    Ok(BridgeOutput::DirectResult(result))
}

fn translate_publish(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let idea_id = call.get_string("idea_id")?;
    let relay_urls = call.get_string_opt("relay_urls");

    let mut result = SkillResult::success(format!("Publish requested: {idea_id}"))
        .with_data("idea_id", idea_id)
        .with_data("action", "publish".to_string());

    if let Some(ref urls) = relay_urls {
        result = result.with_data("relay_urls_json", urls.clone());
    }

    Ok(BridgeOutput::DirectResult(result))
}

fn translate_set_visibility(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let idea_id = call.get_string("idea_id")?;
    let visibility = call.get_string("visibility")?;

    // Validate visibility level.
    if !["private", "shared", "public"].contains(&visibility.as_str()) {
        return Err(AdvisorError::InvalidSkillParameters {
            id: call.skill_id.clone(),
            reason: format!("invalid visibility level: {visibility} (expected private, shared, or public)"),
        });
    }

    Ok(BridgeOutput::DirectResult(
        SkillResult::success(format!(
            "Visibility set: {idea_id} -> {visibility}"
        ))
        .with_data("idea_id", idea_id)
        .with_data("visibility", visibility)
        .with_data("action", "setVisibility".to_string()),
    ))
}

fn translate_create_collection(call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
    let name = call.get_string("name")?;
    let description = call.get_string_opt("description");
    let parent = call.get_string_opt("parent_collection");

    let collection_id = Uuid::new_v4().to_string();

    let mut result = SkillResult::success(format!("Collection created: {name}"))
        .with_data("collection_id", collection_id)
        .with_data("name", name)
        .with_data("action", "createCollection".to_string());

    if let Some(ref desc) = description {
        result = result.with_data("description", desc.clone());
    }
    if let Some(ref p) = parent {
        result = result.with_data("parent_collection", p.clone());
    }

    Ok(BridgeOutput::DirectResult(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organize_produces_direct_result() {
        let call = SkillCall::new("c1", "library.organize")
            .with_argument("idea_id", serde_json::Value::String("idea-123".into()))
            .with_argument("destination", serde_json::Value::String("/designs/logos".into()));

        let result = LibraryBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn tag_produces_direct_result() {
        let call = SkillCall::new("c1", "library.tag")
            .with_argument("idea_id", serde_json::Value::String("idea-123".into()))
            .with_argument("tags", serde_json::Value::String(r#"["design","logo"]"#.into()));

        let result = LibraryBridge.translate(&call).unwrap();
        assert!(matches!(result, BridgeOutput::DirectResult(_)));
    }

    #[test]
    fn publish_produces_direct_result() {
        let call = SkillCall::new("c1", "library.publish")
            .with_argument("idea_id", serde_json::Value::String("idea-123".into()));

        let result = LibraryBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref sr) => {
                assert!(sr.success);
                assert_eq!(sr.data.get("action").unwrap(), "publish");
            }
            _ => panic!("expected DirectResult"),
        }
    }

    #[test]
    fn invalid_visibility_fails() {
        let call = SkillCall::new("c1", "library.setVisibility")
            .with_argument("idea_id", serde_json::Value::String("idea-123".into()))
            .with_argument("visibility", serde_json::Value::String("secret".into()));

        let result = LibraryBridge.translate(&call);
        assert!(result.is_err());
    }

    #[test]
    fn create_collection_produces_direct_result() {
        let call = SkillCall::new("c1", "library.createCollection")
            .with_argument("name", serde_json::Value::String("Logos".into()));

        let result = LibraryBridge.translate(&call).unwrap();
        match result {
            BridgeOutput::DirectResult(ref sr) => {
                assert!(sr.success);
                assert!(sr.data.contains_key("collection_id"));
            }
            _ => panic!("expected DirectResult"),
        }
    }
}
