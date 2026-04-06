//! Trust graph — adjacency-list indexed, O(V+E) BFS traversal.
//!
//! The graph stores verification edges between people. Each person (pubkey)
//! has outgoing edges (people they verified) and incoming edges (people who
//! verified them). Queries traverse the graph via BFS from a querier's
//! perspective.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JailError;

use super::edge::VerificationEdge;

/// A directed graph of verification relationships.
///
/// Dual adjacency lists (outgoing + incoming) enable efficient traversal
/// in both directions. All edges are stored by ID for O(1) lookup.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrustGraph {
    /// All edges by their unique ID.
    edges: HashMap<Uuid, VerificationEdge>,
    /// Pubkey → outgoing edge IDs (people this person verified).
    outgoing: HashMap<String, Vec<Uuid>>,
    /// Pubkey → incoming edge IDs (people who verified this person).
    incoming: HashMap<String, Vec<Uuid>>,
}

impl TrustGraph {
    /// Create an empty trust graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a verification edge to the graph.
    pub fn add_edge(&mut self, edge: VerificationEdge) -> Result<(), JailError> {
        if edge.verifier_pubkey == edge.verified_pubkey {
            return Err(JailError::SelfFlag); // Can't verify yourself
        }

        let id = edge.id;
        let verifier = edge.verifier_pubkey.clone();
        let verified = edge.verified_pubkey.clone();

        self.edges.insert(id, edge);
        self.outgoing.entry(verifier).or_default().push(id);
        self.incoming.entry(verified).or_default().push(id);

        Ok(())
    }

    /// Remove an edge by ID.
    pub fn remove_edge(&mut self, edge_id: &Uuid) -> Result<VerificationEdge, JailError> {
        let edge = self
            .edges
            .remove(edge_id)
            .ok_or_else(|| JailError::EdgeNotFound(edge_id.to_string()))?;

        if let Some(out) = self.outgoing.get_mut(&edge.verifier_pubkey) {
            out.retain(|id| id != edge_id);
        }
        if let Some(inc) = self.incoming.get_mut(&edge.verified_pubkey) {
            inc.retain(|id| id != edge_id);
        }

        Ok(edge)
    }

    /// Get an edge by ID.
    pub fn get_edge(&self, edge_id: &Uuid) -> Option<&VerificationEdge> {
        self.edges.get(edge_id)
    }

