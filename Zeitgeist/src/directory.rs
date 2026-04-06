//! Tower directory — built from gospel Semantic Profiles + lighthouse announcements.
//!
//! Every node on the network knows every Tower via gospel. The directory
//! is the decoded, queryable view of that data. Zeitgeist reads this map
//! to route queries to the right Towers.
//!
//! # Data Sources
//!
//! - **Lighthouse announcements** (kind 7032) — stored in the EventStore
//!   (not the GospelRegistry). Queried via OmniFilter.
//! - **Semantic Profiles** (kind 26000) — stored in the GospelRegistry's
//!   `profiles` bucket. Queryable via `registry.lookup_profile(author)`.
//!
//! The directory takes raw `OmniEvent` slices via `update()`, which is
//! source-agnostic. Callers can feed events from EventStore queries,
//! gospel sync, or any other source.

use std::collections::HashMap;

use globe::event::OmniEvent;
use globe::kind;
use serde::{Deserialize, Serialize};

use crate::error::ZeitgeistError;

/// A Tower's search capabilities, parsed from a kind 26000 Semantic Profile.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TowerCapabilities {
    /// Whether this Tower supports keyword (FTS) search.
    pub keyword_search: bool,
    /// Whether this Tower supports semantic (vector) search.
    pub semantic_search: bool,
    /// Whether this Tower can provide concept suggestions.
    pub suggestions: bool,
    /// Topic labels this Tower is strong in (from Semantic Profile).
    #[serde(default)]
    pub topics: Vec<String>,
    /// Embedding model identifier (for vector comparability).
    #[serde(default)]
    pub embedding_model: Option<String>,
    /// Content count this Tower has indexed.
    #[serde(default)]
    pub content_count: u64,
}

impl TowerCapabilities {
    /// Parse capabilities from a Semantic Profile event's content (JSON).
    pub fn from_profile_event(event: &OmniEvent) -> Result<Self, ZeitgeistError> {
        if event.kind != kind::SEMANTIC_PROFILE {
            return Err(ZeitgeistError::InvalidProfile(format!(
                "expected kind {}, got {}",
                kind::SEMANTIC_PROFILE,
                event.kind
            )));
        }
        serde_json::from_str(&event.content).map_err(|e| {
            ZeitgeistError::InvalidProfile(format!("parse: {e}"))
        })
    }

    /// Whether this Tower can handle any searches at all.
    pub fn can_search(&self) -> bool {
        self.keyword_search || self.semantic_search
    }
}

/// A Tower's announcement data, parsed from a kind 7032 lighthouse event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerInfo {
    /// Tower operating mode ("pharos" or "harbor").
    pub mode: String,
    /// Public relay URL (WebSocket).
    pub relay_url: String,
    /// Human-readable name.
    pub name: String,
    /// Number of gospel records cached.
    pub gospel_count: u64,
    /// Number of stored events.
    pub event_count: u64,
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Software version.
    pub version: String,
    /// Community pubkeys served (Harbor only).
    #[serde(default)]
    pub communities: Vec<String>,
}

impl TowerInfo {
    /// Parse from a lighthouse announcement event's content.
    pub fn from_lighthouse_event(event: &OmniEvent) -> Result<Self, ZeitgeistError> {
        if event.kind != kind::LIGHTHOUSE_ANNOUNCE {
            return Err(ZeitgeistError::InvalidAnnouncement(format!(
                "expected kind {}, got {}",
                kind::LIGHTHOUSE_ANNOUNCE,
                event.kind
            )));
        }
        serde_json::from_str(&event.content).map_err(|e| {
            ZeitgeistError::InvalidAnnouncement(format!("parse: {e}"))
        })
    }
}

/// A complete entry in the Tower directory — announcement + capabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerEntry {
    /// The Tower's public key (author of the lighthouse event).
    pub pubkey: String,
    /// Lighthouse info (mode, relay URL, uptime, etc.).
    pub info: TowerInfo,
    /// Search capabilities (from Semantic Profile). None if no profile published.
    pub capabilities: Option<TowerCapabilities>,
    /// When the lighthouse event was last seen (created_at).
    pub last_seen: i64,
}

impl TowerEntry {
    /// Whether this Tower can handle search queries.
    pub fn can_search(&self) -> bool {
        self.capabilities.as_ref().is_some_and(|c| c.can_search())
    }

    /// Whether this Tower is a Harbor (stores content, not just gospel).
    pub fn is_harbor(&self) -> bool {
        self.info.mode == "harbor"
    }

