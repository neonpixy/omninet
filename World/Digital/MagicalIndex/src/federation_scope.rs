//! Federation scoping — controls which communities' content is searchable.
//!
//! In a federated Omninet, a Tower node may index content from many
//! communities. `FederationScope` restricts query results to content
//! belonging to specific communities, based on the `"community"` tag
//! on indexed events.
//!
//! An empty (unrestricted) scope matches all content, including events
//! with no community tag. A non-empty scope matches only events tagged
//! with at least one of the visible communities.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Controls which communities' content is visible in search results.
///
/// Used as a filter for scoped query methods on `KeywordIndex`.
/// An empty scope is unrestricted — all content is visible.
/// A non-empty scope limits results to events tagged with one of the
/// listed community IDs.
///
/// # Examples
///
/// ```
/// use magical_index::FederationScope;
///
/// // Unrestricted — sees everything.
/// let scope = FederationScope::new();
/// assert!(scope.is_unrestricted());
/// assert!(scope.is_visible("any-community"));
///
/// // Scoped to specific communities.
/// let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
/// assert!(scope.is_visible("guild-a"));
/// assert!(!scope.is_visible("guild-c"));
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationScope {
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

    /// Check whether content from `community_id` is visible in this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty) or if
    /// `community_id` is in the visible set.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Whether this scope imposes no restrictions.
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the scope (0 = unrestricted).
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty (equivalent to `is_unrestricted`).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Add a community to the visible set.
    pub fn add_community(&mut self, community_id: impl Into<String>) {
        self.visible_communities.insert(community_id.into());
    }

    /// Remove a community from the visible set.
    pub fn remove_community(&mut self, community_id: &str) -> bool {
        self.visible_communities.remove(community_id)
    }

    /// Get an iterator over visible community IDs.
    pub fn communities(&self) -> impl Iterator<Item = &str> {
        self.visible_communities.iter().map(|s| s.as_str())
    }

    /// Build SQL condition fragments for filtering by community tag.
    ///
    /// Returns `None` for unrestricted scopes (no filtering needed).
    /// For restricted scopes, returns the SQL condition and parameter values.
    pub(crate) fn sql_condition(
        &self,
        param_offset: usize,
    ) -> Option<(String, Vec<Box<dyn rusqlite::types::ToSql>>)> {
        if self.visible_communities.is_empty() {
            return None;
        }

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let community_key_idx = param_offset + 1;
        params.push(Box::new("community".to_string()));

        let placeholders: Vec<String> = self
            .visible_communities
            .iter()
            .enumerate()
            .map(|(i, community)| {
                let idx = param_offset + 2 + i;
                params.push(Box::new(community.clone()));
                format!("?{idx}")
            })
            .collect();

        let condition = format!(
            "EXISTS (SELECT 1 FROM search_tags st_fed WHERE st_fed.event_id = m.event_id AND st_fed.tag_key = ?{community_key_idx} AND st_fed.tag_value IN ({}))",
            placeholders.join(",")
        );

        Some((condition, params))
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
    fn from_communities_restricts() {
        let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
        assert!(!scope.is_unrestricted());
        assert_eq!(scope.len(), 2);
        assert!(scope.is_visible("guild-a"));
        assert!(scope.is_visible("guild-b"));
        assert!(!scope.is_visible("guild-c"));
    }

    #[test]
    fn unrestricted_sees_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_visible("any-community"));
        assert!(scope.is_visible(""));
    }

    #[test]
    fn add_and_remove_community() {
        let mut scope = FederationScope::new();
        scope.add_community("guild-a");
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("guild-a"));

        assert!(scope.remove_community("guild-a"));
        assert!(scope.is_unrestricted());
        assert!(!scope.remove_community("nonexistent"));
    }

    #[test]
    fn communities_iterator() {
        let scope = FederationScope::from_communities(["a", "b", "c"]);
        let mut ids: Vec<&str> = scope.communities().collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn serde_round_trip() {
        let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
        let json = serde_json::to_string(&scope).unwrap();
        let loaded: FederationScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, loaded);
    }

    #[test]
    fn serde_empty_round_trip() {
        let scope = FederationScope::new();
        let json = serde_json::to_string(&scope).unwrap();
        let loaded: FederationScope = serde_json::from_str(&json).unwrap();
        assert!(loaded.is_unrestricted());
    }

    #[test]
    fn sql_condition_none_for_unrestricted() {
        let scope = FederationScope::new();
        assert!(scope.sql_condition(0).is_none());
    }

    #[test]
    fn sql_condition_generates_for_restricted() {
        let scope = FederationScope::from_communities(["guild-a"]);
        let (condition, params) = scope.sql_condition(0).unwrap();
        assert!(condition.contains("st_fed.tag_key = ?1"));
        assert!(condition.contains("st_fed.tag_value IN (?2)"));
        assert_eq!(params.len(), 2); // "community" key + 1 community value
    }

    #[test]
    fn sql_condition_respects_offset() {
        let scope = FederationScope::from_communities(["guild-a", "guild-b"]);
        let (condition, params) = scope.sql_condition(5).unwrap();
        assert!(condition.contains("?6")); // community key at offset+1
        assert!(condition.contains("?7")); // first community value
        assert!(condition.contains("?8")); // second community value
        assert_eq!(params.len(), 3); // key + 2 values
    }

    #[test]
    fn deduplicates_communities() {
        let scope = FederationScope::from_communities(["guild-a", "guild-a", "guild-b"]);
        assert_eq!(scope.len(), 2);
    }
}
