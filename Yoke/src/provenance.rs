//! Provenance Scoring — structural transparency for information.
//!
//! Not fact-checking. Not content moderation. Not a trust score on people.
//!
//! What this IS:
//! - **Source transparency** — where did this come from?
//! - **Chain of custody** — how did it get to you?
//! - **Corroboration visibility** — who else says the same thing independently?
//! - **Challenge visibility** — who disagrees?
//!
//! Any .idea viewer can display `ProvenanceScore` alongside content.
//! One tap shows the full `ProvenanceChain`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::graph::RelationshipGraph;
use crate::relationship::RelationType;

/// Computed provenance score for a single event.
///
/// Score is 0.0 (no provenance) to 1.0 (maximum provenance).
/// The score is a weighted composite of `ProvenanceFactors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceScore {
    /// The event being scored.
    pub event_id: String,
    /// The factors that contributed to this score.
    pub factors: ProvenanceFactors,
    /// Composite score (0.0 to 1.0).
    pub score: f64,
    /// When this score was computed.
    pub computed_at: DateTime<Utc>,
}

impl ProvenanceScore {
    /// Whether this score indicates strong provenance (>= 0.7).
    pub fn is_strong(&self) -> bool {
        self.score >= 0.7
    }

    /// Whether this score indicates weak provenance (< 0.3).
    pub fn is_weak(&self) -> bool {
        self.score < 0.3
    }
}

/// The individual factors that compose a provenance score.
///
/// Each factor captures a different dimension of information
/// transparency. The `ProvenanceComputer` combines them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceFactors {
    /// Is the original creator identifiable (Crown pubkey, not anonymous)?
    pub source_identified: bool,
    /// Bulwark reputation score of the original creator (0.0 to 1.0).
    pub source_reputation: f64,
    /// How many independent sources from distinct communities
    /// have published substantially similar content.
    pub corroboration_count: usize,
    /// Community diversity of corroborating sources (0.0 to 1.0).
    /// Inverted Gini — more diverse = higher score.
    pub corroboration_diversity: f64,
    /// How old is the original event (days).
    /// Older = more time for corroboration or refutation.
    pub age_days: u64,
    /// How many times the content has been modified from the original.
    /// Longer chains = lower provenance.
    pub modification_chain_length: usize,
    /// Has anyone published a challenge or refutation?
    pub has_been_challenged: bool,
    /// Number of challenges that exist.
    pub challenge_count: usize,
}

impl Default for ProvenanceFactors {
    fn default() -> Self {
        Self {
            source_identified: false,
            source_reputation: 0.0,
            corroboration_count: 0,
            corroboration_diversity: 0.0,
            age_days: 0,
            modification_chain_length: 0,
            has_been_challenged: false,
            challenge_count: 0,
        }
    }
}

/// The full provenance chain for an event — from current to original.
///
/// Shows the chain of custody: who created the original, how it
/// got to the current form, and who corroborates it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceChain {
    /// The original event at the root of the chain.
    pub original_event_id: String,
    /// The original author's pubkey.
    pub original_author: String,
    /// The community the original was published in, if known.
    pub original_community: Option<String>,
    /// The chain of links from current to original.
    pub chain: Vec<ProvenanceLink>,
    /// Number of modifications in the chain.
    pub modifications: usize,
    /// Independent corroborations from other sources.
    pub corroborations: Vec<Corroboration>,
}

impl ProvenanceChain {
    /// Whether the chain traces back to an identifiable original.
    pub fn has_known_origin(&self) -> bool {
        !self.original_author.is_empty()
    }

    /// The depth of the modification chain.
    pub fn chain_depth(&self) -> usize {
        self.chain.len()
    }

    /// How many distinct communities corroborate this content.
    pub fn corroborating_communities(&self) -> usize {
        let mut communities = std::collections::HashSet::new();
        for c in &self.corroborations {
            if let Some(ref cid) = c.community_id {
                communities.insert(cid.as_str());
            }
        }
        communities.len()
    }
}

