//! Semantic digests — concept knowledge exchanged during Tower peering.
//!
//! When two Towers peer, they can exchange Semantic Digests alongside
//! gospel events. Digests carry concept equivalences (e.g., "woodworking"
//! ≈ "carpentry") and Synapse edges (weighted concept relationships).
//! Over time, the network builds a distributed knowledge graph.

use serde::{Deserialize, Serialize};

/// An equivalence between two concepts discovered by a Tower's index.
///
/// For example, a Tower that indexes both "woodworking" and "carpentry"
/// content might learn they're semantically equivalent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConceptEquivalence {
    /// First concept.
    pub concept_a: String,
    /// Second concept.
    pub concept_b: String,
    /// Confidence in the equivalence (0.0 = uncertain, 1.0 = definite).
    pub confidence: f64,
}

/// A weighted edge in the Synapse concept graph.
///
/// Represents a directional relationship: `from` is related to `to`
/// with a given weight. Weight encodes strength (0.0 = barely, 1.0 = strongly).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SynapseEdge {
    /// Source concept.
    pub from: String,
    /// Target concept.
    pub to: String,
    /// Relationship weight (0.0 = weak, 1.0 = strong).
    pub weight: f64,
}

/// A bundle of concept knowledge exchanged between Towers during peering.
///
/// Each Tower builds this from its local MagicalIndex + Advisor analysis.
/// During gospel peering, Towers exchange digests so concept knowledge
/// propagates across the network.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemanticDigest {
    /// Concept equivalences discovered by this Tower.
    pub equivalences: Vec<ConceptEquivalence>,
    /// Synapse graph edges (concept relationships).
    pub edges: Vec<SynapseEdge>,
}

impl SemanticDigest {
    /// Create an empty digest.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether this digest has no content.
    pub fn is_empty(&self) -> bool {
        self.equivalences.is_empty() && self.edges.is_empty()
    }

    /// Merge another digest into this one.
    ///
    /// Deduplicates by concept pair. When duplicates exist, the higher
    /// confidence/weight wins.
    pub fn merge(&mut self, other: &SemanticDigest) {
        // Merge equivalences.
        for eq in &other.equivalences {
            let existing = self.equivalences.iter_mut().find(|e| {
                (e.concept_a == eq.concept_a && e.concept_b == eq.concept_b)
                    || (e.concept_a == eq.concept_b && e.concept_b == eq.concept_a)
            });
            match existing {
                Some(e) => {
                    if eq.confidence > e.confidence {
                        e.confidence = eq.confidence;
                    }
                }
                None => self.equivalences.push(eq.clone()),
            }
        }

        // Merge edges.
        for edge in &other.edges {
            let existing = self
                .edges
                .iter_mut()
                .find(|e| e.from == edge.from && e.to == edge.to);
            match existing {
                Some(e) => {
                    if edge.weight > e.weight {
                        e.weight = edge.weight;
                    }
                }
                None => self.edges.push(edge.clone()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_digest() {
        let d = SemanticDigest::empty();
        assert!(d.is_empty());
        assert!(d.equivalences.is_empty());
        assert!(d.edges.is_empty());
    }

    #[test]
    fn serde_round_trip() {
        let d = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "woodworking".into(),
                concept_b: "carpentry".into(),
                confidence: 0.9,
            }],
            edges: vec![SynapseEdge {
                from: "woodworking".into(),
                to: "tools".into(),
                weight: 0.7,
            }],
        };
        let json = serde_json::to_string(&d).unwrap();
        let loaded: SemanticDigest = serde_json::from_str(&json).unwrap();
        assert_eq!(d, loaded);
    }

    #[test]
    fn merge_dedup_equivalences() {
        let mut a = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "wood".into(),
                concept_b: "timber".into(),
                confidence: 0.5,
            }],
            edges: vec![],
        };
        let b = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "wood".into(),
                concept_b: "timber".into(),
                confidence: 0.8,
            }],
            edges: vec![],
        };
        a.merge(&b);
        assert_eq!(a.equivalences.len(), 1);
        assert_eq!(a.equivalences[0].confidence, 0.8);
    }

    #[test]
    fn merge_dedup_reversed_equivalence() {
        let mut a = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "wood".into(),
                concept_b: "timber".into(),
                confidence: 0.5,
            }],
            edges: vec![],
        };
        // Same pair, reversed order.
        let b = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "timber".into(),
                concept_b: "wood".into(),
                confidence: 0.3,
            }],
            edges: vec![],
        };
        a.merge(&b);
        // Should dedup (reversed pair is the same equivalence).
        assert_eq!(a.equivalences.len(), 1);
        // Lower confidence doesn't replace higher.
        assert_eq!(a.equivalences[0].confidence, 0.5);
    }

    #[test]
    fn merge_higher_confidence_wins() {
        let mut a = SemanticDigest {
            equivalences: vec![],
            edges: vec![SynapseEdge {
                from: "a".into(),
                to: "b".into(),
                weight: 0.3,
            }],
        };
        let b = SemanticDigest {
            equivalences: vec![],
            edges: vec![SynapseEdge {
                from: "a".into(),
                to: "b".into(),
                weight: 0.9,
            }],
        };
        a.merge(&b);
        assert_eq!(a.edges.len(), 1);
        assert_eq!(a.edges[0].weight, 0.9);
    }

    #[test]
    fn merge_adds_new_entries() {
        let mut a = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "a".into(),
                concept_b: "b".into(),
                confidence: 0.5,
            }],
            edges: vec![SynapseEdge {
                from: "x".into(),
                to: "y".into(),
                weight: 0.5,
            }],
        };
        let b = SemanticDigest {
            equivalences: vec![ConceptEquivalence {
                concept_a: "c".into(),
                concept_b: "d".into(),
                confidence: 0.7,
            }],
            edges: vec![SynapseEdge {
                from: "m".into(),
                to: "n".into(),
                weight: 0.8,
            }],
        };
        a.merge(&b);
        assert_eq!(a.equivalences.len(), 2);
        assert_eq!(a.edges.len(), 2);
    }
}
