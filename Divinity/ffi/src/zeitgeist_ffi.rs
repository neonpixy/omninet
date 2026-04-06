//! FFI bridge for Zeitgeist — discovery & culture.
//!
//! Exposes TowerDirectory, QueryRouter, ResultMerger, LocalCache, and
//! TrendTracker to C callers via opaque pointers and JSON round-trip.

use std::ffi::c_char;
use std::sync::Mutex;

use globe::event::OmniEvent;
use zeitgeist::cache::{CacheConfig, CacheSnapshot};
use zeitgeist::merger::TowerResultBatch;
use zeitgeist::router::RouterConfig;
use zeitgeist::trending::TrendConfig;
use zeitgeist::{LocalCache, QueryRouter, ResultMerger, TowerDirectory, TrendSignal, TrendTracker};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// TowerDirectory — opaque pointer (10 functions)
// ===================================================================

/// Opaque wrapper around `TowerDirectory` for FFI.
pub struct ZeitgeistDirectory(pub(crate) Mutex<TowerDirectory>);

/// Create a new empty Tower directory.
///
/// Free with `divi_zeitgeist_directory_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_zeitgeist_directory_new() -> *mut ZeitgeistDirectory {
    Box::into_raw(Box::new(ZeitgeistDirectory(Mutex::new(
        TowerDirectory::new(),
    ))))
}

/// Free a Tower directory.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_free(ptr: *mut ZeitgeistDirectory) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Update the directory from a JSON array of `OmniEvent`.
///
/// Returns 0 on success, -1 on error (invalid JSON).
///
/// # Safety
/// `dir` must be a valid pointer. `events_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_update(
    dir: *const ZeitgeistDirectory,
    events_json: *const c_char,
) -> i32 {
    clear_last_error();

    let dir = unsafe { &*dir };
    let Some(ej) = c_str_to_str(events_json) else {
        set_last_error("divi_zeitgeist_directory_update: invalid events_json");
        return -1;
    };

    let events: Vec<OmniEvent> = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_zeitgeist_directory_update: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&dir.0);
    guard.update(&events);
    0
}

/// Look up a Tower by pubkey.
///
/// Returns JSON `TowerEntry`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `dir` must be a valid pointer. `pubkey` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_get(
    dir: *const ZeitgeistDirectory,
    pubkey: *const c_char,
) -> *mut c_char {
    let dir = unsafe { &*dir };
    let Some(pk) = c_str_to_str(pubkey) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&dir.0);
    match guard.get(pk) {
        Some(entry) => json_to_c(entry),
        None => std::ptr::null_mut(),
    }
}

/// Get all Towers in the directory.
///
/// Returns JSON array of `TowerEntry`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `dir` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_all(
    dir: *const ZeitgeistDirectory,
) -> *mut c_char {
    let dir = unsafe { &*dir };
    let guard = lock_or_recover(&dir.0);
    let all: Vec<_> = guard.all_towers();
    json_to_c(&all)
}

/// Get all searchable Towers.
///
/// Returns JSON array of `TowerEntry`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `dir` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_searchable(
    dir: *const ZeitgeistDirectory,
) -> *mut c_char {
    let dir = unsafe { &*dir };
    let guard = lock_or_recover(&dir.0);
    let searchable: Vec<_> = guard.searchable_towers();
    json_to_c(&searchable)
}

/// Get all Harbor Towers.
///
/// Returns JSON array of `TowerEntry`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `dir` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_harbors(
    dir: *const ZeitgeistDirectory,
) -> *mut c_char {
    let dir = unsafe { &*dir };
    let guard = lock_or_recover(&dir.0);
    let harbors: Vec<_> = guard.harbors();
    json_to_c(&harbors)
}