/// A single link in the provenance chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceLink {
    /// The event ID at this point in the chain.
    pub event_id: String,
    /// The author at this point.
    pub author: String,
    /// The relationship to the next link (e.g., DerivedFrom, VersionOf).
    pub relation: RelationType,
    /// When this link was created.
    pub timestamp: DateTime<Utc>,
}

/// An independent corroboration from a different source.
///
/// Corroborations must come from distinct authors in distinct
/// communities to count toward provenance diversity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corroboration {
    /// The corroborating event ID.
    pub event_id: String,
    /// The author of the corroboration.
    pub author: String,
    /// The community the corroboration came from.
    pub community_id: Option<String>,
    /// How similar the content is (0.0 to 1.0).
    pub similarity: f64,
}

/// Computes provenance scores from relationship graphs.
///
/// Pure computation — no I/O, no state. Feed it data, get scores.
pub struct ProvenanceComputer;

/// Minimal event data needed for provenance computation.
///
/// Avoids depending on Globe's `OmniEvent` directly — Yoke is a
/// lower-layer crate that shouldn't import Globe.
#[derive(Debug, Clone)]
pub struct EventData {
    /// The event ID.
    pub id: String,
    /// The event author's pubkey.
    pub author: String,
    /// The community this event belongs to, if any.
    pub community_id: Option<String>,
    /// When the event was created.
    pub created_at: DateTime<Utc>,
}

// -- Factor weight constants --

const WEIGHT_SOURCE_IDENTIFIED: f64 = 0.20;
const WEIGHT_REPUTATION: f64 = 0.15;
const WEIGHT_CORROBORATION: f64 = 0.20;
const WEIGHT_DIVERSITY: f64 = 0.15;
const WEIGHT_AGE: f64 = 0.05;
const WEIGHT_CHAIN_LENGTH: f64 = 0.10;
const WEIGHT_CHALLENGE: f64 = 0.15;

impl ProvenanceComputer {
    /// Compute the provenance score for an event.
    ///
    /// Traces the `RelationshipGraph` to find the original source,
    /// then computes factors from the chain, corroborations, and challenges.
    ///
    /// # Arguments
    ///
    /// * `event_id` — the event to score
    /// * `graph` — the relationship graph
    /// * `events` — event metadata for chain tracing
    /// * `corroborations` — known corroborations for the original event
    /// * `challenge_count` — number of challenges against this content
    /// * `source_reputation` — the original author's Bulwark reputation (0.0-1.0)
    pub fn compute(
        event_id: &str,
        graph: &RelationshipGraph,
        events: &[EventData],
        corroborations: &[Corroboration],
        challenge_count: usize,
        source_reputation: f64,
    ) -> ProvenanceScore {
        let event_map: std::collections::HashMap<&str, &EventData> =
            events.iter().map(|e| (e.id.as_str(), e)).collect();

        // Trace the chain back to the original.
        let chain = Self::trace_chain(event_id, graph, &event_map);
        let original = chain.last().map(|l| l.event_id.as_str()).unwrap_or(event_id);
        let original_data = event_map.get(original);

        // Build factors.
        let source_identified = original_data
            .map(|e| !e.author.is_empty())
            .unwrap_or(false);

        let age_days = original_data
            .map(|e| {
                let now = Utc::now();
                let diff = now.signed_duration_since(e.created_at);
                diff.num_days().max(0) as u64
            })
            .unwrap_or(0);

        // Corroboration diversity: how many distinct communities corroborate.
        let corroboration_diversity = Self::compute_diversity(corroborations);

        let factors = ProvenanceFactors {
            source_identified,
            source_reputation: source_reputation.clamp(0.0, 1.0),
            corroboration_count: corroborations.len(),
            corroboration_diversity,
            age_days,
            modification_chain_length: chain.len(),
            has_been_challenged: challenge_count > 0,
            challenge_count,
        };

        let score = Self::compute_score(&factors);

        ProvenanceScore {
            event_id: event_id.to_string(),
            factors,
            score,
            computed_at: Utc::now(),
        }
    }

