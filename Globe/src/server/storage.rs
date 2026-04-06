use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::event::OmniEvent;
use crate::filter::OmniFilter;

use super::database::RelayDatabase;

/// Statistics about the events in the store.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoreStats {
    /// Total number of events stored.
    pub event_count: usize,
    /// Unix timestamp of the oldest event, if any.
    pub oldest_event: Option<i64>,
    /// Unix timestamp of the newest event, if any.
    pub newest_event: Option<i64>,
    /// Number of events grouped by kind.
    pub events_by_kind: HashMap<u32, usize>,
}

/// Configuration for the event store.
#[derive(Clone, Debug)]
pub struct StoreConfig {
    /// Maximum number of events to keep. Oldest are evicted when full.
    /// `None` means no limit.
    pub max_events: Option<usize>,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            max_events: Some(100_000),
        }
    }
}

/// SQLite-backed event store for a relay server.
///
/// Thread-safe via `Arc<Mutex<Connection>>` inside `RelayDatabase`.
/// Events are indexed by kind, author, and created_at for fast queries.
/// Oldest events are evicted when the store reaches its configured capacity.
#[derive(Clone)]
pub struct EventStore {
    db: RelayDatabase,
    config: StoreConfig,
}

impl EventStore {
    /// Create a new store with default configuration (in-memory, 100K max events).
    pub fn new() -> Self {
        Self::with_config(StoreConfig::default())
    }

    /// Create a new store with custom configuration (in-memory).
    pub fn with_config(config: StoreConfig) -> Self {
        Self {
            db: RelayDatabase::in_memory(),
            config,
        }
    }

    /// Create a store backed by a shared database.
    pub fn from_db(db: RelayDatabase, config: StoreConfig) -> Self {
        Self { db, config }
    }

    /// Get a reference to the underlying database (for sharing with AssetStore).
    pub fn database(&self) -> &RelayDatabase {
        &self.db
    }

    /// Store an event. Returns `true` if it was new, `false` if duplicate.
    pub fn insert(&self, event: OmniEvent) -> bool {
        let conn = self.db.lock();
        let tags_json = serde_json::to_string(&event.tags).unwrap_or_else(|_| "[]".into());

        let result = conn.execute(
            "INSERT OR IGNORE INTO events (id, author, created_at, kind, tags, content, sig) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![event.id, event.author, event.created_at, event.kind, tags_json, event.content, event.sig],
        );

        match result {
            Ok(0) => false, // Duplicate — INSERT OR IGNORE did nothing.
            Ok(_) => {
                // Evict if over capacity.
                if let Some(max) = self.config.max_events {
                    self.evict_excess(&conn, max);
                }
                true
            }
            Err(e) => {
                log::error!("event insert failed: {e}");
                false
            }
        }
    }

    /// Query events matching a filter.
    pub fn query(&self, filter: &OmniFilter) -> Vec<OmniEvent> {
        let conn = self.db.lock();
        self.query_inner(&conn, filter)
    }

    /// Query events matching ANY of the given filters (OR'd).
    pub fn query_any(&self, filters: &[OmniFilter]) -> Vec<OmniEvent> {
        let conn = self.db.lock();
        let mut seen = std::collections::HashSet::new();
        let mut all_results = Vec::new();

        for filter in filters {
            for event in self.query_inner(&conn, filter) {
                if seen.insert(event.id.clone()) {
                    all_results.push(event);
                }
            }
        }

        all_results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        all_results
    }

    /// Get a single event by ID.
    pub fn get(&self, id: &str) -> Option<OmniEvent> {
        let conn = self.db.lock();
        conn.query_row(
            "SELECT id, author, created_at, kind, tags, content, sig FROM events WHERE id = ?",
            params![id],
            event_from_row,
        )
        .ok()
    }

