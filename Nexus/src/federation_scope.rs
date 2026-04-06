//! Federation scoping -- controls which communities are visible for bridging.
//!
//! `FederationScope` determines which bridges are active based on community
//! federation status. Defederated communities should not have their content
//! bridged to external protocols (no SMTP, no ActivityPub, no RSS).
//!
//! An empty scope means unrestricted (all communities visible). A populated
//! scope acts as an allowlist -- only listed communities participate in
//! federation.

use std::collections::HashSet;

/// Controls which communities are visible for federation bridging.
///
/// Used to filter bridge/export operations so that content from defederated
/// communities is not forwarded to external protocols.
///
/// # Semantics
///
/// - **Empty** (`is_unrestricted() == true`): all communities are visible.
///   This is the default -- no federation restrictions in place.
/// - **Populated**: only the listed community IDs are visible. Any operation
///   targeting a community not in the set will be filtered out.
///
/// # Examples
///
/// ```
/// use nexus::FederationScope;
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
#[derive(Debug, Clone, Default)]
pub struct FederationScope {
    visible_communities: HashSet<String>,
}

impl FederationScope {
    /// Create an unrestricted scope (all communities visible).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope from an iterator of community IDs.
    ///
    /// Only the listed communities will be visible for federation.
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
}
