//! Federation scope — data visibility boundary for cross-community queries.
//!
//! From Constellation Art. 3 section 3 — federation is a data boundary. When communities
//! defederate, trust data (edges, flags, verifications) from those communities
//! becomes invisible. FederationScope captures which communities are visible
//! to a given query, enabling filtered views without modifying the underlying data.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Optional federation scope for filtering query results.
///
/// When set, only data from visible communities is returned.
/// When empty (default), all data is visible (backward compatible).
///
/// From Constellation Art. 3 section 3 — federation is a data boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationScope {
    /// Communities visible to this query.
    /// If empty, all communities are visible (no filtering).
    visible_communities: HashSet<String>,
}

impl FederationScope {
    /// Create an unrestricted scope (all communities visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope restricted to specific communities.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Check if a community is visible in this scope.
    ///
    /// Returns `true` if scope is unrestricted (empty) or community is in the set.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Check if a community_id (possibly `None`) is visible in this scope.
    ///
    /// Flags without a community_id are always visible — they are personal,
    /// not community-scoped, and federation doesn't affect them.
    pub fn is_visible_opt(&self, community_id: Option<&str>) -> bool {
        match community_id {
            None => true, // No community = personal, always visible.
            Some(id) => self.is_visible(id),
        }
    }

    /// Check if scope is unrestricted (empty = see everything).
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the visible set.
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty (unrestricted).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unrestricted() {
        let scope = FederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
    }

    #[test]
    fn unrestricted_sees_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_visible("any_community"));
        assert!(scope.is_visible("another_one"));
    }

    #[test]
    fn restricted_scope_filters() {
        let scope = FederationScope::from_communities(["comm_a", "comm_b"]);
        assert!(!scope.is_unrestricted());
        assert_eq!(scope.len(), 2);

        assert!(scope.is_visible("comm_a"));
        assert!(scope.is_visible("comm_b"));
        assert!(!scope.is_visible("comm_c"));
        assert!(!scope.is_visible("defederated"));
    }

    #[test]
    fn is_visible_opt_with_none() {
        let scope = FederationScope::from_communities(["comm_a"]);
        // Flags without community_id are always visible.
        assert!(scope.is_visible_opt(None));
    }

    #[test]
    fn is_visible_opt_with_some() {
        let scope = FederationScope::from_communities(["comm_a"]);
        assert!(scope.is_visible_opt(Some("comm_a")));
        assert!(!scope.is_visible_opt(Some("comm_b")));
    }

    #[test]
    fn from_communities_deduplicates() {
        let scope = FederationScope::from_communities(["comm_a", "comm_a", "comm_b"]);
        assert_eq!(scope.len(), 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let scope = FederationScope::from_communities(["comm_a", "comm_b"]);
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: FederationScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn empty_scope_serialization_roundtrip() {
        let scope = FederationScope::new();
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: FederationScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, deserialized);
    }
}