    /// Trace the provenance chain from an event back to its original.
    ///
    /// Follows `DerivedFrom`, `VersionOf`, `BranchedFrom` links backward.
    /// Cycle-safe via visited set.
    pub fn trace_chain(
        event_id: &str,
        graph: &RelationshipGraph,
        events: &std::collections::HashMap<&str, &EventData>,
    ) -> Vec<ProvenanceLink> {
        let mut chain = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut current = event_id.to_string();

        loop {
            if !visited.insert(current.clone()) {
                break; // Cycle detected — stop.
            }

            // Find provenance links pointing from current to its source.
            let links = graph.links_from(&current);
            let provenance_link = links
                .into_iter()
                .find(|l| l.relationship.is_provenance());

            match provenance_link {
                Some(link) => {
                    let event_data = events.get(link.target.as_str());
                    chain.push(ProvenanceLink {
                        event_id: link.target.clone(),
                        author: event_data
                            .map(|e| e.author.clone())
                            .unwrap_or_default(),
                        relation: link.relationship.clone(),
                        timestamp: link.created_at,
                    });
                    current = link.target.clone();
                }
                None => break, // Reached the original.
            }
        }

        chain
    }

    /// Build a full `ProvenanceChain` for an event.
    pub fn build_chain(
        event_id: &str,
        graph: &RelationshipGraph,
        events: &[EventData],
        corroborations: Vec<Corroboration>,
    ) -> ProvenanceChain {
        let event_map: std::collections::HashMap<&str, &EventData> =
            events.iter().map(|e| (e.id.as_str(), e)).collect();

        let chain = Self::trace_chain(event_id, graph, &event_map);

        let (original_event_id, original_author, original_community) =
            if let Some(last) = chain.last() {
                let data = event_map.get(last.event_id.as_str());
                (
                    last.event_id.clone(),
                    data.map(|e| e.author.clone()).unwrap_or_default(),
                    data.and_then(|e| e.community_id.clone()),
                )
            } else {
                // Event is itself the original.
                let data = event_map.get(event_id);
                (
                    event_id.to_string(),
                    data.map(|e| e.author.clone()).unwrap_or_default(),
                    data.and_then(|e| e.community_id.clone()),
                )
            };

        let modifications = chain.len();

        ProvenanceChain {
            original_event_id,
            original_author,
            original_community,
            chain,
            modifications,
            corroborations,
        }
    }

    /// Compute corroboration diversity using an inverted Simpson's index.
    ///
    /// Range: 0.0 (all from same community) to approaching 1.0 (evenly distributed).
    fn compute_diversity(corroborations: &[Corroboration]) -> f64 {
        if corroborations.is_empty() {
            return 0.0;
        }

        let mut community_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        let mut total = 0usize;

        for c in corroborations {
            if let Some(ref cid) = c.community_id {
                *community_counts.entry(cid.as_str()).or_default() += 1;
                total += 1;
            }
        }

        if total == 0 {
            return 0.0;
        }

        // Simpson's diversity index: 1 - sum((n_i / N)^2)
        let sum_squares: f64 = community_counts
            .values()
            .map(|&count| {
                let share = count as f64 / total as f64;
                share * share
            })
            .sum();

        1.0 - sum_squares
    }

