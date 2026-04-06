use serde::{Deserialize, Serialize};

use super::impulse::Thought;
use crate::error::AdvisorError;

/// Streaming chunks from an ongoing thought generation.
///
/// Used when the advisor is generating a thought via an LLM and
/// wants to stream partial results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThoughtChunk {
    /// A partial text fragment
    Text(String),
    /// A completed thought
    Complete(Thought),
    /// A skill started executing
    SkillStarted {
        skill_id: String,
        skill_name: String,
    },
    /// A skill finished
    SkillCompleted {
        skill_id: String,
        result: String,
    },
    /// An error occurred during generation
    Error(String),
}

impl ThoughtChunk {
    /// Whether this chunk indicates the stream is finished.
    pub fn is_terminal(&self) -> bool {
        matches!(self, ThoughtChunk::Complete(_) | ThoughtChunk::Error(_))
    }

    /// Extract text content if this is a text chunk.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ThoughtChunk::Text(s) => Some(s),
            _ => None,
        }
    }
}

impl From<AdvisorError> for ThoughtChunk {
    fn from(err: AdvisorError) -> Self {
        ThoughtChunk::Error(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thought::impulse::{Thought, ThoughtSource};
    use uuid::Uuid;

    #[test]
    fn text_chunk_is_not_terminal() {
        let chunk = ThoughtChunk::Text("partial...".into());
        assert!(!chunk.is_terminal());
        assert_eq!(chunk.as_text(), Some("partial..."));
    }

    #[test]
    fn complete_chunk_is_terminal() {
        let thought = Thought::new(Uuid::new_v4(), "done", ThoughtSource::Autonomous);
        let chunk = ThoughtChunk::Complete(thought);
        assert!(chunk.is_terminal());
        assert!(chunk.as_text().is_none());
    }

    #[test]
    fn error_chunk_is_terminal() {
        let chunk = ThoughtChunk::Error("failed".into());
        assert!(chunk.is_terminal());
    }

    #[test]
    fn skill_chunks() {
        let started = ThoughtChunk::SkillStarted {
            skill_id: "search".into(),
            skill_name: "Web Search".into(),
        };
        assert!(!started.is_terminal());

        let completed = ThoughtChunk::SkillCompleted {
            skill_id: "search".into(),
            result: "found 3 results".into(),
        };
        assert!(!completed.is_terminal());
    }

    #[test]
    fn error_conversion() {
        let err = AdvisorError::GenerationFailed("timeout".into());
        let chunk: ThoughtChunk = err.into();
        assert!(matches!(chunk, ThoughtChunk::Error(_)));
    }
}
