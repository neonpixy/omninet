use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Social connections graph.
///
/// Five relationship categories (all crown ID strings):
/// - `following` — people you follow
/// - `followers` — people who follow you (discovered via network)
/// - `blocked` — blocked users (cannot interact, auto-removed from following)
/// - `muted` — hidden but not blocked
/// - `trusted` — web-of-trust verified keys
///
/// Plus custom named lists (e.g., "close friends", "work").
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SocialGraph {
    /// Crown IDs of people you follow.
    pub following: HashSet<String>,
    /// Crown IDs of people who follow you (discovered via network).
    pub followers: HashSet<String>,
    /// Crown IDs you have blocked. Blocking auto-removes from following.
    pub blocked: HashSet<String>,
    /// Crown IDs you have muted (hidden but not blocked).
    pub muted: HashSet<String>,
    /// Crown IDs in your web-of-trust (verified keys you trust).
    pub trusted: HashSet<String>,
    /// Custom named lists mapping list name to a set of crown IDs.
    pub lists: HashMap<String, HashSet<String>>,
}

impl SocialGraph {
    /// Create an empty social graph.
    pub fn empty() -> Self {
        Self {
            following: HashSet::new(),
            followers: HashSet::new(),
            blocked: HashSet::new(),
            muted: HashSet::new(),
            trusted: HashSet::new(),
            lists: HashMap::new(),
        }
    }

    // -- Queries --

    /// Whether you are following this crown ID.
    pub fn is_following(&self, crown_id: &str) -> bool {
        self.following.contains(crown_id)
    }

    /// Whether this crown ID is blocked.
    pub fn is_blocked(&self, crown_id: &str) -> bool {
        self.blocked.contains(crown_id)
    }

    /// Whether this crown ID is muted (hidden but not blocked).
    pub fn is_muted(&self, crown_id: &str) -> bool {
        self.muted.contains(crown_id)
    }

    /// Whether this crown ID is in your web-of-trust.
    pub fn is_trusted(&self, crown_id: &str) -> bool {
        self.trusted.contains(crown_id)
    }

