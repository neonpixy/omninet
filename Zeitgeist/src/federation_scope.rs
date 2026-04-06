//! Federation scoping — controls which Towers and trends are visible
//! based on federated community membership.
//!
//! A `FederationScope` restricts discovery to Towers that serve
//! specific communities and trends from those communities. When
//! unrestricted (empty visible set), all communities are visible.
//!
//! Used by `_scoped` methods on `TowerDirectory`, `QueryRouter`,
//! and `GlobalTrendTracker` to filter results without modifying
//! existing APIs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Restricts discovery to Towers and trends from specific communities.
///
/// An empty `visible_communities` set means unrestricted — everything
/// is visible. This is the default and acts as a fast-path passthrough.
///
/// # Examples
///
/// ```
/// use zeitgeist::FederationScope;
///
/// // Unrestricted — sees everything.
/// let scope = FederationScope::new();
/// assert!(scope.is_visible("any-community"));
///
/// // Scoped to specific communities.
/// let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
/// assert!(scope.is_visible("guild-a"));
/// assert!(!scope.is_visible("guild-c"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FederationScope {
    /// Communities visible under this scope. Empty = unrestricted.
    visible_communities: HashSet<String>,
}

impl FederationScope {
    /// Create an unrestricted scope (all communities visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope restricted to the given communities.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Whether a community is visible under this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty set)
    /// or if the community is in the visible set.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Whether this scope is unrestricted (sees all communities).
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the scope (0 = unrestricted).
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty (which means unrestricted).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Add a community to the visible set.
    pub fn add_community(&mut self, community_id: impl Into<String>) {
        self.visible_communities.insert(community_id.into());
    }

    /// Remove a community from the visible set.
    pub fn remove_community(&mut self, community_id: &str) {
        self.visible_communities.remove(community_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_unrestricted() {
        let scope = FederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
    }

    #[test]
    fn unrestricted_sees_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_visible("any-community"));
        assert!(scope.is_visible("another-community"));
    }

    #[test]
    fn from_communities_restricts() {
        let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
        assert!(!scope.is_unrestricted());
        assert_eq!(scope.len(), 2);
        assert!(scope.is_visible("guild-a"));
        assert!(scope.is_visible("guild-b"));
        assert!(!scope.is_visible("guild-c"));
    }

    #[test]
    fn from_communities_string_owned() {
        let scope =
            FederationScope::from_communities(vec!["a".to_string(), "b".to_string()]);
        assert!(scope.is_visible("a"));
        assert!(scope.is_visible("b"));
        assert!(!scope.is_visible("c"));
    }

    #[test]
    fn from_empty_iterator_is_unrestricted() {
        let scope = FederationScope::from_communities(Vec::<String>::new());
        assert!(scope.is_unrestricted());
        assert!(scope.is_visible("anything"));
    }

    #[test]
    fn add_community() {
        let mut scope = FederationScope::new();
        assert!(scope.is_unrestricted());

        scope.add_community("guild-a");
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("guild-a"));
        assert!(!scope.is_visible("guild-b"));
    }

    #[test]
    fn remove_community() {
        let mut scope = FederationScope::from_communities(["guild-a", "guild-b"]);
        scope.remove_community("guild-a");

        assert_eq!(scope.len(), 1);
        assert!(!scope.is_visible("guild-a"));
        assert!(scope.is_visible("guild-b"));
    }

    #[test]
    fn remove_last_community_becomes_unrestricted() {
        let mut scope = FederationScope::from_communities(["guild-a"]);
        scope.remove_community("guild-a");

        assert!(scope.is_unrestricted());
        assert!(scope.is_visible("anything"));
    }

    #[test]
    fn default_is_unrestricted() {
        let scope = FederationScope::default();
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn serde_round_trip_unrestricted() {
        let scope = FederationScope::new();
        let json = serde_json::to_string(&scope).unwrap();
        let restored: FederationScope = serde_json::from_str(&json).unwrap();
        assert!(restored.is_unrestricted());
    }

    #[test]
    fn serde_round_trip_scoped() {
        let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
        let json = serde_json::to_string(&scope).unwrap();
        let restored: FederationScope = serde_json::from_str(&json).unwrap();
        assert!(!restored.is_unrestricted());
        assert!(restored.is_visible("guild-a"));
        assert!(restored.is_visible("guild-b"));
        assert!(!restored.is_visible("guild-c"));
    }

    #[test]
    fn duplicate_communities_deduped() {
        let scope = FederationScope::from_communities(["a", "a", "b"]);
        assert_eq!(scope.len(), 2);
    }
}
