use std::collections::HashMap;

use crate::error::AdvisorError;

use super::definition::SkillDefinition;

/// Registry of available skills.
pub struct SkillRegistry {
    skills: HashMap<String, SkillDefinition>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Register a skill.
    pub fn register(&mut self, skill: SkillDefinition) {
        self.skills.insert(skill.id.clone(), skill);
    }

    /// Unregister a skill.
    pub fn unregister(&mut self, id: &str) -> Result<(), AdvisorError> {
        if self.skills.remove(id).is_none() {
            return Err(AdvisorError::SkillNotFound(id.into()));
        }
        Ok(())
    }

    /// Get a skill by ID.
    pub fn get(&self, id: &str) -> Option<&SkillDefinition> {
        self.skills.get(id)
    }

    /// Get a skill by sanitized ID (for LLM API responses).
    pub fn get_by_sanitized(&self, sanitized_id: &str) -> Option<&SkillDefinition> {
        let original_id = SkillDefinition::unsanitize_id(sanitized_id);
        self.skills.get(&original_id)
    }

    /// All registered skills.
    pub fn all(&self) -> Vec<&SkillDefinition> {
        self.skills.values().collect()
    }

    /// Search skills by name/description.
    pub fn search(&self, query: &str) -> Vec<&SkillDefinition> {
        let query_lower = query.to_lowercase();
        self.skills
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut reg = SkillRegistry::new();
        reg.register(SkillDefinition::new("web.search", "Web Search", "Search the web"));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("web.search").is_some());
    }

    #[test]
    fn get_by_sanitized() {
        let mut reg = SkillRegistry::new();
        reg.register(SkillDefinition::new("calendar.check", "Check Calendar", "Check events"));
        assert!(reg.get_by_sanitized("calendar_check").is_some());
    }

    #[test]
    fn unregister() {
        let mut reg = SkillRegistry::new();
        reg.register(SkillDefinition::new("test", "Test", "A test"));
        assert!(reg.unregister("test").is_ok());
        assert!(reg.is_empty());
        assert!(reg.unregister("test").is_err());
    }

    #[test]
    fn search_skills() {
        let mut reg = SkillRegistry::new();
        reg.register(SkillDefinition::new("web.search", "Web Search", "Search the web"));
        reg.register(SkillDefinition::new("mem.search", "Memory Search", "Search memories"));
        reg.register(SkillDefinition::new("cal.check", "Calendar Check", "Check calendar"));

        let results = reg.search("search");
        assert_eq!(results.len(), 2);

        let results = reg.search("calendar");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn all_skills() {
        let mut reg = SkillRegistry::new();
        reg.register(SkillDefinition::new("a", "A", "First"));
        reg.register(SkillDefinition::new("b", "B", "Second"));
        assert_eq!(reg.all().len(), 2);
    }
}
