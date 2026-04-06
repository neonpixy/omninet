//! Result merging — deduplication and ranking across multiple Tower responses.
//!
//! When Zeitgeist queries N Towers in parallel, each returns its own ranked
//! results. The merger combines them into a single ordered list:
//! - Dedup by event ID (content-addressed — any copy is identical)
//! - Weight results by Tower relevance (a specialist Tower's results rank higher)
//! - Sort by combined score
//! - Aggregate concept suggestions

use std::collections::{HashMap, HashSet};

use magical_index::SearchResult;
use serde::{Deserialize, Serialize};

/// A search result with provenance — which Tower provided it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MergedResult {
    /// The search result.
    pub result: SearchResult,
    /// The Tower(s) that returned this result (pubkey hex).
    pub sources: Vec<String>,
    /// Combined relevance score (weighted by Tower relevance).
    pub combined_score: f64,
}

/// Aggregated merged response from multiple Towers.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MergedResponse {
    /// Deduplicated, ranked results.
    pub results: Vec<MergedResult>,
    /// Aggregated suggestions from all Towers (deduplicated).
    pub suggestions: Vec<String>,
    /// How many Towers contributed results.
    pub tower_count: usize,
    /// Total results before deduplication.
    pub total_raw_results: usize,
}

/// A batch of results from one Tower, ready to be merged.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerResultBatch {
    /// The Tower's pubkey.
    pub tower_pubkey: String,
    /// How relevant this Tower was for the query (from router).
    pub tower_relevance: f64,
    /// The results from this Tower.
    pub results: Vec<SearchResult>,
    /// Concept suggestions from this Tower.
    pub suggestions: Vec<String>,
}

/// Merges results from multiple Tower responses.
pub struct ResultMerger {
    /// Maximum results in the final merged response.
    max_results: usize,
}

impl ResultMerger {
    /// Create a merger with default settings (max 50 results).
    pub fn new() -> Self {
        Self { max_results: 50 }
    }

    /// Create a merger with a custom result limit.
    pub fn with_max_results(max_results: usize) -> Self {
        Self { max_results }
    }

    /// Merge results from multiple Tower batches.
    ///
    /// Deduplicates by event ID, weights scores by Tower relevance,
    /// and sorts by combined score.
    pub fn merge(&self, batches: Vec<TowerResultBatch>) -> MergedResponse {
        let tower_count = batches.len();
        let mut total_raw = 0;

        // Accumulate: event_id → (best result, sources, combined score).
        let mut by_event: HashMap<String, MergedResult> = HashMap::new();
        let mut all_suggestions: HashSet<String> = HashSet::new();

        for batch in batches {
            total_raw += batch.results.len();

            for suggestion in &batch.suggestions {
                all_suggestions.insert(suggestion.clone());
            }

            for result in &batch.results {
                let weighted_score = result.relevance * (1.0 + batch.tower_relevance);

                match by_event.get_mut(&result.event_id) {
                    Some(existing) => {
                        // Same event from another Tower — merge sources, keep best score.
                        existing.sources.push(batch.tower_pubkey.clone());
                        if weighted_score > existing.combined_score {
                            existing.combined_score = weighted_score;
                            existing.result = result.clone();
                        }
                        // Merge per-result suggestions.
                        for s in &result.suggestions {
                            all_suggestions.insert(s.clone());
                        }
                    }
                    None => {
                        by_event.insert(
                            result.event_id.clone(),
                            MergedResult {
                                result: result.clone(),
                                sources: vec![batch.tower_pubkey.clone()],
                                combined_score: weighted_score,
                            },
                        );
                        for s in &result.suggestions {
                            all_suggestions.insert(s.clone());
                        }
                    }
                }
            }
        }

        // Sort by combined score (highest first).
        let mut results: Vec<MergedResult> = by_event.into_values().collect();
        results.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.max_results);

        let mut suggestions: Vec<String> = all_suggestions.into_iter().collect();
        suggestions.sort();

        MergedResponse {
            results,
            suggestions,
            tower_count,
            total_raw_results: total_raw,
        }
    }
}

