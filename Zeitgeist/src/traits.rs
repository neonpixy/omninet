//! The DiscoveryProvider trait — the core abstraction.
//!
//! Default implementation routes to Towers via the directory and router.
//! Could be extended with local-only search, community-specific search,
//! AI-assisted search, or anything else.

use crate::error::ZeitgeistError;
use crate::merger::MergedResponse;
use crate::trending::TrendSignal;

/// Configuration for a discovery query.
#[derive(Clone, Debug)]
pub struct DiscoveryQuery {
    /// The search text.
    pub text: String,
    /// Maximum results to return.
    pub limit: usize,
    /// Filter by event kinds.
    pub kinds: Option<Vec<u32>>,
    /// Filter by community pubkey.
    pub community: Option<String>,
    /// Whether to include cached results (default: true).
    pub use_cache: bool,
}

impl DiscoveryQuery {
    /// Create a simple text search query.
    pub fn search(text: &str) -> Self {
        Self {
            text: text.into(),
            limit: 20,
            kinds: None,
            community: None,
            use_cache: true,
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

    /// Filter by community.
    pub fn with_community(mut self, community: &str) -> Self {
        self.community = Some(community.into());
        self
    }

    /// Disable cache.
    pub fn without_cache(mut self) -> Self {
        self.use_cache = false;
        self
    }
}

/// A category for browsing (not search — curated discovery).
#[derive(Clone, Debug)]
pub struct BrowseCategory {
    /// Category name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Number of items in this category (approximate).
    pub item_count: u64,
}

/// A pluggable discovery strategy.
///
/// The default implementation routes queries to Towers via the directory.
/// Other implementations could provide local-only search, community-scoped
/// search, or AI-assisted discovery.
pub trait DiscoveryProvider: Send + Sync {
    /// Search for content across the network.
    ///
    /// Routes to the most relevant Towers, queries them, merges results.
    fn search(&self, query: &DiscoveryQuery) -> Result<MergedResponse, ZeitgeistError>;

    /// Browse available categories.
    ///
    /// Returns curated categories for exploration (not search).
    fn browse(&self) -> Result<Vec<BrowseCategory>, ZeitgeistError>;

    /// Get trending topics.
    ///
    /// Returns the top N trending signals from the network.
    fn trending(&self, count: usize) -> Result<Vec<TrendSignal>, ZeitgeistError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merger::MergedResponse;

    /// Test implementation to verify trait is object-safe.
    struct NullProvider;

    impl DiscoveryProvider for NullProvider {
        fn search(&self, _query: &DiscoveryQuery) -> Result<MergedResponse, ZeitgeistError> {
            Ok(MergedResponse::default())
        }
        fn browse(&self) -> Result<Vec<BrowseCategory>, ZeitgeistError> {
            Ok(vec![])
        }
        fn trending(&self, _count: usize) -> Result<Vec<TrendSignal>, ZeitgeistError> {
            Ok(vec![])
        }
    }

    #[test]
    fn trait_is_object_safe() {
        let provider: Box<dyn DiscoveryProvider> = Box::new(NullProvider);
        let result = provider.search(&DiscoveryQuery::search("test")).unwrap();
        assert!(result.results.is_empty());
    }

    #[test]
    fn query_builder() {
        let q = DiscoveryQuery::search("woodworking")
            .with_limit(10)
            .with_kinds(vec![1])
            .with_community("abc123");
        assert_eq!(q.text, "woodworking");
        assert_eq!(q.limit, 10);
        assert_eq!(q.kinds, Some(vec![1]));
        assert_eq!(q.community, Some("abc123".into()));
        assert!(q.use_cache);
    }

    #[test]
    fn query_without_cache() {
        let q = DiscoveryQuery::search("test").without_cache();
        assert!(!q.use_cache);
    }

    #[test]
    fn browse_returns_empty() {
        let provider = NullProvider;
        let categories = provider.browse().unwrap();
        assert!(categories.is_empty());
    }

    #[test]
    fn trending_returns_empty() {
        let provider = NullProvider;
        let trends = provider.trending(10).unwrap();
        assert!(trends.is_empty());
    }
}
