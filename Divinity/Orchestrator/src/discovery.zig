// Discovery orchestration — Zeitgeist (search + trends + cache) + Yoke (history + versioning).
//
// Finding things and tracking their provenance.
// Operations previously here are now served by the pipeline executor.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Module State ──

var directory: ?*c.ZeitgeistDirectory = null;
var router: ?*c.ZeitgeistRouter = null;
var cache: ?*c.ZeitgeistCache = null;
var trends: ?*c.ZeitgeistTrends = null;
var health_history: ?*c.UndercraftHistory = null;

/// Mutex protecting all module-level state.
var mod_mutex: std.Thread.Mutex = .{};

// ── Init Functions ──────────────────────────────────────────────

/// Initialize the Tower directory.
/// Returns 0 on success.
export fn orch_directory_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (directory != null) return 0;
    directory = c.divi_zeitgeist_directory_new();
    return if (directory != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the trend tracker.
/// config_json may be null for defaults.
/// Returns 0 on success.
export fn orch_trends_init(config_json: ?[*:0]const u8) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (trends != null) return 0;
    trends = c.divi_zeitgeist_trends_new(config_json);
    return if (trends != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the search cache.
/// config_json may be null for defaults.
/// Returns 0 on success.
export fn orch_cache_init(config_json: ?[*:0]const u8) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (cache != null) return 0;
    cache = c.divi_zeitgeist_cache_new(config_json);
    return if (cache != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the query router.
/// config_json may be null for defaults.
/// Returns 0 on success.
export fn orch_router_init(config_json: ?[*:0]const u8) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (router != null) return 0;
    router = c.divi_zeitgeist_router_new(config_json);
    return if (router != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the health history buffer.
/// Returns 0 on success.
export fn orch_health_history_init(capacity: usize) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (health_history != null) return 0;
    health_history = c.divi_undercroft_history_new(capacity);
    return if (health_history != null) @as(i32, 0) else @as(i32, -1);
}

// ── Handle Accessors ──────────────────────────────────────────────

/// Get the active ZeitgeistDirectory handle. Thread-safe.
pub fn getDirectory() ?*c.ZeitgeistDirectory {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return directory;
}

/// Get the active ZeitgeistRouter handle. Thread-safe.
pub fn getRouter() ?*c.ZeitgeistRouter {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return router;
}

/// Get the active ZeitgeistCache handle. Thread-safe.
pub fn getCache() ?*c.ZeitgeistCache {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return cache;
}

/// Get the active ZeitgeistTrends handle. Thread-safe.
pub fn getTrends() ?*c.ZeitgeistTrends {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return trends;
}

/// Get the active UndercraftHistory handle. Thread-safe.
pub fn getHealthHistory() ?*c.UndercraftHistory {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return health_history;
}

// ── Shutdown ──

/// Free all discovery module state.
pub export fn orch_discovery_shutdown() void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (health_history) |p| c.divi_undercroft_history_free(p);
    if (trends) |p| c.divi_zeitgeist_trends_free(p);
    if (cache) |p| c.divi_zeitgeist_cache_free(p);
    if (router) |p| c.divi_zeitgeist_router_free(p);
    if (directory) |p| c.divi_zeitgeist_directory_free(p);

    health_history = null;
    trends = null;
    cache = null;
    router = null;
    directory = null;
}

// ── Tests ─────────────────────────────────────────────────────

test "directory init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_discovery_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_directory_init());
}

test "health history init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_discovery_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_health_history_init(100));
}