impl Default for ResultMerger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(event_id: &str, relevance: f64) -> SearchResult {
        SearchResult {
            event_id: event_id.into(),
            author: "author1".into(),
            kind: 1,
            created_at: 1000,
            relevance,
            snippet: Some(format!("snippet for {event_id}")),
            suggestions: vec![],
        }
    }

    fn make_result_with_suggestions(
        event_id: &str,
        relevance: f64,
        suggestions: Vec<&str>,
    ) -> SearchResult {
        SearchResult {
            event_id: event_id.into(),
            author: "author1".into(),
            kind: 1,
            created_at: 1000,
            relevance,
            snippet: None,
            suggestions: suggestions.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn merge_single_batch() {
        let merger = ResultMerger::new();
        let batch = TowerResultBatch {
            tower_pubkey: "tower_a".into(),
            tower_relevance: 0.8,
            results: vec![
                make_result("event_1", 0.9),
                make_result("event_2", 0.5),
            ],
            suggestions: vec!["related topic".into()],
        };

        let merged = merger.merge(vec![batch]);
        assert_eq!(merged.results.len(), 2);
        assert_eq!(merged.tower_count, 1);
        assert_eq!(merged.total_raw_results, 2);
        assert_eq!(merged.suggestions, vec!["related topic"]);
        // Results should be sorted by combined score.
        assert!(merged.results[0].combined_score >= merged.results[1].combined_score);
    }

    #[test]
    fn merge_deduplicates_by_event_id() {
        let merger = ResultMerger::new();
        let batch_a = TowerResultBatch {
            tower_pubkey: "tower_a".into(),
            tower_relevance: 0.8,
            results: vec![make_result("shared_event", 0.9)],
            suggestions: vec![],
        };
        let batch_b = TowerResultBatch {
            tower_pubkey: "tower_b".into(),
            tower_relevance: 0.5,
            results: vec![make_result("shared_event", 0.7)],
            suggestions: vec![],
        };

        let merged = merger.merge(vec![batch_a, batch_b]);
        // Only one copy of the event.
        assert_eq!(merged.results.len(), 1);
        assert_eq!(merged.total_raw_results, 2);
        // Both sources recorded.
        assert_eq!(merged.results[0].sources.len(), 2);
        assert!(merged.results[0].sources.contains(&"tower_a".to_string()));
        assert!(merged.results[0].sources.contains(&"tower_b".to_string()));
    }

    #[test]
    fn merge_keeps_best_score() {
        let merger = ResultMerger::new();
        let batch_a = TowerResultBatch {
            tower_pubkey: "tower_a".into(),
            tower_relevance: 1.0, // High tower relevance
            results: vec![make_result("event_1", 0.9)],
            suggestions: vec![],
        };
        let batch_b = TowerResultBatch {
            tower_pubkey: "tower_b".into(),
            tower_relevance: 0.1, // Low tower relevance
            results: vec![make_result("event_1", 0.9)],
            suggestions: vec![],
        };

        let merged = merger.merge(vec![batch_a, batch_b]);
        // Combined score should use the higher tower relevance.
        let score = merged.results[0].combined_score;
        assert!(score > 1.0, "score {score} should be > 1.0 (weighted by tower relevance)");
    }

    #[test]
    fn merge_respects_max_results() {
        let merger = ResultMerger::with_max_results(3);
        let results: Vec<SearchResult> = (0..10)
            .map(|i| make_result(&format!("event_{i}"), 1.0 - i as f64 * 0.1))
            .collect();

        let batch = TowerResultBatch {
            tower_pubkey: "tower".into(),
            tower_relevance: 0.5,
            results,
            suggestions: vec![],
        };

        let merged = merger.merge(vec![batch]);
        assert_eq!(merged.results.len(), 3);
        assert_eq!(merged.total_raw_results, 10);
    }

    #[test]
    fn merge_aggregates_suggestions() {
        let merger = ResultMerger::new();
        let batch_a = TowerResultBatch {
            tower_pubkey: "tower_a".into(),
            tower_relevance: 0.5,
            results: vec![make_result_with_suggestions(
                "e1", 0.5, vec!["dovetail joints"],
            )],
            suggestions: vec!["woodworking".into()],
        };
        let batch_b = TowerResultBatch {
            tower_pubkey: "tower_b".into(),
            tower_relevance: 0.5,
            results: vec![make_result_with_suggestions(
                "e2", 0.5, vec!["hand tools"],
            )],
            suggestions: vec!["carpentry".into(), "woodworking".into()], // duplicate
        };

        let merged = merger.merge(vec![batch_a, batch_b]);
        // Suggestions should be deduplicated.
        assert!(merged.suggestions.contains(&"woodworking".into()));
        assert!(merged.suggestions.contains(&"carpentry".into()));
        assert!(merged.suggestions.contains(&"dovetail joints".into()));
        assert!(merged.suggestions.contains(&"hand tools".into()));
        // "woodworking" appears only once.
        assert_eq!(
            merged.suggestions.iter().filter(|s| *s == "woodworking").count(),
            1
        );
    }

    #[test]
    fn merge_empty_batches() {
        let merger = ResultMerger::new();
        let merged = merger.merge(vec![]);
        assert!(merged.results.is_empty());
        assert!(merged.suggestions.is_empty());
        assert_eq!(merged.tower_count, 0);
        assert_eq!(merged.total_raw_results, 0);
    }

    #[test]
    fn merge_sorts_by_combined_score() {
        let merger = ResultMerger::new();
        let batch = TowerResultBatch {
            tower_pubkey: "tower".into(),
            tower_relevance: 0.5,
            results: vec![
                make_result("low", 0.1),
                make_result("high", 0.9),
                make_result("mid", 0.5),
            ],
            suggestions: vec![],
        };

        let merged = merger.merge(vec![batch]);
        assert_eq!(merged.results[0].result.event_id, "high");
        assert_eq!(merged.results[1].result.event_id, "mid");
        assert_eq!(merged.results[2].result.event_id, "low");
    }

    #[test]
    fn merged_response_serde_round_trip() {
        let resp = MergedResponse {
            results: vec![MergedResult {
                result: make_result("e1", 0.8),
                sources: vec!["tower_a".into()],
                combined_score: 1.2,
            }],
            suggestions: vec!["related".into()],
            tower_count: 1,
            total_raw_results: 1,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let loaded: MergedResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.results.len(), 1);
        assert_eq!(loaded.tower_count, 1);
    }
}
