use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::relationship::{RelationType, YokeLink};

/// Result of a graph traversal.
#[derive(Debug, Clone)]
pub struct TraversalNode {
    pub entity_id: String,
    pub depth: usize,
    pub path: Vec<String>,
}

/// Serializable snapshot of a relationship graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub links: Vec<YokeLink>,
}

/// In-memory relationship graph with traversal queries.
///
/// Maintains forward (source → targets) and reverse (target → sources)
/// adjacency lists for efficient lookups in both directions.
#[derive(Debug, Clone, Default)]
pub struct RelationshipGraph {
    forward: HashMap<String, Vec<YokeLink>>,
    reverse: HashMap<String, Vec<YokeLink>>,
}

/// Direction for graph traversal.
pub enum Direction {
    Forward,
    Backward,
}

impl RelationshipGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a link to the graph.
    pub fn add_link(&mut self, link: YokeLink) {
        self.reverse
            .entry(link.target.clone())
            .or_default()
            .push(link.clone());
        self.forward
            .entry(link.source.clone())
            .or_default()
            .push(link);
    }

    /// All links originating from this entity.
    pub fn links_from(&self, source: &str) -> Vec<&YokeLink> {
        self.forward
            .get(source)
            .map(|links| links.iter().collect())
            .unwrap_or_default()
    }

    /// All links pointing to this entity.
    pub fn links_to(&self, target: &str) -> Vec<&YokeLink> {
        self.reverse
            .get(target)
            .map(|links| links.iter().collect())
            .unwrap_or_default()
    }

    /// Links of a specific type from this entity.
    pub fn links_of_type(&self, source: &str, rel_type: &RelationType) -> Vec<&YokeLink> {
        self.links_from(source)
            .into_iter()
            .filter(|l| &l.relationship == rel_type)
            .collect()
    }

    /// Links of a specific type pointing to this entity.
    pub fn reverse_links_of_type(
        &self,
        target: &str,
        rel_type: &RelationType,
    ) -> Vec<&YokeLink> {
        self.links_to(target)
            .into_iter()
            .filter(|l| &l.relationship == rel_type)
            .collect()
    }

    /// BFS forward traversal (follow outgoing links).
    pub fn traverse_forward(&self, start: &str, max_depth: usize) -> Vec<TraversalNode> {
        self.bfs(start, max_depth, Direction::Forward)
    }

    /// BFS backward traversal (follow incoming links).
    pub fn traverse_backward(&self, start: &str, max_depth: usize) -> Vec<TraversalNode> {
        self.bfs(start, max_depth, Direction::Backward)
    }

    /// Find all ancestors via provenance links (DerivedFrom, VersionOf, etc.).
    pub fn ancestors(&self, id: &str) -> Vec<TraversalNode> {
        self.bfs_filtered(id, usize::MAX, Direction::Forward, |link| {
            link.relationship.is_provenance()
        })
    }

    /// Find all descendants via provenance links.
    pub fn descendants(&self, id: &str) -> Vec<TraversalNode> {
        self.bfs_filtered(id, usize::MAX, Direction::Backward, |link| {
            link.relationship.is_provenance()
        })
    }

    /// All version-of links pointing to this entity.
    pub fn versions_of(&self, id: &str) -> Vec<&YokeLink> {
        self.reverse_links_of_type(id, &RelationType::VersionOf)
    }

    /// All comments on this entity.
    pub fn comments_on(&self, id: &str) -> Vec<&YokeLink> {
        self.reverse_links_of_type(id, &RelationType::CommentOn)
    }

    /// All endorsements of this entity.
    pub fn endorsements_of(&self, id: &str) -> Vec<&YokeLink> {
        self.reverse_links_of_type(id, &RelationType::Endorses)
    }

    /// All things this entity supersedes.
    pub fn superseded_by(&self, id: &str) -> Vec<&YokeLink> {
        self.reverse_links_of_type(id, &RelationType::Supersedes)
    }

    /// BFS traversal filtering by relationship type.
    pub fn traverse_by_type(
        &self,
        start: &str,
        rel_type: &RelationType,
        max_depth: usize,
        direction: Direction,
    ) -> Vec<TraversalNode> {
        let rt = rel_type.clone();
        self.bfs_filtered(start, max_depth, direction, move |link| {
            link.relationship == rt
        })
    }

    /// Links from an entity filtered by author.
    pub fn links_from_by_author(&self, source: &str, author: &str) -> Vec<&YokeLink> {
        self.links_from(source)
            .into_iter()
            .filter(|l| l.author == author)
            .collect()
    }

    /// Links to an entity filtered by author.
    pub fn links_to_by_author(&self, target: &str, author: &str) -> Vec<&YokeLink> {
        self.links_to(target)
            .into_iter()
            .filter(|l| l.author == author)
            .collect()
    }

    /// Links from an entity created within a time range.
    pub fn links_from_between(
        &self,
        source: &str,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Vec<&YokeLink> {
        self.links_from(source)
            .into_iter()
            .filter(|l| l.created_at >= since && l.created_at <= until)
            .collect()
    }

    /// Links to an entity created within a time range.
    pub fn links_to_between(
        &self,
        target: &str,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Vec<&YokeLink> {
        self.links_to(target)
            .into_iter()
            .filter(|l| l.created_at >= since && l.created_at <= until)
            .collect()
    }

    /// Create a serializable snapshot of the entire graph.
    pub fn snapshot(&self) -> GraphSnapshot {
        let links: Vec<YokeLink> = self
            .forward
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect();
        GraphSnapshot { links }
    }

    /// Restore a graph from a snapshot.
    pub fn from_snapshot(snapshot: GraphSnapshot) -> Self {
        let mut graph = Self::new();
        for link in snapshot.links {
            graph.add_link(link);
        }
        graph
    }

    /// Remove all links involving an entity (both directions).
    pub fn remove_entity(&mut self, entity_id: &str) {
        // Remove forward links from this entity
        if let Some(links) = self.forward.remove(entity_id) {
            for link in &links {
                if let Some(rev) = self.reverse.get_mut(&link.target) {
                    rev.retain(|l| l.source != entity_id);
                }
            }
        }
        // Remove reverse links to this entity
        if let Some(links) = self.reverse.remove(entity_id) {
            for link in &links {
                if let Some(fwd) = self.forward.get_mut(&link.source) {
                    fwd.retain(|l| l.target != entity_id);
                }
            }
        }
    }

    /// Find shortest path between two entities (BFS).
    pub fn path_between(&self, from: &str, to: &str) -> Option<Vec<String>> {
        if from == to {
            return Some(vec![from.to_string()]);
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(from.to_string());
        queue.push_back(vec![from.to_string()]);

        while let Some(path) = queue.pop_front() {
            let current = path.last().expect("BFS path always has at least one element");

            // Check both forward and reverse links for undirected path
            let mut neighbors = Vec::new();
            if let Some(fwd) = self.forward.get(current.as_str()) {
                neighbors.extend(fwd.iter().map(|l| l.target.as_str()));
            }
            if let Some(rev) = self.reverse.get(current.as_str()) {
                neighbors.extend(rev.iter().map(|l| l.source.as_str()));
            }

            for next in neighbors {
                if !visited.contains(next) {
                    let mut new_path = path.clone();
                    new_path.push(next.to_string());
                    if next == to {
                        return Some(new_path);
                    }
                    visited.insert(next.to_string());
                    queue.push_back(new_path);
                }
            }
        }

        None
    }

    /// Total number of links in the graph.
    pub fn link_count(&self) -> usize {
        self.forward.values().map(|v| v.len()).sum()
    }

    /// Total number of unique entities in the graph.
    pub fn entity_count(&self) -> usize {
        let mut entities = HashSet::new();
        for key in self.forward.keys() {
            entities.insert(key.as_str());
        }
        for key in self.reverse.keys() {
            entities.insert(key.as_str());
        }
        entities.len()
    }

    fn bfs(&self, start: &str, max_depth: usize, direction: Direction) -> Vec<TraversalNode> {
        self.bfs_filtered(start, max_depth, direction, |_| true)
    }

    fn bfs_filtered(
        &self,
        start: &str,
        max_depth: usize,
        direction: Direction,
        filter: impl Fn(&YokeLink) -> bool,
    ) -> Vec<TraversalNode> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        visited.insert(start.to_string());
        queue.push_back((start.to_string(), 0, vec![start.to_string()]));

        while let Some((current, depth, path)) = queue.pop_front() {
            if depth > 0 {
                results.push(TraversalNode {
                    entity_id: current.clone(),
                    depth,
                    path: path.clone(),
                });
            }

            if depth >= max_depth {
                continue;
            }

            let neighbors: Vec<&YokeLink> = match direction {
                Direction::Forward => self
                    .forward
                    .get(current.as_str())
                    .map(|v| v.iter().collect())
                    .unwrap_or_default(),
                Direction::Backward => self
                    .reverse
                    .get(current.as_str())
                    .map(|v| v.iter().collect())
                    .unwrap_or_default(),
            };

            for link in neighbors {
                if !filter(link) {
                    continue;
                }
                let next = match direction {
                    Direction::Forward => &link.target,
                    Direction::Backward => &link.source,
                };
                if !visited.contains(next.as_str()) {
                    visited.insert(next.clone());
                    let mut new_path = path.clone();
                    new_path.push(next.clone());
                    queue.push_back((next.clone(), depth + 1, new_path));
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(source: &str, target: &str, rel: RelationType) -> YokeLink {
        YokeLink::new(source, target, rel, "cpub1test")
    }

    #[test]
    fn empty_graph() {
        let graph = RelationshipGraph::new();
        assert_eq!(graph.link_count(), 0);
        assert_eq!(graph.entity_count(), 0);
        assert!(graph.links_from("x").is_empty());
        assert!(graph.links_to("x").is_empty());
    }

    #[test]
    fn add_and_query_links() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("a", "c", RelationType::References));

        assert_eq!(graph.links_from("a").len(), 2);
        assert_eq!(graph.links_to("b").len(), 1);
        assert_eq!(graph.links_to("c").len(), 1);
        assert_eq!(graph.link_count(), 2);
        assert_eq!(graph.entity_count(), 3);
    }

    #[test]
    fn links_of_type() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("a", "c", RelationType::References));
        graph.add_link(link("a", "d", RelationType::DerivedFrom));

        assert_eq!(
            graph.links_of_type("a", &RelationType::DerivedFrom).len(),
            2
        );
        assert_eq!(
            graph.links_of_type("a", &RelationType::References).len(),
            1
        );
        assert_eq!(
            graph.links_of_type("a", &RelationType::CommentOn).len(),
            0
        );
    }

    #[test]
    fn reverse_links_of_type() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("comment-1", "post", RelationType::CommentOn));
        graph.add_link(link("comment-2", "post", RelationType::CommentOn));
        graph.add_link(link("reply", "post", RelationType::RespondsTo));

        assert_eq!(graph.comments_on("post").len(), 2);
        assert_eq!(
            graph
                .reverse_links_of_type("post", &RelationType::RespondsTo)
                .len(),
            1
        );
    }

    #[test]
    fn versions_of() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("logo-v2", "logo-v1", RelationType::VersionOf));
        graph.add_link(link("logo-v3", "logo-v1", RelationType::VersionOf));

        assert_eq!(graph.versions_of("logo-v1").len(), 2);
        assert_eq!(graph.versions_of("logo-v3").len(), 0);
    }

    #[test]
    fn endorsements() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("endorsement-1", "asset-x", RelationType::Endorses));
        graph.add_link(link("endorsement-2", "asset-x", RelationType::Endorses));

        assert_eq!(graph.endorsements_of("asset-x").len(), 2);
    }

    #[test]
    fn forward_traversal() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::DerivedFrom));
        graph.add_link(link("c", "d", RelationType::DerivedFrom));

        let nodes = graph.traverse_forward("a", 10);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].entity_id, "b");
        assert_eq!(nodes[0].depth, 1);
        assert_eq!(nodes[1].entity_id, "c");
        assert_eq!(nodes[1].depth, 2);
        assert_eq!(nodes[2].entity_id, "d");
        assert_eq!(nodes[2].depth, 3);
    }

    #[test]
    fn backward_traversal() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "root", RelationType::DerivedFrom));
        graph.add_link(link("b", "root", RelationType::DerivedFrom));
        graph.add_link(link("c", "a", RelationType::DerivedFrom));

        let nodes = graph.traverse_backward("root", 10);
        let ids: Vec<&str> = nodes.iter().map(|n| n.entity_id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        // c derives from a, which derives from root — should appear at depth 2
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn traversal_depth_limit() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::DerivedFrom));
        graph.add_link(link("c", "d", RelationType::DerivedFrom));

        let nodes = graph.traverse_forward("a", 1);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].entity_id, "b");
    }

    #[test]
    fn traversal_handles_cycles() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::References));
        graph.add_link(link("b", "c", RelationType::References));
        graph.add_link(link("c", "a", RelationType::References));

        let nodes = graph.traverse_forward("a", 100);
        // Should visit b and c but not loop back to a
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn ancestors_follow_provenance_only() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("remix", "original", RelationType::DerivedFrom));
        graph.add_link(link("original", "root", RelationType::VersionOf));
        // This comment link should NOT be followed
        graph.add_link(link("remix", "unrelated", RelationType::CommentOn));

        let ancestors = graph.ancestors("remix");
        let ids: Vec<&str> = ancestors.iter().map(|n| n.entity_id.as_str()).collect();
        assert!(ids.contains(&"original"));
        assert!(ids.contains(&"root"));
        assert!(!ids.contains(&"unrelated"));
    }

    #[test]
    fn descendants_follow_provenance_only() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("v2", "v1", RelationType::DerivedFrom));
        graph.add_link(link("v3", "v1", RelationType::DerivedFrom));
        graph.add_link(link("comment", "v1", RelationType::CommentOn));

        let descendants = graph.descendants("v1");
        let ids: Vec<&str> = descendants.iter().map(|n| n.entity_id.as_str()).collect();
        assert!(ids.contains(&"v2"));
        assert!(ids.contains(&"v3"));
        assert!(!ids.contains(&"comment")); // not provenance
    }

    #[test]
    fn path_between_connected() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::References));

        let path = graph.path_between("a", "c").unwrap();
        assert_eq!(path, vec!["a", "b", "c"]);
    }

    #[test]
    fn path_between_same_node() {
        let graph = RelationshipGraph::new();
        let path = graph.path_between("a", "a").unwrap();
        assert_eq!(path, vec!["a"]);
    }

    #[test]
    fn path_between_disconnected() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("c", "d", RelationType::DerivedFrom));

        assert!(graph.path_between("a", "d").is_none());
    }

    #[test]
    fn path_uses_reverse_links() {
        let mut graph = RelationshipGraph::new();
        // a → b, c → b (so b connects a and c via reverse)
        graph.add_link(link("a", "b", RelationType::References));
        graph.add_link(link("c", "b", RelationType::References));

        let path = graph.path_between("a", "c").unwrap();
        assert_eq!(path.len(), 3); // a → b → c
    }

    #[test]
    fn traversal_path_tracking() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::DerivedFrom));

        let nodes = graph.traverse_forward("a", 10);
        let c_node = nodes.iter().find(|n| n.entity_id == "c").unwrap();
        assert_eq!(c_node.path, vec!["a", "b", "c"]);
    }

    #[test]
    fn superseded_by() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("logo-v2", "logo-v1", RelationType::Supersedes));
        graph.add_link(link("logo-v3", "logo-v1", RelationType::Supersedes));

        assert_eq!(graph.superseded_by("logo-v1").len(), 2);
    }

    fn link_by(source: &str, target: &str, rel: RelationType, author: &str) -> YokeLink {
        YokeLink::new(source, target, rel, author)
    }

    #[test]
    fn links_from_by_author() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link_by("a", "b", RelationType::DerivedFrom, "cpub1alice"));
        graph.add_link(link_by("a", "c", RelationType::DerivedFrom, "cpub1bob"));
        graph.add_link(link_by("a", "d", RelationType::References, "cpub1alice"));

        assert_eq!(graph.links_from_by_author("a", "cpub1alice").len(), 2);
        assert_eq!(graph.links_from_by_author("a", "cpub1bob").len(), 1);
        assert_eq!(graph.links_from_by_author("a", "cpub1nobody").len(), 0);
    }

    #[test]
    fn links_to_by_author() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link_by("x", "target", RelationType::CommentOn, "cpub1alice"));
        graph.add_link(link_by("y", "target", RelationType::CommentOn, "cpub1bob"));

        assert_eq!(graph.links_to_by_author("target", "cpub1alice").len(), 1);
        assert_eq!(graph.links_to_by_author("target", "cpub1bob").len(), 1);
    }

    #[test]
    fn links_from_between_time_range() {
        let mut graph = RelationshipGraph::new();
        let now = chrono::Utc::now();

        let mut old_link = link("a", "b", RelationType::DerivedFrom);
        old_link.created_at = now - chrono::Duration::days(30);
        graph.add_link(old_link);

        let mut recent_link = link("a", "c", RelationType::References);
        recent_link.created_at = now;
        graph.add_link(recent_link);

        let since = now - chrono::Duration::days(7);
        let until = now + chrono::Duration::days(1);
        let results = graph.links_from_between("a", since, until);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target, "c");
    }

    #[test]
    fn traverse_by_type() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::DerivedFrom));
        graph.add_link(link("b", "d", RelationType::CommentOn)); // different type

        let nodes =
            graph.traverse_by_type("a", &RelationType::DerivedFrom, 10, Direction::Forward);
        let ids: Vec<&str> = nodes.iter().map(|n| n.entity_id.as_str()).collect();
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
        assert!(!ids.contains(&"d")); // not DerivedFrom
    }

    #[test]
    fn snapshot_and_restore() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::References));
        graph.add_link(link("x", "y", RelationType::CommentOn));

        let snapshot = graph.snapshot();
        assert_eq!(snapshot.links.len(), 3);

        // Serialize and deserialize
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored_snap: GraphSnapshot = serde_json::from_str(&json).unwrap();
        let restored = RelationshipGraph::from_snapshot(restored_snap);

        assert_eq!(restored.link_count(), 3);
        assert_eq!(restored.entity_count(), 5);
        assert_eq!(restored.links_from("a").len(), 1);
        assert_eq!(restored.links_to("c").len(), 1);
    }

    #[test]
    fn remove_entity() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("b", "c", RelationType::References));
        graph.add_link(link("d", "b", RelationType::CommentOn));

        assert_eq!(graph.link_count(), 3);
        graph.remove_entity("b");
        assert_eq!(graph.link_count(), 0); // all links involved b
        assert!(graph.links_from("a").is_empty());
        assert!(graph.links_to("c").is_empty());
    }

    #[test]
    fn remove_entity_partial() {
        let mut graph = RelationshipGraph::new();
        graph.add_link(link("a", "b", RelationType::DerivedFrom));
        graph.add_link(link("c", "d", RelationType::References));
        graph.add_link(link("a", "d", RelationType::CommentOn));

        graph.remove_entity("b");
        assert_eq!(graph.link_count(), 2); // a→d and c→d remain
        assert_eq!(graph.links_from("a").len(), 1);
        assert_eq!(graph.links_to("d").len(), 2);
    }

    #[test]
    fn complex_graph() {
        let mut graph = RelationshipGraph::new();
        // Creative lineage
        graph.add_link(link("remix", "original", RelationType::DerivedFrom));
        graph.add_link(link("remix-v2", "remix", RelationType::VersionOf));
        // Social
        graph.add_link(link("comment-1", "remix-v2", RelationType::CommentOn));
        graph.add_link(link("endorsement", "remix-v2", RelationType::Endorses));
        // Approval
        graph.add_link(link("proposal-1", "remix-v2", RelationType::ApprovedBy));

        assert_eq!(graph.entity_count(), 6);
        assert_eq!(graph.link_count(), 5);
        assert_eq!(graph.comments_on("remix-v2").len(), 1);
        assert_eq!(graph.endorsements_of("remix-v2").len(), 1);

        // Provenance chain: remix → original, remix-v2 → remix
        let ancestors = graph.ancestors("remix-v2");
        let ids: Vec<&str> = ancestors.iter().map(|n| n.entity_id.as_str()).collect();
        assert!(ids.contains(&"remix"));
        assert!(ids.contains(&"original"));
    }
}
