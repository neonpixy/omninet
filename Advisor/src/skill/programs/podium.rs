//! Podium skill definitions — 5 skills for the presentation program.
//!
//! Podium is the presentation tool in Throne. Skills cover presentation
//! creation, slides, transitions, speaker notes, and reordering.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Podium skills (5 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "podium.createPresentation",
            "Create Presentation",
            "Create a new presentation document",
        )
        .with_parameter(SkillParameter::new("title", "string", "Presentation title", true))
        .with_parameter(SkillParameter::new("template", "string", "Optional template name", false)),
    );

    registry.register(
        SkillDefinition::new(
            "podium.addSlide",
            "Add Slide",
            "Add a new slide to the presentation",
        )
        .with_parameter(SkillParameter::new("presentation_id", "string", "ID of the presentation", true))
        .with_parameter(SkillParameter::new("layout", "string", "Slide layout: title, content, twocolumn, blank", true))
        .with_parameter(SkillParameter::new("title", "string", "Optional slide title", false))
        .with_parameter(SkillParameter::new("order", "number", "Position in slide sequence (0-based)", false)),
    );

    registry.register(
        SkillDefinition::new(
            "podium.setTransition",
            "Set Transition",
            "Set the transition effect for a slide",
        )
        .with_parameter(SkillParameter::new("slide_id", "string", "ID of the slide digit", true))
        .with_parameter(SkillParameter::new("transition", "string", "Transition type: fade, slide, push, dissolve, or custom:name", true)),
    );

    registry.register(
        SkillDefinition::new(
            "podium.addSpeakerNotes",
            "Add Speaker Notes",
            "Add or update speaker notes on a slide",
        )
        .with_parameter(SkillParameter::new("slide_id", "string", "ID of the slide digit", true))
        .with_parameter(SkillParameter::new("notes", "string", "Speaker notes text content", true)),
    );

    registry.register(
        SkillDefinition::new(
            "podium.reorderSlides",
            "Reorder Slides",
            "Change the order of slides in the presentation",
        )
        .with_parameter(SkillParameter::new("presentation_id", "string", "ID of the presentation", true))
        .with_parameter(SkillParameter::new("slide_ids", "string", "JSON array of slide IDs in new order", true)),
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
    fn all_skills_have_podium_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("podium."), "skill {} missing podium prefix", skill.id);
        }
    }

    #[test]
    fn add_slide_has_required_params() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("podium.addSlide").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"presentation_id"));
        assert!(required.contains(&"layout"));
    }
}
