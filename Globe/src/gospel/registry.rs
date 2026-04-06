use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::event::OmniEvent;
use crate::kind;
use crate::server::database::RelayDatabase;

use super::config::GospelConfig;
use super::tier::{GospelTier, gospel_tier};

/// Result of attempting to insert a record into the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertResult {
    /// Record was inserted (new or replaced older).
    Inserted,
    /// Record was rejected (older than existing, or wrong kind).
    Rejected,
    /// Record was a duplicate (same event ID already present).
    Duplicate,
}

/// The local cache of gospel registry records.
///
/// Thread-safe via `Arc<RwLock<...>>`. Stores the winning event for each
/// registry key. Conflict resolution:
///
/// - **Names (same d-tag, same author):** latest `created_at` wins (update).
/// - **Names (same d-tag, different authors):** earliest `created_at` wins (first-claim).
/// - **Hints (keyed by author):** latest `created_at` wins.
#[derive(Clone)]
pub struct GospelRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

struct RegistryInner {
    /// Name records: d-tag (e.g., "sam.com") → winning OmniEvent.
    names: HashMap<String, OmniEvent>,
    /// Relay hint records: author pubkey → latest OmniEvent.
    hints: HashMap<String, OmniEvent>,
    /// Asset announcements: d-tag (SHA-256 hash) → latest OmniEvent.
    assets: HashMap<String, OmniEvent>,
    /// Semantic profiles: author pubkey → latest OmniEvent.
    profiles: HashMap<String, OmniEvent>,
    /// Relay-local timestamp when the winning name event was received.
    name_received_at: HashMap<String, i64>,
    /// When each name expires (received_at + TTL).
    name_expires_at: HashMap<String, i64>,
    /// Timestamp of the most recent event processed (sync cursor).
    high_water_mark: i64,
    /// Capacity limits.
    max_name_records: usize,
    max_hint_records: usize,
    /// Name TTL from config (seconds).
    name_ttl: i64,
}

/// Serializable snapshot of the registry (for JSON persistence).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    /// All name events.
    pub names: Vec<OmniEvent>,
    /// All relay hint events.
    pub hints: Vec<OmniEvent>,
    /// All asset announcement events.
    #[serde(default)]
    pub assets: Vec<OmniEvent>,
    /// All semantic profile events.
    #[serde(default)]
    pub profiles: Vec<OmniEvent>,
    /// The high water mark at snapshot time.
    pub high_water_mark: i64,
    /// Relay-local received timestamps for name events.
    #[serde(default)]
    pub name_received_at: HashMap<String, i64>,
    /// Expiration timestamps for name registrations.
    #[serde(default)]
    pub name_expires_at: HashMap<String, i64>,
}

