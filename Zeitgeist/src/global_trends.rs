//! Cross-Community Zeitgeist — global trends with diversity weighting.
//!
//! Extends the local `TrendTracker` with a global scope that weights
//! trends by community diversity. A topic popular in 50 small communities
//! scores higher than one popular in 1 large community. This prevents
//! any single community from manufacturing global trends.
//!
//! # Diversity Weighting
//!
//! Uses Simpson's diversity index:
//! ```text
//! global_score = sum(local_scores) * diversity_index
//! diversity_index = 1 - sum((community_share)^2)
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The scope for a Zeitgeist query.
///
/// Controls whether results come from a single community,
/// a selection, or the entire network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ZeitgeistScope {
    /// Single community.
    Local(String),
    /// Selected communities.
    Communities(Vec<String>),
    /// All participating communities.
    Global,
}

impl ZeitgeistScope {
    /// Whether this scope is global (all communities).
    pub fn is_global(&self) -> bool {
        matches!(self, ZeitgeistScope::Global)
    }

    /// Whether this scope includes a specific community.
    pub fn includes(&self, community_id: &str) -> bool {
        match self {
            ZeitgeistScope::Local(id) => id == community_id,
            ZeitgeistScope::Communities(ids) => ids.iter().any(|id| id == community_id),
            ZeitgeistScope::Global => true,
        }
    }
}

/// Sentiment of a community toward a trending topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrendSentiment {
    /// Predominantly positive engagement.
    Positive,
    /// Predominantly negative engagement.
    Negative,
    /// Balanced or factual engagement.
    Neutral,
    /// Significant split in sentiment.
    Mixed,
    /// Not enough data to determine sentiment.
    Unknown,
}

impl std::fmt::Display for TrendSentiment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendSentiment::Positive => write!(f, "positive"),
            TrendSentiment::Negative => write!(f, "negative"),
            TrendSentiment::Neutral => write!(f, "neutral"),
            TrendSentiment::Mixed => write!(f, "mixed"),
            TrendSentiment::Unknown => write!(f, "unknown"),
        }
    }
}

/// A community's view of a trending topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityTrendView {
    /// The community ID.
    pub community_id: String,
    /// How strongly this topic trends locally (raw score).
    pub local_score: f64,
    /// The community's overall sentiment toward the topic.
    pub sentiment: TrendSentiment,
}

/// A globally trending topic with diversity weighting.
///
/// `global_score` is NOT just the sum of local scores — it is
/// weighted by community diversity so that broad interest across
/// many communities outweighs concentrated interest in one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalTrend {
    /// The topic label.
    pub topic: String,
    /// Diversity-weighted global score.
    pub global_score: f64,
    /// How many communities are discussing this topic.
    pub community_count: usize,
    /// Simpson's diversity index for this topic (0.0 to 1.0).
    pub diversity_index: f64,
    /// Per-community breakdown.
    pub community_breakdown: Vec<CommunityTrendView>,
}

/// How different communities view the same topic.
///
/// A perspective aggregation that shows consensus or disagreement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPerspective {
    /// The topic.
    pub topic: String,
    /// Total communities discussing this topic.
    pub communities_discussing: usize,
    /// Communities with positive sentiment.
    pub communities_positive: usize,
    /// Communities with negative sentiment.
    pub communities_negative: usize,
    /// Communities with neutral sentiment.
    pub communities_neutral: usize,
    /// Known communities NOT discussing this topic.
    pub communities_not_discussing: usize,
    /// Consensus level (0.0 = total disagreement, 1.0 = total agreement).
    pub consensus_level: f64,
}

/// Tracks trending topics across communities with diversity weighting.
///
/// Records per-community trend data and computes global trends
/// that prevent any single community from dominating the narrative.
#[derive(Debug, Clone, Default)]
pub struct GlobalTrendTracker {
    /// Per-topic, per-community trend data.
    topics: HashMap<String, HashMap<String, CommunityTrendView>>,
    /// Set of all known community IDs (for "not discussing" counts).
    known_communities: std::collections::HashSet<String>,
}