    /// Number of events stored.
    pub fn len(&self) -> usize {
        let conn = self.db.lock();
        conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get statistics about the stored events.
    pub fn stats(&self) -> StoreStats {
        let conn = self.db.lock();

        let event_count: usize = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize;

        let oldest_event: Option<i64> = conn
            .query_row(
                "SELECT MIN(created_at) FROM events",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap_or(None);

        let newest_event: Option<i64> = conn
            .query_row(
                "SELECT MAX(created_at) FROM events",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap_or(None);

        let mut events_by_kind = HashMap::new();
        if let Ok(mut stmt) = conn.prepare("SELECT kind, COUNT(*) FROM events GROUP BY kind") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, i64>(1)?))
            }) {
                for row in rows.flatten() {
                    events_by_kind.insert(row.0, row.1 as usize);
                }
            }
        }

        StoreStats {
            event_count,
            oldest_event,
            newest_event,
            events_by_kind,
        }
    }

    // --- Private helpers ---

    fn query_inner(
        &self,
        conn: &rusqlite::Connection,
        filter: &OmniFilter,
    ) -> Vec<OmniEvent> {
        let (sql, params) = build_query(filter);

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                log::error!("query prepare failed: {e}");
                return Vec::new();
            }
        };

        let rows = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            event_from_row(row)
        }) {
            Ok(r) => r,
            Err(e) => {
                log::error!("query execution failed: {e}");
                return Vec::new();
            }
        };

        let mut results: Vec<OmniEvent> = rows.filter_map(|r| r.ok()).collect();

        // Post-filter: tag matching (SQL handles kind/author/time, Rust handles tags).
        if !filter.tag_filters.is_empty() {
            results.retain(|event| {
                for (tag_name, filter_values) in &filter.tag_filters {
                    let tag_str = tag_name.to_string();
                    let event_values = event.tag_values(&tag_str);
                    if !filter_values
                        .iter()
                        .any(|fv| event_values.contains(&fv.as_str()))
                    {
                        return false;
                    }
                }
                true
            });
        }

        results
    }

    fn evict_excess(&self, conn: &rusqlite::Connection, max: usize) {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap_or(0);

        if count as usize > max {
            let excess = count as usize - max;
            let _ = conn.execute(
                "DELETE FROM events WHERE id IN (SELECT id FROM events ORDER BY created_at ASC LIMIT ?)",
                params![excess as i64],
            );
        }
    }
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a SQL query from an OmniFilter.
///
/// Tag filters are NOT included in the SQL — they're post-filtered in Rust
/// because tag matching involves nested arrays that are awkward in SQL.
fn build_query(filter: &OmniFilter) -> (String, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value;

    let mut conditions = Vec::new();
    let mut params: Vec<Value> = Vec::new();

    if let Some(ids) = &filter.ids {
        let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
        conditions.push(format!("id IN ({})", placeholders.join(",")));
        for id in ids {
            params.push(Value::Text(id.clone()));
        }
    }

    if let Some(authors) = &filter.authors {
        let placeholders: Vec<&str> = authors.iter().map(|_| "?").collect();
        conditions.push(format!("author IN ({})", placeholders.join(",")));
        for author in authors {
            params.push(Value::Text(author.clone()));
        }
    }

    if let Some(kinds) = &filter.kinds {
        let placeholders: Vec<&str> = kinds.iter().map(|_| "?").collect();
        conditions.push(format!("kind IN ({})", placeholders.join(",")));
        for kind in kinds {
            params.push(Value::Integer(*kind as i64));
        }
    }

    if let Some(since) = filter.since {
        conditions.push("created_at >= ?".into());
        params.push(Value::Integer(since));
    }

    if let Some(until) = filter.until {
        conditions.push("created_at <= ?".into());
        params.push(Value::Integer(until));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit_clause = match filter.limit {
        Some(limit) => format!("LIMIT {limit}"),
        None => String::new(),
    };

    let sql = format!(
        "SELECT id, author, created_at, kind, tags, content, sig FROM events {where_clause} ORDER BY created_at DESC {limit_clause}"
    );

    (sql, params)
}

/// Convert a SQLite row to an OmniEvent.
fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OmniEvent> {
    let tags_json: String = row.get(4)?;
    let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(OmniEvent {
        id: row.get(0)?,
        author: row.get(1)?,
        created_at: row.get(2)?,
        kind: row.get::<_, u32>(3)?,
        tags,
        content: row.get(5)?,
        sig: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(id_char: char, kind: u32, created_at: i64) -> OmniEvent {
        OmniEvent {
            id: id_char.to_string().repeat(64),
            author: "b".repeat(64),
            created_at,
            kind,
            tags: vec![],
            content: format!("event {id_char}"),
            sig: "c".repeat(128),
        }
    }

    fn make_event_with_author(
        id_char: char,
        kind: u32,
        author_char: char,
        created_at: i64,
    ) -> OmniEvent {
        OmniEvent {
            id: id_char.to_string().repeat(64),
            author: author_char.to_string().repeat(64),
            created_at,
            kind,
            tags: vec![],
            content: format!("event {id_char}"),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn insert_and_get() {
        let store = EventStore::new();
        let event = make_event('a', 1, 1000);

        assert!(store.insert(event.clone()));
        assert_eq!(store.len(), 1);

        let retrieved = store.get(&"a".repeat(64)).unwrap();
        assert_eq!(retrieved.content, "event a");
    }

    #[test]
    fn duplicate_rejected() {
        let store = EventStore::new();
        let event = make_event('a', 1, 1000);

        assert!(store.insert(event.clone()));
        assert!(!store.insert(event));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn query_by_kind_uses_index() {
        let store = EventStore::new();
        store.insert(make_event('a', 1, 1000));
        store.insert(make_event('b', 7000, 2000));
        store.insert(make_event('c', 1, 3000));

        let filter = OmniFilter {
            kinds: Some(vec![1]),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, "event c");
        assert_eq!(results[1].content, "event a");
    }

    #[test]
    fn query_by_author_uses_index() {
        let store = EventStore::new();
        store.insert(make_event_with_author('a', 1, 'x', 1000));
        store.insert(make_event_with_author('b', 1, 'y', 2000));
        store.insert(make_event_with_author('c', 1, 'x', 3000));

        let filter = OmniFilter {
            authors: Some(vec!["x".repeat(64)]),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_by_kind_and_author_intersects() {
        let store = EventStore::new();
        store.insert(make_event_with_author('a', 1, 'x', 1000));
        store.insert(make_event_with_author('b', 7000, 'x', 2000));
        store.insert(make_event_with_author('c', 1, 'y', 3000));

        let filter = OmniFilter {
            kinds: Some(vec![1]),
            authors: Some(vec!["x".repeat(64)]),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "event a");
    }

    #[test]
    fn query_with_limit() {
        let store = EventStore::new();
        for i in 0..10u8 {
            let c = (b'a' + i) as char;
            store.insert(make_event(c, 1, i as i64));
        }

        let filter = OmniFilter {
            kinds: Some(vec![1]),
            limit: Some(3),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_any_or_logic() {
        let store = EventStore::new();
        store.insert(make_event('a', 1, 1000));
        store.insert(make_event('b', 7000, 2000));
        store.insert(make_event('c', 3, 3000));

        let filters = vec![
            OmniFilter {
                kinds: Some(vec![1]),
                ..Default::default()
            },
            OmniFilter {
                kinds: Some(vec![3]),
                ..Default::default()
            },
        ];
        let results = store.query_any(&filters);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn eviction_removes_oldest() {
        let store = EventStore::with_config(StoreConfig {
            max_events: Some(3),
        });

        store.insert(make_event('a', 1, 100)); // oldest
        store.insert(make_event('b', 1, 200));
        store.insert(make_event('c', 1, 300));
        assert_eq!(store.len(), 3);

        // Inserting a 4th should evict 'a' (oldest).
        store.insert(make_event('d', 1, 400));
        assert_eq!(store.len(), 3);
        assert!(store.get(&"a".repeat(64)).is_none()); // Evicted.
        assert!(store.get(&"d".repeat(64)).is_some()); // New one kept.
    }

    #[test]
    fn eviction_cleans_indexes() {
        let store = EventStore::with_config(StoreConfig {
            max_events: Some(2),
        });

        store.insert(make_event('a', 1, 100));
        store.insert(make_event('b', 7000, 200));
        store.insert(make_event('c', 1, 300)); // Evicts 'a'.

        // Query kind 1 should only find 'c', not 'a'.
        let filter = OmniFilter {
            kinds: Some(vec![1]),
            ..Default::default()
        };
        let results = store.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "event c");
    }

    #[test]
    fn unlimited_store() {
        let store = EventStore::with_config(StoreConfig { max_events: None });

        for i in 0..1000u16 {
            store.insert(OmniEvent {
                id: format!("{i:064}"),
                author: "b".repeat(64),
                created_at: i as i64,
                kind: 1,
                tags: vec![],
                content: format!("event {i}"),
                sig: "c".repeat(128),
            });
        }
        assert_eq!(store.len(), 1000); // No eviction.
    }

    #[test]
    fn empty_store() {
        let store = EventStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn thread_safe_clone() {
        let store = EventStore::new();
        let store2 = store.clone();

        store.insert(make_event('a', 1, 1000));
        assert_eq!(store2.len(), 1);
    }

    #[test]
    fn tags_round_trip() {
        let store = EventStore::new();
        let event = OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![
                vec!["e".into(), "event123".into()],
                vec!["p".into(), "pubkey456".into()],
                vec!["d".into(), "sam.com".into()],
            ],
            content: "hello".into(),
            sig: "c".repeat(128),
        };
        store.insert(event.clone());

        let retrieved = store.get(&"a".repeat(64)).unwrap();
        assert_eq!(retrieved.tags, event.tags);
        assert_eq!(retrieved.d_tag(), Some("sam.com"));
    }

    #[test]
    fn query_with_tag_filter() {
        let store = EventStore::new();
        store.insert(OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![vec!["d".into(), "sam.com".into()]],
            content: "match".into(),
            sig: "c".repeat(128),
        });
        store.insert(OmniEvent {
            id: "x".repeat(64),
            author: "b".repeat(64),
            created_at: 2000,
            kind: 1,
            tags: vec![vec!["d".into(), "other.com".into()]],
            content: "no match".into(),
            sig: "c".repeat(128),
        });

        let mut tag_filters = std::collections::HashMap::new();
        tag_filters.insert('d', vec!["sam.com".into()]);
        let filter = OmniFilter {
            kinds: Some(vec![1]),
            tag_filters,
            ..Default::default()
        };

        let results = store.query(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "match");
    }

    #[test]
    fn persistence_round_trip() {
        let dir = std::env::temp_dir().join(format!("globe_events_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        // Write events.
        {
            let db = RelayDatabase::open_unencrypted(&db_path).unwrap();
            let store = EventStore::from_db(db, StoreConfig::default());
            store.insert(make_event('a', 1, 1000));
            store.insert(make_event('b', 7000, 2000));
            assert_eq!(store.len(), 2);
        }

        // Reopen and verify.
        {
            let db = RelayDatabase::open_unencrypted(&db_path).unwrap();
            let store = EventStore::from_db(db, StoreConfig::default());
            assert_eq!(store.len(), 2);
            let event = store.get(&"a".repeat(64)).unwrap();
            assert_eq!(event.content, "event a");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stats_empty_store() {
        let store = EventStore::new();
        let stats = store.stats();

        assert_eq!(stats.event_count, 0);
        assert!(stats.oldest_event.is_none());
        assert!(stats.newest_event.is_none());
        assert!(stats.events_by_kind.is_empty());
    }

    #[test]
    fn stats_with_events() {
        let store = EventStore::new();
        store.insert(make_event('a', 1, 1000));
        store.insert(make_event('b', 7000, 2000));
        store.insert(make_event('c', 1, 3000));

        let stats = store.stats();
        assert_eq!(stats.event_count, 3);
        assert_eq!(stats.oldest_event, Some(1000));
        assert_eq!(stats.newest_event, Some(3000));
        assert_eq!(stats.events_by_kind.len(), 2);
        assert_eq!(stats.events_by_kind[&1], 2);
        assert_eq!(stats.events_by_kind[&7000], 1);
    }

    #[test]
    fn stats_serde_round_trip() {
        let store = EventStore::new();
        store.insert(make_event('a', 1, 1000));
        store.insert(make_event('b', 42, 2000));

        let stats = store.stats();
        let json = serde_json::to_string(&stats).unwrap();
        let loaded: StoreStats = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.event_count, stats.event_count);
        assert_eq!(loaded.oldest_event, stats.oldest_event);
        assert_eq!(loaded.newest_event, stats.newest_event);
        assert_eq!(loaded.events_by_kind, stats.events_by_kind);
    }
}
