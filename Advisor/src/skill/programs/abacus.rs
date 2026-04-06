//! Abacus skill definitions — 8 skills for the spreadsheet/database program.
//!
//! Abacus is the data engine in Throne. Skills cover sheet creation,
//! row/column management, cell editing, formulas, views, filtering, and sorting.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Abacus skills (8 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "abacus.createSheet",
            "Create Sheet",
            "Create a new spreadsheet sheet with column definitions",
        )
        .with_parameter(SkillParameter::new("name", "string", "Sheet name", true))
        .with_parameter(SkillParameter::new("columns", "string", "JSON array of column definitions [{name, cell_type, required, unique}]", true))
        .with_parameter(SkillParameter::new("default_view", "string", "Default view mode: grid, kanban, calendar, gallery", false))
        .with_parameter(SkillParameter::new("parent_id", "string", "Parent digit ID", false)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.addRow",
            "Add Row",
            "Add a new row to a sheet",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("values", "string", "JSON object of column-name to value mappings", false)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.addColumn",
            "Add Column",
            "Add a new column to an existing sheet",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("name", "string", "Column name", true))
        .with_parameter(SkillParameter::new("cell_type", "string", "Cell type: text, number, date, boolean, formula, reference, rich", true))
        .with_parameter(SkillParameter::new("required", "boolean", "Whether the column is required", false))
        .with_parameter(SkillParameter::new("unique", "boolean", "Whether column values must be unique", false)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.setCell",
            "Set Cell",
            "Set the value of a specific cell",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("address", "string", "Cell address (e.g., A1, B3)", true))
        .with_parameter(SkillParameter::new("value", "string", "Cell value (formulas start with =)", true))
        .with_parameter(SkillParameter::new("cell_type", "string", "Cell type override", false)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.applyFormula",
            "Apply Formula",
            "Apply a formula to a cell or range of cells",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("address", "string", "Cell address to apply formula to", true))
        .with_parameter(SkillParameter::new("formula", "string", "Formula expression (e.g., =SUM(A1:A10))", true)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.createView",
            "Create View",
            "Create a new view of the sheet data (grid, kanban, calendar, gallery)",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("view_mode", "string", "View mode: grid, kanban, calendar, gallery", true))
        .with_parameter(SkillParameter::new("name", "string", "View name", false)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.filter",
            "Filter",
            "Apply a filter to the current sheet view",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("column", "string", "Column name to filter on", true))
        .with_parameter(SkillParameter::new("operator", "string", "Filter operator: eq, neq, gt, lt, gte, lte, contains, starts_with", true))
        .with_parameter(SkillParameter::new("value", "string", "Filter value", true)),
    );

    registry.register(
        SkillDefinition::new(
            "abacus.sort",
            "Sort",
            "Sort the current sheet view by a column",
        )
        .with_parameter(SkillParameter::new("sheet_id", "string", "ID of the sheet digit", true))
        .with_parameter(SkillParameter::new("column", "string", "Column name to sort by", true))
        .with_parameter(SkillParameter::new("direction", "string", "Sort direction: asc, desc", false)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_eight_skills() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        assert_eq!(registry.len(), 8);
    }

    #[test]
    fn all_skills_have_abacus_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("abacus."), "skill {} missing abacus prefix", skill.id);
        }
    }

    #[test]
    fn set_cell_has_required_params() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("abacus.setCell").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"sheet_id"));
        assert!(required.contains(&"address"));
        assert!(required.contains(&"value"));
    }
}