impl GlobalTrendTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a community's trend signal for a topic.
    ///
    /// Overwrites the previous score for this topic+community pair.
    pub fn record_community_trend(
        &mut self,
        topic: impl Into<String>,
        community_id: impl Into<String>,
        score: f64,
        sentiment: TrendSentiment,
    ) {
        let topic = topic.into().to_lowercase();
        let community_id = community_id.into();

        self.known_communities.insert(community_id.clone());

        let view = CommunityTrendView {
            community_id: community_id.clone(),
            local_score: score,
            sentiment,
        };

        self.topics
            .entry(topic)
            .or_default()
            .insert(community_id, view);
    }

    /// Register a community as known (even if it hasn't reported any trends).
    ///
    /// Used for accurate "not discussing" counts in perspectives.
    pub fn register_community(&mut self, community_id: impl Into<String>) {
        self.known_communities.insert(community_id.into());
    }

    /// Get the top N globally trending topics, ranked by diversity-weighted score.
    pub fn top_global(&self, n: usize) -> Vec<GlobalTrend> {
        let mut trends: Vec<GlobalTrend> = self
            .topics
            .iter()
            .map(|(topic, communities)| self.compute_global_trend(topic, communities))
            .collect();

        trends.sort_by(|a, b| {
            b.global_score
                .partial_cmp(&a.global_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        trends.truncate(n);
        trends
    }

    /// Get a perspective on how different communities view a topic.
    pub fn perspective(&self, topic: &str) -> Option<TrendPerspective> {
        let normalized = topic.to_lowercase();
        let communities = self.topics.get(&normalized)?;

        let mut positive = 0usize;
        let mut negative = 0usize;
        let mut neutral = 0usize;

        for view in communities.values() {
            match view.sentiment {
                TrendSentiment::Positive => positive += 1,
                TrendSentiment::Negative => negative += 1,
                TrendSentiment::Neutral => neutral += 1,
                TrendSentiment::Mixed | TrendSentiment::Unknown => {}
            }
        }

        let discussing = communities.len();
        let not_discussing = self.known_communities.len().saturating_sub(discussing);

        // Consensus: ratio of the dominant sentiment to total discussants.
        let consensus_level = if discussing == 0 {
            0.0
        } else {
            let max_group = positive.max(negative).max(neutral) as f64;
            max_group / discussing as f64
        };

        Some(TrendPerspective {
            topic: normalized,
            communities_discussing: discussing,
            communities_positive: positive,
            communities_negative: negative,
            communities_neutral: neutral,
            communities_not_discussing: not_discussing,
            consensus_level,
        })
    }

    /// Get a specific topic's global trend, if tracked.
    pub fn get_trend(&self, topic: &str) -> Option<GlobalTrend> {
        let normalized = topic.to_lowercase();
        self.topics
            .get(&normalized)
            .map(|communities| self.compute_global_trend(&normalized, communities))
    }

    /// Get the top N globally trending topics, filtered by federation scope.
    ///
    /// When unrestricted, delegates to `top_global()` (fast-path).
    /// Otherwise, recomputes trends using only data from visible communities.
    pub fn top_global_scoped(
        &self,
        n: usize,
        scope: &crate::FederationScope,
    ) -> Vec<GlobalTrend> {
        if scope.is_unrestricted() {
            return self.top_global(n);
        }

        let mut trends: Vec<GlobalTrend> = self
            .topics
            .iter()
            .filter_map(|(topic, communities)| {
                let filtered: HashMap<String, CommunityTrendView> = communities
                    .iter()
                    .filter(|(cid, _)| scope.is_visible(cid))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                if filtered.is_empty() {
                    return None;
                }

                Some(self.compute_global_trend(topic, &filtered))
            })
            .collect();

        trends.sort_by(|a, b| {
            b.global_score
                .partial_cmp(&a.global_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        trends.truncate(n);
        trends
    }

    /// Get a perspective on how visible communities view a topic.
    ///
    /// When unrestricted, delegates to `perspective()` (fast-path).
    /// Otherwise, only includes data from communities in the scope.
    pub fn perspective_scoped(
        &self,
        topic: &str,
        scope: &crate::FederationScope,
    ) -> Option<TrendPerspective> {
        if scope.is_unrestricted() {
            return self.perspective(topic);
        }

        let normalized = topic.to_lowercase();
        let communities = self.topics.get(&normalized)?;

        let mut positive = 0usize;
        let mut negative = 0usize;
        let mut neutral = 0usize;
        let mut discussing = 0usize;

        for (cid, view) in communities {
            if !scope.is_visible(cid) {
                continue;
            }
            discussing += 1;
            match view.sentiment {
                TrendSentiment::Positive => positive += 1,
                TrendSentiment::Negative => negative += 1,
                TrendSentiment::Neutral => neutral += 1,
                TrendSentiment::Mixed | TrendSentiment::Unknown => {}
            }
        }

        if discussing == 0 {
            return None;
        }

        let visible_known = self
            .known_communities
            .iter()
            .filter(|c| scope.is_visible(c))
            .count();
        let not_discussing = visible_known.saturating_sub(discussing);

        let consensus_level = {
            let max_group = positive.max(negative).max(neutral) as f64;
            max_group / discussing as f64
        };

        Some(TrendPerspective {
            topic: normalized,
            communities_discussing: discussing,
            communities_positive: positive,
            communities_negative: negative,
            communities_neutral: neutral,
            communities_not_discussing: not_discussing,
            consensus_level,
        })
    }

    /// Total number of tracked topics.
    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// Total number of known communities.
    pub fn community_count(&self) -> usize {
        self.known_communities.len()
    }

    /// Compute a `GlobalTrend` for a topic from its community data.
    fn compute_global_trend(
        &self,
        topic: &str,
        communities: &HashMap<String, CommunityTrendView>,
    ) -> GlobalTrend {
        let breakdown: Vec<CommunityTrendView> = communities.values().cloned().collect();
        let community_count = breakdown.len();

        // Sum of local scores.
        let sum_local: f64 = breakdown.iter().map(|v| v.local_score).sum();

        // Simpson's diversity index: 1 - sum((share)^2).
        let diversity_index = if sum_local > 0.0 {
            let sum_squares: f64 = breakdown
                .iter()
                .map(|v| {
                    let share = v.local_score / sum_local;
                    share * share
                })
                .sum();
            1.0 - sum_squares
        } else {
            0.0
        };

        // Global score = sum * diversity.
        let global_score = sum_local * diversity_index;

        GlobalTrend {
            topic: topic.to_string(),
            global_score,
            community_count,
            diversity_index,
            community_breakdown: breakdown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ZeitgeistScope tests ---

    #[test]
    fn scope_local() {
        let scope = ZeitgeistScope::Local("guild-a".into());
        assert!(!scope.is_global());
        assert!(scope.includes("guild-a"));
        assert!(!scope.includes("guild-b"));
    }

    #[test]
    fn scope_communities() {
        let scope = ZeitgeistScope::Communities(vec!["a".into(), "b".into()]);
        assert!(!scope.is_global());
        assert!(scope.includes("a"));
        assert!(scope.includes("b"));
        assert!(!scope.includes("c"));
    }

    #[test]
    fn scope_global() {
        let scope = ZeitgeistScope::Global;
        assert!(scope.is_global());
        assert!(scope.includes("anything"));
    }

    #[test]
    fn scope_serde() {
        let scope = ZeitgeistScope::Communities(vec!["a".into(), "b".into()]);
        let json = serde_json::to_string(&scope).unwrap();
        let restored: ZeitgeistScope = serde_json::from_str(&json).unwrap();
        assert_eq!(scope, restored);
    }

    // --- TrendSentiment tests ---

    #[test]
    fn sentiment_display() {
        assert_eq!(TrendSentiment::Positive.to_string(), "positive");
        assert_eq!(TrendSentiment::Negative.to_string(), "negative");
        assert_eq!(TrendSentiment::Neutral.to_string(), "neutral");
        assert_eq!(TrendSentiment::Mixed.to_string(), "mixed");
        assert_eq!(TrendSentiment::Unknown.to_string(), "unknown");
    }

    #[test]
    fn sentiment_serde() {
        let s = TrendSentiment::Mixed;
        let json = serde_json::to_string(&s).unwrap();
        let restored: TrendSentiment = serde_json::from_str(&json).unwrap();
        assert_eq!(s, restored);
    }

    // --- GlobalTrendTracker tests ---

    #[test]
    fn empty_tracker() {
        let tracker = GlobalTrendTracker::new();
        assert_eq!(tracker.topic_count(), 0);
        assert_eq!(tracker.community_count(), 0);
        assert!(tracker.top_global(10).is_empty());
    }

    #[test]
    fn single_community_single_topic() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("rust", "guild-a", 5.0, TrendSentiment::Positive);

        assert_eq!(tracker.topic_count(), 1);
        assert_eq!(tracker.community_count(), 1);

        let trends = tracker.top_global(10);
        assert_eq!(trends.len(), 1);
        assert_eq!(trends[0].topic, "rust");
        assert_eq!(trends[0].community_count, 1);
        // Single community: diversity = 1 - 1^2 = 0, so global_score = 5 * 0 = 0.
        assert_eq!(trends[0].diversity_index, 0.0);
        assert_eq!(trends[0].global_score, 0.0);
    }

    #[test]
    fn diversity_boosts_multi_community_topics() {
        let mut tracker = GlobalTrendTracker::new();

        // Topic A: popular in one community (score 100).
        tracker.record_community_trend("mono-topic", "big-guild", 100.0, TrendSentiment::Positive);

        // Topic B: popular across 5 communities (total score 50, each 10).
        for i in 0..5 {
            tracker.record_community_trend(
                "diverse-topic",
                format!("guild-{i}"),
                10.0,
                TrendSentiment::Positive,
            );
        }

        let trends = tracker.top_global(2);
        assert_eq!(trends.len(), 2);

        let diverse = trends.iter().find(|t| t.topic == "diverse-topic").unwrap();
        let mono = trends.iter().find(|t| t.topic == "mono-topic").unwrap();

        // Diverse topic should rank higher despite lower raw total.
        assert!(
            diverse.global_score > mono.global_score,
            "diverse={} should be > mono={}",
            diverse.global_score,
            mono.global_score
        );
        assert!(diverse.diversity_index > 0.5);
    }

    #[test]
    fn two_equal_communities() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "a", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "b", 10.0, TrendSentiment::Positive);

        let trend = tracker.get_trend("topic").unwrap();
        // diversity = 1 - (0.5^2 + 0.5^2) = 0.5
        assert!((trend.diversity_index - 0.5).abs() < 0.001);
        // global = 20 * 0.5 = 10
        assert!((trend.global_score - 10.0).abs() < 0.001);
    }

    #[test]
    fn three_equal_communities() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "a", 10.0, TrendSentiment::Neutral);
        tracker.record_community_trend("topic", "b", 10.0, TrendSentiment::Neutral);
        tracker.record_community_trend("topic", "c", 10.0, TrendSentiment::Neutral);

        let trend = tracker.get_trend("topic").unwrap();
        // diversity = 1 - 3*(1/3)^2 = 1 - 1/3 ≈ 0.667
        assert!((trend.diversity_index - 0.6667).abs() < 0.01);
        // global = 30 * 0.667 ≈ 20
        assert!((trend.global_score - 20.0).abs() < 0.5);
    }

    #[test]
    fn unequal_communities() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "big", 90.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "small", 10.0, TrendSentiment::Positive);

        let trend = tracker.get_trend("topic").unwrap();
        // shares: 0.9 and 0.1
        // diversity = 1 - (0.81 + 0.01) = 0.18
        assert!((trend.diversity_index - 0.18).abs() < 0.01);
    }

    #[test]
    fn top_global_ordering() {
        let mut tracker = GlobalTrendTracker::new();

        // Topic with good diversity.
        tracker.record_community_trend("diverse", "a", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("diverse", "b", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("diverse", "c", 10.0, TrendSentiment::Positive);

        // Topic with poor diversity but higher raw.
        tracker.record_community_trend("concentrated", "x", 100.0, TrendSentiment::Positive);

        let top = tracker.top_global(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].topic, "diverse");
    }

    #[test]
    fn top_global_limit() {
        let mut tracker = GlobalTrendTracker::new();
        for i in 0..10 {
            tracker.record_community_trend(
                format!("topic-{i}"),
                "guild",
                (10 - i) as f64,
                TrendSentiment::Neutral,
            );
        }

        let top = tracker.top_global(3);
        assert_eq!(top.len(), 3);
    }

    #[test]
    fn case_insensitive_topics() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("Rust", "a", 5.0, TrendSentiment::Positive);
        tracker.record_community_trend("RUST", "b", 5.0, TrendSentiment::Positive);
        tracker.record_community_trend("rust", "c", 5.0, TrendSentiment::Positive);

        assert_eq!(tracker.topic_count(), 1);

        let trend = tracker.get_trend("rust").unwrap();
        assert_eq!(trend.community_count, 3);
    }

    #[test]
    fn overwrite_community_score() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "guild", 5.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "guild", 15.0, TrendSentiment::Negative);

        let trend = tracker.get_trend("topic").unwrap();
        assert_eq!(trend.community_count, 1);
        assert_eq!(trend.community_breakdown[0].local_score, 15.0);
        assert_eq!(trend.community_breakdown[0].sentiment, TrendSentiment::Negative);
    }

    // --- Perspective tests ---

    #[test]
    fn perspective_basic() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.register_community("guild-a");
        tracker.register_community("guild-b");
        tracker.register_community("guild-c");
        tracker.register_community("guild-d");

        tracker.record_community_trend("topic", "guild-a", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "guild-b", 8.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "guild-c", 5.0, TrendSentiment::Negative);

        let p = tracker.perspective("topic").unwrap();
        assert_eq!(p.communities_discussing, 3);
        assert_eq!(p.communities_positive, 2);
        assert_eq!(p.communities_negative, 1);
        assert_eq!(p.communities_neutral, 0);
        assert_eq!(p.communities_not_discussing, 1); // guild-d
    }

    #[test]
    fn perspective_consensus() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "a", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "b", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "c", 10.0, TrendSentiment::Positive);

        let p = tracker.perspective("topic").unwrap();
        // All positive: consensus = 3/3 = 1.0
        assert!((p.consensus_level - 1.0).abs() < 0.001);
    }

    #[test]
    fn perspective_disagreement() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "a", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("topic", "b", 10.0, TrendSentiment::Negative);

        let p = tracker.perspective("topic").unwrap();
        // Split: consensus = 1/2 = 0.5
        assert!((p.consensus_level - 0.5).abs() < 0.001);
    }

    #[test]
    fn perspective_unknown_topic() {
        let tracker = GlobalTrendTracker::new();
        assert!(tracker.perspective("nonexistent").is_none());
    }

    #[test]
    fn perspective_case_insensitive() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("Rust", "a", 10.0, TrendSentiment::Positive);

        assert!(tracker.perspective("rust").is_some());
        assert!(tracker.perspective("RUST").is_some());
    }

    // --- Serde tests ---

    #[test]
    fn global_trend_serde() {
        let trend = GlobalTrend {
            topic: "rust".into(),
            global_score: 15.5,
            community_count: 3,
            diversity_index: 0.67,
            community_breakdown: vec![CommunityTrendView {
                community_id: "guild-a".into(),
                local_score: 10.0,
                sentiment: TrendSentiment::Positive,
            }],
        };
        let json = serde_json::to_string(&trend).unwrap();
        let restored: GlobalTrend = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.topic, "rust");
        assert_eq!(restored.community_count, 3);
    }

    #[test]
    fn community_trend_view_serde() {
        let view = CommunityTrendView {
            community_id: "guild".into(),
            local_score: 7.5,
            sentiment: TrendSentiment::Mixed,
        };
        let json = serde_json::to_string(&view).unwrap();
        let restored: CommunityTrendView = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.community_id, "guild");
        assert_eq!(restored.sentiment, TrendSentiment::Mixed);
    }

    #[test]
    fn trend_perspective_serde() {
        let p = TrendPerspective {
            topic: "rust".into(),
            communities_discussing: 5,
            communities_positive: 3,
            communities_negative: 1,
            communities_neutral: 1,
            communities_not_discussing: 2,
            consensus_level: 0.6,
        };
        let json = serde_json::to_string(&p).unwrap();
        let restored: TrendPerspective = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.communities_discussing, 5);
        assert!((restored.consensus_level - 0.6).abs() < 0.001);
    }

    // --- Register community tests ---

    #[test]
    fn register_community_without_trends() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.register_community("silent-guild");
        assert_eq!(tracker.community_count(), 1);
        assert_eq!(tracker.topic_count(), 0);
    }

    #[test]
    fn zero_score_diversity() {
        let mut tracker = GlobalTrendTracker::new();
        tracker.record_community_trend("topic", "a", 0.0, TrendSentiment::Neutral);
        tracker.record_community_trend("topic", "b", 0.0, TrendSentiment::Neutral);

        let trend = tracker.get_trend("topic").unwrap();
        assert_eq!(trend.diversity_index, 0.0);
        assert_eq!(trend.global_score, 0.0);
    }

    // --- Federation scope tests ---

    fn scoped_tracker() -> GlobalTrendTracker {
        let mut tracker = GlobalTrendTracker::new();
        tracker.register_community("guild-a");
        tracker.register_community("guild-b");
        tracker.register_community("guild-c");
        tracker.register_community("guild-d");

        // "rust" discussed by guild-a and guild-b
        tracker.record_community_trend("rust", "guild-a", 10.0, TrendSentiment::Positive);
        tracker.record_community_trend("rust", "guild-b", 8.0, TrendSentiment::Positive);

        // "art" discussed by guild-c and guild-d
        tracker.record_community_trend("art", "guild-c", 12.0, TrendSentiment::Neutral);
        tracker.record_community_trend("art", "guild-d", 6.0, TrendSentiment::Negative);

        // "news" discussed by all four
        tracker.record_community_trend("news", "guild-a", 5.0, TrendSentiment::Positive);
        tracker.record_community_trend("news", "guild-b", 5.0, TrendSentiment::Neutral);
        tracker.record_community_trend("news", "guild-c", 5.0, TrendSentiment::Negative);
        tracker.record_community_trend("news", "guild-d", 5.0, TrendSentiment::Positive);

        tracker
    }

    #[test]
    fn top_global_scoped_unrestricted_matches_unscoped() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::new();

        let unscoped = tracker.top_global(10);
        let scoped = tracker.top_global_scoped(10, &scope);

        assert_eq!(unscoped.len(), scoped.len());
        for (u, s) in unscoped.iter().zip(scoped.iter()) {
            assert_eq!(u.topic, s.topic);
            assert!((u.global_score - s.global_score).abs() < 0.001);
        }
    }

    #[test]
    fn top_global_scoped_filters_communities() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::from_communities(["guild-a", "guild-b"]);

        let trends = tracker.top_global_scoped(10, &scope);

        // "rust" (guild-a + guild-b) and "news" (guild-a + guild-b) should appear.
        // "art" (guild-c + guild-d) should NOT appear.
        let topics: Vec<&str> = trends.iter().map(|t| t.topic.as_str()).collect();
        assert!(topics.contains(&"rust"));
        assert!(topics.contains(&"news"));
        assert!(!topics.contains(&"art"));
    }

    #[test]
    fn top_global_scoped_recomputes_diversity() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::from_communities(["guild-a"]);

        let trends = tracker.top_global_scoped(10, &scope);

        // Only guild-a visible. All topics from guild-a are single-community → diversity=0.
        for trend in &trends {
            assert_eq!(
                trend.diversity_index, 0.0,
                "single-community scope should have diversity 0"
            );
            assert_eq!(
                trend.global_score, 0.0,
                "single-community scope should have global_score 0"
            );
        }
    }

    #[test]
    fn top_global_scoped_no_matching_returns_empty() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::from_communities(["nonexistent"]);

        let trends = tracker.top_global_scoped(10, &scope);
        assert!(trends.is_empty());
    }

    #[test]
    fn perspective_scoped_unrestricted_matches_unscoped() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::new();

        let unscoped = tracker.perspective("rust").unwrap();
        let scoped = tracker.perspective_scoped("rust", &scope).unwrap();

        assert_eq!(unscoped.communities_discussing, scoped.communities_discussing);
        assert_eq!(unscoped.communities_positive, scoped.communities_positive);
    }

    #[test]
    fn perspective_scoped_filters_communities() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::from_communities(["guild-a", "guild-b"]);

        // "news" has 4 communities. Scoped to guild-a + guild-b = 2 discussing.
        let p = tracker.perspective_scoped("news", &scope).unwrap();
        assert_eq!(p.communities_discussing, 2);
        assert_eq!(p.communities_positive, 1); // guild-a
        assert_eq!(p.communities_neutral, 1); // guild-b
        assert_eq!(p.communities_negative, 0); // guild-c filtered out
    }

    #[test]
    fn perspective_scoped_no_visible_returns_none() {
        let tracker = scoped_tracker();
        let scope = crate::FederationScope::from_communities(["guild-a"]);

        // "art" is only in guild-c and guild-d — neither visible.
        assert!(tracker.perspective_scoped("art", &scope).is_none());
    }

    #[test]
    fn perspective_scoped_not_discussing_count() {
        let tracker = scoped_tracker();
        // Scope with guild-a, guild-b, guild-c visible.
        let scope = crate::FederationScope::from_communities(["guild-a", "guild-b", "guild-c"]);

        // "rust" is discussed by guild-a and guild-b. guild-c is visible but not discussing.
        let p = tracker.perspective_scoped("rust", &scope).unwrap();
        assert_eq!(p.communities_discussing, 2);
        assert_eq!(p.communities_not_discussing, 1); // guild-c
    }
}
