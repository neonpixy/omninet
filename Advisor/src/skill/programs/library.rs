//! Library skill definitions — 5 skills for the .idea/asset management program.
//!
//! Library is the content management program in Throne. Skills cover
//! organizing, tagging, publishing, visibility, and collections.
//! Library operations are metadata-only — they don't produce Magic digit operations.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Library skills (5 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "library.organize",
            "Organize",
            "Move an .idea to a different folder or collection in the library",
        )
        .with_parameter(SkillParameter::new("idea_id", "string", "ID of the .idea to organize", true))
        .with_parameter(SkillParameter::new("destination", "string", "Destination folder or collection path", true)),
    );

    registry.register(
        SkillDefinition::new(
            "library.tag",
            "Tag",
            "Add or remove tags on an .idea in the library",
        )
        .with_parameter(SkillParameter::new("idea_id", "string", "ID of the .idea to tag", true))
        .with_parameter(SkillParameter::new("tags", "string", "JSON array of tag strings to add", true))
        .with_parameter(SkillParameter::new("remove_tags", "string", "JSON array of tag strings to remove", false)),
    );

    registry.register(
        SkillDefinition::new(
            "library.publish",
            "Publish",
            "Publish an .idea to Globe (make it available on the network)",
        )
        .with_parameter(SkillParameter::new("idea_id", "string", "ID of the .idea to publish", true))
        .with_parameter(SkillParameter::new("relay_urls", "string", "JSON array of relay URLs to publish to", false)),
    );

    registry.register(
        SkillDefinition::new(
            "library.setVisibility",
            "Set Visibility",
            "Change the visibility level of an .idea",
        )
        .with_parameter(SkillParameter::new("idea_id", "string", "ID of the .idea", true))
        .with_parameter(SkillParameter::new("visibility", "string", "Visibility level: private, shared, public", true)),
    );

    registry.register(
        SkillDefinition::new(
            "library.createCollection",
            "Create Collection",
            "Create a new collection to organize .ideas",
        )
        .with_parameter(SkillParameter::new("name", "string", "Collection name", true))
        .with_parameter(SkillParameter::new("description", "string", "Collection description", false))
        .with_parameter(SkillParameter::new("parent_collection", "string", "Parent collection ID for nesting", false)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_five_skills() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 5);
    }

    #[test]
    fn all_skills_have_library_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("library."), "skill {} missing library prefix", skill.id);
        }
    }

    #[test]
    fn publish_has_required_idea_id() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("library.publish").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"idea_id"));
    }
}