    /// Get all outgoing edges from a pubkey (people they verified).
    pub fn edges_from(&self, pubkey: &str) -> Vec<&VerificationEdge> {
        self.outgoing
            .get(pubkey)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.edges.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all incoming edges to a pubkey (people who verified them).
    pub fn edges_to(&self, pubkey: &str) -> Vec<&VerificationEdge> {
        self.incoming
            .get(pubkey)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.edges.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Number of unique nodes (pubkeys) in the graph.
    pub fn node_count(&self) -> usize {
        let mut nodes = HashSet::new();
        for edge in self.edges.values() {
            nodes.insert(&edge.verifier_pubkey);
            nodes.insert(&edge.verified_pubkey);
        }
        nodes.len()
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Check if a pubkey exists in the graph (as verifier or verified).
    pub fn contains_node(&self, pubkey: &str) -> bool {
        self.outgoing.contains_key(pubkey) || self.incoming.contains_key(pubkey)
    }

    /// Get all pubkeys that a given pubkey has verified (outgoing neighbors).
    pub fn verified_by(&self, pubkey: &str) -> Vec<String> {
        self.edges_from(pubkey)
            .into_iter()
            .map(|e| e.verified_pubkey.clone())
            .collect()
    }

    /// BFS traversal from a source pubkey, returning visited nodes with their
    /// degree of separation. Used internally by query functions.
    pub(crate) fn bfs_traverse(
        &self,
        source: &str,
        max_degrees: usize,
    ) -> Vec<(String, usize)> {
        let mut visited = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut results = Vec::new();

        queue.push_back((source.to_string(), 0usize));
        visited.insert(source.to_string());

        while let Some((current, degree)) = queue.pop_front() {
            results.push((current.clone(), degree));

            if degree < max_degrees {
                for edge in self.edges_from(&current) {
                    let next = &edge.verified_pubkey;
                    if !visited.contains(next) {
                        visited.insert(next.clone());
                        queue.push_back((next.clone(), degree + 1));
                    }
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust_graph::edge::VerificationSentiment;

    fn make_edge(verifier: &str, verified: &str) -> VerificationEdge {
        VerificationEdge::new(
            verifier,
            verified,
            "mutual_vouch",
            VerificationSentiment::Positive,
            0.9,
        )
    }

    #[test]
    fn empty_graph() {
        let graph = TrustGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn add_and_query_edge() {
        let mut graph = TrustGraph::new();
        let edge = make_edge("alice", "bob");
        let edge_id = edge.id;

        graph.add_edge(edge).unwrap();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        let retrieved = graph.get_edge(&edge_id).unwrap();
        assert_eq!(retrieved.verifier_pubkey, "alice");
        assert_eq!(retrieved.verified_pubkey, "bob");
    }

    #[test]
    fn cannot_verify_self() {
        let mut graph = TrustGraph::new();
        let edge = make_edge("alice", "alice");
        assert!(graph.add_edge(edge).is_err());
    }

    #[test]
    fn remove_edge() {
        let mut graph = TrustGraph::new();
        let edge = make_edge("alice", "bob");
        let edge_id = edge.id;

        graph.add_edge(edge).unwrap();
        assert_eq!(graph.edge_count(), 1);

        let removed = graph.remove_edge(&edge_id).unwrap();
        assert_eq!(removed.verifier_pubkey, "alice");
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn remove_nonexistent_edge() {
        let mut graph = TrustGraph::new();
        assert!(graph.remove_edge(&Uuid::new_v4()).is_err());
    }

    #[test]
    fn edges_from_and_to() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("alice", "carol")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();

        assert_eq!(graph.edges_from("alice").len(), 2);
        assert_eq!(graph.edges_from("bob").len(), 1);
        assert_eq!(graph.edges_to("carol").len(), 2);
        assert_eq!(graph.edges_to("alice").len(), 0);
    }

    #[test]
    fn contains_node() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();

        assert!(graph.contains_node("alice"));
        assert!(graph.contains_node("bob"));
        assert!(!graph.contains_node("carol"));
    }

    #[test]
    fn verified_by() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("alice", "carol")).unwrap();

        let verified = graph.verified_by("alice");
        assert_eq!(verified.len(), 2);
        assert!(verified.contains(&"bob".to_string()));
        assert!(verified.contains(&"carol".to_string()));
    }

    #[test]
    fn bfs_linear_chain() {
        // alice → bob → carol → dave
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();
        graph.add_edge(make_edge("carol", "dave")).unwrap();

        let visited = graph.bfs_traverse("alice", 3);
        let names: Vec<&str> = visited.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"alice"));
        assert!(names.contains(&"bob"));
        assert!(names.contains(&"carol"));
        assert!(names.contains(&"dave"));

        // Check degrees
        let degree_of = |name: &str| visited.iter().find(|(n, _)| n == name).map(|(_, d)| *d);
        assert_eq!(degree_of("alice"), Some(0));
        assert_eq!(degree_of("bob"), Some(1));
        assert_eq!(degree_of("carol"), Some(2));
        assert_eq!(degree_of("dave"), Some(3));
    }

    #[test]
    fn bfs_respects_max_depth() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();
        graph.add_edge(make_edge("carol", "dave")).unwrap();

        let visited = graph.bfs_traverse("alice", 1);
        let names: Vec<&str> = visited.iter().map(|(name, _)| name.as_str()).collect();
        assert!(names.contains(&"alice"));
        assert!(names.contains(&"bob"));
        assert!(!names.contains(&"carol"));
    }

    #[test]
    fn bfs_handles_cycles() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();
        graph.add_edge(make_edge("carol", "alice")).unwrap(); // cycle

        let visited = graph.bfs_traverse("alice", 5);
        // Each node visited exactly once despite cycle
        assert_eq!(visited.len(), 3);
    }

    #[test]
    fn graph_serialization_roundtrip() {
        let mut graph = TrustGraph::new();
        graph.add_edge(make_edge("alice", "bob")).unwrap();
        graph.add_edge(make_edge("bob", "carol")).unwrap();

        let json = serde_json::to_string(&graph).unwrap();
        let deserialized: TrustGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.edge_count(), 2);
        assert_eq!(deserialized.node_count(), 3);
    }
}