    /// All crown IDs in a named list. Returns empty set if the list doesn't exist.
    pub fn users_in_list(&self, list_name: &str) -> HashSet<&String> {
        self.lists
            .get(list_name)
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    /// List names, sorted alphabetically.
    pub fn list_names(&self) -> Vec<&String> {
        let mut names: Vec<&String> = self.lists.keys().collect();
        names.sort();
        names
    }

    // -- Mutations --

    /// Start following someone.
    pub fn follow(&mut self, crown_id: &str) {
        self.following.insert(crown_id.to_string());
    }

    /// Stop following someone.
    pub fn unfollow(&mut self, crown_id: &str) {
        self.following.remove(crown_id);
    }

    /// Block a user. **Also removes from following** — you cannot follow
    /// someone you've blocked.
    pub fn block(&mut self, crown_id: &str) {
        self.blocked.insert(crown_id.to_string());
        self.following.remove(crown_id);
    }

    /// Remove someone from the blocked list.
    pub fn unblock(&mut self, crown_id: &str) {
        self.blocked.remove(crown_id);
    }

    /// Mute someone (hide their content without blocking).
    pub fn mute(&mut self, crown_id: &str) {
        self.muted.insert(crown_id.to_string());
    }

    /// Remove someone from the muted list.
    pub fn unmute(&mut self, crown_id: &str) {
        self.muted.remove(crown_id);
    }

    /// Add someone to your web-of-trust.
    pub fn trust(&mut self, crown_id: &str) {
        self.trusted.insert(crown_id.to_string());
    }

    /// Remove someone from your web-of-trust.
    pub fn untrust(&mut self, crown_id: &str) {
        self.trusted.remove(crown_id);
    }

    /// Add a crown ID to a named list. Creates the list if it doesn't exist.
    pub fn add_to_list(&mut self, crown_id: &str, list: &str) {
        self.lists
            .entry(list.to_string())
            .or_default()
            .insert(crown_id.to_string());
    }

    /// Remove a crown ID from a named list. No-op if the list doesn't exist.
    pub fn remove_from_list(&mut self, crown_id: &str, list: &str) {
        if let Some(members) = self.lists.get_mut(list) {
            members.remove(crown_id);
        }
    }

    /// Create an empty list. No-op if it already exists.
    pub fn create_list(&mut self, name: &str) {
        self.lists.entry(name.to_string()).or_default();
    }

    /// Delete a named list and all its members. No-op if it doesn't exist.
    pub fn delete_list(&mut self, name: &str) {
        self.lists.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let graph = SocialGraph::empty();
        assert!(graph.following.is_empty());
        assert!(graph.followers.is_empty());
        assert!(graph.blocked.is_empty());
        assert!(graph.muted.is_empty());
        assert!(graph.trusted.is_empty());
        assert!(graph.lists.is_empty());
    }

    #[test]
    fn follow_unfollow() {
        let mut graph = SocialGraph::empty();
        graph.follow("cpub1alice");
        assert!(graph.is_following("cpub1alice"));

        graph.unfollow("cpub1alice");
        assert!(!graph.is_following("cpub1alice"));
    }

    #[test]
    fn block_removes_from_following() {
        let mut graph = SocialGraph::empty();
        graph.follow("cpub1alice");
        assert!(graph.is_following("cpub1alice"));

        graph.block("cpub1alice");
        assert!(!graph.is_following("cpub1alice"));
        assert!(graph.is_blocked("cpub1alice"));
    }

    #[test]
    fn unblock() {
        let mut graph = SocialGraph::empty();
        graph.block("cpub1eve");
        assert!(graph.is_blocked("cpub1eve"));

        graph.unblock("cpub1eve");
        assert!(!graph.is_blocked("cpub1eve"));
    }

    #[test]
    fn mute_unmute() {
        let mut graph = SocialGraph::empty();
        graph.mute("cpub1bob");
        assert!(graph.is_muted("cpub1bob"));

        graph.unmute("cpub1bob");
        assert!(!graph.is_muted("cpub1bob"));
    }

    #[test]
    fn trust_untrust() {
        let mut graph = SocialGraph::empty();
        graph.trust("cpub1carol");
        assert!(graph.is_trusted("cpub1carol"));

        graph.untrust("cpub1carol");
        assert!(!graph.is_trusted("cpub1carol"));
    }

    #[test]
    fn list_crud() {
        let mut graph = SocialGraph::empty();
        graph.create_list("friends");
        graph.add_to_list("cpub1alice", "friends");
        graph.add_to_list("cpub1bob", "friends");

        let members = graph.users_in_list("friends");
        assert_eq!(members.len(), 2);
        assert!(members.contains(&"cpub1alice".to_string()));

        graph.remove_from_list("cpub1alice", "friends");
        assert_eq!(graph.users_in_list("friends").len(), 1);

        graph.delete_list("friends");
        assert!(graph.users_in_list("friends").is_empty());
    }

    #[test]
    fn list_names_sorted() {
        let mut graph = SocialGraph::empty();
        graph.create_list("zebra");
        graph.create_list("apple");
        graph.create_list("mango");

        let names = graph.list_names();
        assert_eq!(names, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn social_graph_serde_round_trip() {
        let mut graph = SocialGraph::empty();
        graph.follow("cpub1alice");
        graph.follow("cpub1bob");
        graph.block("cpub1eve");
        graph.mute("cpub1spammer");
        graph.trust("cpub1carol");
        graph.add_to_list("cpub1alice", "close friends");

        let json = serde_json::to_string(&graph).unwrap();
        let loaded: SocialGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(graph, loaded);
    }

    #[test]
    fn block_idempotent() {
        let mut graph = SocialGraph::empty();
        graph.block("cpub1eve");
        graph.block("cpub1eve");
        assert_eq!(graph.blocked.len(), 1);
    }
}
