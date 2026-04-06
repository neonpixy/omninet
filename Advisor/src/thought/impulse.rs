use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single cognitive impulse — the atomic unit of thinking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Thought {
    pub id: Uuid,
    /// Which session this thought belongs to
    pub session_id: Uuid,
    /// The thought content
    pub content: String,
    /// Brief summary (auto-capped at 100 chars)
    pub summary: String,
    /// Where this thought originated
    pub source: ThoughtSource,
    /// Priority level
    pub priority: ThoughtPriority,
    /// When this thought was created
    pub created_at: DateTime<Utc>,
    /// What the advisor was focused on when this thought arose
    pub attention_focus: Vec<String>,
    // ── Lifecycle tracking ───────────────────────────────────────
    /// Whether this thought was expressed to the user
    pub was_expressed: bool,
    /// Whether the user saw it
    pub was_viewed: bool,
    /// Whether the user engaged with it
    pub was_discussed: bool,
    /// Session where it was discussed (if different from origin)
    pub discussion_session_id: Option<Uuid>,
}

impl Thought {
    /// Create a new thought with the given content and source.
    /// The summary is auto-truncated to 100 characters.
    pub fn new(
        session_id: Uuid,
        content: impl Into<String>,
        source: ThoughtSource,
    ) -> Self {
        let content = content.into();
        let summary = if content.len() > 100 {
            format!("{}...", &content[..97])
        } else {
            content.clone()
        };
        Self {
            id: Uuid::new_v4(),
            session_id,
            content,
            summary,
            source,
            priority: ThoughtPriority::Normal,
            created_at: Utc::now(),
            attention_focus: Vec::new(),
            was_expressed: false,
            was_viewed: false,
            was_discussed: false,
            discussion_session_id: None,
        }
    }

    /// Builder: set the priority level for this thought.
    pub fn with_priority(mut self, priority: ThoughtPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: set the attention focus tags for this thought.
    pub fn with_focus(mut self, focus: Vec<String>) -> Self {
        self.attention_focus = focus;
        self
    }

    /// Mark as expressed to the user.
    pub fn mark_expressed(&mut self) {
        self.was_expressed = true;
    }

    /// Mark as viewed by the user.
    pub fn mark_viewed(&mut self) {
        self.was_viewed = true;
    }

    /// Mark as discussed in a session.
    pub fn mark_discussed(&mut self, session_id: Uuid) {
        self.was_discussed = true;
        self.discussion_session_id = Some(session_id);
    }
}

/// Where a thought originated.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ThoughtSource {
    /// Autonomous — arose from the advisor's own cognition
    Autonomous,
    /// In response to user input
    User,
    /// From reflecting on existing content
    Reflection,
    /// Triggered by a memory recall
    MemoryEcho,
    /// Result of executing a skill
    Skill,
    /// Injected by an external source (calendar, email, etc.)
    External,
}

/// Priority level for thoughts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThoughtPriority {
    /// Background thought, low urgency.
    Low = 0,
    /// Standard thought, processed in normal order.
    Normal = 1,
    /// Important thought, prioritized for expression.
    High = 2,
    /// Time-sensitive thought, may interrupt the current flow.
    Urgent = 3,
}

/// A thought injected from an external source (calendar, notifications, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalThought {
    /// The content to process
    pub content: String,
    /// Contextual info (e.g., "source" → "calendar", "event_id" → "abc")
    pub context: std::collections::HashMap<String, String>,
    /// How important this is
    pub priority: ThoughtPriority,
    /// Who/what sent it (e.g., "calendar", "email")
    pub source_id: String,
}

impl ExternalThought {
    /// Create a new external thought from the given source with a priority level.
    pub fn new(
        content: impl Into<String>,
        source_id: impl Into<String>,
        priority: ThoughtPriority,
    ) -> Self {
        Self {
            content: content.into(),
            context: std::collections::HashMap::new(),
            priority,
            source_id: source_id.into(),
        }
    }

    /// Builder: add a key-value context pair (e.g., "event_id" = "cal-123").
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thought_creation() {
        let session_id = Uuid::new_v4();
        let thought = Thought::new(session_id, "a new idea", ThoughtSource::Autonomous);
        assert_eq!(thought.session_id, session_id);
        assert_eq!(thought.content, "a new idea");
        assert_eq!(thought.source, ThoughtSource::Autonomous);
        assert_eq!(thought.priority, ThoughtPriority::Normal);
        assert!(!thought.was_expressed);
    }

    #[test]
    fn thought_summary_truncation() {
        let long = "a".repeat(200);
        let thought = Thought::new(Uuid::new_v4(), &long, ThoughtSource::Reflection);
        assert_eq!(thought.summary.len(), 100);
        assert!(thought.summary.ends_with("..."));
    }

    #[test]
    fn thought_summary_no_truncation() {
        let thought = Thought::new(Uuid::new_v4(), "short", ThoughtSource::User);
        assert_eq!(thought.summary, "short");
    }

    #[test]
    fn thought_lifecycle() {
        let mut thought = Thought::new(Uuid::new_v4(), "hello", ThoughtSource::Autonomous);
        assert!(!thought.was_expressed);
        assert!(!thought.was_viewed);
        assert!(!thought.was_discussed);

        thought.mark_expressed();
        assert!(thought.was_expressed);

        thought.mark_viewed();
        assert!(thought.was_viewed);

        let discussion_session = Uuid::new_v4();
        thought.mark_discussed(discussion_session);
        assert!(thought.was_discussed);
        assert_eq!(thought.discussion_session_id, Some(discussion_session));
    }

    #[test]
    fn thought_priority_ordering() {
        assert!(ThoughtPriority::Low < ThoughtPriority::Normal);
        assert!(ThoughtPriority::Normal < ThoughtPriority::High);
        assert!(ThoughtPriority::High < ThoughtPriority::Urgent);
    }

    #[test]
    fn thought_with_priority_and_focus() {
        let thought = Thought::new(Uuid::new_v4(), "urgent matter", ThoughtSource::External)
            .with_priority(ThoughtPriority::Urgent)
            .with_focus(vec!["security".into(), "breach".into()]);
        assert_eq!(thought.priority, ThoughtPriority::Urgent);
        assert_eq!(thought.attention_focus.len(), 2);
    }

    #[test]
    fn external_thought_with_context() {
        let ext = ExternalThought::new("meeting in 5 min", "calendar", ThoughtPriority::High)
            .with_context("event_id", "cal-123")
            .with_context("attendees", "3");
        assert_eq!(ext.source_id, "calendar");
        assert_eq!(ext.context.len(), 2);
    }

    #[test]
    fn thought_source_variants() {
        let sources = [
            ThoughtSource::Autonomous,
            ThoughtSource::User,
            ThoughtSource::Reflection,
            ThoughtSource::MemoryEcho,
            ThoughtSource::Skill,
            ThoughtSource::External,
        ];
        assert_eq!(sources.len(), 6);
    }

    #[test]
    fn thought_serialization_roundtrip() {
        let thought = Thought::new(Uuid::new_v4(), "test", ThoughtSource::Autonomous);
        let json = serde_json::to_string(&thought).unwrap();
        let deserialized: Thought = serde_json::from_str(&json).unwrap();
        assert_eq!(thought, deserialized);
    }
}