    /// Compute the composite provenance score from factors.
    fn compute_score(factors: &ProvenanceFactors) -> f64 {
        let mut score = 0.0;

        // Source identified: binary.
        if factors.source_identified {
            score += WEIGHT_SOURCE_IDENTIFIED;
        }

        // Source reputation: linear.
        score += factors.source_reputation * WEIGHT_REPUTATION;

        // Corroboration count: diminishing returns (log scale, cap at 10).
        let corr_score = if factors.corroboration_count > 0 {
            ((factors.corroboration_count as f64).ln_1p() / 10.0_f64.ln_1p()).min(1.0)
        } else {
            0.0
        };
        score += corr_score * WEIGHT_CORROBORATION;

        // Corroboration diversity: direct.
        score += factors.corroboration_diversity * WEIGHT_DIVERSITY;

        // Age: diminishing returns (older is better, caps at 365 days).
        let age_score = (factors.age_days as f64 / 365.0).min(1.0);
        score += age_score * WEIGHT_AGE;

        // Chain length: shorter is better (inverse, caps at 10).
        let chain_score = if factors.modification_chain_length == 0 {
            1.0
        } else {
            1.0 - (factors.modification_chain_length as f64 / 10.0).min(1.0)
        };
        score += chain_score * WEIGHT_CHAIN_LENGTH;

        // Challenge: no challenges = full score, more challenges = lower score.
        let challenge_score = if factors.challenge_count == 0 {
            1.0
        } else {
            1.0 / (1.0 + factors.challenge_count as f64)
        };
        score += challenge_score * WEIGHT_CHALLENGE;

        score.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relationship::YokeLink;

    fn make_event(id: &str, author: &str, community: Option<&str>) -> EventData {
        EventData {
            id: id.to_string(),
            author: author.to_string(),
            community_id: community.map(String::from),
            created_at: Utc::now() - chrono::Duration::days(30),
        }
    }

    fn make_old_event(id: &str, author: &str, days_ago: i64) -> EventData {
        EventData {
            id: id.to_string(),
            author: author.to_string(),
            community_id: None,
            created_at: Utc::now() - chrono::Duration::days(days_ago),
        }
    }

    // --- ProvenanceScore tests ---

    #[test]
    fn score_strength_boundaries() {
        let factors = ProvenanceFactors::default();
        let strong = ProvenanceScore {
            event_id: "a".into(),
            factors: factors.clone(),
            score: 0.75,
            computed_at: Utc::now(),
        };
        assert!(strong.is_strong());
        assert!(!strong.is_weak());

        let weak = ProvenanceScore {
            event_id: "b".into(),
            factors,
            score: 0.2,
            computed_at: Utc::now(),
        };
        assert!(weak.is_weak());
        assert!(!weak.is_strong());
    }

    #[test]
    fn score_serde() {
        let score = ProvenanceScore {
            event_id: "test-event".into(),
            factors: ProvenanceFactors {
                source_identified: true,
                source_reputation: 0.8,
                corroboration_count: 3,
                corroboration_diversity: 0.6,
                age_days: 30,
                modification_chain_length: 1,
                has_been_challenged: false,
                challenge_count: 0,
            },
            score: 0.72,
            computed_at: Utc::now(),
        };
        let json = serde_json::to_string(&score).unwrap();
        let restored: ProvenanceScore = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.event_id, "test-event");
        assert!((restored.score - 0.72).abs() < 0.001);
    }

    // --- ProvenanceFactors tests ---

    #[test]
    fn default_factors() {
        let f = ProvenanceFactors::default();
        assert!(!f.source_identified);
        assert_eq!(f.source_reputation, 0.0);
        assert_eq!(f.corroboration_count, 0);
        assert_eq!(f.age_days, 0);
        assert_eq!(f.challenge_count, 0);
    }

    #[test]
    fn factors_serde() {
        let f = ProvenanceFactors {
            source_identified: true,
            source_reputation: 0.5,
            corroboration_count: 2,
            corroboration_diversity: 0.4,
            age_days: 10,
            modification_chain_length: 3,
            has_been_challenged: true,
            challenge_count: 1,
        };
        let json = serde_json::to_string(&f).unwrap();
        let restored: ProvenanceFactors = serde_json::from_str(&json).unwrap();
        assert!(restored.source_identified);
        assert_eq!(restored.corroboration_count, 2);
        assert_eq!(restored.challenge_count, 1);
    }

    // --- ProvenanceChain tests ---

    #[test]
    fn chain_known_origin() {
        let chain = ProvenanceChain {
            original_event_id: "orig".into(),
            original_author: "cpub1alice".into(),
            original_community: Some("guild".into()),
            chain: vec![],
            modifications: 0,
            corroborations: vec![],
        };
        assert!(chain.has_known_origin());
        assert_eq!(chain.chain_depth(), 0);
    }

    #[test]
    fn chain_unknown_origin() {
        let chain = ProvenanceChain {
            original_event_id: "orphan".into(),
            original_author: String::new(),
            original_community: None,
            chain: vec![],
            modifications: 0,
            corroborations: vec![],
        };
        assert!(!chain.has_known_origin());
    }

