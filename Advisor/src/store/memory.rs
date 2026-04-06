use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A persistent memory with optional embedding for semantic search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Memory {
    pub id: Uuid,
    /// The memory content
    pub content: String,
    /// Embedding vector (if generated)
    pub embedding: Option<Vec<f32>>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    /// How many times this memory has been recalled
    pub access_count: u32,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Memory {
    pub fn new(content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            embedding: None,
            created_at: now,
            last_accessed_at: now,
            access_count: 0,
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Record that this memory was accessed (recalled).
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed_at = Utc::now();
    }

    /// Whether this memory has an embedding for semantic search.
    pub fn has_embedding(&self) -> bool {
        self.embedding.is_some()
    }
}

/// Result of a memory search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryResult {
    pub content: String,
    /// Relevance score (0.0–1.0)
    pub relevance: f64,
    pub memory_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_creation() {
        let mem = Memory::new("the sky is blue");
        assert_eq!(mem.content, "the sky is blue");
        assert_eq!(mem.access_count, 0);
        assert!(!mem.has_embedding());
    }

    #[test]
    fn memory_with_tags_and_embedding() {
        let mem = Memory::new("design pattern")
            .with_tags(vec!["architecture".into(), "patterns".into()])
            .with_embedding(vec![0.1, 0.2, 0.3]);
        assert_eq!(mem.tags.len(), 2);
        assert!(mem.has_embedding());
    }

    #[test]
    fn record_access() {
        let mut mem = Memory::new("something");
        let before = mem.last_accessed_at;
        mem.record_access();
        assert_eq!(mem.access_count, 1);
        assert!(mem.last_accessed_at >= before);
        mem.record_access();
        assert_eq!(mem.access_count, 2);
    }

    #[test]
    fn memory_serialization_roundtrip() {
        let mem = Memory::new("test").with_tags(vec!["tag".into()]);
        let json = serde_json::to_string(&mem).unwrap();
        let deserialized: Memory = serde_json::from_str(&json).unwrap();
        assert_eq!(mem, deserialized);
    }
}
