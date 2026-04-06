//! Courier skill definitions — 5 skills for the mail/messaging program.
//!
//! Courier is the asynchronous messaging program in Throne. Skills cover
//! composing, recipients, sending, scheduling, and newsletters.
//! Courier routes through Equipment's Mail system, not Magic Actions.

use crate::skill::definition::{SkillDefinition, SkillParameter};
use crate::skill::registry::SkillRegistry;

/// Register all Courier skills (5 total).
pub fn register(registry: &mut SkillRegistry) {
    registry.register(
        SkillDefinition::new(
            "courier.composeMail",
            "Compose Mail",
            "Create a new mail draft with subject and body",
        )
        .with_parameter(SkillParameter::new("subject", "string", "Mail subject line", true))
        .with_parameter(SkillParameter::new("body", "string", "Mail body content (as .idea JSON or plain text)", true))
        .with_parameter(SkillParameter::new("thread_id", "string", "Thread ID if replying to an existing conversation", false))
        .with_parameter(SkillParameter::new("in_reply_to", "string", "Message ID being replied to", false)),
    );

    registry.register(
        SkillDefinition::new(
            "courier.addRecipients",
            "Add Recipients",
            "Add recipients to a mail draft",
        )
        .with_parameter(SkillParameter::new("draft_id", "string", "ID of the draft message", true))
        .with_parameter(SkillParameter::new("recipients", "string", "JSON array of {crown_id, role} objects (role: to, cc, bcc)", true)),
    );

    registry.register(
        SkillDefinition::new(
            "courier.send",
            "Send",
            "Send a composed mail message",
        )
        .with_parameter(SkillParameter::new("draft_id", "string", "ID of the draft message to send", true)),
    );

    registry.register(
        SkillDefinition::new(
            "courier.schedule",
            "Schedule",
            "Schedule a mail message for later delivery",
        )
        .with_parameter(SkillParameter::new("draft_id", "string", "ID of the draft message", true))
        .with_parameter(SkillParameter::new("send_at", "string", "ISO 8601 datetime for scheduled delivery", true)),
    );

    registry.register(
        SkillDefinition::new(
            "courier.createNewsletter",
            "Create Newsletter",
            "Create a bulk send newsletter from a template",
        )
        .with_parameter(SkillParameter::new("subject", "string", "Newsletter subject", true))
        .with_parameter(SkillParameter::new("body", "string", "Newsletter body template", true))
        .with_parameter(SkillParameter::new("recipients", "string", "JSON array of recipient crown IDs", true)),
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
    fn all_skills_have_courier_prefix() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        for skill in registry.all() {
            assert!(skill.id.starts_with("courier."), "skill {} missing courier prefix", skill.id);
        }
    }

    #[test]
    fn send_has_required_draft_id() {
        let mut registry = SkillRegistry::new();
        register(&mut registry);
        let skill = registry.get("courier.send").unwrap();
        let required: Vec<&str> = skill.parameters.iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();
        assert!(required.contains(&"draft_id"));
    }
}
