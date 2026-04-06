//! # Federation Scope -- Data Boundary for Gamification
//!
//! From Constellation Art. 3 §3 -- federation is a data boundary.
//!
//! When communities federate, cooperative challenges, group achievements,
//! raids, and leaderboards can span the federation. `FederationScope`
//! controls which communities' gamification data is visible.
//!
//! When set, only activities from visible communities are returned by
//! scoped queries. When empty (unrestricted), all activities are visible --
//! this is the default and preserves full backward compatibility.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Optional federation scope for gamification operations.
///
/// When set, cooperative activities and leaderboards are scoped to
/// federated communities. When empty, all communities are visible.
///
/// From Constellation Art. 3 §3 -- federation is a data boundary.
///
/// # Examples
///
/// ```
/// use quest::FederationScope;
///
/// // Unrestricted -- sees everything.
/// let open = FederationScope::new();
/// assert!(open.is_unrestricted());
/// assert!(open.is_visible("any_community"));
///
/// // Scoped to specific communities.
/// let scoped = FederationScope::from_communities(["alpha", "beta"]);
/// assert!(!scoped.is_unrestricted());
/// assert!(scoped.is_visible("alpha"));
/// assert!(!scoped.is_visible("gamma"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationScope {
    visible_communities: HashSet<String>,
}

impl Default for FederationScope {
    fn default() -> Self {
        Self::new()
    }
}

impl FederationScope {
    /// Create an unrestricted scope -- all communities are visible.
    pub fn new() -> Self {
        Self {
            visible_communities: HashSet::new(),
        }
    }

    /// Create a scope limited to the given communities.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Whether a community is visible within this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty) or if
    /// the community is in the visible set.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Whether a community (identified by UUID) is visible within this scope.
    ///
    /// Convenience method for types that use `Uuid` for community IDs.
    /// Converts the UUID to its hyphenated string representation for lookup.
    pub fn is_visible_uuid(&self, community_id: &uuid::Uuid) -> bool {
        self.is_visible(&community_id.to_string())
    }

    /// Whether this scope has no restrictions (all communities visible).
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the visible set.
    ///
    /// Returns 0 for unrestricted scopes -- this does NOT mean "no communities",
    /// it means "all communities". Use [`is_unrestricted`](Self::is_unrestricted)
    /// to distinguish.
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty (i.e., unrestricted).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Filter a slice of community IDs to only those visible in this scope.
    ///
    /// Returns all items if unrestricted.
    pub fn filter_communities<'a>(&self, community_ids: &'a [String]) -> Vec<&'a String> {
        if self.is_unrestricted() {
            community_ids.iter().collect()
        } else {
            community_ids
                .iter()
                .filter(|id| self.visible_communities.contains(id.as_str()))
                .collect()
        }
    }

    /// Add a community to the visible set.
    ///
    /// Returns `true` if the community was newly inserted.
    pub fn add_community(&mut self, community_id: impl Into<String>) -> bool {
        self.visible_communities.insert(community_id.into())
    }

    /// Remove a community from the visible set.
    ///
    /// Returns `true` if the community was present and removed.
    pub fn remove_community(&mut self, community_id: &str) -> bool {
        self.visible_communities.remove(community_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrestricted_scope_sees_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
        assert!(scope.is_visible("any_community"));
        assert!(scope.is_visible("another_community"));
    }

    #[test]
    fn scoped_scope_filters_communities() {
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        assert!(!scope.is_unrestricted());
        assert!(!scope.is_empty());
        assert_eq!(scope.len(), 2);
        assert!(scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert!(!scope.is_visible("gamma"));
        assert!(!scope.is_visible("delta"));
    }

    #[test]
    fn default_is_unrestricted() {
        let scope = FederationScope::default();
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn add_and_remove_communities() {
        let mut scope = FederationScope::new();
        assert!(scope.is_unrestricted());

        assert!(scope.add_community("alpha"));
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("alpha"));
        assert!(!scope.is_visible("beta"));

        // Adding again returns false.
        assert!(!scope.add_community("alpha"));

        assert!(scope.remove_community("alpha"));
        assert!(scope.is_unrestricted());

        // Removing nonexistent returns false.
        assert!(!scope.remove_community("alpha"));
    }

    #[test]
    fn filter_communities_unrestricted() {
        let scope = FederationScope::new();
        let ids: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let filtered = scope.filter_communities(&ids);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn filter_communities_scoped() {
        let scope = FederationScope::from_communities(["a", "c"]);
        let ids: Vec<String> = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let filtered = scope.filter_communities(&ids);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&&"a".to_string()));
        assert!(filtered.contains(&&"c".to_string()));
    }

    #[test]
    fn empty_communities_iterator() {
        let scope = FederationScope::from_communities(Vec::<String>::new());
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn is_visible_uuid() {
        let id = uuid::Uuid::new_v4();
        let scope = FederationScope::from_communities([id.to_string()]);
        assert!(scope.is_visible_uuid(&id));

        let other = uuid::Uuid::new_v4();
        assert!(!scope.is_visible_uuid(&other));
    }

    #[test]
    fn is_visible_uuid_unrestricted() {
        let scope = FederationScope::new();
        assert!(scope.is_visible_uuid(&uuid::Uuid::new_v4()));
    }

    #[test]
    fn serialization_roundtrip() {
        let scope = FederationScope::from_communities(["alpha", "beta", "gamma"]);
        let json = serde_json::to_string(&scope).expect("serialize");
        let deserialized: FederationScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, deserialized);
    }

    #[test]
    fn unrestricted_serialization_roundtrip() {
        let scope = FederationScope::new();
        let json = serde_json::to_string(&scope).expect("serialize");
        let deserialized: FederationScope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(scope, deserialized);
        assert!(deserialized.is_unrestricted());
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FederationScope>();
    }
}