    /// Whether this Tower serves a particular community.
    pub fn serves_community(&self, community_pubkey: &str) -> bool {
        self.info.communities.contains(&community_pubkey.to_string())
    }
}

/// The Tower directory — a decoded, queryable view of gospel data.
///
/// Built from lighthouse announcements and semantic profiles.
/// Source-agnostic: call `update()` with events from any source.
#[derive(Clone, Debug, Default)]
pub struct TowerDirectory {
    /// All known Towers, keyed by pubkey.
    entries: HashMap<String, TowerEntry>,
}

impl TowerDirectory {
    /// Create an empty directory.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the directory from a batch of events.
    ///
    /// Processes lighthouse announcements (kind 7032) and semantic profiles
    /// (kind 26000). Non-relevant events are silently ignored.
    /// Replaces existing entries if newer events arrive.
    pub fn update(&mut self, events: &[OmniEvent]) {
        for event in events {
            match event.kind {
                kind::LIGHTHOUSE_ANNOUNCE => {
                    if let Ok(info) = TowerInfo::from_lighthouse_event(event) {
                        let pubkey = &event.author;
                        match self.entries.get_mut(pubkey) {
                            Some(existing) if event.created_at > existing.last_seen => {
                                existing.info = info;
                                existing.last_seen = event.created_at;
                            }
                            Some(_) => {} // Older event, skip.
                            None => {
                                self.entries.insert(
                                    pubkey.clone(),
                                    TowerEntry {
                                        pubkey: pubkey.clone(),
                                        info,
                                        capabilities: None,
                                        last_seen: event.created_at,
                                    },
                                );
                            }
                        }
                    }
                }
                kind::SEMANTIC_PROFILE => {
                    if let Ok(caps) = TowerCapabilities::from_profile_event(event) {
                        let pubkey = &event.author;
                        match self.entries.get_mut(pubkey) {
                            Some(existing) => {
                                existing.capabilities = Some(caps);
                            }
                            None => {
                                // Profile arrived before lighthouse.
                                // Store capabilities anyway — lighthouse will fill in info later.
                                // For now, skip (a Tower needs a lighthouse to be routable).
                            }
                        }
                    }
                }
                _ => {} // Ignore other event kinds.
            }
        }
    }

    /// Build a directory from a batch of events (convenience).
    pub fn from_events(events: &[OmniEvent]) -> Self {
        let mut dir = Self::new();
        dir.update(events);
        dir
    }

    /// Look up a Tower by pubkey.
    pub fn get(&self, pubkey: &str) -> Option<&TowerEntry> {
        self.entries.get(pubkey)
    }

    /// All known Towers.
    pub fn all_towers(&self) -> Vec<&TowerEntry> {
        self.entries.values().collect()
    }

    /// All Towers that can handle search queries.
    pub fn searchable_towers(&self) -> Vec<&TowerEntry> {
        self.entries.values().filter(|t| t.can_search()).collect()
    }

    /// All Harbor Towers.
    pub fn harbors(&self) -> Vec<&TowerEntry> {
        self.entries.values().filter(|t| t.is_harbor()).collect()
    }

    /// All Towers serving a particular community.
    pub fn towers_for_community(&self, community_pubkey: &str) -> Vec<&TowerEntry> {
        self.entries
            .values()
            .filter(|t| t.serves_community(community_pubkey))
            .collect()
    }

    /// Number of known Towers.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Number of searchable Towers.
    pub fn searchable_count(&self) -> usize {
        self.entries.values().filter(|t| t.can_search()).count()
    }

    /// All Towers visible under a federation scope.
    ///
    /// When unrestricted, returns all Towers (fast-path).
    /// Otherwise, returns Towers that serve at least one visible community.
    pub fn all_towers_scoped(&self, scope: &crate::FederationScope) -> Vec<&TowerEntry> {
        if scope.is_unrestricted() {
            return self.all_towers();
        }
        self.entries
            .values()
            .filter(|t| t.info.communities.iter().any(|c| scope.is_visible(c)))
            .collect()
    }

    /// All searchable Towers visible under a federation scope.
    ///
    /// When unrestricted, returns all searchable Towers (fast-path).
    pub fn searchable_towers_scoped(&self, scope: &crate::FederationScope) -> Vec<&TowerEntry> {
        if scope.is_unrestricted() {
            return self.searchable_towers();
        }
        self.entries
            .values()
            .filter(|t| {
                t.can_search() && t.info.communities.iter().any(|c| scope.is_visible(c))
            })
            .collect()
    }

