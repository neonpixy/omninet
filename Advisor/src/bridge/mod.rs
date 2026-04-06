//! Action bridges — translate SkillCalls into Magic Actions or direct results.
//!
//! Each program has a bridge that understands its skills and can convert them
//! into the appropriate Magic `Action` (for document mutations) or produce
//! a `DirectResult` (for read-only or Equipment operations).

pub mod abacus;
pub mod courier;
pub mod library;
pub mod podium;
pub mod quill;
pub mod studio;
pub mod tome;

use std::collections::HashMap;

use crate::error::AdvisorError;
use crate::skill::call::{SkillCall, SkillResult};

/// Output of a bridge translation.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum BridgeOutput {
    /// Produces a Magic Action to apply to DocumentState.
    Action(magic::Action),
    /// Direct result without going through Magic (for read-only or Equipment operations).
    DirectResult(SkillResult),
}

/// Translates SkillCalls into Magic Actions or direct results.
///
/// Each bridge handles one or more skill IDs, typically all skills within
/// a single program (e.g., `quill.*`).
pub trait ActionBridge: Send + Sync {
    /// The skill ID prefix this bridge handles (e.g., `"quill"` matches `quill.*`).
    fn program_prefix(&self) -> &str;

    /// Translate a SkillCall into a BridgeOutput.
    fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError>;
}

/// Registry of action bridges, keyed by program prefix.
pub struct BridgeRegistry {
    bridges: HashMap<String, Box<dyn ActionBridge>>,
}

impl BridgeRegistry {
    /// Create an empty bridge registry.
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
        }
    }

    /// Register a bridge for a program.
    pub fn register(&mut self, bridge: Box<dyn ActionBridge>) {
        self.bridges
            .insert(bridge.program_prefix().to_string(), bridge);
    }

    /// Translate a skill call by finding the appropriate bridge.
    ///
    /// The program prefix is extracted from the skill ID (everything before the first dot).
    pub fn translate(&self, call: &SkillCall) -> Result<BridgeOutput, AdvisorError> {
        let prefix = call
            .skill_id
            .split('.')
            .next()
            .unwrap_or(&call.skill_id);

        let bridge = self
            .bridges
            .get(prefix)
            .ok_or_else(|| AdvisorError::SkillNotFound(call.skill_id.clone()))?;

        bridge.translate(call)
    }

    /// Number of registered bridges.
    pub fn len(&self) -> usize {
        self.bridges.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.bridges.is_empty()
    }

    /// Create a registry with all 7 program bridges registered.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(studio::StudioBridge));
        registry.register(Box::new(abacus::AbacusBridge));
        registry.register(Box::new(quill::QuillBridge));
        registry.register(Box::new(podium::PodiumBridge));
        registry.register(Box::new(courier::CourierBridge));
        registry.register(Box::new(library::LibraryBridge));
        registry.register(Box::new(tome::TomeBridge));
        registry
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_defaults_has_seven_bridges() {
        let registry = BridgeRegistry::with_defaults();
        assert_eq!(registry.len(), 7);
    }

    #[test]
    fn translate_unknown_skill_returns_not_found() {
        let registry = BridgeRegistry::with_defaults();
        let call = SkillCall::new("call-1", "unknown.skill");
        let result = registry.translate(&call);
        assert!(result.is_err());
    }

    #[test]
    fn default_is_empty() {
        let registry = BridgeRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn translate_routes_to_correct_bridge() {
        let registry = BridgeRegistry::with_defaults();

        // A quill call should route to the quill bridge
        let call = SkillCall::new("c1", "quill.addHeading")
            .with_argument("parent_id", serde_json::Value::String(uuid::Uuid::new_v4().to_string()))
            .with_argument("level", serde_json::json!(2))
            .with_argument("text", serde_json::Value::String("Hello".into()));

        let result = registry.translate(&call);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), BridgeOutput::Action(_)));
    }
}