/// Get all Towers serving a specific community.
///
/// Returns JSON array of `TowerEntry`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `dir` must be a valid pointer. `community` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_for_community(
    dir: *const ZeitgeistDirectory,
    community: *const c_char,
) -> *mut c_char {
    let dir = unsafe { &*dir };
    let Some(c) = c_str_to_str(community) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&dir.0);
    let towers: Vec<_> = guard.towers_for_community(c);
    json_to_c(&towers)
}

/// Get the total number of Towers in the directory.
///
/// # Safety
/// `dir` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_count(
    dir: *const ZeitgeistDirectory,
) -> u32 {
    let dir = unsafe { &*dir };
    let guard = lock_or_recover(&dir.0);
    guard.count() as u32
}

/// Get the number of searchable Towers.
///
/// # Safety
/// `dir` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_directory_searchable_count(
    dir: *const ZeitgeistDirectory,
) -> u32 {
    let dir = unsafe { &*dir };
    let guard = lock_or_recover(&dir.0);
    guard.searchable_count() as u32
}

// ===================================================================
// QueryRouter — opaque pointer (3 functions)
// ===================================================================

/// Opaque wrapper around `QueryRouter` for FFI.
pub struct ZeitgeistRouter(pub(crate) Mutex<QueryRouter>);

/// Create a new query router.
///
/// `config_json` is a JSON `RouterConfig`, or null for defaults.
/// Free with `divi_zeitgeist_router_free`.
///
/// # Safety
/// `config_json` may be null (uses default config).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_router_new(
    config_json: *const c_char,
) -> *mut ZeitgeistRouter {
    let config = if config_json.is_null() {
        RouterConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        RouterConfig::default()
    };

    let router = QueryRouter::with_config(config);
    Box::into_raw(Box::new(ZeitgeistRouter(Mutex::new(router))))
}

/// Free a query router.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_router_free(ptr: *mut ZeitgeistRouter) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Route a query to the best Towers.
///
/// Locks `dir` first, then `router`, to prevent deadlocks (consistent ordering).
/// Returns JSON array of `RoutedTower`. Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// `router` and `dir` must be valid pointers. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_router_route(
    router: *const ZeitgeistRouter,
    query: *const c_char,
    dir: *const ZeitgeistDirectory,
) -> *mut c_char {
    clear_last_error();

    let router_ref = unsafe { &*router };
    let dir_ref = unsafe { &*dir };

    let Some(q) = c_str_to_str(query) else {
        set_last_error("divi_zeitgeist_router_route: invalid query");
        return std::ptr::null_mut();
    };

    // Lock directory first, then router — consistent ordering prevents deadlocks.
    let dir_guard = lock_or_recover(&dir_ref.0);
    let router_guard = lock_or_recover(&router_ref.0);

    match router_guard.route(q, &dir_guard) {
        Ok(routed) => json_to_c(&routed),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// ResultMerger — stateless (1 function)
// ===================================================================

/// Merge results from multiple Tower batches.
///
/// `batches_json` is a JSON array of `TowerResultBatch`.
/// Returns JSON `MergedResponse`. Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// `batches_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_merge_results(
    batches_json: *const c_char,
    max_results: u32,
) -> *mut c_char {
    clear_last_error();

    let Some(bj) = c_str_to_str(batches_json) else {
        set_last_error("divi_zeitgeist_merge_results: invalid batches_json");
        return std::ptr::null_mut();
    };

    let batches: Vec<TowerResultBatch> = match serde_json::from_str(bj) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(format!("divi_zeitgeist_merge_results: {e}"));
            return std::ptr::null_mut();
        }
    };

    let merger = ResultMerger::with_max_results(max_results as usize);
    let response = merger.merge(batches);
    json_to_c(&response)
}

// ===================================================================
// LocalCache — opaque pointer (10 functions)
// ===================================================================

/// Opaque wrapper around `LocalCache` for FFI.
pub struct ZeitgeistCache(pub(crate) Mutex<LocalCache>);

