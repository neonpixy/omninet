//! Trending — aggregate signals from the network.
//!
//! What's trending is what people actually search for and create,
//! not what an algorithm promotes. Signals come from:
//! - Search queries (popular queries)
//! - Tower Semantic Profiles (topic vectors)
//! - Content creation rates

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A trending signal — a topic with a score.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendSignal {
    /// The topic label.
    pub topic: String,
    /// Trend score (higher = more trending). Not normalized.
    pub score: f64,
    /// Number of data points contributing to this score.
    pub signal_count: u64,
    /// When this signal was last updated (Unix timestamp).
    pub last_updated: i64,
}

/// Configuration for the trend tracker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendConfig {
    /// Maximum number of trends to track (default: 100).
    pub max_trends: usize,
    /// Decay factor applied per update cycle (default: 0.95).
    /// Trends lose relevance over time unless refreshed.
    pub decay_factor: f64,
    /// Minimum score to keep a trend (default: 0.01).
    /// Trends below this are pruned.
    pub min_score: f64,
}

impl Default for TrendConfig {
    fn default() -> Self {
        Self {
            max_trends: 100,
            decay_factor: 0.95,
            min_score: 0.01,
        }
    }
}

/// Tracks trending topics across the network.
///
/// Fed by search queries, Tower semantic profiles, and content signals.
/// Applies time decay so stale trends fade naturally.
#[derive(Clone, Debug, Default)]
pub struct TrendTracker {
    /// Current trends, keyed by normalized topic.
    trends: HashMap<String, TrendSignal>,
    /// Configuration.
    config: TrendConfig,
}

impl TrendTracker {
    /// Create a new trend tracker with default config.
    pub fn new() -> Self {
        Self {
            trends: HashMap::new(),
            config: TrendConfig::default(),
        }
    }

    /// Create a trend tracker with custom config.
    pub fn with_config(config: TrendConfig) -> Self {
        Self {
            trends: HashMap::new(),
            config,
        }
    }

    /// Record a search query as a trend signal.
    ///
    /// Extracts terms from the query and boosts their scores.
    pub fn record_query(&mut self, query: &str, now: i64) {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() > 2) // Skip tiny words
            .collect();

