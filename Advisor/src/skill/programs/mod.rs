//! Program skill definitions — registers all 41 skills across Throne's seven programs.
//!
//! Each program module defines and registers its skills into a shared `SkillRegistry`.
//! Skills follow the `program.action` naming convention.

pub mod abacus;
pub mod courier;
pub mod library;
pub mod podium;
pub mod quill;
pub mod studio;
pub mod tome;

/// Register all 41 program skills into the given registry.
pub fn register_all_skills(registry: &mut super::SkillRegistry) {
    studio::register(registry);
    abacus::register(registry);
    quill::register(registry);
    podium::register(registry);
    courier::register(registry);
    library::register(registry);
    tome::register(registry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SkillRegistry;

    #[test]
    fn register_all_skills_count() {
        let mut registry = SkillRegistry::new();
        register_all_skills(&mut registry);
        assert_eq!(registry.len(), 41);
    }

    #[test]
    fn no_duplicate_skill_ids() {
        let mut registry = SkillRegistry::new();
        register_all_skills(&mut registry);
        let all = registry.all();
        let mut ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate skill IDs found");
    }

    #[test]
    fn all_skills_have_parameters() {
        let mut registry = SkillRegistry::new();
        register_all_skills(&mut registry);
        // Every skill should have at least a description
        for skill in registry.all() {
            assert!(!skill.description.is_empty(), "skill {} has no description", skill.id);
            assert!(!skill.name.is_empty(), "skill {} has no name", skill.id);
        }
    }

    #[test]
    fn skills_searchable_by_program() {
        let mut registry = SkillRegistry::new();
        register_all_skills(&mut registry);

        // Each program prefix should find its skills
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("studio.")).count(), 7);
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("abacus.")).count(), 8);
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("quill.")).count(), 7);
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("podium.")).count(), 5);
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("courier.")).count(), 5);
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("library.")).count(), 5);
        assert_eq!(registry.all().iter().filter(|s| s.id.starts_with("tome.")).count(), 4);
    }
}
