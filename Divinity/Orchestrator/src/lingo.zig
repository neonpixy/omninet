// Lingo orchestration — Babel handle management.
//
// Babel obfuscates/deobfuscates text using Unicode symbols seeded by a
// vocabulary key (typically from Vault). The pipeline uses Babel as a
// mandatory modifier — every operation's text output gets processed
// through Babel when active.
//
// One Babel handle is created on init and lives in module-level state.
// Encode/decode operations delegate to the underlying Rust FFI.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Module State ──

/// The active Babel instance. Created by `orch_lingo_init`, freed by
/// `orch_lingo_shutdown`. All encode/decode operations use this handle.
var babel: ?*c.LingoBabel = null;

/// Mutex protecting all module-level state.
var mod_mutex: std.Thread.Mutex = .{};

/// Whether the module has been successfully initialized.
var initialized: bool = false;

// ── Init / Shutdown ──────────────────────────────────────────────

/// Initialize the Lingo module with a vocabulary seed.
///
/// `seed_ptr` must point to `seed_len` bytes of seed data (typically
/// 32 bytes from Vault's vocabulary seed or an ECDH shared secret).
/// A null `seed_ptr` or zero `seed_len` is an error (returns -1).
///
/// Idempotent — calling again after a successful init returns 0
/// without creating a second handle. To reinitialize with a different
/// seed, call `orch_lingo_shutdown()` first.
///
/// Returns 0 on success, -1 on failure.
pub export fn orch_lingo_init(seed_ptr: ?[*]const u8, seed_len: usize) callconv(.c) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (initialized) return 0;

    // Null seed or zero length is invalid — Babel needs real entropy.
    if (seed_ptr == null or seed_len == 0) return -1;

    babel = c.divi_lingo_babel_new(seed_ptr, seed_len);
    if (babel == null) return -1;

    initialized = true;
    return 0;
}

/// Shut down the Lingo module. Frees the Babel handle and resets state.
///
/// Safe to call multiple times (idempotent). Safe to call even if
/// `orch_lingo_init` was never called.
pub export fn orch_lingo_shutdown() callconv(.c) void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (babel) |p| c.divi_lingo_babel_free(p);
    babel = null;
    initialized = false;
}

// ── Handle Accessor ──────────────────────────────────────────────

/// Get the active Babel handle for use by the pipeline executor or
/// other modules. Returns null if the module has not been initialized.
///
/// Thread-safe — acquires and releases the module mutex.
pub fn getBabel() ?*c.LingoBabel {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return babel;
}

// ── Encode / Decode ──────────────────────────────────────────────

/// Encode plaintext into Babel symbols (hardened, non-deterministic).
///
/// `text` must be a valid null-terminated C string.
/// Returns a null-terminated encoded string on success. The caller
/// must free the returned string with `divi_free_string`.
/// Returns null if the module is not initialized or encoding fails.
pub export fn orch_lingo_encode(text: ?[*:0]const u8) callconv(.c) ?[*:0]u8 {
    if (text == null) return null;

    const handle = getBabel() orelse return null;
    return c.divi_lingo_babel_encode(handle, text);
}

/// Decode Babel symbols back into plaintext.
///
/// `encoded` must be a valid null-terminated C string of Babel symbols.
/// Returns a null-terminated decoded string on success. The caller
/// must free the returned string with `divi_free_string`.
/// Returns null if the module is not initialized or decoding fails.
pub export fn orch_lingo_decode(encoded: ?[*:0]const u8) callconv(.c) ?[*:0]u8 {
    if (encoded == null) return null;

    const handle = getBabel() orelse return null;
    return c.divi_lingo_babel_decode(handle, encoded);
}

/// Decode Babel symbols back into plaintext using language-aware token
/// rejoining for better results with specific languages.
///
/// `encoded` must be a valid null-terminated C string of Babel symbols.
/// `language` must be a valid null-terminated C string identifying the
/// source language (e.g. "en", "ja", "zh").
/// Returns a null-terminated decoded string on success. The caller
/// must free the returned string with `divi_free_string`.
/// Returns null if the module is not initialized or decoding fails.
pub export fn orch_lingo_decode_for_language(encoded: ?[*:0]const u8, language: ?[*:0]const u8) callconv(.c) ?[*:0]u8 {
    if (encoded == null or language == null) return null;

    const handle = getBabel() orelse return null;
    return c.divi_lingo_babel_decode_for_language(handle, encoded, language);
}

// ── Tests ─────────────────────────────────────────────────────

test "shutdown without init is safe" {
    orch_lingo_shutdown(); // no-op, must not crash
}

test "double init is idempotent" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_lingo_shutdown();
        state.orch_shutdown();
    }

    const seed = [_]u8{ 0x01, 0x02, 0x03, 0x04 } ** 8; // 32 bytes
    try std.testing.expectEqual(@as(i32, 0), orch_lingo_init(&seed, seed.len));
    try std.testing.expectEqual(@as(i32, 0), orch_lingo_init(&seed, seed.len)); // idempotent
}

test "init with null seed fails" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_lingo_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, -1), orch_lingo_init(null, 0));
    try std.testing.expectEqual(@as(i32, -1), orch_lingo_init(null, 32));
}

test "init with zero length fails" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_lingo_shutdown();
        state.orch_shutdown();
    }

    const seed = [_]u8{0x42} ** 32;
    try std.testing.expectEqual(@as(i32, -1), orch_lingo_init(&seed, 0));
}

test "getBabel returns null before init" {
    // Make sure no leftover state from other tests
    orch_lingo_shutdown();
    try std.testing.expect(getBabel() == null);
}

test "getBabel returns handle after init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_lingo_shutdown();
        state.orch_shutdown();
    }

    const seed = [_]u8{0xAB} ** 32;
    try std.testing.expectEqual(@as(i32, 0), orch_lingo_init(&seed, seed.len));
    try std.testing.expect(getBabel() != null);
}

test "encode returns null when not initialized" {
    orch_lingo_shutdown();
    try std.testing.expect(orch_lingo_encode("hello") == null);
}

test "decode returns null when not initialized" {
    orch_lingo_shutdown();
    try std.testing.expect(orch_lingo_decode("hello") == null);
}

test "encode and decode round-trip" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_lingo_shutdown();
        state.orch_shutdown();
    }

    const seed = [_]u8{0xDE} ** 32;
    try std.testing.expectEqual(@as(i32, 0), orch_lingo_init(&seed, seed.len));

    const plaintext: [*:0]const u8 = "hello world";
    const encoded = orch_lingo_encode(plaintext);
    if (encoded) |enc| {
        defer c.divi_free_string(enc);

        const decoded = orch_lingo_decode(enc);
        if (decoded) |dec| {
            defer c.divi_free_string(dec);

            // Decoded text should match original
            const dec_slice = std.mem.span(dec);
            try std.testing.expectEqualStrings("hello world", dec_slice);
        } else {
            return error.TestUnexpectedResult; // decode should not return null
        }
    } else {
        return error.TestUnexpectedResult; // encode should not return null
    }
}
