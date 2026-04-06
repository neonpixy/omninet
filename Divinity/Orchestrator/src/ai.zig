// AI orchestration — Advisor (cognitive loop) + Oracle (guidance + onboarding).
//
// The wise counselor and the source of truth.
// Operations previously here are now served by the pipeline executor.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Module State ──

var advisor_loop: ?*c.AdvisorLoop = null;
var advisor_store: ?*c.AdvisorStore = null;
var advisor_router: ?*c.AdvisorRouter = null;
var advisor_skills: ?*c.AdvisorSkills = null;
var oracle: ?*c.OracleWorkflowRegistry = null;

/// Mutex protecting all module-level state.
var mod_mutex: std.Thread.Mutex = .{};

// ── Init Functions ──────────────────────────────────────────────

/// Create a new cognitive loop.
/// config_json may be null for defaults. session_id is a UUID.
/// Returns 0 on success.
export fn orch_advisor_create(config_json: ?[*:0]const u8, session_id: [*:0]const u8) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (advisor_loop) |old| c.divi_advisor_loop_free(old);
    advisor_loop = c.divi_advisor_loop_new(config_json, session_id);
    return if (advisor_loop != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the thought store.
/// Returns 0 on success.
export fn orch_advisor_store_init(clipboard_max: usize) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (advisor_store != null) return 0;
    advisor_store = c.divi_advisor_store_new(clipboard_max);
    return if (advisor_store != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the provider router.
/// Returns 0 on success.
export fn orch_advisor_router_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (advisor_router != null) return 0;
    advisor_router = c.divi_advisor_router_new();
    return if (advisor_router != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the skill registry.
/// Returns 0 on success.
export fn orch_advisor_skills_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (advisor_skills != null) return 0;
    advisor_skills = c.divi_advisor_skills_new();
    return if (advisor_skills != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the Oracle workflow registry.
/// Returns 0 on success.
export fn orch_oracle_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (oracle != null) return 0;
    oracle = c.divi_oracle_registry_new();
    return if (oracle != null) @as(i32, 0) else @as(i32, -1);
}

// ── Handle Accessors ──────────────────────────────────────────────

/// Get the active AdvisorLoop handle. Thread-safe.
pub fn getAdvisorLoop() ?*c.AdvisorLoop {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return advisor_loop;
}

/// Get the active AdvisorStore handle. Thread-safe.
pub fn getAdvisorStore() ?*c.AdvisorStore {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return advisor_store;
}

/// Get the active AdvisorRouter handle. Thread-safe.
pub fn getAdvisorRouter() ?*c.AdvisorRouter {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return advisor_router;
}

/// Get the active AdvisorSkills handle. Thread-safe.
pub fn getAdvisorSkills() ?*c.AdvisorSkills {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return advisor_skills;
}

/// Get the active OracleWorkflowRegistry handle. Thread-safe.
pub fn getOracleRegistry() ?*c.OracleWorkflowRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return oracle;
}

// ── Shutdown ──

/// Free all AI module state.
pub export fn orch_ai_shutdown() void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (advisor_skills) |p| c.divi_advisor_skills_free(p);
    if (advisor_router) |p| c.divi_advisor_router_free(p);
    if (advisor_store) |p| c.divi_advisor_store_free(p);
    if (advisor_loop) |p| c.divi_advisor_loop_free(p);
    if (oracle) |p| c.divi_oracle_registry_free(p);

    advisor_skills = null;
    advisor_router = null;
    advisor_store = null;
    advisor_loop = null;
    oracle = null;
}

// ── Tests ─────────────────────────────────────────────────────

test "advisor skills init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_ai_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_advisor_skills_init());
}
