//! The SearchIndex trait — the core abstraction.
//!
//! Any search backend implements this trait. KeywordIndex (FTS5) is the
//! default. Semantic index wraps a CognitiveProvider from Advisor.
//! Both can compose (keyword + semantic results merged and ranked).

use globe::event::OmniEvent;

use crate::error::MagicalError;
use crate::query::{SearchQuery, SearchResponse};

/// A pluggable search index backend.
///
/// Implementations may be keyword-based (FTS5), semantic (vector embeddings),
/// or composite (both). The trait is the contract — implementations compete.
///
/// Default methods return empty results so partial implementations
/// (keyword-only, semantic-only) work without stubs.
pub trait SearchIndex: Send + Sync {
    /// Index an event's content for future search.
    ///
    /// Extracts searchable text from the event's content and tags,
    /// stores it in the index. Duplicate event IDs are silently ignored.
    fn index_event(&self, event: &OmniEvent) -> Result<(), MagicalError>;

    /// Remove an event from the index. Returns true if it existed.
    fn remove_event(&self, event_id: &str) -> Result<bool, MagicalError>;

    /// Execute a search query. Returns results ranked by relevance.
    fn search(&self, query: &SearchQuery) -> Result<SearchResponse, MagicalError>;

    /// Number of indexed events.
    fn indexed_count(&self) -> Result<usize, MagicalError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial implementation for testing the trait is object-safe.
    struct NullIndex;

    impl SearchIndex for NullIndex {
        fn index_event(&self, _event: &OmniEvent) -> Result<(), MagicalError> {
            Ok(())
        }
        fn remove_event(&self, _event_id: &str) -> Result<bool, MagicalError> {
            Ok(false)
        }
        fn search(&self, _query: &SearchQuery) -> Result<SearchResponse, MagicalError> {
            Ok(SearchResponse::default())
        }
        fn indexed_count(&self) -> Result<usize, MagicalError> {
            Ok(0)
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let index: Box<dyn SearchIndex> = Box::new(NullIndex);
        assert_eq!(index.indexed_count().unwrap(), 0);
    }

    #[test]
    fn null_index_search_returns_empty() {
        let index = NullIndex;
        let response = index.search(&SearchQuery::new("test")).unwrap();
        assert!(response.results.is_empty());
    }
}
