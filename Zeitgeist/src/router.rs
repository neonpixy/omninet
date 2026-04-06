//! Query routing — smart Tower selection for each search.
//!
//! Reads the Tower directory and picks the N most relevant Towers
//! for a given query. Uses topic matching (keyword overlap between
//! query terms and Tower topics). When semantic search is available,
//! vector similarity can be used for finer routing.

use serde::{Deserialize, Serialize};

use crate::directory::{TowerDirectory, TowerEntry};
use crate::error::ZeitgeistError;

/// Configuration for query routing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Maximum Towers to query per search (default: 5).
    pub max_towers: usize,
    /// Minimum topic score to consider a Tower relevant (default: 0.0).
    /// Towers below this threshold are excluded even if we have capacity.
    pub min_relevance: f64,
    /// Whether to prefer Harbors (which store content) over Pharos nodes.
    pub prefer_harbors: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            max_towers: 5,
            min_relevance: 0.0,
            prefer_harbors: true,
        }
    }
}

/// A Tower selected for a query, with its relevance score.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutedTower {
    /// The Tower's public key.
    pub pubkey: String,
    /// The Tower's relay URL (for direct connection).
    pub relay_url: String,
    /// Relevance score for this query (0.0 = fallback, higher = better match).
    pub relevance: f64,
    /// Whether this Tower supports semantic search.
    pub has_semantic: bool,
}

/// Routes search queries to the most relevant Towers.
pub struct QueryRouter {
    config: RouterConfig,
}

impl QueryRouter {
    /// Create a router with default configuration.
    pub fn new() -> Self {
        Self {
            config: RouterConfig::default(),
        }
    }

    /// Create a router with custom configuration.
    pub fn with_config(config: RouterConfig) -> Self {
        Self { config }
    }

    /// Select the best Towers for a text query.
    ///
    /// Scoring is based on topic overlap: how many of the query's words
    /// match the Tower's topic labels. Harbors get a bonus when preferred.
    /// Results are sorted by score (highest first) and capped at `max_towers`.
    pub fn route(
        &self,
        query: &str,
        directory: &TowerDirectory,
    ) -> Result<Vec<RoutedTower>, ZeitgeistError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(ZeitgeistError::EmptyQuery);
        }

        let searchable = directory.searchable_towers();
        if searchable.is_empty() {
            return Err(ZeitgeistError::NoTowersAvailable);
        }

        let query_terms: Vec<&str> = query
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();

        let mut scored: Vec<RoutedTower> = searchable
            .iter()
            .map(|tower| {
                let score = self.score_tower(tower, &query_terms);
                RoutedTower {
                    pubkey: tower.pubkey.clone(),
                    relay_url: tower.info.relay_url.clone(),
                    relevance: score,
                    has_semantic: tower
                        .capabilities
                        .as_ref()
                        .is_some_and(|c| c.semantic_search),
                }
            })
            .filter(|r| r.relevance >= self.config.min_relevance)
            .collect();

        // Sort by relevance (highest first). Ties broken by pubkey for stability.
        scored.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.pubkey.cmp(&b.pubkey))
        });

        // Cap at max_towers.
        scored.truncate(self.config.max_towers);

        Ok(scored)
    }

    /// Select the best Towers for a text query, filtered by federation scope.
    ///
    /// When the scope is unrestricted, delegates to `route()` (fast-path).
    /// Otherwise, only considers Towers that serve at least one community
    /// visible under the scope.
    pub fn route_scoped(
        &self,
        query: &str,
        directory: &TowerDirectory,
        scope: &crate::FederationScope,
    ) -> Result<Vec<RoutedTower>, ZeitgeistError> {
        if scope.is_unrestricted() {
            return self.route(query, directory);
        }

        let query = query.trim();
        if query.is_empty() {
            return Err(ZeitgeistError::EmptyQuery);
        }

        let searchable = directory.searchable_towers_scoped(scope);
        if searchable.is_empty() {
            return Err(ZeitgeistError::NoTowersAvailable);
        }

        let query_terms: Vec<&str> = query
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty())
            .collect();

        let mut scored: Vec<RoutedTower> = searchable
            .iter()
            .map(|tower| {
                let score = self.score_tower(tower, &query_terms);
                RoutedTower {
                    pubkey: tower.pubkey.clone(),
                    relay_url: tower.info.relay_url.clone(),
                    relevance: score,
                    has_semantic: tower
                        .capabilities
                        .as_ref()
                        .is_some_and(|c| c.semantic_search),
                }
            })
            .filter(|r| r.relevance >= self.config.min_relevance)
            .collect();

        scored.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.pubkey.cmp(&b.pubkey))
        });

        scored.truncate(self.config.max_towers);
        Ok(scored)
    }

    /// Score a Tower for a set of query terms.
    ///
    /// Components:
    /// - Topic match: fraction of query terms that match Tower topics (0.0-1.0)
    /// - Harbor bonus: +0.1 if prefer_harbors and Tower is a Harbor
    /// - Content bonus: scaled by log of content count (more content = more likely to have results)
    /// - Semantic bonus: +0.05 if Tower has semantic search (better result quality)
    /// - Fallback: Towers with no topic match still get a base score so they're
    ///   included if we don't have enough specialized Towers.
    fn score_tower(&self, tower: &TowerEntry, query_terms: &[&str]) -> f64 {
        let caps = match &tower.capabilities {
            Some(c) => c,
            None => return 0.0,
        };

        let mut score = 0.0;

        // Topic match: case-insensitive substring match.
        if !query_terms.is_empty() && !caps.topics.is_empty() {
            let matches = query_terms
                .iter()
                .filter(|term| {
                    let lower = term.to_lowercase();
                    caps.topics.iter().any(|topic| {
                        topic.to_lowercase().contains(&lower)
                            || lower.contains(&topic.to_lowercase())
                    })
                })
                .count();
            score += matches as f64 / query_terms.len() as f64;
        }

        // Harbor bonus.
        if self.config.prefer_harbors && tower.is_harbor() {
            score += 0.1;
        }

        // Content bonus: log scale, capped at 0.15.
        if caps.content_count > 0 {
            let content_bonus = (caps.content_count as f64).ln() / 100.0;
            score += content_bonus.min(0.15);
        }

        // Semantic search bonus.
        if caps.semantic_search {
            score += 0.05;
        }

        // Base score: all searchable Towers get at least 0.01 so they're
        // included as fallbacks when no Towers match the topic.
        if score == 0.0 && caps.can_search() {
            score = 0.01;
        }

        score
    }
}

