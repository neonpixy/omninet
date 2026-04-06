import COmnideaFFI
import Foundation

// MARK: - TowerDirectory

/// Swift wrapper around the Rust TowerDirectory (Zeitgeist discovery layer).
///
/// Maintains a directory of known Towers on the network, built from
/// OmniEvents. Supports lookup by pubkey, filtering by community,
/// and querying searchable or harbor Towers.
public final class TowerDirectory: @unchecked Sendable {
    let ptr: OpaquePointer

    public init() {
        ptr = divi_zeitgeist_directory_new()!
    }

    deinit {
        divi_zeitgeist_directory_free(ptr)
    }

    /// Update the directory from a JSON array of OmniEvents.
    ///
    /// Parses the events and upserts Tower profiles accordingly.
    public func update(eventsJSON: String) throws {
        let result = divi_zeitgeist_directory_update(ptr, eventsJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update tower directory")
        }
    }

    /// Look up a Tower by pubkey.
    ///
    /// Returns JSON TowerProfile, or nil if not found.
    public func get(pubkey: String) throws -> String? {
        guard let json = divi_zeitgeist_directory_get(ptr, pubkey) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all Towers in the directory.
    ///
    /// Returns a JSON array of TowerEntry.
    public func all() -> String {
        let json = divi_zeitgeist_directory_all(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all searchable Towers.
    ///
    /// Returns a JSON array of TowerEntry.
    public func searchable() -> String {
        let json = divi_zeitgeist_directory_searchable(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all Harbor Towers.
    ///
    /// Returns a JSON array of TowerEntry.
    public func harbors() -> String {
        let json = divi_zeitgeist_directory_harbors(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all Towers serving a specific community.
    ///
    /// Returns a JSON array of TowerEntry.
    public func forCommunity(_ community: String) -> String {
        let json = divi_zeitgeist_directory_for_community(ptr, community)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// The total number of Towers in the directory.
    public var count: Int {
        Int(divi_zeitgeist_directory_count(ptr))
    }

    /// The number of searchable Towers.
    public var searchableCount: Int {
        Int(divi_zeitgeist_directory_searchable_count(ptr))
    }
}

// MARK: - QueryRouter

/// Swift wrapper around the Rust QueryRouter.
///
/// Routes search queries to the best Towers in a TowerDirectory
/// based on relevance, capacity, and topic coverage.
public final class QueryRouter: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new query router.
    ///
    /// - Parameter configJSON: Optional JSON RouterConfig. Pass nil for defaults.
    public init(configJSON: String? = nil) {
        ptr = divi_zeitgeist_router_new(configJSON)!
    }

    deinit {
        divi_zeitgeist_router_free(ptr)
    }

    /// Route a query to the best Towers in the given directory.
    ///
    /// Returns a JSON array of RoutedTower. Throws on error.
    public func route(query: String, directory: TowerDirectory) throws -> String {
        guard let json = divi_zeitgeist_router_route(ptr, query, directory.ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to route query")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - ZeitgeistMerger

/// Stateless merger for combining search results from multiple Towers.
public enum ZeitgeistMerger {

    /// Merge results from multiple Tower batches into a single ranked response.
    ///
    /// - Parameters:
    ///   - batchesJSON: JSON array of TowerResultBatch.
    ///   - maxResults: Maximum number of results in the merged output.
    /// - Returns: JSON MergedResponse. Throws on error.
    public static func mergeResults(batchesJSON: String, maxResults: UInt32) throws -> String {
        guard let json = divi_zeitgeist_merge_results(batchesJSON, maxResults) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to merge results")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - ZeitgeistCache

/// Swift wrapper around the Rust LocalCache (Zeitgeist query cache).
///
/// Caches search results locally to reduce network round-trips.
/// Supports snapshotting for persistence and restoration.
public final class ZeitgeistCache: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new local cache.
    ///
    /// - Parameter configJSON: Optional JSON CacheConfig. Pass nil for defaults.
    public init(configJSON: String? = nil) {
        ptr = divi_zeitgeist_cache_new(configJSON)!
    }

    /// Internal init from an existing opaque pointer (used by `restore`).
    private init(rawPtr: OpaquePointer) {
        ptr = rawPtr
    }

    deinit {
        divi_zeitgeist_cache_free(ptr)
    }

    /// Look up cached results for a query.
    ///
    /// - Parameters:
    ///   - query: The search query string.
    ///   - now: Current Unix timestamp (for hit tracking).
    /// - Returns: JSON CachedQuery, or nil on cache miss.
    public func get(query: String, now: Int64) -> String? {
        guard let json = divi_zeitgeist_cache_get(ptr, query, now) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Store results for a query in the cache.
    ///
    /// - Parameters:
    ///   - query: The search query string.
    ///   - resultsJSON: JSON array of SearchResult.
    ///   - now: Current Unix timestamp.
    public func put(query: String, resultsJSON: String, now: Int64) throws {
        let result = divi_zeitgeist_cache_put(ptr, query, resultsJSON, now)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to put query in cache")
        }
    }

    /// Remove a query from the cache.
    ///
    /// - Returns: True if the query was cached and removed.
    @discardableResult
    public func remove(query: String) -> Bool {
        divi_zeitgeist_cache_remove(ptr, query)
    }

    /// Clear all cached entries.
    public func clear() {
        divi_zeitgeist_cache_clear(ptr)
    }

    /// The number of cached queries.
    public var count: Int {
        Int(divi_zeitgeist_cache_len(ptr))
    }

    /// Whether the cache is empty.
    public var isEmpty: Bool {
        divi_zeitgeist_cache_is_empty(ptr)
    }

    /// Take a snapshot of the cache for persistence.
    ///
    /// Returns a JSON CacheSnapshot that can be stored and later
    /// restored with `ZeitgeistCache.restore(snapshotJSON:)`.
    public func snapshot() -> String {
        let json = divi_zeitgeist_cache_snapshot(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Restore a cache from a previously taken snapshot.
    ///
    /// - Parameter snapshotJSON: JSON CacheSnapshot.
    /// - Returns: A new ZeitgeistCache, or nil on invalid snapshot.
    public static func restore(snapshotJSON: String) -> ZeitgeistCache? {
        guard let rawPtr = divi_zeitgeist_cache_restore(snapshotJSON) else {
            return nil
        }
        return ZeitgeistCache(rawPtr: rawPtr)
    }
}

// MARK: - TrendTracker

/// Swift wrapper around the Rust TrendTracker (Zeitgeist trending topics).
///
/// Tracks search queries and Tower topic profiles to surface
/// trending topics with time-decay scoring.
public final class TrendTracker: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new trend tracker.
    ///
    /// - Parameter configJSON: Optional JSON TrendConfig. Pass nil for defaults.
    public init(configJSON: String? = nil) {
        ptr = divi_zeitgeist_trends_new(configJSON)!
    }

    deinit {
        divi_zeitgeist_trends_free(ptr)
    }

    /// Record a search query as a trend signal.
    ///
    /// - Parameters:
    ///   - query: The search query string.
    ///   - now: Current Unix timestamp.
    public func recordQuery(_ query: String, now: Int64) throws {
        let result = divi_zeitgeist_trends_record_query(ptr, query, now)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record query trend")
        }
    }

    /// Record topics from a Tower's Semantic Profile.
    ///
    /// - Parameters:
    ///   - topicsJSON: JSON array of topic strings.
    ///   - contentCount: Number of content items backing these topics.
    ///   - now: Current Unix timestamp.
    public func recordTowerTopics(topicsJSON: String, contentCount: UInt64, now: Int64) throws {
        let result = divi_zeitgeist_trends_record_tower_topics(ptr, topicsJSON, contentCount, now)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record tower topics")
        }
    }

    /// Apply time decay to all trends and prune dead ones.
    public func decay() {
        divi_zeitgeist_trends_decay(ptr)
    }

    /// Get the top N trending topics, sorted by score.
    ///
    /// Returns a JSON array of TrendSignal.
    public func top(n: UInt32) -> String {
        let json = divi_zeitgeist_trends_top(ptr, n)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get a specific trend by topic name.
    ///
    /// Returns JSON TrendSignal, or nil if not tracked.
    public func get(topic: String) -> String? {
        guard let json = divi_zeitgeist_trends_get(ptr, topic) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// The number of tracked trends.
    public var count: Int {
        Int(divi_zeitgeist_trends_count(ptr))
    }
}
