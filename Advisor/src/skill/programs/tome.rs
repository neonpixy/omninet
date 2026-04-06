//! Tome skill definitions — 4 skills for the notes program.
//!
//! Tome is the quick-capture notes program in Throne. Skills cover
//! note creation, appending, tagging, and searching.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Tome skills (4 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "tome.createNote",
            "Create Note",
            "Create a new note",
        )
        .with_parameter(SkillParameter::new("title", "string", "Note title", true))
        .with_parameter(SkillParameter::new("content", "string", "Initial note content (text or .idea JSON)", false))
        .with_parameter(SkillParameter::new("tags", "string", "JSON array of initial tags", false)),
    );

    registry.register(
        SkillDefinition::new(
            "tome.append",
            "Append",
            "Append content to an existing note",
        )
        .with_parameter(SkillParameter::new("note_id", "string", "ID of the note to append to", true))
        .with_parameter(SkillParameter::new("content", "string", "Content to append (text or .idea JSON)", true)),
    );

    registry.register(
        SkillDefinition::new(
            "tome.tag",
            "Tag",
            "Add or remove tags on a note",
        )
        .with_parameter(SkillParameter::new("note_id", "string", "ID of the note to tag", true))
        .with_parameter(SkillParameter::new("tags", "string", "JSON array of tag strings to add", true))
        .with_parameter(SkillParameter::new("remove_tags", "string", "JSON array of tag strings to remove", false)),
    );

    registry.register(
        SkillDefinition::new(
            "tome.searchNotes",
            "Search Notes",
            "Search notes by content, title, or tags",
        )
        .with_parameter(SkillParameter::new("query", "string", "Search query text", true))
        .with_parameter(SkillParameter::new("tags", "string", "JSON array of tags to filter by", false))
        .with_parameter(SkillParameter::new("limit", "number", "Maximum number of results", false)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_four_skills() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn all_skills_have_tome_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("tome."), "skill {} missing tome prefix", skill.id);
        }
    }

    #[test]
    fn search_notes_has_required_query() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("tome.searchNotes").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"query"));
    }
}
