use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Optional federation scope for economic operations.
///
/// When set, economic visibility and eligibility are scoped to
/// federated communities. When empty, all communities are visible
/// (backward compatible — unrestricted mode).
///
/// From Constellation Art. 3 §3 — federation is a data boundary.
/// A community's economic data (transaction receipts, alerts, balances)
/// should only be visible to communities within the same federation.
///
/// # Examples
///
/// ```
/// use fortune::EconomicFederationScope;
///
/// // Unrestricted — sees everything
/// let unrestricted = EconomicFederationScope::new();
/// assert!(unrestricted.is_unrestricted());
/// assert!(unrestricted.is_visible("any_community"));
///
/// // Scoped to specific communities
/// let scoped = EconomicFederationScope::from_communities(["alpha", "beta"]);
/// assert!(scoped.is_visible("alpha"));
/// assert!(scoped.is_visible("beta"));
/// assert!(!scoped.is_visible("gamma"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EconomicFederationScope {
    visible_communities: HashSet<String>,
}

impl EconomicFederationScope {
    /// Create an unrestricted scope — all communities visible.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope limited to specific communities.
    pub fn from_communities(communities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            visible_communities: communities.into_iter().map(|c| c.into()).collect(),
        }
    }

    /// Check whether a community is visible under this scope.
    ///
    /// Returns `true` if the scope is unrestricted (empty) or if the
    /// community is in the visible set.
    #[must_use]
    pub fn is_visible(&self, community_id: &str) -> bool {
        self.visible_communities.is_empty() || self.visible_communities.contains(community_id)
    }

    /// Whether this scope has no restrictions (all communities visible).
    #[must_use]
    pub fn is_unrestricted(&self) -> bool {
        self.visible_communities.is_empty()
    }

    /// Number of communities in the visible set.
    ///
    /// Returns 0 for unrestricted scopes (which see everything).
    #[must_use]
    pub fn len(&self) -> usize {
        self.visible_communities.len()
    }

    /// Whether the visible set is empty.
    ///
    /// An empty scope is unrestricted — it sees all communities.
    /// This is semantically different from "sees nothing".
    #[must_use]
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

    /// Get the set of visible community IDs.
    #[must_use]
    pub fn visible_communities(&self) -> &HashSet<String> {
        &self.visible_communities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrestricted_scope_sees_everything() {
        let scope = EconomicFederationScope::new();
        assert!(scope.is_unrestricted());
        assert!(scope.is_visible("any_community"));
        assert!(scope.is_visible("another_community"));
        assert_eq!(scope.len(), 0);
        assert!(scope.is_empty());
    }

    #[test]
    fn scoped_to_specific_communities() {
        let scope = EconomicFederationScope::from_communities(["alpha", "beta"]);
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert!(!scope.is_visible("gamma"));
        assert_eq!(scope.len(), 2);
        assert!(!scope.is_empty());
    }

    #[test]
    fn add_and_remove_communities() {
        let mut scope = EconomicFederationScope::new();
        assert!(scope.is_unrestricted());

        scope.add_community("alpha");
        assert!(!scope.is_unrestricted());
        assert!(scope.is_visible("alpha"));
        assert!(!scope.is_visible("beta"));

        scope.add_community("beta");
        assert!(scope.is_visible("beta"));
        assert_eq!(scope.len(), 2);

        scope.remove_community("alpha");
        assert!(!scope.is_visible("alpha"));
        assert!(scope.is_visible("beta"));
        assert_eq!(scope.len(), 1);
    }

    #[test]
    fn from_communities_deduplicates() {
        let scope =
            EconomicFederationScope::from_communities(["alpha", "alpha", "beta"]);
        assert_eq!(scope.len(), 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let scope = EconomicFederationScope::from_communities(["alpha", "beta"]);
        let json = serde_json::to_string(&scope).unwrap();
        let restored: EconomicFederationScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, restored);
    }

    #[test]
    fn default_is_unrestricted() {
        let scope = EconomicFederationScope::default();
        assert!(scope.is_unrestricted());
    }

    #[test]
    fn scope_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EconomicFederationScope>();
    }

    #[test]
    fn visible_communities_accessor() {
        let scope = EconomicFederationScope::from_communities(["x", "y"]);
        let visible = scope.visible_communities();
        assert!(visible.contains("x"));
        assert!(visible.contains("y"));
        assert!(!visible.contains("z"));
    }
}
