// Orchestrator State — the global singleton that holds all opaque handles.
//
// Thread-safe via RwLock: multiple readers (most orch_* calls) proceed
// concurrently. Writers (init, shutdown, handle setters) get exclusive access.
//
// Read path:   state.acquireShared() → read handle → FFI call → state.releaseShared()
// Write path:  state.setKeyring(kr) (acquires exclusive lock internally)
// Lifecycle:   orch_init() and orch_shutdown() acquire exclusive lock

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;

// Module imports for shutdown wiring.
// Circular @import is fine in Zig (lazy resolution) — these modules already import state.zig.
const ai = @import("ai.zig");
const content = @import("content.zig");
const discovery = @import("discovery.zig");
const commerce = @import("commerce.zig");
const governance = @import("governance.zig");
const federation = @import("federation.zig");
const intercom = @import("intercom.zig");
const lingo = @import("lingo.zig");
const reg = @import("registry.zig");
const pipeline = @import("pipeline.zig");

/// All shared handles for the Omnidea orchestrator.
pub const OrchestratorState = struct {
    // ── Core runtime ──
    runtime: ?*c.DiviRuntime = null,

    // ── Identity (written by identity module via setters) ──
    keyring: ?*c.CrownKeyring = null,
    soul: ?*c.CrownSoul = null,

    // ── Storage (written by storage module via setter) ──
    vault: ?*c.DiviVault = null,

    // ── Equipment (created at init) ──
    phone: ?*c.Phone = null,
    email: ?*c.Email = null,
    contacts: ?*c.Contacts = null,
    pager: ?*c.Pager = null,

    // ── Infrastructure (written by infrastructure module via setter) ──
    omnibus: ?*c.Omnibus = null,

    // ── Design (written by theme setter) ──
    theme: ?*c.RegaliaThemeCollection = null,

    // ── Registries (created at init) ──
    schema_registry: ?*c.IdeasSchemaRegistry = null,
    renderer_registry: ?*c.MagicRendererRegistry = null,
    tool_registry: ?*c.MagicToolRegistry = null,

    // ── Lifecycle ──
    initialized: bool = false,
};

/// Global singleton state.
var global: OrchestratorState = .{};

/// RwLock protecting the global state.
/// Multiple readers (shared) can proceed concurrently.
/// Writers (exclusive) block all others — used for init, shutdown, and setters.
var mutex: std.Thread.RwLock = .{};

// ── Thread-safe accessors ──────────────────────────────────────

/// Acquire shared (read) access to the global state.
/// Multiple threads can hold shared access concurrently.
/// The returned pointer is valid until releaseShared() is called.
///
/// IMPORTANT: Do NOT call setters or orch_init/orch_shutdown while
/// holding a shared lock — that would deadlock. Release first.
pub fn acquireShared() *const OrchestratorState {
    mutex.lockShared();
    return &global;
}

/// Release shared access. Must be called after acquireShared().
pub fn releaseShared() void {
    mutex.unlockShared();
}

/// Check initialized flag (quick, acquires and releases shared lock).
pub fn isInitialized() bool {
    mutex.lockShared();
    defer mutex.unlockShared();
    return global.initialized;
}

// ── Thread-safe setters (exclusive lock internally) ─────────────

/// Set the active keyring. Frees any existing keyring.
pub fn setKeyring(kr: ?*c.CrownKeyring) void {
    mutex.lock();
    defer mutex.unlock();
    if (global.keyring) |old| c.divi_crown_keyring_free(old);
    global.keyring = kr;
}

/// Set the active soul. Frees any existing soul.
pub fn setSoul(s: ?*c.CrownSoul) void {
    mutex.lock();
    defer mutex.unlock();
    if (global.soul) |old| c.divi_crown_soul_free(old);
    global.soul = s;
}

/// Set the active vault. Frees any existing vault (locks it first — zeroes keys).
pub fn setVault(v: ?*c.DiviVault) void {
    mutex.lock();
    defer mutex.unlock();
    if (global.vault) |old| c.divi_vault_free(old);
    global.vault = v;
}

/// Set the omnibus instance. Frees any existing instance.
pub fn setOmnibus(o: ?*c.Omnibus) void {
    mutex.lock();
    defer mutex.unlock();
    if (global.omnibus) |old| c.divi_omnibus_free(old);
    global.omnibus = o;
}

/// Set the theme collection. Frees any existing collection.
pub fn setTheme(t: ?*c.RegaliaThemeCollection) void {
    mutex.lock();
    defer mutex.unlock();
    if (global.theme) |old| c.divi_regalia_theme_collection_free(old);
    global.theme = t;
}

/// Set the phone instance (used by intercom fallback init).
/// Frees any existing phone before setting the new one.
pub fn setPhone(p: ?*c.Phone) void {
    mutex.lock();
    defer mutex.unlock();
    if (global.phone) |old| c.divi_phone_free(old);
    global.phone = p;
}

// ── Non-thread-safe access (tests only) ──────────────────────

/// Direct mutable access. NOT thread-safe.
/// Use only in tests or guaranteed single-threaded contexts.
pub fn get() *OrchestratorState {
    return &global;
}

// ── Public C API ──────────────────────────────────────────────