    #[test]
    fn chain_corroborating_communities() {
        let chain = ProvenanceChain {
            original_event_id: "orig".into(),
            original_author: "cpub1alice".into(),
            original_community: None,
            chain: vec![],
            modifications: 0,
            corroborations: vec![
                Corroboration {
                    event_id: "c1".into(),
                    author: "cpub1bob".into(),
                    community_id: Some("guild-a".into()),
                    similarity: 0.9,
                },
                Corroboration {
                    event_id: "c2".into(),
                    author: "cpub1carol".into(),
                    community_id: Some("guild-b".into()),
                    similarity: 0.85,
                },
                Corroboration {
                    event_id: "c3".into(),
                    author: "cpub1dave".into(),
                    community_id: Some("guild-a".into()),
                    similarity: 0.88,
                },
            ],
        };
        assert_eq!(chain.corroborating_communities(), 2);
    }

    #[test]
    fn chain_serde() {
        let chain = ProvenanceChain {
            original_event_id: "orig".into(),
            original_author: "cpub1alice".into(),
            original_community: Some("guild".into()),
            chain: vec![ProvenanceLink {
                event_id: "link-1".into(),
                author: "cpub1bob".into(),
                relation: RelationType::DerivedFrom,
                timestamp: Utc::now(),
            }],
            modifications: 1,
            corroborations: vec![],
        };
        let json = serde_json::to_string(&chain).unwrap();
        let restored: ProvenanceChain = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.original_event_id, "orig");
        assert_eq!(restored.chain.len(), 1);
    }

    // --- ProvenanceComputer tests ---

    #[test]
    fn compute_original_event() {
        // Event with no derivation chain — it IS the original.
        let events = vec![make_event("evt-1", "cpub1alice", Some("guild-a"))];
        let graph = RelationshipGraph::new();

        let score = ProvenanceComputer::compute("evt-1", &graph, &events, &[], 0, 0.8);
        assert!(score.score > 0.0);
        assert!(score.factors.source_identified);
        assert_eq!(score.factors.modification_chain_length, 0);
        assert!(!score.factors.has_been_challenged);
    }

    #[test]
    fn compute_derived_event() {
        let events = vec![
            make_event("original", "cpub1alice", Some("guild-a")),
            make_event("remix", "cpub1bob", Some("guild-b")),
        ];

        let mut graph = RelationshipGraph::new();
        graph.add_link(YokeLink::new(
            "remix",
            "original",
            RelationType::DerivedFrom,
            "cpub1bob",
        ));

        let score = ProvenanceComputer::compute("remix", &graph, &events, &[], 0, 0.8);
        assert_eq!(score.factors.modification_chain_length, 1);
    }

    #[test]
    fn compute_long_chain() {
        let events = vec![
            make_event("e1", "cpub1a", None),
            make_event("e2", "cpub1b", None),
            make_event("e3", "cpub1c", None),
            make_event("e4", "cpub1d", None),
        ];

        let mut graph = RelationshipGraph::new();
        graph.add_link(YokeLink::new("e4", "e3", RelationType::DerivedFrom, "d"));
        graph.add_link(YokeLink::new("e3", "e2", RelationType::VersionOf, "c"));
        graph.add_link(YokeLink::new("e2", "e1", RelationType::DerivedFrom, "b"));

        let score = ProvenanceComputer::compute("e4", &graph, &events, &[], 0, 0.5);
        assert_eq!(score.factors.modification_chain_length, 3);
    }

    #[test]
    fn compute_with_corroborations() {
        let events = vec![make_event("evt", "cpub1alice", Some("guild-a"))];
        let graph = RelationshipGraph::new();

        let corroborations = vec![
            Corroboration {
                event_id: "c1".into(),
                author: "cpub1bob".into(),
                community_id: Some("guild-b".into()),
                similarity: 0.9,
            },
            Corroboration {
                event_id: "c2".into(),
                author: "cpub1carol".into(),
                community_id: Some("guild-c".into()),
                similarity: 0.85,
            },
        ];

        let score =
            ProvenanceComputer::compute("evt", &graph, &events, &corroborations, 0, 0.8);
        assert_eq!(score.factors.corroboration_count, 2);
        assert!(score.factors.corroboration_diversity > 0.0);
    }

    #[test]
    fn compute_with_challenges() {
        let events = vec![make_event("evt", "cpub1alice", None)];
        let graph = RelationshipGraph::new();

        let score = ProvenanceComputer::compute("evt", &graph, &events, &[], 3, 0.8);
        assert!(score.factors.has_been_challenged);
        assert_eq!(score.factors.challenge_count, 3);

        // Compared to unchallenged version.
        let unchallenged = ProvenanceComputer::compute("evt", &graph, &events, &[], 0, 0.8);
        assert!(unchallenged.score > score.score);
    }

    #[test]
    fn compute_high_reputation_helps() {
        let events = vec![make_event("evt", "cpub1alice", None)];
        let graph = RelationshipGraph::new();

        let high_rep = ProvenanceComputer::compute("evt", &graph, &events, &[], 0, 0.9);
        let low_rep = ProvenanceComputer::compute("evt", &graph, &events, &[], 0, 0.1);
        assert!(high_rep.score > low_rep.score);
    }

    #[test]
    fn compute_score_clamped() {
        let events = vec![make_event("evt", "cpub1alice", None)];
        let graph = RelationshipGraph::new();

        let score = ProvenanceComputer::compute("evt", &graph, &events, &[], 0, 2.0);
        assert!(score.score <= 1.0);
        assert!(score.score >= 0.0);
    }

    #[test]
    fn compute_unknown_event() {
        let events: Vec<EventData> = vec![];
        let graph = RelationshipGraph::new();

        let score = ProvenanceComputer::compute("unknown", &graph, &events, &[], 0, 0.0);
        // No event data, so source_identified = false, reputation = 0, etc.
        assert!(!score.factors.source_identified);
        assert_eq!(score.factors.source_reputation, 0.0);
    }

    #[test]
    fn trace_chain_empty() {
        let graph = RelationshipGraph::new();
        let events: std::collections::HashMap<&str, &EventData> =
            std::collections::HashMap::new();

        let chain = ProvenanceComputer::trace_chain("evt", &graph, &events);
        assert!(chain.is_empty());
    }

    #[test]
    fn trace_chain_cycle_safe() {
        let mut graph = RelationshipGraph::new();
        // Create a cycle: a -> b -> a
        graph.add_link(YokeLink::new("a", "b", RelationType::DerivedFrom, "x"));
        graph.add_link(YokeLink::new("b", "a", RelationType::DerivedFrom, "y"));

        let events: std::collections::HashMap<&str, &EventData> =
            std::collections::HashMap::new();

        let chain = ProvenanceComputer::trace_chain("a", &graph, &events);
        // Should terminate due to cycle detection.
        assert!(chain.len() <= 2);
    }

    #[test]
    fn build_chain_basic() {
        let events = vec![
            make_event("original", "cpub1alice", Some("guild-a")),
            make_event("remix", "cpub1bob", Some("guild-b")),
        ];

        let mut graph = RelationshipGraph::new();
        graph.add_link(YokeLink::new(
            "remix",
            "original",
            RelationType::DerivedFrom,
            "cpub1bob",
        ));

        let chain = ProvenanceComputer::build_chain("remix", &graph, &events, vec![]);
        assert_eq!(chain.original_event_id, "original");
        assert_eq!(chain.original_author, "cpub1alice");
        assert_eq!(chain.original_community.as_deref(), Some("guild-a"));
        assert_eq!(chain.modifications, 1);
    }

    #[test]
    fn build_chain_self_is_original() {
        let events = vec![make_event("evt", "cpub1alice", Some("guild"))];
        let graph = RelationshipGraph::new();

        let chain = ProvenanceComputer::build_chain("evt", &graph, &events, vec![]);
        assert_eq!(chain.original_event_id, "evt");
        assert_eq!(chain.original_author, "cpub1alice");
        assert_eq!(chain.modifications, 0);
    }

    // --- Diversity computation tests ---

    #[test]
    fn diversity_empty() {
        assert_eq!(ProvenanceComputer::compute_diversity(&[]), 0.0);
    }

    #[test]
    fn diversity_single_community() {
        let corrs = vec![
            Corroboration {
                event_id: "c1".into(),
                author: "a".into(),
                community_id: Some("guild".into()),
                similarity: 0.9,
            },
            Corroboration {
                event_id: "c2".into(),
                author: "b".into(),
                community_id: Some("guild".into()),
                similarity: 0.8,
            },
        ];
        // All from same community: 1 - (1.0)^2 = 0.0
        assert_eq!(ProvenanceComputer::compute_diversity(&corrs), 0.0);
    }

    #[test]
    fn diversity_two_equal_communities() {
        let corrs = vec![
            Corroboration {
                event_id: "c1".into(),
                author: "a".into(),
                community_id: Some("guild-a".into()),
                similarity: 0.9,
            },
            Corroboration {
                event_id: "c2".into(),
                author: "b".into(),
                community_id: Some("guild-b".into()),
                similarity: 0.8,
            },
        ];
        // 1 - (0.5^2 + 0.5^2) = 1 - 0.5 = 0.5
        let div = ProvenanceComputer::compute_diversity(&corrs);
        assert!((div - 0.5).abs() < 0.001);
    }

    #[test]
    fn diversity_three_equal_communities() {
        let corrs = vec![
            Corroboration {
                event_id: "c1".into(),
                author: "a".into(),
                community_id: Some("a".into()),
                similarity: 0.9,
            },
            Corroboration {
                event_id: "c2".into(),
                author: "b".into(),
                community_id: Some("b".into()),
                similarity: 0.9,
            },
            Corroboration {
                event_id: "c3".into(),
                author: "c".into(),
                community_id: Some("c".into()),
                similarity: 0.9,
            },
        ];
        // 1 - 3 * (1/3)^2 = 1 - 3 * 1/9 = 1 - 1/3 ≈ 0.667
        let div = ProvenanceComputer::compute_diversity(&corrs);
        assert!((div - 0.6667).abs() < 0.01);
    }

    #[test]
    fn diversity_no_community_ids() {
        let corrs = vec![Corroboration {
            event_id: "c1".into(),
            author: "a".into(),
            community_id: None,
            similarity: 0.9,
        }];
        assert_eq!(ProvenanceComputer::compute_diversity(&corrs), 0.0);
    }

    // --- Corroboration tests ---

    #[test]
    fn corroboration_serde() {
        let c = Corroboration {
            event_id: "c1".into(),
            author: "cpub1bob".into(),
            community_id: Some("guild-b".into()),
            similarity: 0.85,
        };
        let json = serde_json::to_string(&c).unwrap();
        let restored: Corroboration = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.author, "cpub1bob");
        assert!((restored.similarity - 0.85).abs() < 0.001);
    }

    // --- ProvenanceLink tests ---

    #[test]
    fn link_serde() {
        let link = ProvenanceLink {
            event_id: "link-1".into(),
            author: "cpub1test".into(),
            relation: RelationType::DerivedFrom,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&link).unwrap();
        let restored: ProvenanceLink = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.event_id, "link-1");
        assert_eq!(restored.relation, RelationType::DerivedFrom);
    }

    // --- Age and old events ---

    #[test]
    fn older_events_score_higher_age_factor() {
        let events_old = vec![make_old_event("evt", "cpub1a", 365)];
        let events_new = vec![make_old_event("evt", "cpub1a", 1)];
        let graph = RelationshipGraph::new();

        let old_score = ProvenanceComputer::compute("evt", &graph, &events_old, &[], 0, 0.5);
        let new_score = ProvenanceComputer::compute("evt", &graph, &events_new, &[], 0, 0.5);

        // Old event should have higher age contribution.
        assert!(old_score.factors.age_days > new_score.factors.age_days);
    }

    // --- EventData tests ---

    #[test]
    fn event_data_construction() {
        let evt = make_event("test", "cpub1alice", Some("guild"));
        assert_eq!(evt.id, "test");
        assert_eq!(evt.author, "cpub1alice");
        assert_eq!(evt.community_id.as_deref(), Some("guild"));
    }
}
