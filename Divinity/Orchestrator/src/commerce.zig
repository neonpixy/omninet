// Commerce orchestration — Fortune (Ledger + Treasury + UBI) + Commerce (Cart + Products + Orders).
//
// Your money, your rules. Everything runs on Cool.
// Operations previously here are now served by the pipeline executor.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Module State ──

var ledger: ?*c.FortuneLedger = null;
var treasury: ?*c.FortuneTreasury = null;
var ubi: ?*c.FortuneUbi = null;
var cart: ?*c.CommerceCart = null;

/// Mutex protecting all module-level state.
var mod_mutex: std.Thread.Mutex = .{};

// ── Init Functions ──────────────────────────────────────────────

/// Initialize the ledger.
/// Returns 0 on success.
export fn orch_ledger_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (ledger != null) return 0;
    ledger = c.divi_fortune_ledger_new();
    return if (ledger != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the treasury with a policy.
/// policy_json may be null for default policy.
/// Returns 0 on success.
export fn orch_treasury_init(policy_json: ?[*:0]const u8) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (treasury != null) return 0;
    treasury = c.divi_fortune_treasury_new(policy_json);
    return if (treasury != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize UBI engine.
/// Returns 0 on success.
export fn orch_ubi_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (ubi != null) return 0;
    ubi = c.divi_fortune_ubi_new();
    return if (ubi != null) @as(i32, 0) else @as(i32, -1);
}

/// Create a new cart.
/// Returns 0 on success.
export fn orch_cart_create() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (cart) |old| c.divi_commerce_cart_free(old);
    cart = c.divi_commerce_cart_new();
    return if (cart != null) @as(i32, 0) else @as(i32, -1);
}

// ── Handle Accessors ──────────────────────────────────────────────

/// Get the active FortuneLedger handle. Thread-safe.
pub fn getLedger() ?*c.FortuneLedger {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return ledger;
}

/// Get the active FortuneTreasury handle. Thread-safe.
pub fn getTreasury() ?*c.FortuneTreasury {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return treasury;
}

/// Get the active FortuneUbi handle. Thread-safe.
pub fn getUbi() ?*c.FortuneUbi {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return ubi;
}

/// Get the active CommerceCart handle. Thread-safe.
pub fn getCart() ?*c.CommerceCart {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return cart;
}

// ── Shutdown ──

/// Free all commerce module state.
pub export fn orch_commerce_shutdown() void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (cart) |p| c.divi_commerce_cart_free(p);
    if (ubi) |p| c.divi_fortune_ubi_free(p);
    if (treasury) |p| c.divi_fortune_treasury_free(p);
    if (ledger) |p| c.divi_fortune_ledger_free(p);

    cart = null;
    ubi = null;
    treasury = null;
    ledger = null;
}

// ── Tests ─────────────────────────────────────────────────────

test "ledger init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_commerce_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_ledger_init());
}

test "cart lifecycle" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_commerce_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_cart_create());
}
