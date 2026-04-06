//! Federation scoping -- controls which communities are visible for module routing.
//!
//! `FederationScope` determines which modules are reachable based on community
//! federation status. Programs in defederated communities become unreachable --
//! their Phone handlers, Email events, and Communicator channels are filtered
//! out of discovery and routing.
//!
//! An empty scope means unrestricted (all communities visible). A populated
//! scope acts as an allowlist -- only listed communities participate in
//! communication.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Controls which communities are visible for module routing and discovery.
///
/// Used to filter Contacts queries, Phone routing, and Email delivery so that
/// modules from defederated communities are not reachable.
///
/// # Semantics
///
/// - **Empty** (`is_unrestricted() == true`): all communities are visible.
///   This is the default -- no federation restrictions in place.
/// - **Populated**: only the listed community IDs are visible. Any module
///   associated with a community not in the set will be filtered out.
///
/// # Module visibility rules
///
/// - Modules **without** a `community_id` are always visible -- they are
///   system-level modules (e.g., Sentinal, Crown) that belong to no community.
/// - Modules **with** a `community_id` are only visible if that community is
///   in the scope.
///
/// # Examples
///
/// ```
/// use equipment::FederationScope;
///
/// // Unrestricted -- everything passes.
/// let scope = FederationScope::new();
/// assert!(scope.is_visible("any-community"));
/// assert!(scope.is_unrestricted());
///
/// // Scoped to specific communities.
/// let scope = FederationScope::from_communities(["alpha", "beta"]);
/// assert!(scope.is_visible("alpha"));
/// assert!(!scope.is_visible("gamma"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FederationScope {
    /// Communities visible to this scope.
    /// If empty, all communities are visible (no filtering).
    visible_communities: HashSet<String>,
}

impl FederationScope {
    /// Create an unrestricted scope (all communities visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope from an iterator of community IDs.
    ///
    /// Only the listed communities will be visible for routing.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Check if a community is visible under this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty) or if the
    /// community ID is in the allowlist.
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Check if a community_id (possibly `None`) is visible in this scope.
    ///
    /// Modules without a community_id are always visible -- they are
    /// system-level modules, not community-scoped, and federation
    /// doesn't affect them.
    pub fn is_visible_opt(&self, community_id: Option<&str>) -> bool {
        match community_id {
            None => true, // No community = system-level, always visible.
            Some(id) => self.is_visible(id),
        }
    }

    /// Whether this scope imposes no restrictions.
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the allowlist.
    ///
    /// Returns 0 for an unrestricted scope.
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the allowlist is empty (unrestricted).
    pub fn is_empty(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Add a community to the allowlist.
    pub fn add_community(&mut self, community_id: impl Into<String>) {
        self.visible_communities.insert(community_id.into());
    }

    /// Remove a community from the allowlist (defederate it).
    ///
    /// Returns `true` if the community was present.
    pub fn remove_community(&mut self, community_id: &str) -> bool {
        self.visible_communities.remove(community_id)
    }

    /// Iterate over the visible community IDs.
    pub fn communities(&self) -> impl Iterator<Item = &str> {
        self.visible_communities.iter().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_scope_is_unrestricted() {
        let scope = FederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_empty());
        assert_eq!(scope.len(), 0);
    }

    #[test]
    fn default_scope_is_unrestricted() {
        let scope = FederationScope::default();
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn unrestricted_scope_allows_everything() {
        let scope = FederationScope::new();
        assert!(scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert!(scope.is_visible(""));
        assert!(scope.is_visible("any-random-community"));
    }

    #[test]
    fn from_communities_creates_allowlist() {
        let scope = FederationScope::from_communities(["alpha", "beta", "gamma"]);
        assert!(!scope.is_unrestricted());
        assert_eq!(scope.len(), 3);
        assert!(scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert!(scope.is_visible("gamma"));
        assert!(!scope.is_visible("delta"));
        assert!(!scope.is_visible(""));
    }

    #[test]
    fn from_communities_with_strings() {
        let communities = vec!["one".to_string(), "two".to_string()];
        let scope = FederationScope::from_communities(communities);
        assert_eq!(scope.len(), 2);
        assert!(scope.is_visible("one"));
        assert!(scope.is_visible("two"));
    }

    #[test]
    fn from_empty_iterator_is_unrestricted() {
        let scope = FederationScope::from_communities(Vec::<String>::new());
        assert!(scope.is_unrestricted());
        assert!(scope.is_visible("anything"));
    }

    #[test]
    fn is_visible_opt_with_none() {
        let scope = FederationScope::from_communities(["alpha"]);
        // System-level modules (no community) are always visible.
        assert!(scope.is_visible_opt(None));
    }

    #[test]
    fn is_visible_opt_with_some_visible() {
        let scope = FederationScope::from_communities(["alpha"]);
        assert!(scope.is_visible_opt(Some("alpha")));
    }

    #[test]
    fn is_visible_opt_with_some_not_visible() {
        let scope = FederationScope::from_communities(["alpha"]);
        assert!(!scope.is_visible_opt(Some("beta")));
    }

    #[test]
    fn is_visible_opt_unrestricted_allows_all() {
        let scope = FederationScope::new();
        assert!(scope.is_visible_opt(None));
        assert!(scope.is_visible_opt(Some("anything")));
    }

    #[test]
    fn add_community() {
        let mut scope = FederationScope::new();
        assert!(scope.is_unrestricted());

        scope.add_community("alpha");
        assert!(!scope.is_unrestricted());
        assert_eq!(scope.len(), 1);
        assert!(scope.is_visible("alpha"));
        assert!(!scope.is_visible("beta"));
    }

    #[test]
    fn remove_community() {
        let mut scope = FederationScope::from_communities(["alpha", "beta"]);
        assert_eq!(scope.len(), 2);

        assert!(scope.remove_community("alpha"));
        assert_eq!(scope.len(), 1);
        assert!(!scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));

        // Removing non-existent returns false.
        assert!(!scope.remove_community("gamma"));
    }

    #[test]
    fn remove_all_returns_to_unrestricted() {
        let mut scope = FederationScope::from_communities(["alpha"]);
        assert!(!scope.is_unrestricted());

        scope.remove_community("alpha");
        assert!(scope.is_unrestricted());
        assert!(scope.is_visible("anything"));
    }

    #[test]
    fn communities_iterator() {
        let scope = FederationScope::from_communities(["alpha", "beta"]);
        let mut ids: Vec<&str> = scope.communities().collect();
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[test]
    fn clone_is_independent() {
        let mut scope = FederationScope::from_communities(["alpha"]);
        let cloned = scope.clone();

        scope.add_community("beta");
        assert!(scope.is_visible("beta"));
        assert!(!cloned.is_visible("beta"));
    }

    #[test]
    fn deduplicates_communities() {
        let scope = FederationScope::from_communities(["alpha", "alpha", "beta"]);
        assert_eq!(scope.len(), 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let scope = FederationScope::from_communities(["alpha", "beta"]);
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
