use serde::{Deserialize, Serialize};

/// A skill the advisor can invoke (tool/function calling).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillDefinition {
    /// Unique identifier (e.g., "web_search", "calendar.check")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// What this skill does
    pub description: String,
    /// Parameters the skill accepts
    pub parameters: Vec<SkillParameter>,
}

/// A parameter for a skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillParameter {
    pub name: String,
    /// Type hint (e.g., "string", "number", "boolean")
    pub param_type: String,
    pub description: String,
    pub required: bool,
}

/// A group of related skills.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillCategory {
    pub id: String,
    pub name: String,
    pub skills: Vec<SkillDefinition>,
}

impl SkillDefinition {
    /// Create a new skill definition with the given ID, name, and description.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            parameters: Vec::new(),
        }
    }

    /// Builder: add a typed parameter to this skill definition.
    pub fn with_parameter(mut self, param: SkillParameter) -> Self {
        self.parameters.push(param);
        self
    }

    /// Sanitize the ID for LLM APIs (only alphanumeric, underscore, hyphen).
    pub fn sanitized_id(&self) -> String {
        self.id.replace('.', "_")
    }

    /// Unsanitize an ID back from LLM API format.
    pub fn unsanitize_id(sanitized: &str) -> String {
        sanitized.replace('_', ".")
    }
}

impl SkillParameter {
    pub fn new(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_definition_builder() {
        let skill = SkillDefinition::new("web.search", "Web Search", "Search the web")
            .with_parameter(SkillParameter::new("query", "string", "Search query", true))
            .with_parameter(SkillParameter::new("limit", "number", "Max results", false));

        assert_eq!(skill.id, "web.search");
        assert_eq!(skill.parameters.len(), 2);
        assert!(skill.parameters[0].required);
        assert!(!skill.parameters[1].required);
    }

    #[test]
    fn sanitize_id() {
        let skill = SkillDefinition::new("calendar.check", "Check Calendar", "Check events");
        assert_eq!(skill.sanitized_id(), "calendar_check");
        assert_eq!(SkillDefinition::unsanitize_id("calendar_check"), "calendar.check");
    }

    #[test]
    fn skill_category() {
        let cat = SkillCategory {
            id: "search".into(),
            name: "Search Tools".into(),
            skills: vec![
                SkillDefinition::new("web.search", "Web Search", "Search the web"),
                SkillDefinition::new("memory.search", "Memory Search", "Search memories"),
            ],
        };
        assert_eq!(cat.skills.len(), 2);
    }

    #[test]
    fn skill_serialization_roundtrip() {
        let skill = SkillDefinition::new("test", "Test", "A test skill")
            .with_parameter(SkillParameter::new("arg", "string", "An argument", true));
        let json = serde_json::to_string(&skill).unwrap();
        let deserialized: SkillDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(skill, deserialized);
    }
}