        for term in terms {
            self.boost_topic(&term, 1.0, now);
        }
    }

    /// Record topics from a Tower's Semantic Profile.
    ///
    /// Topics from Towers with high content counts get stronger signals.
    pub fn record_tower_topics(&mut self, topics: &[String], content_count: u64, now: i64) {
        // Scale weight by log of content count (diminishing returns).
        let weight = if content_count > 0 {
            (content_count as f64).ln().max(1.0) * 0.5
        } else {
            0.5
        };

        for topic in topics {
            self.boost_topic(&topic.to_lowercase(), weight, now);
        }
    }

    /// Boost a topic's trend score.
    fn boost_topic(&mut self, topic: &str, weight: f64, now: i64) {
        match self.trends.get_mut(topic) {
            Some(signal) => {
                signal.score += weight;
                signal.signal_count += 1;
                signal.last_updated = now;
            }
            None => {
                if self.trends.len() >= self.config.max_trends {
                    self.prune();
                }
                self.trends.insert(
                    topic.into(),
                    TrendSignal {
                        topic: topic.into(),
                        score: weight,
                        signal_count: 1,
                        last_updated: now,
                    },
                );
            }
        }
    }

    /// Apply time decay to all trends and prune dead ones.
    pub fn decay(&mut self) {
        let factor = self.config.decay_factor;
        let min = self.config.min_score;

        self.trends.retain(|_, signal| {
            signal.score *= factor;
            signal.score >= min
        });
    }

    /// Get the top N trending topics, sorted by score.
    pub fn top(&self, n: usize) -> Vec<&TrendSignal> {
        let mut sorted: Vec<&TrendSignal> = self.trends.values().collect();
        sorted.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(n);
        sorted
    }

    /// Get a specific trend by topic name.
    pub fn get(&self, topic: &str) -> Option<&TrendSignal> {
        self.trends.get(&topic.to_lowercase())
    }

    /// Number of tracked trends.
    pub fn count(&self) -> usize {
        self.trends.len()
    }

    /// Remove trends below the minimum score.
    fn prune(&mut self) {
        let min = self.config.min_score;
        self.trends.retain(|_, signal| signal.score >= min);

        // If still at capacity, remove the lowest-scored.
        while self.trends.len() >= self.config.max_trends {
            if let Some(weakest) = self
                .trends
                .iter()
                .min_by(|(_, a), (_, b)| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(k, _)| k.clone())
            {
                self.trends.remove(&weakest);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker() {
        let tracker = TrendTracker::new();
        assert_eq!(tracker.count(), 0);
        assert!(tracker.top(10).is_empty());
    }

    #[test]
    fn record_query() {
        let mut tracker = TrendTracker::new();
        tracker.record_query("woodworking joints dovetail", 1000);

        assert_eq!(tracker.count(), 3);
        assert!(tracker.get("woodworking").is_some());
        assert!(tracker.get("joints").is_some());
        assert!(tracker.get("dovetail").is_some());
    }

    #[test]
    fn short_words_filtered() {
        let mut tracker = TrendTracker::new();
        tracker.record_query("a is the and woodworking", 1000);

        // Only "the", "and", "woodworking" have len > 2.
        assert!(tracker.get("a").is_none());
        assert!(tracker.get("is").is_none());
        assert!(tracker.get("woodworking").is_some());
    }

    #[test]
    fn repeated_queries_boost_score() {
        let mut tracker = TrendTracker::new();
        tracker.record_query("woodworking", 1000);
        tracker.record_query("woodworking", 2000);
        tracker.record_query("woodworking", 3000);

        let signal = tracker.get("woodworking").unwrap();
        assert_eq!(signal.signal_count, 3);
        assert!(signal.score > 2.0);
        assert_eq!(signal.last_updated, 3000);
    }

    #[test]
    fn record_tower_topics() {
        let mut tracker = TrendTracker::new();
        tracker.record_tower_topics(
            &["Art".into(), "Design".into()],
            5000,
            1000,
        );

        assert!(tracker.get("art").is_some());
        assert!(tracker.get("design").is_some());
        // Higher content count → higher weight.
        let score = tracker.get("art").unwrap().score;
        assert!(score > 1.0, "score {score} should be > 1.0 for high content count");
    }

    #[test]
    fn top_n() {
        let mut tracker = TrendTracker::new();
        tracker.record_query("rust", 1000);
        tracker.record_query("rust", 2000);
        tracker.record_query("rust", 3000);
        tracker.record_query("python", 1000);
        tracker.record_query("python", 2000);
        tracker.record_query("javascript", 1000);

        let top2 = tracker.top(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].topic, "rust"); // Most boosted.
        assert_eq!(top2[1].topic, "python");
    }

    #[test]
    fn decay() {
        let mut tracker = TrendTracker::with_config(TrendConfig {
            decay_factor: 0.5,
            min_score: 0.1,
            ..Default::default()
        });
        tracker.record_query("woodworking", 1000); // score = 1.0

        tracker.decay(); // score = 0.5
        let signal = tracker.get("woodworking").unwrap();
        assert!((signal.score - 0.5).abs() < 0.001);

        tracker.decay(); // score = 0.25
        tracker.decay(); // score = 0.125
        tracker.decay(); // score = 0.0625 < 0.1 → pruned

        assert!(tracker.get("woodworking").is_none());
    }

    #[test]
    fn capacity_eviction() {
        let mut tracker = TrendTracker::with_config(TrendConfig {
            max_trends: 3,
            ..Default::default()
        });

        tracker.record_query("alpha", 1000);
        tracker.record_query("beta", 1000);
        tracker.record_query("gamma", 1000);
        assert_eq!(tracker.count(), 3);

        // Boost alpha so it's strongest.
        tracker.record_query("alpha", 2000);
        tracker.record_query("alpha", 3000);

        // Adding a 4th should evict the weakest.
        tracker.record_query("delta", 4000);
        assert!(tracker.count() <= 3);
        // Alpha should survive (highest score).
        assert!(tracker.get("alpha").is_some());
    }

    #[test]
    fn case_insensitive() {
        let mut tracker = TrendTracker::new();
        tracker.record_query("Woodworking", 1000);
        tracker.record_query("WOODWORKING", 2000);
        tracker.record_query("woodworking", 3000);

        assert_eq!(tracker.count(), 1);
        let signal = tracker.get("woodworking").unwrap();
        assert_eq!(signal.signal_count, 3);
    }

    #[test]
    fn config_serde() {
        let config = TrendConfig {
            max_trends: 50,
            decay_factor: 0.9,
            min_score: 0.05,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: TrendConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.max_trends, 50);
        assert_eq!(loaded.decay_factor, 0.9);
    }

    #[test]
    fn signal_serde() {
        let signal = TrendSignal {
            topic: "rust".into(),
            score: 4.2,
            signal_count: 10,
            last_updated: 5000,
        };
        let json = serde_json::to_string(&signal).unwrap();
        let loaded: TrendSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.topic, "rust");
        assert_eq!(loaded.score, 4.2);
        assert_eq!(loaded.signal_count, 10);
    }
}
