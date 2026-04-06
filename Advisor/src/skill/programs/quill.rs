//! Quill skill definitions — 7 skills for the document editing program.
//!
//! Quill is the rich text document editor in Throne. Skills cover document
//! creation, headings, paragraphs, lists, images, styling, and export.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Quill skills (7 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "quill.createDocument",
            "Create Document",
            "Create a new rich text document",
        )
        .with_parameter(SkillParameter::new("title", "string", "Document title", true))
        .with_parameter(SkillParameter::new("template", "string", "Optional template name to start from", false)),
    );

    registry.register(
        SkillDefinition::new(
            "quill.addHeading",
            "Add Heading",
            "Add a heading block to the document",
        )
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent document or container ID", true))
        .with_parameter(SkillParameter::new("level", "number", "Heading level (1-6)", true))
        .with_parameter(SkillParameter::new("text", "string", "Heading text content", true)),
    );

    registry.register(
        SkillDefinition::new(
            "quill.addParagraph",
            "Add Paragraph",
            "Add a paragraph block to the document",
        )
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent document or container ID", true))
        .with_parameter(SkillParameter::new("text", "string", "Paragraph text content", true)),
    );

    registry.register(
        SkillDefinition::new(
            "quill.addList",
            "Add List",
            "Add a list block to the document",
        )
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent document or container ID", true))
        .with_parameter(SkillParameter::new("style", "string", "List style: ordered, unordered, checklist", true))
        .with_parameter(SkillParameter::new("items", "string", "JSON array of list item strings", true)),
    );

    registry.register(
        SkillDefinition::new(
            "quill.addImage",
            "Add Image",
            "Add an image block to the document",
        )
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent document or container ID", true))
        .with_parameter(SkillParameter::new("image_ref", "string", "Reference to the image .idea or asset hash", true))
        .with_parameter(SkillParameter::new("alt_text", "string", "Accessibility alt text for the image", false))
        .with_parameter(SkillParameter::new("caption", "string", "Image caption", false)),
    );

    registry.register(
        SkillDefinition::new(
            "quill.setStyle",
            "Set Style",
            "Apply a text style to a block or selection within the document",
        )
        .with_parameter(SkillParameter::new("digit_id", "string", "ID of the text block to style", true))
        .with_parameter(SkillParameter::new("style", "string", "Style to apply: bold, italic, underline, strikethrough, code, highlight", true)),
    );

    registry.register(
        SkillDefinition::new(
            "quill.exportAs",
            "Export As",
            "Export the document to a file format",
        )
        .with_parameter(SkillParameter::new("document_id", "string", "ID of the document to export", true))
        .with_parameter(SkillParameter::new("format", "string", "Export format: pdf, markdown, html, docx", true)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_seven_skills() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 7);
    }

    #[test]
    fn all_skills_have_quill_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("quill."), "skill {} missing quill prefix", skill.id);
        }
    }

    #[test]
    fn add_heading_has_required_params() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("quill.addHeading").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"parent_id"));
        assert!(required.contains(&"level"));
        assert!(required.contains(&"text"));
    }
}