impl Default for QueryRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directory::{TowerCapabilities, TowerInfo};
    use globe::event::OmniEvent;
    use globe::kind;

    fn make_lighthouse_event(
        author: &str,
        mode: &str,
        relay_url: &str,
        created_at: i64,
    ) -> OmniEvent {
        let info = TowerInfo {
            mode: mode.into(),
            relay_url: relay_url.into(),
            name: format!("Tower {author}"),
            gospel_count: 10,
            event_count: 100,
            uptime_secs: 3600,
            version: "0.1.0".into(),
            communities: vec![],
        };
        OmniEvent {
            id: format!("lh-{author}-{created_at}"),
            author: author.into(),
            created_at,
            kind: kind::LIGHTHOUSE_ANNOUNCE,
            tags: vec![
                vec!["d".into(), author.into()],
                vec!["mode".into(), mode.into()],
                vec!["r".into(), relay_url.into()],
            ],
            content: serde_json::to_string(&info).unwrap(),
            sig: "c".repeat(128),
        }
    }

    fn make_profile_event(
        author: &str,
        caps: &TowerCapabilities,
        created_at: i64,
    ) -> OmniEvent {
        OmniEvent {
            id: format!("sp-{author}-{created_at}"),
            author: author.into(),
            created_at,
            kind: kind::SEMANTIC_PROFILE,
            tags: vec![vec!["d".into(), author.into()]],
            content: serde_json::to_string(caps).unwrap(),
            sig: "c".repeat(128),
        }
    }

    fn test_directory() -> TowerDirectory {
        let events = vec![
            // Tower A: woodworking specialist (keyword only)
            make_lighthouse_event("tower_a", "pharos", "wss://a.idea", 1000),
            make_profile_event("tower_a", &TowerCapabilities {
                keyword_search: true,
                topics: vec!["woodworking".into(), "crafts".into(), "carpentry".into()],
                content_count: 1000,
                ..Default::default()
            }, 1001),

            // Tower B: art + design (semantic)
            make_lighthouse_event("tower_b", "harbor", "wss://b.idea", 2000),
            make_profile_event("tower_b", &TowerCapabilities {
                keyword_search: true,
                semantic_search: true,
                suggestions: true,
                topics: vec!["art".into(), "design".into(), "illustration".into()],
                content_count: 5000,
                ..Default::default()
            }, 2001),

            // Tower C: general purpose (keyword only, no topics)
            make_lighthouse_event("tower_c", "pharos", "wss://c.idea", 3000),
            make_profile_event("tower_c", &TowerCapabilities {
                keyword_search: true,
                content_count: 200,
                ..Default::default()
            }, 3001),

            // Tower D: programming specialist (semantic)
            make_lighthouse_event("tower_d", "harbor", "wss://d.idea", 4000),
            make_profile_event("tower_d", &TowerCapabilities {
                keyword_search: true,
                semantic_search: true,
                topics: vec!["rust".into(), "programming".into(), "code".into()],
                content_count: 3000,
                ..Default::default()
            }, 4001),
        ];

        TowerDirectory::from_events(&events)
    }

    #[test]
    fn route_woodworking_query() {
        let dir = test_directory();
        let router = QueryRouter::new();

        let results = router.route("woodworking joints", &dir).unwrap();

        // Tower A should be first (topic match: "woodworking").
        assert!(!results.is_empty());
        assert_eq!(results[0].pubkey, "tower_a");
        assert!(results[0].relevance > results.last().unwrap().relevance);
    }

    #[test]
    fn route_art_query() {
        let dir = test_directory();
        let router = QueryRouter::new();

        let results = router.route("digital art", &dir).unwrap();

        // Tower B should be first (topic match: "art", harbor bonus, semantic bonus).
        assert!(!results.is_empty());
        assert_eq!(results[0].pubkey, "tower_b");
        assert!(results[0].has_semantic);
    }

    #[test]
    fn route_respects_max_towers() {
        let dir = test_directory();
        let router = QueryRouter::with_config(RouterConfig {
            max_towers: 2,
            ..Default::default()
        });

        let results = router.route("something", &dir).unwrap();
        assert!(results.len() <= 2);
    }

    #[test]
    fn route_empty_query_errors() {
        let dir = test_directory();
        let router = QueryRouter::new();

        let result = router.route("", &dir);
        assert!(result.is_err());
    }

    #[test]
    fn route_whitespace_query_errors() {
        let dir = test_directory();
        let router = QueryRouter::new();

        let result = router.route("   ", &dir);
        assert!(result.is_err());
    }

    #[test]
    fn route_no_towers_errors() {
        let dir = TowerDirectory::new();
        let router = QueryRouter::new();

        let result = router.route("test", &dir);
        assert!(result.is_err());
    }

    #[test]
    fn route_generic_query_includes_all_searchable() {
        let dir = test_directory();
        let router = QueryRouter::new();

        // "something" doesn't match any topics — all Towers get fallback scores.
        let results = router.route("something random", &dir).unwrap();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn harbor_preferred_over_pharos() {
        let dir = test_directory();
        let router = QueryRouter::with_config(RouterConfig {
            prefer_harbors: true,
            ..Default::default()
        });

        // For a generic query with no topic match, harbors should score higher.
        let results = router.route("generic query", &dir).unwrap();
        let harbor_positions: Vec<usize> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.pubkey == "tower_b" || r.pubkey == "tower_d")
            .map(|(i, _)| i)
            .collect();
        // At least one harbor should be in the top 2.
        assert!(harbor_positions.iter().any(|&pos| pos < 2));
    }

    #[test]
    fn min_relevance_filters() {
        let dir = test_directory();
        let router = QueryRouter::with_config(RouterConfig {
            min_relevance: 0.5,
            ..Default::default()
        });

        // High min relevance — only Towers with strong topic match should pass.
        let results = router.route("woodworking", &dir).unwrap();
        // Only tower_a should match at 0.5+ (full topic match for "woodworking").
        for r in &results {
            assert!(r.relevance >= 0.5, "relevance {} below min", r.relevance);
        }
    }

    #[test]
    fn routed_tower_has_relay_url() {
        let dir = test_directory();
        let router = QueryRouter::new();

        let results = router.route("art", &dir).unwrap();
        let art_tower = results.iter().find(|r| r.pubkey == "tower_b").unwrap();
        assert_eq!(art_tower.relay_url, "wss://b.idea");
    }

    #[test]
    fn programming_query_routes_to_code_tower() {
        let dir = test_directory();
        let router = QueryRouter::new();

        let results = router.route("rust programming", &dir).unwrap();
        // Tower D should be first (matches both "rust" and "programming").
        assert_eq!(results[0].pubkey, "tower_d");
    }

    #[test]
    fn default_router_config() {
        let config = RouterConfig::default();
        assert_eq!(config.max_towers, 5);
        assert_eq!(config.min_relevance, 0.0);
        assert!(config.prefer_harbors);
    }

    // --- Federation scope tests ---

    fn make_harbor_event(
        author: &str,
        relay_url: &str,
        communities: Vec<&str>,
        created_at: i64,
    ) -> OmniEvent {
        let info = TowerInfo {
            mode: "harbor".into(),
            relay_url: relay_url.into(),
            name: format!("Harbor {author}"),
            gospel_count: 10,
            event_count: 5000,
            uptime_secs: 86400,
            version: "0.1.0".into(),
            communities: communities.iter().map(|s| s.to_string()).collect(),
        };
        OmniEvent {
            id: format!("lh-harbor-{author}-{created_at}"),
            author: author.into(),
            created_at,
            kind: kind::LIGHTHOUSE_ANNOUNCE,
            tags: vec![
                vec!["d".into(), author.into()],
                vec!["mode".into(), "harbor".into()],
                vec!["r".into(), relay_url.into()],
            ],
            content: serde_json::to_string(&info).unwrap(),
            sig: "c".repeat(128),
        }
    }

    fn scoped_directory() -> TowerDirectory {
        let events = vec![
            // Tower A: Harbor, community_art, topics = art
            make_harbor_event("tower_a", "wss://a.idea", vec!["community_art"], 1000),
            make_profile_event(
                "tower_a",
                &TowerCapabilities {
                    keyword_search: true,
                    topics: vec!["art".into(), "illustration".into()],
                    content_count: 2000,
                    ..Default::default()
                },
                1001,
            ),
            // Tower B: Harbor, community_tech, topics = rust
            make_harbor_event("tower_b", "wss://b.idea", vec!["community_tech"], 2000),
            make_profile_event(
                "tower_b",
                &TowerCapabilities {
                    keyword_search: true,
                    semantic_search: true,
                    topics: vec!["rust".into(), "code".into()],
                    content_count: 3000,
                    ..Default::default()
                },
                2001,
            ),
            // Tower C: Pharos, no community (global), topics = general
            make_lighthouse_event("tower_c", "pharos", "wss://c.idea", 3000),
            make_profile_event(
                "tower_c",
                &TowerCapabilities {
                    keyword_search: true,
                    topics: vec!["general".into()],
                    content_count: 500,
                    ..Default::default()
                },
                3001,
            ),
        ];
        TowerDirectory::from_events(&events)
    }

    #[test]
    fn route_scoped_unrestricted_matches_route() {
        let dir = scoped_directory();
        let router = QueryRouter::new();
        let scope = crate::FederationScope::new();

        let unscoped = router.route("art", &dir).unwrap();
        let scoped = router.route_scoped("art", &dir, &scope).unwrap();

        assert_eq!(unscoped.len(), scoped.len());
        for (u, s) in unscoped.iter().zip(scoped.iter()) {
            assert_eq!(u.pubkey, s.pubkey);
        }
    }

    #[test]
    fn route_scoped_filters_by_community() {
        let dir = scoped_directory();
        let router = QueryRouter::new();
        let scope = crate::FederationScope::from_communities(["community_art"]);

        let results = router.route_scoped("art", &dir, &scope).unwrap();
        // Only tower_a serves community_art and is searchable.
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].pubkey, "tower_a");
    }

    #[test]
    fn route_scoped_no_matching_towers_errors() {
        let dir = scoped_directory();
        let router = QueryRouter::new();
        let scope = crate::FederationScope::from_communities(["nonexistent"]);

        let result = router.route_scoped("anything", &dir, &scope);
        assert!(result.is_err());
    }

    #[test]
    fn route_scoped_empty_query_errors() {
        let dir = scoped_directory();
        let router = QueryRouter::new();
        let scope = crate::FederationScope::from_communities(["community_art"]);

        let result = router.route_scoped("", &dir, &scope);
        assert!(result.is_err());
    }
}
