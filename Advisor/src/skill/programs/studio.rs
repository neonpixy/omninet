//! Studio skill definitions — 7 skills for the visual design program.
//!
//! Studio is the design canvas in Throne. Skills cover frame creation,
//! shape drawing, styling, text placement, components, data binding, and export.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Studio skills (7 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "studio.createFrame",
            "Create Frame",
            "Create a new frame on the canvas at the specified position and size",
        )
        .with_parameter(SkillParameter::new("x", "number", "X position of the frame", true))
        .with_parameter(SkillParameter::new("y", "number", "Y position of the frame", true))
        .with_parameter(SkillParameter::new("width", "number", "Width of the frame", true))
        .with_parameter(SkillParameter::new("height", "number", "Height of the frame", true))
        .with_parameter(SkillParameter::new("name", "string", "Optional name for the frame", false))
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent frame ID to nest within", false)),
    );

    registry.register(
        SkillDefinition::new(
            "studio.addShape",
            "Add Shape",
            "Add a shape (rectangle, ellipse, polygon) to the canvas",
        )
        .with_parameter(SkillParameter::new("shape_type", "string", "Shape type: rectangle, ellipse, polygon, line", true))
        .with_parameter(SkillParameter::new("x", "number", "X position", true))
        .with_parameter(SkillParameter::new("y", "number", "Y position", true))
        .with_parameter(SkillParameter::new("width", "number", "Width", true))
        .with_parameter(SkillParameter::new("height", "number", "Height", true))
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent frame ID", false)),
    );

    registry.register(
        SkillDefinition::new(
            "studio.setFill",
            "Set Fill",
            "Set the fill color of a digit on the canvas",
        )
        .with_parameter(SkillParameter::new("digit_id", "string", "ID of the digit to update", true))
        .with_parameter(SkillParameter::new("color", "string", "Fill color as hex string (e.g., #FF0000)", true))
        .with_parameter(SkillParameter::new("opacity", "number", "Opacity from 0.0 to 1.0", false)),
    );

    registry.register(
        SkillDefinition::new(
            "studio.setText",
            "Set Text",
            "Place or update text on the canvas",
        )
        .with_parameter(SkillParameter::new("x", "number", "X position", true))
        .with_parameter(SkillParameter::new("y", "number", "Y position", true))
        .with_parameter(SkillParameter::new("text", "string", "Text content to place", true))
        .with_parameter(SkillParameter::new("font_size", "number", "Font size in points", false))
        .with_parameter(SkillParameter::new("font_family", "string", "Font family name", false))
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent frame ID", false)),
    );

    registry.register(
        SkillDefinition::new(
            "studio.applyComponentStyle",
            "Apply Component Style",
            "Apply a named component style or Regalia token set to a digit",
        )
        .with_parameter(SkillParameter::new("digit_id", "string", "ID of the digit to style", true))
        .with_parameter(SkillParameter::new("style_name", "string", "Component style or token set name", true)),
    );

    registry.register(
        SkillDefinition::new(
            "studio.connectToDataSource",
            "Connect to Data Source",
            "Bind a canvas element to an external data source (.idea reference)",
        )
        .with_parameter(SkillParameter::new("digit_id", "string", "ID of the digit to bind", true))
        .with_parameter(SkillParameter::new("source_ref", "string", "Data source .idea reference", true))
        .with_parameter(SkillParameter::new("source_path", "string", "Path within the source .idea", true))
        .with_parameter(SkillParameter::new("live", "boolean", "Whether the binding is live-updating", false)),
    );

    registry.register(
        SkillDefinition::new(
            "studio.exportAs",
            "Export As",
            "Export the current design or selection to a file format",
        )
        .with_parameter(SkillParameter::new("format", "string", "Export format: png, svg, pdf, jpg", true))
        .with_parameter(SkillParameter::new("digit_id", "string", "Specific digit to export (omit for whole canvas)", false))
        .with_parameter(SkillParameter::new("scale", "number", "Export scale factor (default 1.0)", false)),
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
    fn all_skills_have_studio_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("studio."), "skill {} missing studio prefix", skill.id);
        }
    }

    #[test]
    fn create_frame_has_required_params() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("studio.createFrame").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"x"));
        assert!(required.contains(&"y"));
        assert!(required.contains(&"width"));
        assert!(required.contains(&"height"));
    }
}