/// Create a new local cache.
///
/// `config_json` is a JSON `CacheConfig`, or null for defaults.
/// Free with `divi_zeitgeist_cache_free`.
///
/// # Safety
/// `config_json` may be null (uses default config).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_new(
    config_json: *const c_char,
) -> *mut ZeitgeistCache {
    let config = if config_json.is_null() {
        CacheConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        CacheConfig::default()
    };

    let cache = LocalCache::with_config(config);
    Box::into_raw(Box::new(ZeitgeistCache(Mutex::new(cache))))
}

/// Free a local cache.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_free(ptr: *mut ZeitgeistCache) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Look up cached results for a query.
///
/// `now` is the current Unix timestamp (used for hit tracking).
/// Returns JSON `CachedQuery`, or null if not cached.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `cache` must be a valid pointer. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_get(
    cache: *const ZeitgeistCache,
    query: *const c_char,
    now: i64,
) -> *mut c_char {
    let cache = unsafe { &*cache };
    let Some(q) = c_str_to_str(query) else {
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&cache.0);
    match guard.get(q, now) {
        Some(cached) => json_to_c(cached),
        None => std::ptr::null_mut(),
    }
}

/// Store results for a query in the cache.
///
/// `results_json` is a JSON array of `SearchResult`. `now` is the current Unix timestamp.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `cache` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_put(
    cache: *const ZeitgeistCache,
    query: *const c_char,
    results_json: *const c_char,
    now: i64,
) -> i32 {
    clear_last_error();

    let cache = unsafe { &*cache };
    let Some(q) = c_str_to_str(query) else {
        set_last_error("divi_zeitgeist_cache_put: invalid query");
        return -1;
    };

    let Some(rj) = c_str_to_str(results_json) else {
        set_last_error("divi_zeitgeist_cache_put: invalid results_json");
        return -1;
    };

    let results: Vec<magical_index::SearchResult> = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_zeitgeist_cache_put: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&cache.0);
    guard.put(q, results, now);
    0
}

/// Remove a query from the cache.
///
/// Returns true if the query was cached and removed.
///
/// # Safety
/// `cache` must be a valid pointer. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_remove(
    cache: *const ZeitgeistCache,
    query: *const c_char,
) -> bool {
    let cache = unsafe { &*cache };
    let Some(q) = c_str_to_str(query) else {
        return false;
    };

    let mut guard = lock_or_recover(&cache.0);
    guard.remove(q)
}

/// Clear all cached entries.
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_clear(cache: *const ZeitgeistCache) {
    let cache = unsafe { &*cache };
    let mut guard = lock_or_recover(&cache.0);
    guard.clear();
}

/// Get the number of cached queries.
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_len(cache: *const ZeitgeistCache) -> u32 {
    let cache = unsafe { &*cache };
    let guard = lock_or_recover(&cache.0);
    guard.len() as u32
}

/// Check whether the cache is empty.
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_is_empty(cache: *const ZeitgeistCache) -> bool {
    let cache = unsafe { &*cache };
    let guard = lock_or_recover(&cache.0);
    guard.is_empty()
}

/// Take a snapshot of the cache for persistence.
///
/// Returns JSON `CacheSnapshot`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_snapshot(
    cache: *const ZeitgeistCache,
) -> *mut c_char {
    let cache = unsafe { &*cache };
    let guard = lock_or_recover(&cache.0);
    let snap = guard.snapshot();
    json_to_c(&snap)
}

/// Restore a cache from a snapshot.
///
/// `snapshot_json` is a JSON `CacheSnapshot`.
/// Returns a new cache pointer. Caller must free via `divi_zeitgeist_cache_free`.
/// Returns null on error.
///
/// # Safety
/// `snapshot_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_cache_restore(
    snapshot_json: *const c_char,
) -> *mut ZeitgeistCache {
    let Some(sj) = c_str_to_str(snapshot_json) else {
        set_last_error("divi_zeitgeist_cache_restore: invalid snapshot_json");
        return std::ptr::null_mut();
    };

    let snapshot: CacheSnapshot = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_zeitgeist_cache_restore: {e}"));
            return std::ptr::null_mut();
        }
    };

    let cache = LocalCache::from_snapshot(snapshot);
    Box::into_raw(Box::new(ZeitgeistCache(Mutex::new(cache))))
}