/// Initialize the orchestrator. Creates the async runtime, Equipment instances,
/// and default registries. Must be called once before any other orch_* function.
/// Returns 0 on success, negative on error.
pub export fn orch_init() i32 {
    mutex.lock();
    defer mutex.unlock();

    if (global.initialized) return 0;

    // Async runtime (panics on failure — tokio must be available)
    global.runtime = c.divi_runtime_new();

    // Equipment
    global.phone = c.divi_phone_new();
    if (global.phone == null) {
        cleanup_partial();
        return -1;
    }

    global.email = c.divi_email_new();
    if (global.email == null) {
        cleanup_partial();
        return -2;
    }

    global.contacts = c.divi_contacts_new();
    if (global.contacts == null) {
        cleanup_partial();
        return -3;
    }

    global.pager = c.divi_pager_new();
    if (global.pager == null) {
        cleanup_partial();
        return -4;
    }

    // Registries
    global.schema_registry = c.divi_ideas_schema_registry_new();
    global.renderer_registry = c.divi_magic_renderer_registry_new_with_all();
    global.tool_registry = c.divi_magic_tool_registry_new_default();

    global.initialized = true;

    // Operation registry + pipeline (no lock contention — they use their own locks)
    _ = reg.init();
    content.registerContentOps();
    _ = pipeline.orch_pipeline_init();

    return 0;
}

/// Shut down the orchestrator. Frees all handles in reverse creation order.
/// Blocks until all in-flight shared readers complete.
/// Safe to call multiple times.
///
/// Module shutdowns are called BEFORE acquiring the exclusive lock on global
/// state. Each module shutdown acquires its own mod_mutex independently —
/// this avoids lock ordering issues. Module shutdowns are idempotent
/// (null-check handles before freeing), so calling them when modules were
/// never initialized is safe.
pub export fn orch_shutdown() void {
    // Pipeline + registry first (they depend on registry being available)
    pipeline.orch_pipeline_shutdown();
    reg.deinit();

    // Shut down modules in reverse dependency order.
    // AI and Discovery have no deps on other modules.
    // Commerce depends on Fortune handles only (self-contained).
    // Governance depends on Polity/Bulwark/Jail handles only (self-contained).
    // Intercom last — it depends on the shared Phone (owned by global state).
    ai.orch_ai_shutdown();
    discovery.orch_discovery_shutdown();
    commerce.orch_commerce_shutdown();
    governance.orch_governance_shutdown();
    federation.orch_federation_shutdown();
    lingo.orch_lingo_shutdown();
    intercom.orch_intercom_shutdown();

    mutex.lock();
    defer mutex.unlock();

    if (!global.initialized) return;

    // Registries
    if (global.tool_registry) |p| c.divi_magic_tool_registry_free(p);
    if (global.renderer_registry) |p| c.divi_magic_renderer_registry_free(p);
    if (global.schema_registry) |p| c.divi_ideas_schema_registry_free(p);

    // Design
    if (global.theme) |p| c.divi_regalia_theme_collection_free(p);

    // Infrastructure
    if (global.omnibus) |p| c.divi_omnibus_free(p);

    // Equipment (shutdown contacts before freeing)
    if (global.pager) |p| c.divi_pager_free(p);
    if (global.contacts) |p| {
        c.divi_contacts_shutdown_all(p);
        c.divi_contacts_free(p);
    }
    if (global.email) |p| c.divi_email_free(p);
    if (global.phone) |p| c.divi_phone_free(p);

    // Storage (locks before dropping — zeroes keys from memory)
    if (global.vault) |p| c.divi_vault_free(p);

    // Identity
    if (global.soul) |p| c.divi_crown_soul_free(p);
    if (global.keyring) |p| c.divi_crown_keyring_free(p);

    // Runtime
    if (global.runtime) |p| c.divi_runtime_free(p);

    // Reset all fields
    global = .{};
}

/// Check if the orchestrator is initialized.
export fn orch_is_initialized() bool {
    return isInitialized();
}

/// Get the last error message from the FFI layer.
/// Returns null if no error. Do NOT free the returned pointer —
/// it is valid only until the next FFI call on this thread.
export fn orch_last_error() ?[*:0]const u8 {
    const err = c.divi_last_error();
    if (err == null) return null;
    return err;
}

// ── Internal ──────────────────────────────────────────────────

/// Clean up partially initialized state (for error paths in orch_init).
/// Caller must hold exclusive lock.
fn cleanup_partial() void {
    if (global.pager) |p| c.divi_pager_free(p);
    if (global.contacts) |p| c.divi_contacts_free(p);
    if (global.email) |p| c.divi_email_free(p);
    if (global.phone) |p| c.divi_phone_free(p);
    if (global.runtime) |p| c.divi_runtime_free(p);
    global = .{};
}

// ── Tests ─────────────────────────────────────────────────────

test "init and shutdown" {
    try std.testing.expectEqual(@as(i32, 0), orch_init());
    try std.testing.expect(global.initialized);
    try std.testing.expect(global.runtime != null);
    try std.testing.expect(global.phone != null);
    try std.testing.expect(global.email != null);
    try std.testing.expect(global.contacts != null);
    try std.testing.expect(global.pager != null);
    orch_shutdown();
    try std.testing.expect(!global.initialized);
}

test "double init is idempotent" {
    try std.testing.expectEqual(@as(i32, 0), orch_init());
    try std.testing.expectEqual(@as(i32, 0), orch_init());
    orch_shutdown();
}

test "double shutdown is safe" {
    try std.testing.expectEqual(@as(i32, 0), orch_init());
    orch_shutdown();
    orch_shutdown(); // should not crash
}

test "isInitialized is thread-safe" {
    try std.testing.expect(!isInitialized());
    try std.testing.expectEqual(@as(i32, 0), orch_init());
    try std.testing.expect(isInitialized());
    orch_shutdown();
    try std.testing.expect(!isInitialized());
}

test "acquireShared returns valid state" {
    try std.testing.expectEqual(@as(i32, 0), orch_init());
    defer orch_shutdown();

    const s = acquireShared();
    defer releaseShared();
    try std.testing.expect(s.initialized);
    try std.testing.expect(s.phone != null);
}
