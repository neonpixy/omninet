use std::collections::HashSet;

use crate::event::OmniEvent;
use crate::filter::OmniFilter;

use super::config::GospelConfig;
use super::registry::{GospelRegistry, InsertResult};
use super::tier::{GospelTier, kinds_for_tiers};

/// Helpers for bilateral sync between gospel registries.
///
/// GospelSync provides the building blocks for evangelizing registry
/// records between peers. The actual transport (WebSocket, etc.) is
/// handled by [`GospelPeer`](super::peer::GospelPeer).
pub struct GospelSync;

/// Statistics from a merge operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeStats {
    /// Events that were new and accepted.
    pub inserted: usize,
    /// Events that were older or conflicting and rejected.
    pub rejected: usize,
    /// Events that were already present (same event ID).
    pub duplicates: usize,
}

impl GospelSync {
    /// Build an `OmniFilter` to request all gospel registry records
    /// since a given timestamp. Used when initiating sync with a peer.
    pub fn sync_filter(since: Option<i64>) -> OmniFilter {
        OmniFilter::for_gospel(since)
    }

    /// Build a tier-aware sync filter. Only includes kinds for the given tiers.
    ///
    /// A Pharos passing `[Universal]` will only request names, relay hints,
    /// and lighthouse announcements. A Harbor passing `[Universal, Community]`
    /// also gets beacons and asset announcements.
    pub fn sync_filter_for_tiers(since: Option<i64>, tiers: &[GospelTier]) -> OmniFilter {
        let kinds = kinds_for_tiers(tiers);
        if kinds.is_empty() {
            return OmniFilter::for_gospel(since);
        }
        OmniFilter {
            kinds: Some(kinds),
            since,
            ..Default::default()
        }
    }

    /// Merge a batch of incoming events into a registry.
    ///
    /// Uses the current wall-clock time as the relay-local received timestamp
    /// for anti-squatting conflict resolution.
    ///
    /// Returns statistics on how many were inserted, rejected, or duplicated.
    pub fn merge_events(registry: &GospelRegistry, events: &[OmniEvent]) -> MergeStats {
        let received_at = chrono::Utc::now().timestamp();
        let mut inserted = 0;
        let mut rejected = 0;
        let mut duplicates = 0;

        for event in events {
            match registry.insert_with_received_at(event, received_at) {
                InsertResult::Inserted => inserted += 1,
                InsertResult::Rejected => rejected += 1,
                InsertResult::Duplicate => duplicates += 1,
            }
        }

        MergeStats {
            inserted,
            rejected,
            duplicates,
        }
    }

    /// Compute events that `local_events` has but `remote_event_ids` doesn't.
    ///
    /// Used to determine what to send to a peer after receiving their events.
    pub fn diff(local_events: &[OmniEvent], remote_event_ids: &[String]) -> Vec<OmniEvent> {
        let remote_set: HashSet<&String> = remote_event_ids.iter().collect();
        local_events
            .iter()
            .filter(|e| !remote_set.contains(&e.id))
            .cloned()
            .collect()
    }

    /// Given two sets of events, produce a correctly resolved merged registry.
    ///
    /// Applies gospel conflict resolution: first to arrive for names
    /// (different authors), latest for hints.
    pub fn merge_sets(
        set_a: &[OmniEvent],
        set_b: &[OmniEvent],
        config: &GospelConfig,
    ) -> GospelRegistry {
        let received_at = chrono::Utc::now().timestamp();
        let registry = GospelRegistry::new(config);
        for event in set_a.iter().chain(set_b.iter()) {
            registry.insert_with_received_at(event, received_at);
        }
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tier::GospelTier;
    use crate::kind;

    fn make_name_event(name: &str, author: &str, created_at: i64) -> OmniEvent {
        OmniEvent {
            id: format!("{name}-{author}-{created_at}"),
            author: author.to_string(),
            created_at,
            kind: kind::NAME_CLAIM,
            tags: vec![vec!["d".into(), name.into()]],
            content: String::new(),
            sig: "c".repeat(128),
        }
    }

    fn make_hint_event(author: &str, created_at: i64) -> OmniEvent {
        OmniEvent {
            id: format!("hint-{author}-{created_at}"),
            author: author.to_string(),
            created_at,
            kind: kind::RELAY_HINT,
            tags: vec![vec!["d".into(), "relay-hints".into()]],
            content: r#"{"relays":["wss://relay.test.com"]}"#.into(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn sync_filter_with_since() {
        let filter = GospelSync::sync_filter(Some(1000));
        assert_eq!(filter.since, Some(1000));
        let kinds = filter.kinds.unwrap();
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::RELAY_HINT));
    }

