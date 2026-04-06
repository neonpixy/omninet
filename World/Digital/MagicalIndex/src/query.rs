//! Search queries and results.

use serde::{Deserialize, Serialize};

/// A search query with optional filters.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The search text (required).
    pub text: String,
    /// Filter by event kinds.
    pub kinds: Option<Vec<u32>>,
    /// Filter by authors (pubkey hex).
    pub authors: Option<Vec<String>>,
    /// Events created after this timestamp.
    pub since: Option<i64>,
    /// Events created before this timestamp.
    pub until: Option<i64>,
    /// Maximum results to return (default: 20).
    pub limit: usize,
}

impl SearchQuery {
    /// Create a simple text search query.
    pub fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            limit: 20,
            ..Default::default()
        }
    }

    /// Set the result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Filter by event kinds.
    pub fn with_kinds(mut self, kinds: Vec<u32>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    /// Filter by authors.
    pub fn with_authors(mut self, authors: Vec<String>) -> Self {
        self.authors = Some(authors);
        self
    }

    /// Filter by time range.
    pub fn with_time_range(mut self, since: Option<i64>, until: Option<i64>) -> Self {
        self.since = since;
        self.until = until;
        self
    }
}

/// A single search result with relevance scoring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matching event's ID.
    pub event_id: String,
    /// The event's author (pubkey hex).
    pub author: String,
    /// The event kind.
    pub kind: u32,
    /// When the event was created (Unix timestamp).
    pub created_at: i64,
    /// Relevance score (0.0 = irrelevant, higher = more relevant).
    /// Keyword search uses BM25; semantic search uses cosine similarity.
    pub relevance: f64,
    /// Text snippet with matched terms highlighted (if available).
    pub snippet: Option<String>,
    /// Concept suggestions based on the match (semantic search only).
    pub suggestions: Vec<String>,
}

/// Aggregated search response.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchResponse {
    /// The results, ordered by relevance (highest first).
    pub results: Vec<SearchResult>,
    /// Total number of matches (may be more than results.len() due to limit).
    pub total_matches: usize,
    /// Suggested related concepts (from semantic index, if available).
    pub suggestions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_builder() {
        let q = SearchQuery::new("woodworking")
            .with_limit(10)
            .with_kinds(vec![1]);
        assert_eq!(q.text, "woodworking");
        assert_eq!(q.limit, 10);
        assert_eq!(q.kinds, Some(vec![1]));
    }

    #[test]
    fn query_defaults() {
        let q = SearchQuery::new("test");
        assert_eq!(q.limit, 20);
        assert!(q.kinds.is_none());
        assert!(q.authors.is_none());
        assert!(q.since.is_none());
    }

    #[test]
    fn query_serde_round_trip() {
        let q = SearchQuery::new("hello").with_limit(5);
        let json = serde_json::to_string(&q).unwrap();
        let loaded: SearchQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.text, "hello");
        assert_eq!(loaded.limit, 5);
    }

    #[test]
    fn result_serde_round_trip() {
        let r = SearchResult {
            event_id: "abc123".into(),
            author: "def456".into(),
            kind: 1,
            created_at: 1000,
            relevance: 0.85,
            snippet: Some("matched **text**".into()),
            suggestions: vec!["related".into()],
        };
        let json = serde_json::to_string(&r).unwrap();
        let loaded: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.event_id, "abc123");
        assert_eq!(loaded.relevance, 0.85);
    }

    #[test]
    fn response_defaults_empty() {
        let r = SearchResponse::default();
        assert!(r.results.is_empty());
        assert_eq!(r.total_matches, 0);
        assert!(r.suggestions.is_empty());
    }
}