// ===================================================================
// TrendTracker — opaque pointer (8 functions)
// ===================================================================

/// Opaque wrapper around `TrendTracker` for FFI.
pub struct ZeitgeistTrends(pub(crate) Mutex<TrendTracker>);

/// Create a new trend tracker.
///
/// `config_json` is a JSON `TrendConfig`, or null for defaults.
/// Free with `divi_zeitgeist_trends_free`.
///
/// # Safety
/// `config_json` may be null (uses default config).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_new(
    config_json: *const c_char,
) -> *mut ZeitgeistTrends {
    let config = if config_json.is_null() {
        TrendConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        TrendConfig::default()
    };

    let tracker = TrendTracker::with_config(config);
    Box::into_raw(Box::new(ZeitgeistTrends(Mutex::new(tracker))))
}

/// Free a trend tracker.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_free(ptr: *mut ZeitgeistTrends) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Record a search query as a trend signal.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `trends` must be a valid pointer. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_record_query(
    trends: *const ZeitgeistTrends,
    query: *const c_char,
    now: i64,
) -> i32 {
    clear_last_error();

    let trends = unsafe { &*trends };
    let Some(q) = c_str_to_str(query) else {
        set_last_error("divi_zeitgeist_trends_record_query: invalid query");
        return -1;
    };

    let mut guard = lock_or_recover(&trends.0);
    guard.record_query(q, now);
    0
}

/// Record topics from a Tower's Semantic Profile.
///
/// `topics_json` is a JSON array of strings.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `trends` must be a valid pointer. `topics_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_record_tower_topics(
    trends: *const ZeitgeistTrends,
    topics_json: *const c_char,
    content_count: u64,
    now: i64,
) -> i32 {
    clear_last_error();

    let trends = unsafe { &*trends };
    let Some(tj) = c_str_to_str(topics_json) else {
        set_last_error("divi_zeitgeist_trends_record_tower_topics: invalid topics_json");
        return -1;
    };

    let topics: Vec<String> = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!(
                "divi_zeitgeist_trends_record_tower_topics: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&trends.0);
    guard.record_tower_topics(&topics, content_count, now);
    0
}

/// Apply time decay to all trends and prune dead ones.
///
/// # Safety
/// `trends` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_decay(trends: *const ZeitgeistTrends) {
    let trends = unsafe { &*trends };
    let mut guard = lock_or_recover(&trends.0);
    guard.decay();
}

/// Get the top N trending topics, sorted by score.
///
/// Returns JSON array of `TrendSignal`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `trends` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_top(
    trends: *const ZeitgeistTrends,
    n: u32,
) -> *mut c_char {
    let trends = unsafe { &*trends };
    let guard = lock_or_recover(&trends.0);
    let top: Vec<&TrendSignal> = guard.top(n as usize);
    json_to_c(&top)
}

/// Get a specific trend by topic name.
///
/// Returns JSON `TrendSignal`, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `trends` must be a valid pointer. `topic` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_get(
    trends: *const ZeitgeistTrends,
    topic: *const c_char,
) -> *mut c_char {
    let trends = unsafe { &*trends };
    let Some(t) = c_str_to_str(topic) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&trends.0);
    match guard.get(t) {
        Some(signal) => json_to_c(signal),
        None => std::ptr::null_mut(),
    }
}

/// Get the number of tracked trends.
///
/// # Safety
/// `trends` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_zeitgeist_trends_count(
    trends: *const ZeitgeistTrends,
) -> u32 {
    let trends = unsafe { &*trends };
    let guard = lock_or_recover(&trends.0);
    guard.count() as u32
}