    #[test]
    fn sync_filter_without_since() {
        let filter = GospelSync::sync_filter(None);
        assert!(filter.since.is_none());
    }

    #[test]
    fn merge_events_counts() {
        let config = GospelConfig::default();
        let registry = GospelRegistry::new(&config);

        let events = vec![
            make_name_event("a.com", "alice", 1000),
            make_name_event("b.com", "bob", 2000),
        ];

        let stats = GospelSync::merge_events(&registry, &events);
        assert_eq!(stats.inserted, 2);
        assert_eq!(stats.rejected, 0);
        assert_eq!(stats.duplicates, 0);

        // Inserting again → duplicates.
        let stats2 = GospelSync::merge_events(&registry, &events);
        assert_eq!(stats2.inserted, 0);
        assert_eq!(stats2.duplicates, 2);
    }

    #[test]
    fn diff_finds_missing() {
        let local = vec![
            make_name_event("a.com", "alice", 1000),
            make_name_event("b.com", "bob", 2000),
            make_hint_event("carol", 3000),
        ];
        let remote_ids = vec![local[0].id.clone()]; // Remote has only "a.com".

        let missing = GospelSync::diff(&local, &remote_ids);
        assert_eq!(missing.len(), 2);
        assert!(missing.iter().any(|e| e.id == local[1].id));
        assert!(missing.iter().any(|e| e.id == local[2].id));
    }

    #[test]
    fn diff_empty_when_all_present() {
        let local = vec![make_name_event("a.com", "alice", 1000)];
        let remote_ids = vec![local[0].id.clone()];

        let missing = GospelSync::diff(&local, &remote_ids);
        assert!(missing.is_empty());
    }

    #[test]
    fn merge_sets_resolves_conflicts() {
        let config = GospelConfig::default();

        // Alice claims sam.com at t=1000, Bob claims it at t=2000.
        let set_a = vec![make_name_event("sam.com", "alice", 1000)];
        let set_b = vec![make_name_event("sam.com", "bob", 2000)];

        let merged = GospelSync::merge_sets(&set_a, &set_b, &config);
        assert_eq!(merged.name_count(), 1);

        // Alice wins (first-claim).
        let winner = merged.lookup_name("sam.com").unwrap();
        assert_eq!(winner.author, "alice");
    }

    #[test]
    fn merge_sets_combines_different_records() {
        let config = GospelConfig::default();

        let set_a = vec![make_name_event("a.com", "alice", 1000)];
        let set_b = vec![
            make_name_event("b.com", "bob", 2000),
            make_hint_event("carol", 3000),
        ];

        let merged = GospelSync::merge_sets(&set_a, &set_b, &config);
        assert_eq!(merged.name_count(), 2);
        assert_eq!(merged.hint_count(), 1);
    }

    #[test]
    fn tier_filter_universal_only() {
        let filter = GospelSync::sync_filter_for_tiers(Some(500), &[GospelTier::Universal]);
        let kinds = filter.kinds.unwrap();
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::RELAY_HINT));
        assert!(kinds.contains(&kind::LIGHTHOUSE_ANNOUNCE));
        assert!(!kinds.contains(&kind::BEACON));
        assert!(!kinds.contains(&kind::ASSET_ANNOUNCE));
        assert_eq!(filter.since, Some(500));
    }

    #[test]
    fn tier_filter_universal_and_community() {
        let filter = GospelSync::sync_filter_for_tiers(
            None,
            &[GospelTier::Universal, GospelTier::Community],
        );
        let kinds = filter.kinds.unwrap();
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::BEACON));
        assert!(kinds.contains(&kind::ASSET_ANNOUNCE));
    }

    #[test]
    fn tier_filter_empty_falls_back_to_all_gospel() {
        let filter = GospelSync::sync_filter_for_tiers(None, &[]);
        let kinds = filter.kinds.unwrap();
        // Falls back to full gospel filter
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::BEACON));
    }
}