impl GospelRegistry {
    /// Create a new empty registry with the given configuration.
    pub fn new(config: &GospelConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                names: HashMap::new(),
                hints: HashMap::new(),
                assets: HashMap::new(),
                profiles: HashMap::new(),
                name_received_at: HashMap::new(),
                name_expires_at: HashMap::new(),
                high_water_mark: 0,
                max_name_records: config.max_name_records,
                max_hint_records: config.max_hint_records,
                name_ttl: config.name_policy.name_ttl_secs,
            })),
        }
    }

    /// Insert a registry event. Applies conflict resolution.
    ///
    /// Only accepts gospel registry kinds (names + relay hints).
    /// Returns whether the event was accepted, rejected, or duplicate.
    /// Uses the current wall-clock time as the relay-local received timestamp.
    pub fn insert(&self, event: &OmniEvent) -> InsertResult {
        self.insert_with_received_at(event, chrono::Utc::now().timestamp())
    }

    /// Insert a registry event with an explicit relay-local received timestamp.
    ///
    /// This is the core insertion method. `received_at` is used for name
    /// conflict resolution (first to reach the relay wins) and expiration
    /// tracking. Use [`insert`] for the common case where you want "now".
    pub fn insert_with_received_at(&self, event: &OmniEvent, received_at: i64) -> InsertResult {
        if !kind::is_gospel_registry(event.kind) {
            return InsertResult::Rejected;
        }

        let mut inner = self.inner.write().expect("gospel registry write lock poisoned");

        // Advance high water mark.
        if event.created_at > inner.high_water_mark {
            inner.high_water_mark = event.created_at;
        }

        match event.kind {
            kind::NAME_CLAIM
            | kind::NAME_UPDATE
            | kind::NAME_TRANSFER
            | kind::NAME_DELEGATE
            | kind::NAME_REVOKE => Self::insert_name(&mut inner, event, received_at),
            kind::NAME_RENEWAL => Self::insert_renewal(&mut inner, event, received_at),
            kind::RELAY_HINT => Self::insert_hint(&mut inner, event),
            kind::ASSET_ANNOUNCE => Self::insert_asset(&mut inner, event),
            kind::SEMANTIC_PROFILE => Self::insert_profile(&mut inner, event),
            _ => InsertResult::Rejected,
        }
    }

    /// Look up a name record by domain name string.
    pub fn lookup_name(&self, name: &str) -> Option<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        inner.names.get(name).cloned()
    }

    /// Look up relay hints by author public key hex.
    pub fn lookup_hints(&self, author: &str) -> Option<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        inner.hints.get(author).cloned()
    }

    /// Look up an asset announcement by SHA-256 hash.
    pub fn lookup_asset(&self, hash: &str) -> Option<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        inner.assets.get(hash).cloned()
    }

    /// Look up a semantic profile by author public key hex.
    pub fn lookup_profile(&self, author: &str) -> Option<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        inner.profiles.get(author).cloned()
    }

    /// Get all events created after a given timestamp (for sync).
    pub fn events_since(&self, since: i64) -> Vec<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        let mut events = Vec::new();

        for event in inner.names.values() {
            if event.created_at > since {
                events.push(event.clone());
            }
        }
        for event in inner.hints.values() {
            if event.created_at > since {
                events.push(event.clone());
            }
        }
        for event in inner.assets.values() {
            if event.created_at > since {
                events.push(event.clone());
            }
        }
        for event in inner.profiles.values() {
            if event.created_at > since {
                events.push(event.clone());
            }
        }

        events.sort_by_key(|e| e.created_at);
        events
    }

    /// Get events since a timestamp, filtered by gospel tier.
    ///
    /// Only returns events whose kind belongs to one of the given tiers.
    /// Used for tier-aware propagation (e.g., Pharos only sends Universal).
    pub fn events_since_for_tiers(
        &self,
        since: i64,
        tiers: &[GospelTier],
    ) -> Vec<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        let mut events = Vec::new();

        let all_values = inner
            .names
            .values()
            .chain(inner.hints.values())
            .chain(inner.assets.values())
            .chain(inner.profiles.values());

        for event in all_values {
            if event.created_at > since && tiers.contains(&gospel_tier(event.kind)) {
                events.push(event.clone());
            }
        }

        events.sort_by_key(|e| e.created_at);
        events
    }

    /// Get all events in the registry.
    pub fn all_events(&self) -> Vec<OmniEvent> {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        let mut events: Vec<OmniEvent> = inner
            .names
            .values()
            .chain(inner.hints.values())
            .chain(inner.assets.values())
            .chain(inner.profiles.values())
            .cloned()
            .collect();
        events.sort_by_key(|e| e.created_at);
        events
    }

    /// Number of name records.
    pub fn name_count(&self) -> usize {
        self.inner.read().expect("gospel registry read lock poisoned").names.len()
    }

    /// Number of hint records.
    pub fn hint_count(&self) -> usize {
        self.inner.read().expect("gospel registry read lock poisoned").hints.len()
    }

    /// Number of asset announcement records.
    pub fn asset_count(&self) -> usize {
        self.inner.read().expect("gospel registry read lock poisoned").assets.len()
    }

    /// Number of semantic profile records.
    pub fn profile_count(&self) -> usize {
        self.inner.read().expect("gospel registry read lock poisoned").profiles.len()
    }

    /// Total records in the registry.
    pub fn total_count(&self) -> usize {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        inner.names.len() + inner.hints.len() + inner.assets.len() + inner.profiles.len()
    }

    /// The high water mark (latest event timestamp processed).
    pub fn high_water_mark(&self) -> i64 {
        self.inner.read().expect("gospel registry read lock poisoned").high_water_mark
    }

    /// Remove all expired name registrations.
    ///
    /// Iterates the expiration map, collects names where `now > expires_at`,
    /// and removes them from `names`, `name_received_at`, and `name_expires_at`.
    /// Returns the number of purged names.
    pub fn purge_expired(&self) -> usize {
        let now = chrono::Utc::now().timestamp();
        let mut inner = self.inner.write().expect("gospel registry write lock poisoned");

        let expired: Vec<String> = inner
            .name_expires_at
            .iter()
            .filter(|(_, exp)| now > **exp)
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired.len();
        for d_tag in &expired {
            inner.names.remove(d_tag);
            inner.name_received_at.remove(d_tag);
            inner.name_expires_at.remove(d_tag);
        }

        count
    }

    /// Create a serializable snapshot.
    pub fn snapshot(&self) -> RegistrySnapshot {
        let inner = self.inner.read().expect("gospel registry read lock poisoned");
        RegistrySnapshot {
            names: inner.names.values().cloned().collect(),
            hints: inner.hints.values().cloned().collect(),
            assets: inner.assets.values().cloned().collect(),
            profiles: inner.profiles.values().cloned().collect(),
            high_water_mark: inner.high_water_mark,
            name_received_at: inner.name_received_at.clone(),
            name_expires_at: inner.name_expires_at.clone(),
        }
    }

    /// Restore from a snapshot.
    pub fn restore(snapshot: &RegistrySnapshot, config: &GospelConfig) -> Self {
        let registry = Self::new(config);
        for event in &snapshot.names {
            registry.insert(event);
        }
        for event in &snapshot.hints {
            registry.insert(event);
        }
        for event in &snapshot.assets {
            registry.insert(event);
        }
        for event in &snapshot.profiles {
            registry.insert(event);
        }
        // Restore high water mark and expiration maps.
        {
            let mut inner = registry.inner.write().expect("gospel registry write lock poisoned");
            if snapshot.high_water_mark > inner.high_water_mark {
                inner.high_water_mark = snapshot.high_water_mark;
            }
            // Overlay persisted received_at/expires_at maps. These are
            // authoritative — insert() above sets them from "now" but
            // the snapshot values are the real relay-local times.
            for (k, v) in &snapshot.name_received_at {
                inner.name_received_at.insert(k.clone(), *v);
            }
            for (k, v) in &snapshot.name_expires_at {
                inner.name_expires_at.insert(k.clone(), *v);
            }
        }
        registry
    }

    /// Save the registry snapshot to a shared database.
    pub fn save_to_db(&self, db: &RelayDatabase) {
        let snap = self.snapshot();
        let json = match serde_json::to_string(&snap) {
            Ok(j) => j,
            Err(e) => {
                log::error!("gospel snapshot serialization failed: {e}");
                return;
            }
        };
        let conn = db.lock();
        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO gospel_snapshot (id, snapshot_json) VALUES (1, ?)",
            params![json],
        ) {
            log::error!("gospel snapshot save failed: {e}");
        }
    }

    /// Load a registry from a shared database. Returns a new empty registry
    /// if no snapshot exists.
    pub fn load_from_db(db: &RelayDatabase, config: &GospelConfig) -> Self {
        let conn = db.lock();
        let json: Option<String> = conn
            .query_row(
                "SELECT snapshot_json FROM gospel_snapshot WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .ok();

        match json {
            Some(j) => match serde_json::from_str::<RegistrySnapshot>(&j) {
                Ok(snap) => {
                    drop(conn); // Release lock before restore (which doesn't need DB).
                    Self::restore(&snap, config)
                }
                Err(e) => {
                    log::error!("gospel snapshot deserialization failed: {e}");
                    Self::new(config)
                }
            },
            None => Self::new(config),
        }
    }

    // --- Private conflict resolution ---

    /// Insert a name event with conflict resolution.
    ///
    /// Same author, same d-tag: latest `created_at` wins (update).
    /// Different author, same d-tag: first to reach the relay wins
    /// (compared by `received_at`, not `created_at`). This prevents
    /// backdated events from stealing names.
    ///
    /// Expired names are treated as unclaimed — a new author can take them.
    fn insert_name(inner: &mut RegistryInner, event: &OmniEvent, received_at: i64) -> InsertResult {
        let d_tag = match event.d_tag() {
            Some(d) => d.to_string(),
            None => return InsertResult::Rejected,
        };

        match inner.names.get(&d_tag) {
            Some(existing) => {
                if existing.id == event.id {
                    return InsertResult::Duplicate;
                }

                // Check if the existing name has expired.
                let expired = inner
                    .name_expires_at
                    .get(&d_tag)
                    .is_some_and(|&exp| received_at > exp);

                if existing.author == event.author {
                    // Same author: latest created_at wins (they're updating their own name).
                    if event.created_at > existing.created_at {
                        inner.names.insert(d_tag.clone(), event.clone());
                        inner.name_received_at.insert(d_tag.clone(), received_at);
                        inner.name_expires_at.insert(d_tag, received_at + inner.name_ttl);
                        InsertResult::Inserted
                    } else {
                        InsertResult::Rejected
                    }
                } else if expired {
                    // Existing name expired — treat as new claim.
                    inner.names.insert(d_tag.clone(), event.clone());
                    inner.name_received_at.insert(d_tag.clone(), received_at);
                    inner.name_expires_at.insert(d_tag, received_at + inner.name_ttl);
                    InsertResult::Inserted
                } else {
                    // Different author, not expired: first to reach the relay wins.
                    let existing_received = inner
                        .name_received_at
                        .get(&d_tag)
                        .copied()
                        .unwrap_or(i64::MAX);
                    if received_at < existing_received {
                        inner.names.insert(d_tag.clone(), event.clone());
                        inner.name_received_at.insert(d_tag.clone(), received_at);
                        inner.name_expires_at.insert(d_tag, received_at + inner.name_ttl);
                        InsertResult::Inserted
                    } else {
                        InsertResult::Rejected
                    }
                }
            }
            None => {
                if inner.names.len() >= inner.max_name_records {
                    return InsertResult::Rejected;
                }
                inner.names.insert(d_tag.clone(), event.clone());
                inner.name_received_at.insert(d_tag.clone(), received_at);
                inner.name_expires_at.insert(d_tag, received_at + inner.name_ttl);
                InsertResult::Inserted
            }
        }
    }

    /// Insert a name renewal event.
    ///
    /// Only the current owner can renew. Extends `expires_at` by another
    /// TTL period from `received_at`. Returns `Rejected` if the name
    /// doesn't exist or the author doesn't match the current owner.
    fn insert_renewal(
        inner: &mut RegistryInner,
        event: &OmniEvent,
        received_at: i64,
    ) -> InsertResult {
        let d_tag = match event.d_tag() {
            Some(d) => d.to_string(),
            None => return InsertResult::Rejected,
        };

        match inner.names.get(&d_tag) {
            Some(existing) => {
                if existing.id == event.id {
                    return InsertResult::Duplicate;
                }
                // Only the owner can renew.
                if event.author != existing.author {
                    return InsertResult::Rejected;
                }
                // Extend expiration by another TTL from received_at.
                inner.name_expires_at.insert(d_tag, received_at + inner.name_ttl);
                InsertResult::Inserted
            }
            None => {
                // Can't renew a name that doesn't exist.
                InsertResult::Rejected
            }
        }
    }

    /// Insert an asset announcement. D-tag (asset hash) is the key.
    /// Latest from the same author wins (they're updating their announcement).
    fn insert_asset(inner: &mut RegistryInner, event: &OmniEvent) -> InsertResult {
        let d_tag = match event.d_tag() {
            Some(d) => d.to_string(),
            None => return InsertResult::Rejected,
        };

        match inner.assets.get(&d_tag) {
            Some(existing) => {
                if existing.id == event.id {
                    return InsertResult::Duplicate;
                }
                // Latest announcement for this asset wins.
                if event.created_at > existing.created_at {
                    inner.assets.insert(d_tag, event.clone());
                    InsertResult::Inserted
                } else {
                    InsertResult::Rejected
                }
            }
            None => {
                // Use hint capacity for assets too (same scale).
                if inner.assets.len() >= inner.max_hint_records {
                    return InsertResult::Rejected;
                }
                inner.assets.insert(d_tag, event.clone());
                InsertResult::Inserted
            }
        }
    }

    /// Insert a relay hint event. Latest from same author wins.
    fn insert_hint(inner: &mut RegistryInner, event: &OmniEvent) -> InsertResult {
        let author = &event.author;

        match inner.hints.get(author) {
            Some(existing) => {
                if existing.id == event.id {
                    return InsertResult::Duplicate;
                }
                if event.created_at > existing.created_at {
                    inner.hints.insert(author.clone(), event.clone());
                    InsertResult::Inserted
                } else {
                    InsertResult::Rejected
                }
            }
            None => {
                if inner.hints.len() >= inner.max_hint_records {
                    return InsertResult::Rejected;
                }
                inner.hints.insert(author.clone(), event.clone());
                InsertResult::Inserted
            }
        }
    }

    /// Insert a semantic profile event. Latest from same author wins.
    /// Same logic as hints — keyed by author, latest `created_at` wins.
    fn insert_profile(inner: &mut RegistryInner, event: &OmniEvent) -> InsertResult {
        let author = &event.author;

        match inner.profiles.get(author) {
            Some(existing) => {
                if existing.id == event.id {
                    return InsertResult::Duplicate;
                }
                if event.created_at > existing.created_at {
                    inner.profiles.insert(author.clone(), event.clone());
                    InsertResult::Inserted
                } else {
                    InsertResult::Rejected
                }
            }
            None => {
                // Use hint capacity for profiles too (same scale).
                if inner.profiles.len() >= inner.max_hint_records {
                    return InsertResult::Rejected;
                }
                inner.profiles.insert(author.clone(), event.clone());
                InsertResult::Inserted
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tier::GospelTier;
    use crate::server::database::RelayDatabase;

    fn default_config() -> GospelConfig {
        GospelConfig::default()
    }

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
    fn new_registry_is_empty() {
        let reg = GospelRegistry::new(&default_config());
        assert_eq!(reg.name_count(), 0);
        assert_eq!(reg.hint_count(), 0);
        assert_eq!(reg.total_count(), 0);
        assert_eq!(reg.high_water_mark(), 0);
    }

    #[test]
    fn insert_name_claim() {
        let reg = GospelRegistry::new(&default_config());
        let event = make_name_event("sam.com", "alice", 1000);
        assert_eq!(reg.insert(&event), InsertResult::Inserted);
        assert_eq!(reg.name_count(), 1);
        assert!(reg.lookup_name("sam.com").is_some());
    }

    #[test]
    fn insert_duplicate_returns_duplicate() {
        let reg = GospelRegistry::new(&default_config());
        let event = make_name_event("sam.com", "alice", 1000);
        assert_eq!(reg.insert(&event), InsertResult::Inserted);
        assert_eq!(reg.insert(&event), InsertResult::Duplicate);
        assert_eq!(reg.name_count(), 1);
    }

    #[test]
    fn name_conflict_same_author_latest_wins() {
        let reg = GospelRegistry::new(&default_config());
        let old = make_name_event("sam.com", "alice", 1000);
        let new = make_name_event("sam.com", "alice", 2000);

        assert_eq!(reg.insert(&old), InsertResult::Inserted);
        assert_eq!(reg.insert(&new), InsertResult::Inserted);
        assert_eq!(reg.name_count(), 1);

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.created_at, 2000);
    }

    #[test]
    fn name_conflict_same_author_older_rejected() {
        let reg = GospelRegistry::new(&default_config());
        let new = make_name_event("sam.com", "alice", 2000);
        let old = make_name_event("sam.com", "alice", 1000);

        assert_eq!(reg.insert(&new), InsertResult::Inserted);
        assert_eq!(reg.insert(&old), InsertResult::Rejected);

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.created_at, 2000);
    }

    #[test]
    fn name_conflict_different_author_earliest_wins() {
        let reg = GospelRegistry::new(&default_config());
        let alice = make_name_event("sam.com", "alice", 1000);
        let bob = make_name_event("sam.com", "bob", 2000);

        // Alice claims first.
        assert_eq!(reg.insert(&alice), InsertResult::Inserted);
        // Bob tries to claim later — rejected (first-claim rule).
        assert_eq!(reg.insert(&bob), InsertResult::Rejected);

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "alice");
    }

    #[test]
    fn name_conflict_first_arrival_wins() {
        // Anti-squatting: first to reach the relay wins, regardless of created_at.
        // A later arrival with an earlier created_at cannot displace the first.
        let reg = GospelRegistry::new(&default_config());
        let bob = make_name_event("sam.com", "bob", 2000);
        let alice = make_name_event("sam.com", "alice", 1000);

        // Bob's claim arrives first at received_at=100.
        assert_eq!(
            reg.insert_with_received_at(&bob, 100),
            InsertResult::Inserted
        );
        // Alice's claim arrives later at received_at=200 — rejected
        // even though her created_at is earlier. First arrival wins.
        assert_eq!(
            reg.insert_with_received_at(&alice, 200),
            InsertResult::Rejected
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "bob");
    }

    #[test]
    fn insert_hint() {
        let reg = GospelRegistry::new(&default_config());
        let event = make_hint_event("alice", 1000);
        assert_eq!(reg.insert(&event), InsertResult::Inserted);
        assert_eq!(reg.hint_count(), 1);
        assert!(reg.lookup_hints("alice").is_some());
    }

    #[test]
    fn hint_latest_wins() {
        let reg = GospelRegistry::new(&default_config());
        let old = make_hint_event("alice", 1000);
        let new = make_hint_event("alice", 2000);

        assert_eq!(reg.insert(&old), InsertResult::Inserted);
        assert_eq!(reg.insert(&new), InsertResult::Inserted);
        assert_eq!(reg.hint_count(), 1);

        let stored = reg.lookup_hints("alice").unwrap();
        assert_eq!(stored.created_at, 2000);
    }

    #[test]
    fn hint_older_rejected() {
        let reg = GospelRegistry::new(&default_config());
        let new = make_hint_event("alice", 2000);
        let old = make_hint_event("alice", 1000);

        assert_eq!(reg.insert(&new), InsertResult::Inserted);
        assert_eq!(reg.insert(&old), InsertResult::Rejected);
    }

    #[test]
    fn lookup_not_found() {
        let reg = GospelRegistry::new(&default_config());
        assert!(reg.lookup_name("nobody.com").is_none());
        assert!(reg.lookup_hints("nobody").is_none());
    }

    #[test]
    fn events_since() {
        let reg = GospelRegistry::new(&default_config());
        reg.insert(&make_name_event("old.com", "alice", 1000));
        reg.insert(&make_name_event("new.com", "bob", 2000));
        reg.insert(&make_hint_event("carol", 3000));

        let since_1500 = reg.events_since(1500);
        assert_eq!(since_1500.len(), 2);
        assert!(since_1500.iter().all(|e| e.created_at > 1500));

        let since_0 = reg.events_since(0);
        assert_eq!(since_0.len(), 3);
    }

    #[test]
    fn high_water_mark_advances() {
        let reg = GospelRegistry::new(&default_config());
        assert_eq!(reg.high_water_mark(), 0);

        reg.insert(&make_name_event("a.com", "alice", 1000));
        assert_eq!(reg.high_water_mark(), 1000);

        reg.insert(&make_hint_event("bob", 2000));
        assert_eq!(reg.high_water_mark(), 2000);

        // Older event doesn't reduce the mark.
        reg.insert(&make_name_event("b.com", "carol", 500));
        assert_eq!(reg.high_water_mark(), 2000);
    }

    #[test]
    fn non_gospel_kind_rejected() {
        let reg = GospelRegistry::new(&default_config());
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: 1, // TEXT_NOTE — not a gospel kind
            tags: vec![],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert_eq!(reg.insert(&event), InsertResult::Rejected);
    }

    #[test]
    fn name_without_d_tag_rejected() {
        let reg = GospelRegistry::new(&default_config());
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: kind::NAME_CLAIM,
            tags: vec![], // No d-tag.
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert_eq!(reg.insert(&event), InsertResult::Rejected);
    }

    #[test]
    fn snapshot_and_restore() {
        let config = default_config();
        let reg = GospelRegistry::new(&config);
        reg.insert(&make_name_event("sam.com", "alice", 1000));
        reg.insert(&make_hint_event("bob", 2000));

        let snap = reg.snapshot();
        assert_eq!(snap.names.len(), 1);
        assert_eq!(snap.hints.len(), 1);
        assert_eq!(snap.high_water_mark, 2000);

        let restored = GospelRegistry::restore(&snap, &config);
        assert_eq!(restored.name_count(), 1);
        assert_eq!(restored.hint_count(), 1);
        assert!(restored.lookup_name("sam.com").is_some());
        assert!(restored.lookup_hints("bob").is_some());
        assert_eq!(restored.high_water_mark(), 2000);
    }

    #[test]
    fn snapshot_serde_round_trip() {
        let config = default_config();
        let reg = GospelRegistry::new(&config);
        reg.insert(&make_name_event("x.com", "alice", 500));

        let snap = reg.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let loaded: RegistrySnapshot = serde_json::from_str(&json).unwrap();

        let restored = GospelRegistry::restore(&loaded, &config);
        assert_eq!(restored.name_count(), 1);
        assert!(restored.lookup_name("x.com").is_some());
    }

    #[test]
    fn capacity_limit_names() {
        let mut config = default_config();
        config.max_name_records = 2;
        let reg = GospelRegistry::new(&config);

        reg.insert(&make_name_event("a.com", "alice", 1000));
        reg.insert(&make_name_event("b.com", "bob", 2000));
        assert_eq!(reg.name_count(), 2);

        // Third name rejected due to capacity.
        let result = reg.insert(&make_name_event("c.com", "carol", 3000));
        assert_eq!(result, InsertResult::Rejected);
        assert_eq!(reg.name_count(), 2);
    }

    #[test]
    fn save_and_load_from_db() {
        let config = default_config();
        let db = RelayDatabase::in_memory();

        // Populate and save.
        let reg = GospelRegistry::new(&config);
        reg.insert(&make_name_event("sam.com", "alice", 1000));
        reg.insert(&make_hint_event("bob", 2000));
        reg.save_to_db(&db);

        // Load from the same DB.
        let restored = GospelRegistry::load_from_db(&db, &config);
        assert_eq!(restored.name_count(), 1);
        assert_eq!(restored.hint_count(), 1);
        assert!(restored.lookup_name("sam.com").is_some());
        assert!(restored.lookup_hints("bob").is_some());
        assert_eq!(restored.high_water_mark(), 2000);
    }

    #[test]
    fn load_from_empty_db_returns_new() {
        let config = default_config();
        let db = RelayDatabase::in_memory();

        let reg = GospelRegistry::load_from_db(&db, &config);
        assert_eq!(reg.total_count(), 0);
        assert_eq!(reg.high_water_mark(), 0);
    }

    #[test]
    fn events_since_for_tiers_universal_only() {
        let reg = GospelRegistry::new(&default_config());
        // Name = Universal, Asset = Community (registry stores both)
        reg.insert(&make_name_event("a.com", "alice", 1000));
        reg.insert(&OmniEvent {
            id: "asset-1".into(),
            author: "bob".into(),
            created_at: 2000,
            kind: kind::ASSET_ANNOUNCE,
            tags: vec![vec!["d".into(), "abcdef123456".into()]],
            content: String::new(),
            sig: "c".repeat(128),
        });

        let universal = reg.events_since_for_tiers(0, &[GospelTier::Universal]);
        assert_eq!(universal.len(), 1);
        assert_eq!(universal[0].kind, kind::NAME_CLAIM);

        let community = reg.events_since_for_tiers(0, &[GospelTier::Community]);
        assert_eq!(community.len(), 1);
        assert_eq!(community[0].kind, kind::ASSET_ANNOUNCE);

        let both = reg.events_since_for_tiers(
            0,
            &[GospelTier::Universal, GospelTier::Community],
        );
        assert_eq!(both.len(), 2);
    }

    fn make_profile_event(author: &str, created_at: i64) -> OmniEvent {
        OmniEvent {
            id: format!("profile-{author}-{created_at}"),
            author: author.to_string(),
            created_at,
            kind: kind::SEMANTIC_PROFILE,
            tags: vec![vec!["d".into(), author.to_string()]],
            content: r#"{"keyword_search":true}"#.into(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn insert_profile() {
        let reg = GospelRegistry::new(&default_config());
        let event = make_profile_event("alice", 1000);
        assert_eq!(reg.insert(&event), InsertResult::Inserted);
        assert_eq!(reg.profile_count(), 1);
        assert!(reg.lookup_profile("alice").is_some());
    }

    #[test]
    fn profile_latest_wins() {
        let reg = GospelRegistry::new(&default_config());
        let old = make_profile_event("alice", 1000);
        let new = make_profile_event("alice", 2000);

        assert_eq!(reg.insert(&old), InsertResult::Inserted);
        assert_eq!(reg.insert(&new), InsertResult::Inserted);
        assert_eq!(reg.profile_count(), 1);

        let stored = reg.lookup_profile("alice").unwrap();
        assert_eq!(stored.created_at, 2000);
    }

    #[test]
    fn profile_snapshot_round_trip() {
        let config = default_config();
        let reg = GospelRegistry::new(&config);
        reg.insert(&make_profile_event("alice", 1000));

        let snap = reg.snapshot();
        assert_eq!(snap.profiles.len(), 1);

        let json = serde_json::to_string(&snap).unwrap();
        let loaded: RegistrySnapshot = serde_json::from_str(&json).unwrap();
        let restored = GospelRegistry::restore(&loaded, &config);
        assert_eq!(restored.profile_count(), 1);
        assert!(restored.lookup_profile("alice").is_some());
    }

    #[test]
    fn profile_in_events_since() {
        let reg = GospelRegistry::new(&default_config());
        reg.insert(&make_profile_event("alice", 2000));
        reg.insert(&make_name_event("a.com", "bob", 1000));

        let since_1500 = reg.events_since(1500);
        assert_eq!(since_1500.len(), 1);
        assert_eq!(since_1500[0].kind, kind::SEMANTIC_PROFILE);
    }

    #[test]
    fn profile_tier_filtering() {
        let reg = GospelRegistry::new(&default_config());
        reg.insert(&make_profile_event("alice", 1000));
        reg.insert(&make_name_event("a.com", "bob", 1000));

        // Both are Universal tier.
        let universal = reg.events_since_for_tiers(0, &[GospelTier::Universal]);
        assert_eq!(universal.len(), 2);

        // Community tier should exclude both.
        let community = reg.events_since_for_tiers(0, &[GospelTier::Community]);
        assert_eq!(community.len(), 0);
    }

    #[test]
    fn capacity_limit_hints() {
        let mut config = default_config();
        config.max_hint_records = 1;
        let reg = GospelRegistry::new(&config);

        reg.insert(&make_hint_event("alice", 1000));
        assert_eq!(reg.hint_count(), 1);

        // Second hint from different author rejected due to capacity.
        let result = reg.insert(&make_hint_event("bob", 2000));
        assert_eq!(result, InsertResult::Rejected);
        assert_eq!(reg.hint_count(), 1);

        // But update from same author still works.
        let result = reg.insert(&make_hint_event("alice", 3000));
        assert_eq!(result, InsertResult::Inserted);
    }

    // --- Anti-squatting tests ---

    fn make_renewal_event(name: &str, author: &str, created_at: i64) -> OmniEvent {
        OmniEvent {
            id: format!("renew-{name}-{author}-{created_at}"),
            author: author.to_string(),
            created_at,
            kind: kind::NAME_RENEWAL,
            tags: vec![vec!["d".into(), name.into()]],
            content: String::new(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn backdated_event_loses_to_first_arrival() {
        // A squatter creates an event with created_at=0 (backdated)
        // but it arrives at the relay AFTER a legitimate claim.
        let reg = GospelRegistry::new(&default_config());

        let legitimate = make_name_event("sam.com", "alice", 1000);
        let backdated = make_name_event("sam.com", "bob", 0); // created_at=0

        // Legitimate arrives first (received_at=1000).
        assert_eq!(
            reg.insert_with_received_at(&legitimate, 1000),
            InsertResult::Inserted
        );
        // Backdated arrives later (received_at=2000) — rejected.
        assert_eq!(
            reg.insert_with_received_at(&backdated, 2000),
            InsertResult::Rejected
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "alice");
    }

    #[test]
    fn first_arrival_wins_regardless_of_created_at() {
        let reg = GospelRegistry::new(&default_config());

        let early_created = make_name_event("sam.com", "alice", 500);
        let late_created = make_name_event("sam.com", "bob", 2000);

        // Bob's event has a later created_at but arrives first.
        assert_eq!(
            reg.insert_with_received_at(&late_created, 100),
            InsertResult::Inserted
        );
        // Alice's event has an earlier created_at but arrives later — rejected.
        assert_eq!(
            reg.insert_with_received_at(&early_created, 200),
            InsertResult::Rejected
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "bob");
    }

    #[test]
    fn same_author_updates_still_use_created_at() {
        let reg = GospelRegistry::new(&default_config());

        let old = make_name_event("sam.com", "alice", 1000);
        let new = make_name_event("sam.com", "alice", 2000);

        assert_eq!(
            reg.insert_with_received_at(&old, 100),
            InsertResult::Inserted
        );
        assert_eq!(
            reg.insert_with_received_at(&new, 200),
            InsertResult::Inserted
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.created_at, 2000);
    }

    #[test]
    fn expired_name_can_be_claimed_by_new_author() {
        let mut config = default_config();
        config.name_policy.name_ttl_secs = 100; // Very short TTL.
        let reg = GospelRegistry::new(&config);

        // Alice claims at received_at=1000. Expires at 1100.
        let alice = make_name_event("sam.com", "alice", 1000);
        assert_eq!(
            reg.insert_with_received_at(&alice, 1000),
            InsertResult::Inserted
        );

        // Bob tries at received_at=1200 (after expiration).
        let bob = make_name_event("sam.com", "bob", 1200);
        assert_eq!(
            reg.insert_with_received_at(&bob, 1200),
            InsertResult::Inserted
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "bob");
    }

    #[test]
    fn non_expired_name_cannot_be_stolen() {
        let mut config = default_config();
        config.name_policy.name_ttl_secs = 1000; // Long TTL.
        let reg = GospelRegistry::new(&config);

        // Alice claims at received_at=1000. Expires at 2000.
        let alice = make_name_event("sam.com", "alice", 1000);
        assert_eq!(
            reg.insert_with_received_at(&alice, 1000),
            InsertResult::Inserted
        );

        // Bob tries at received_at=1500 (before expiration) — rejected.
        let bob = make_name_event("sam.com", "bob", 1500);
        assert_eq!(
            reg.insert_with_received_at(&bob, 1500),
            InsertResult::Rejected
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "alice");
    }

    #[test]
    fn renewal_extends_expiration() {
        let mut config = default_config();
        config.name_policy.name_ttl_secs = 100;
        let reg = GospelRegistry::new(&config);

        // Alice claims at received_at=1000. Expires at 1100.
        let claim = make_name_event("sam.com", "alice", 1000);
        assert_eq!(
            reg.insert_with_received_at(&claim, 1000),
            InsertResult::Inserted
        );

        // Alice renews at received_at=1050. Expires at 1150 now.
        let renewal = make_renewal_event("sam.com", "alice", 1050);
        assert_eq!(
            reg.insert_with_received_at(&renewal, 1050),
            InsertResult::Inserted
        );

        // Bob tries at received_at=1120 (after original 1100 but before renewed 1150).
        let bob = make_name_event("sam.com", "bob", 1120);
        assert_eq!(
            reg.insert_with_received_at(&bob, 1120),
            InsertResult::Rejected
        );

        let stored = reg.lookup_name("sam.com").unwrap();
        assert_eq!(stored.author, "alice");
    }

    #[test]
    fn only_owner_can_renew() {
        let reg = GospelRegistry::new(&default_config());

        let claim = make_name_event("sam.com", "alice", 1000);
        assert_eq!(
            reg.insert_with_received_at(&claim, 1000),
            InsertResult::Inserted
        );

        // Bob tries to renew alice's name — rejected.
        let renewal = make_renewal_event("sam.com", "bob", 2000);
        assert_eq!(
            reg.insert_with_received_at(&renewal, 2000),
            InsertResult::Rejected
        );
    }

    #[test]
    fn renewal_of_nonexistent_name_rejected() {
        let reg = GospelRegistry::new(&default_config());

        let renewal = make_renewal_event("sam.com", "alice", 1000);
        assert_eq!(
            reg.insert_with_received_at(&renewal, 1000),
            InsertResult::Rejected
        );
    }

    #[test]
    fn purge_expired_removes_old_names() {
        let mut config = default_config();
        config.name_policy.name_ttl_secs = 1; // 1 second TTL.
        let reg = GospelRegistry::new(&config);

        // Insert with a received_at far in the past so it's already expired.
        let event = make_name_event("old.com", "alice", 100);
        reg.insert_with_received_at(&event, 100);
        assert_eq!(reg.name_count(), 1);

        // Purge — the name is long expired (received_at=100, TTL=1, now >> 101).
        let purged = reg.purge_expired();
        assert_eq!(purged, 1);
        assert_eq!(reg.name_count(), 0);
        assert!(reg.lookup_name("old.com").is_none());
    }

    #[test]
    fn purge_expired_keeps_fresh_names() {
        let reg = GospelRegistry::new(&default_config()); // 1 year TTL.

        let event = make_name_event("fresh.com", "alice", 1000);
        reg.insert(&event); // Uses "now" as received_at.
        assert_eq!(reg.name_count(), 1);

        let purged = reg.purge_expired();
        assert_eq!(purged, 0);
        assert_eq!(reg.name_count(), 1);
    }

    #[test]
    fn snapshot_preserves_received_at_and_expires_at() {
        let mut config = default_config();
        config.name_policy.name_ttl_secs = 5000;
        let reg = GospelRegistry::new(&config);

        let event = make_name_event("sam.com", "alice", 1000);
        reg.insert_with_received_at(&event, 2000);

        let snap = reg.snapshot();
        assert_eq!(snap.name_received_at.get("sam.com"), Some(&2000));
        assert_eq!(snap.name_expires_at.get("sam.com"), Some(&7000));

        // Restore and verify.
        let restored = GospelRegistry::restore(&snap, &config);
        let snap2 = restored.snapshot();
        assert_eq!(snap2.name_received_at.get("sam.com"), Some(&2000));
        assert_eq!(snap2.name_expires_at.get("sam.com"), Some(&7000));
    }

    #[test]
    fn legacy_snapshot_without_new_fields_deserializes() {
        // Simulates a snapshot from before anti-squatting was added.
        let json = r#"{
            "names": [],
            "hints": [],
            "high_water_mark": 500
        }"#;
        let loaded: RegistrySnapshot = serde_json::from_str(json).unwrap();
        assert!(loaded.name_received_at.is_empty());
        assert!(loaded.name_expires_at.is_empty());
        assert!(loaded.assets.is_empty());
        assert!(loaded.profiles.is_empty());
        assert_eq!(loaded.high_water_mark, 500);

        // Should restore without error.
        let config = default_config();
        let restored = GospelRegistry::restore(&loaded, &config);
        assert_eq!(restored.total_count(), 0);
        assert_eq!(restored.high_water_mark(), 500);
    }

    #[test]
    fn name_renewal_is_gospel_registry_kind() {
        assert!(kind::is_gospel_registry(kind::NAME_RENEWAL));
    }

    #[test]
    fn insert_with_received_at_backward_compatible() {
        // Verify that insert() still works identically for non-name events.
        let reg = GospelRegistry::new(&default_config());
        let hint = make_hint_event("alice", 1000);
        assert_eq!(reg.insert(&hint), InsertResult::Inserted);
        assert_eq!(reg.hint_count(), 1);
    }
}