    /// All Harbors visible under a federation scope.
    ///
    /// When unrestricted, returns all Harbors (fast-path).
    pub fn harbors_scoped(&self, scope: &crate::FederationScope) -> Vec<&TowerEntry> {
        if scope.is_unrestricted() {
            return self.harbors();
        }
        self.entries
            .values()
            .filter(|t| {
                t.is_harbor() && t.info.communities.iter().any(|c| scope.is_visible(c))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lighthouse_event(
        author: &str,
        mode: &str,
        relay_url: &str,
        name: &str,
        created_at: i64,
    ) -> OmniEvent {
        let info = TowerInfo {
            mode: mode.into(),
            relay_url: relay_url.into(),
            name: name.into(),
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

    fn make_harbor_event(
        author: &str,
        relay_url: &str,
        communities: Vec<&str>,
        created_at: i64,
    ) -> OmniEvent {
        let info = TowerInfo {
            mode: "harbor".into(),
            relay_url: relay_url.into(),
            name: "Harbor".into(),
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

    fn make_profile_event(
        author: &str,
        keyword: bool,
        semantic: bool,
        topics: Vec<&str>,
        created_at: i64,
    ) -> OmniEvent {
        let caps = TowerCapabilities {
            keyword_search: keyword,
            semantic_search: semantic,
            suggestions: semantic,
            topics: topics.iter().map(|s| s.to_string()).collect(),
            embedding_model: if semantic {
                Some("test-model-v1".into())
            } else {
                None
            },
            content_count: 500,
        };
        OmniEvent {
            id: format!("sp-{author}-{created_at}"),
            author: author.into(),
            created_at,
            kind: kind::SEMANTIC_PROFILE,
            tags: vec![vec!["d".into(), author.into()]],
            content: serde_json::to_string(&caps).unwrap(),
            sig: "c".repeat(128),
        }
    }

    fn test_events() -> Vec<OmniEvent> {
        vec![
            // Tower A: Pharos with keyword search, topics woodworking + crafts
            make_lighthouse_event("tower_a", "pharos", "wss://a.tower.idea", "Woodworker's Pharos", 1000),
            make_profile_event("tower_a", true, false, vec!["woodworking", "crafts"], 1001),
            // Tower B: Harbor with semantic search, topics art + design
            make_harbor_event("tower_b", "wss://b.tower.idea", vec!["community_art"], 2000),
            make_profile_event("tower_b", true, true, vec!["art", "design", "illustration"], 2001),
            // Tower C: Pharos, no semantic profile (lighthouse only)
            make_lighthouse_event("tower_c", "pharos", "wss://c.tower.idea", "Bare Pharos", 3000),
        ]
    }

    #[test]
    fn empty_directory() {
        let dir = TowerDirectory::new();
        assert_eq!(dir.count(), 0);
        assert_eq!(dir.searchable_count(), 0);
        assert!(dir.all_towers().is_empty());
    }

    #[test]
    fn build_from_events() {
        let dir = TowerDirectory::from_events(&test_events());

        assert_eq!(dir.count(), 3);
        // Tower A and B have profiles (searchable), Tower C does not.
        assert_eq!(dir.searchable_count(), 2);
    }

    #[test]
    fn lookup_tower() {
        let dir = TowerDirectory::from_events(&test_events());

        let tower_a = dir.get("tower_a").expect("tower_a should exist");
        assert_eq!(tower_a.info.name, "Woodworker's Pharos");
        assert!(!tower_a.is_harbor());
        assert!(tower_a.can_search());
    }

    #[test]
    fn harbor_detection() {
        let dir = TowerDirectory::from_events(&test_events());

        let harbors = dir.harbors();
        assert_eq!(harbors.len(), 1);
        assert_eq!(harbors[0].info.name, "Harbor");
        assert!(harbors[0].is_harbor());
    }

    #[test]
    fn community_filter() {
        let dir = TowerDirectory::from_events(&test_events());

        let art_towers = dir.towers_for_community("community_art");
        assert_eq!(art_towers.len(), 1);
        assert_eq!(art_towers[0].pubkey, "tower_b");

        let empty = dir.towers_for_community("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn tower_without_profile_not_searchable() {
        let dir = TowerDirectory::from_events(&test_events());

        let tower_c = dir.get("tower_c").expect("tower_c should exist");
        assert!(!tower_c.can_search());
        assert!(tower_c.capabilities.is_none());
    }

    #[test]
    fn capabilities_from_profile() {
        let dir = TowerDirectory::from_events(&test_events());

        let tower_b = dir.get("tower_b").expect("tower_b should exist");
        let caps = tower_b.capabilities.as_ref().unwrap();
        assert!(caps.keyword_search);
        assert!(caps.semantic_search);
        assert!(caps.suggestions);
        assert!(caps.topics.contains(&"art".to_string()));
        assert!(caps.topics.contains(&"design".to_string()));
        assert_eq!(caps.embedding_model, Some("test-model-v1".into()));
    }

    #[test]
    fn incremental_update() {
        let mut dir = TowerDirectory::new();
        assert_eq!(dir.count(), 0);

        // Add a Tower.
        dir.update(&[make_lighthouse_event(
            "new_tower", "pharos", "wss://new.tower.idea", "New", 5000,
        )]);
        assert_eq!(dir.count(), 1);

        // Add its profile later.
        dir.update(&[make_profile_event(
            "new_tower", true, false, vec!["testing"], 5001,
        )]);
        assert_eq!(dir.count(), 1);
        assert!(dir.get("new_tower").unwrap().can_search());
    }

    #[test]
    fn newer_lighthouse_replaces_older() {
        let mut dir = TowerDirectory::new();
        dir.update(&[make_lighthouse_event(
            "tower", "pharos", "wss://old.idea", "Old Name", 1000,
        )]);
        assert_eq!(dir.get("tower").unwrap().info.name, "Old Name");

        dir.update(&[make_lighthouse_event(
            "tower", "pharos", "wss://new.idea", "New Name", 2000,
        )]);
        assert_eq!(dir.get("tower").unwrap().info.name, "New Name");
    }

    #[test]
    fn older_lighthouse_ignored() {
        let mut dir = TowerDirectory::new();
        dir.update(&[make_lighthouse_event(
            "tower", "pharos", "wss://new.idea", "New Name", 2000,
        )]);
        dir.update(&[make_lighthouse_event(
            "tower", "pharos", "wss://old.idea", "Old Name", 1000,
        )]);
        // Should keep the newer one.
        assert_eq!(dir.get("tower").unwrap().info.name, "New Name");
    }

    #[test]
    fn profile_before_lighthouse_ignored() {
        let mut dir = TowerDirectory::new();
        // Profile arrives before lighthouse — no entry to attach it to.
        dir.update(&[make_profile_event(
            "ghost", true, false, vec!["testing"], 1000,
        )]);
        // Tower not in directory (no lighthouse).
        assert!(dir.get("ghost").is_none());
    }

    #[test]
    fn non_relevant_events_ignored() {
        let mut dir = TowerDirectory::new();
        dir.update(&[OmniEvent {
            id: "irrelevant".into(),
            author: "someone".into(),
            created_at: 1000,
            kind: kind::TEXT_NOTE,
            tags: vec![],
            content: "hello world".into(),
            sig: "c".repeat(128),
        }]);
        assert_eq!(dir.count(), 0);
    }

    #[test]
    fn tower_capabilities_parse_error() {
        let bad_event = OmniEvent {
            id: "bad".into(),
            author: "bad_author".into(),
            created_at: 1000,
            kind: kind::SEMANTIC_PROFILE,
            tags: vec![],
            content: "not json".into(),
            sig: "c".repeat(128),
        };
        let result = TowerCapabilities::from_profile_event(&bad_event);
        assert!(result.is_err());
    }

    #[test]
    fn tower_capabilities_wrong_kind() {
        let event = OmniEvent {
            id: "wrong".into(),
            author: "author".into(),
            created_at: 1000,
            kind: kind::TEXT_NOTE,
            tags: vec![],
            content: "{}".into(),
            sig: "c".repeat(128),
        };
        let result = TowerCapabilities::from_profile_event(&event);
        assert!(result.is_err());
    }

    #[test]
    fn tower_info_wrong_kind() {
        let event = OmniEvent {
            id: "wrong".into(),
            author: "author".into(),
            created_at: 1000,
            kind: kind::TEXT_NOTE,
            tags: vec![],
            content: "{}".into(),
            sig: "c".repeat(128),
        };
        let result = TowerInfo::from_lighthouse_event(&event);
        assert!(result.is_err());
    }

    #[test]
    fn capabilities_can_search() {
        let keyword_only = TowerCapabilities {
            keyword_search: true,
            semantic_search: false,
            ..Default::default()
        };
        assert!(keyword_only.can_search());

        let semantic_only = TowerCapabilities {
            keyword_search: false,
            semantic_search: true,
            ..Default::default()
        };
        assert!(semantic_only.can_search());

        let none = TowerCapabilities::default();
        assert!(!none.can_search());
    }

    #[test]
    fn capabilities_serde_round_trip() {
        let caps = TowerCapabilities {
            keyword_search: true,
            semantic_search: true,
            suggestions: true,
            topics: vec!["rust".into(), "programming".into()],
            embedding_model: Some("model-v1".into()),
            content_count: 42,
        };
        let json = serde_json::to_string(&caps).unwrap();
        let loaded: TowerCapabilities = serde_json::from_str(&json).unwrap();
        assert!(loaded.keyword_search);
        assert_eq!(loaded.topics.len(), 2);
        assert_eq!(loaded.content_count, 42);
    }

    #[test]
    fn searchable_towers_filter() {
        let dir = TowerDirectory::from_events(&test_events());

        let searchable = dir.searchable_towers();
        assert_eq!(searchable.len(), 2);
        // All searchable towers should have capabilities.
        for t in &searchable {
            assert!(t.capabilities.is_some());
            assert!(t.can_search());
        }
    }

    // --- Federation scope tests ---

    fn scoped_test_events() -> Vec<OmniEvent> {
        vec![
            // Tower A: Harbor serving community_art
            make_harbor_event("tower_a", "wss://a.idea", vec!["community_art"], 1000),
            make_profile_event("tower_a", true, false, vec!["art"], 1001),
            // Tower B: Harbor serving community_tech
            make_harbor_event("tower_b", "wss://b.idea", vec!["community_tech"], 2000),
            make_profile_event("tower_b", true, true, vec!["rust", "code"], 2001),
            // Tower C: Harbor serving both communities
            make_harbor_event(
                "tower_c",
                "wss://c.idea",
                vec!["community_art", "community_tech"],
                3000,
            ),
            make_profile_event("tower_c", true, false, vec!["design"], 3001),
            // Tower D: Pharos with no communities (global)
            make_lighthouse_event("tower_d", "pharos", "wss://d.idea", "Global Pharos", 4000),
            make_profile_event("tower_d", true, false, vec!["general"], 4001),
        ]
    }

    #[test]
    fn all_towers_scoped_unrestricted() {
        let dir = TowerDirectory::from_events(&scoped_test_events());
        let scope = crate::FederationScope::new();

        let all = dir.all_towers_scoped(&scope);
        assert_eq!(all.len(), dir.count());
    }

    #[test]
    fn all_towers_scoped_filters_by_community() {
        let dir = TowerDirectory::from_events(&scoped_test_events());
        let scope = crate::FederationScope::from_communities(["community_art"]);

        let scoped = dir.all_towers_scoped(&scope);
        // Tower A (community_art), Tower C (both) — Tower B (community_tech) and Tower D (no communities) excluded.
        assert_eq!(scoped.len(), 2);
        let pubkeys: Vec<&str> = scoped.iter().map(|t| t.pubkey.as_str()).collect();
        assert!(pubkeys.contains(&"tower_a"));
        assert!(pubkeys.contains(&"tower_c"));
        assert!(!pubkeys.contains(&"tower_b"));
        assert!(!pubkeys.contains(&"tower_d"));
    }

    #[test]
    fn searchable_towers_scoped_filters() {
        let dir = TowerDirectory::from_events(&scoped_test_events());
        let scope = crate::FederationScope::from_communities(["community_tech"]);

        let searchable = dir.searchable_towers_scoped(&scope);
        // Tower B (community_tech) and Tower C (both) — Tower A and D excluded.
        assert_eq!(searchable.len(), 2);
        for t in &searchable {
            assert!(t.can_search());
        }
    }

    #[test]
    fn harbors_scoped_filters() {
        let dir = TowerDirectory::from_events(&scoped_test_events());
        let scope = crate::FederationScope::from_communities(["community_art"]);

        let harbors = dir.harbors_scoped(&scope);
        // Tower A and C are harbors with community_art. Tower D is pharos.
        assert_eq!(harbors.len(), 2);
        for t in &harbors {
            assert!(t.is_harbor());
        }
    }

    #[test]
    fn scoped_with_no_matching_communities_returns_empty() {
        let dir = TowerDirectory::from_events(&scoped_test_events());
        let scope = crate::FederationScope::from_communities(["nonexistent"]);

        assert!(dir.all_towers_scoped(&scope).is_empty());
        assert!(dir.searchable_towers_scoped(&scope).is_empty());
        assert!(dir.harbors_scoped(&scope).is_empty());
    }
}
