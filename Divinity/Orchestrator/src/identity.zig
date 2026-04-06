// Identity orchestration — composes Crown + Sentinal FFI calls
// into smart identity lifecycle operations.
//
// Creates/imports identities, manages the Soul (profile), handles signing.
// Uses shared state for keyring and soul handles.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

/// Result of creating or importing an identity.
pub const IdentityResult = extern struct {
    /// The recovery phrase (24 words, space-separated). Caller must free with divi_free_string.
    phrase: ?[*:0]u8,
    /// The public key (crown_id). Caller must free with divi_free_string.
    crown_id: ?[*:0]u8,
    /// 0 = success, negative = error.
    status: i32,
};

// ── Public C API ──────────────────────────────────────────────

/// Create a new identity from scratch.
///
/// Composes: Sentinal recovery_generate → recovery_to_seed → derive_identity_key
///           → Crown recover_from_secret
///
/// Stores the keyring in shared state. Caller must free phrase and crown_id
/// with divi_free_string.
export fn orch_create_identity() IdentityResult {
    if (!state.isInitialized()) return .{ .phrase = null, .crown_id = null, .status = -1 };

    // Generate 24-word recovery phrase
    const phrase_ptr: ?[*:0]u8 = c.divi_sentinal_recovery_generate();
    if (phrase_ptr == null) return .{ .phrase = null, .crown_id = null, .status = -2 };

    // Derive seed from phrase (no passphrase)
    var seed_ptr: [*c]u8 = undefined;
    var seed_len: usize = undefined;
    if (c.divi_sentinal_recovery_to_seed(phrase_ptr, "", &seed_ptr, &seed_len) != 0) {
        c.divi_free_string(phrase_ptr);
        return .{ .phrase = null, .crown_id = null, .status = -3 };
    }

    // Derive identity key via HKDF
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    if (c.divi_sentinal_derive_identity_key(seed_ptr, seed_len, &key_ptr, &key_len) != 0) {
        @memset(seed_ptr[0..seed_len], 0);
        c.divi_free_bytes(seed_ptr, seed_len);
        c.divi_free_string(phrase_ptr);
        return .{ .phrase = null, .crown_id = null, .status = -4 };
    }
    // Seed no longer needed — zero before freeing
    @memset(seed_ptr[0..seed_len], 0);
    c.divi_free_bytes(seed_ptr, seed_len);

    // Create keyring from derived key
    const keyring = c.divi_crown_recover_from_secret(key_ptr, key_len);
    @memset(key_ptr[0..key_len], 0);
    c.divi_free_bytes(key_ptr, key_len);
    if (keyring == null) {
        c.divi_free_string(phrase_ptr);
        return .{ .phrase = null, .crown_id = null, .status = -5 };
    }

    // Get public key
    const crown_id_ptr: ?[*:0]u8 = c.divi_crown_keyring_public_key(keyring);
    if (crown_id_ptr == null) {
        c.divi_crown_keyring_free(keyring);
        c.divi_free_string(phrase_ptr);
        return .{ .phrase = null, .crown_id = null, .status = -6 };
    }

    // Store in shared state (setter frees any existing keyring)
    state.setKeyring(keyring);

    return .{
        .phrase = phrase_ptr,
        .crown_id = crown_id_ptr,
        .status = 0,
    };
}

/// Import an existing identity from a recovery phrase.
///
/// Composes: Sentinal recovery_validate → recovery_to_seed → derive_identity_key
///           → Crown recover_from_secret
///
/// Stores the keyring in shared state. Caller must free crown_id with divi_free_string.
export fn orch_import_identity(phrase: [*:0]const u8) IdentityResult {
    if (!state.isInitialized()) return .{ .phrase = null, .crown_id = null, .status = -1 };

    // Validate phrase format
    if (!c.divi_sentinal_recovery_validate(phrase)) {
        return .{ .phrase = null, .crown_id = null, .status = -2 };
    }

    // Derive seed from phrase
    var seed_ptr: [*c]u8 = undefined;
    var seed_len: usize = undefined;
    if (c.divi_sentinal_recovery_to_seed(phrase, "", &seed_ptr, &seed_len) != 0) {
        return .{ .phrase = null, .crown_id = null, .status = -3 };
    }

    // Derive identity key
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    if (c.divi_sentinal_derive_identity_key(seed_ptr, seed_len, &key_ptr, &key_len) != 0) {
        @memset(seed_ptr[0..seed_len], 0);
        c.divi_free_bytes(seed_ptr, seed_len);
        return .{ .phrase = null, .crown_id = null, .status = -4 };
    }
    @memset(seed_ptr[0..seed_len], 0);
    c.divi_free_bytes(seed_ptr, seed_len);

    // Create keyring from key
    const keyring = c.divi_crown_recover_from_secret(key_ptr, key_len);
    @memset(key_ptr[0..key_len], 0);
    c.divi_free_bytes(key_ptr, key_len);
    if (keyring == null) {
        return .{ .phrase = null, .crown_id = null, .status = -5 };
    }

    // Get public key
    const crown_id_ptr: ?[*:0]u8 = c.divi_crown_keyring_public_key(keyring);
    if (crown_id_ptr == null) {
        c.divi_crown_keyring_free(keyring);
        return .{ .phrase = null, .crown_id = null, .status = -6 };
    }

    // Store in shared state (setter frees any existing keyring)
    state.setKeyring(keyring);

    return .{
        .phrase = null, // Not returned on import (caller already has it)
        .crown_id = crown_id_ptr,
        .status = 0,
    };
}

/// Get the active identity's public key (crown_id).
/// Caller must free with divi_free_string.
export fn orch_identity_public_key() ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const keyring = s.keyring orelse return null;
    return c.divi_crown_keyring_public_key(keyring);
}

/// Sign data with the active identity's private key.
/// Returns JSON Signature. Caller must free with divi_free_string.
export fn orch_identity_sign(data: [*]const u8, data_len: usize) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const keyring = s.keyring orelse return null;
    return c.divi_crown_keyring_sign(keyring, data, data_len);
}

/// Export the keyring as encrypted bytes (for backup/sync).
/// Returns 0 on success. Caller must free out_data with divi_free_bytes.
export fn orch_identity_export(out_data: [*c][*c]u8, out_len: [*c]usize) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const keyring = s.keyring orelse return -1;
    return c.divi_crown_keyring_export(keyring, out_data, out_len);
}

/// Load a keyring from previously exported bytes.
/// Returns 0 on success.
export fn orch_identity_load(data: [*]const u8, data_len: usize) i32 {
    if (!state.isInitialized()) return -1;

    // Check if we already have a keyring
    const existing = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        break :blk s.keyring;
    };

    // Create a new keyring if none exists
    if (existing == null) {
        const new_kr = c.divi_crown_keyring_new();
        if (new_kr == null) return -2;
        state.setKeyring(new_kr);
    }

    // Load into the keyring (read the handle under shared lock)
    const s = state.acquireShared();
    defer state.releaseShared();
    return c.divi_crown_keyring_load(s.keyring, data, data_len);
}

/// Create a new Soul (profile container) at the given path.
/// Returns 0 on success.
export fn orch_soul_create(path: [*:0]const u8) i32 {
    if (!state.isInitialized()) return -1;

    const soul = c.divi_crown_soul_create(path);
    if (soul == null) return -2;

    // Setter frees any existing soul
    state.setSoul(soul);
    return 0;
}

/// Load an existing Soul from the given path.
/// Returns 0 on success.
export fn orch_soul_load(path: [*:0]const u8) i32 {
    if (!state.isInitialized()) return -1;

    const soul = c.divi_crown_soul_load(path);
    if (soul == null) return -2;

    // Setter frees any existing soul
    state.setSoul(soul);
    return 0;
}

/// Create a new Soul with at-rest encryption using the Vault's soul key.
///
/// Requires the Vault to be unlocked (provides the encryption key).
/// The soul.json file is encrypted before writing to disk.
/// Returns 0 on success, -1 = not initialized, -2 = vault not available,
/// -3 = soul key derivation failed, -4 = soul creation failed.
export fn orch_soul_create_encrypted(path: [*:0]const u8) i32 {
    if (!state.isInitialized()) return -1;

    // Hold the shared lock through the entire FFI sequence to prevent TOCTOU:
    // if the vault were freed between reading the handle and using it, we'd
    // dereference a dangling pointer. The lock keeps the vault alive.
    const s = state.acquireShared();
    const vault = s.vault orelse {
        state.releaseShared();
        return -2;
    };

    // Derive the soul encryption key from the vault
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    if (c.divi_vault_soul_key(vault, &key_ptr, &key_len) != 0) {
        state.releaseShared();
        return -3;
    }

    // Create the encrypted soul (vault handle still protected by shared lock)
    const soul = c.divi_crown_soul_create_encrypted(path, key_ptr, key_len);

    // Release the shared lock BEFORE calling setSoul, which acquires an
    // exclusive lock — holding shared while requesting exclusive would deadlock.
    state.releaseShared();

    // Zero and free key material regardless of success/failure
    @memset(key_ptr[0..key_len], 0);
    c.divi_free_bytes(key_ptr, key_len);

    if (soul == null) return -4;

    state.setSoul(soul);
    return 0;
}

/// Load an existing Soul with at-rest decryption using the Vault's soul key.
///
/// Requires the Vault to be unlocked. For backward compatibility, if the
/// soul.json file contains plaintext JSON (pre-encryption), loading still
/// succeeds and subsequent saves will encrypt the data.
/// Returns 0 on success, -1 = not initialized, -2 = vault not available,
/// -3 = soul key derivation failed, -4 = soul load failed.
export fn orch_soul_load_encrypted(path: [*:0]const u8) i32 {
    if (!state.isInitialized()) return -1;

    // Hold the shared lock through the entire FFI sequence to prevent TOCTOU:
    // if the vault were freed between reading the handle and using it, we'd
    // dereference a dangling pointer. The lock keeps the vault alive.
    const s = state.acquireShared();
    const vault = s.vault orelse {
        state.releaseShared();
        return -2;
    };

    // Derive the soul encryption key from the vault
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    if (c.divi_vault_soul_key(vault, &key_ptr, &key_len) != 0) {
        state.releaseShared();
        return -3;
    }

    // Load the encrypted soul (vault handle still protected by shared lock)
    const soul = c.divi_crown_soul_load_encrypted(path, key_ptr, key_len);

    // Release the shared lock BEFORE calling setSoul, which acquires an
    // exclusive lock — holding shared while requesting exclusive would deadlock.
    state.releaseShared();

    // Zero and free key material regardless of success/failure
    @memset(key_ptr[0..key_len], 0);
    c.divi_free_bytes(key_ptr, key_len);

    if (soul == null) return -4;

    state.setSoul(soul);
    return 0;
}

/// Get the active Soul's profile as JSON.
/// Caller must free with divi_free_string.
export fn orch_soul_profile() ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const soul = s.soul orelse return null;
    return c.divi_crown_soul_profile(soul);
}

/// Update the Soul's profile from JSON.
/// Returns 0 on success.
export fn orch_soul_update_profile(json: [*:0]const u8) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const soul = s.soul orelse return -1;
    return c.divi_crown_soul_update_profile(soul, json);
}

/// Save the Soul to disk (if dirty).
/// Returns 0 on success.
export fn orch_soul_save() i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const soul = s.soul orelse return -1;
    return c.divi_crown_soul_save(soul);
}

/// Follow an identity.
export fn orch_soul_follow(crown_id: [*:0]const u8) void {
    const s = state.acquireShared();
    defer state.releaseShared();
    if (s.soul) |soul| c.divi_crown_soul_follow(soul, crown_id);
}

/// Unfollow an identity.
export fn orch_soul_unfollow(crown_id: [*:0]const u8) void {
    const s = state.acquireShared();
    defer state.releaseShared();
    if (s.soul) |soul| c.divi_crown_soul_unfollow(soul, crown_id);
}

/// Block an identity.
export fn orch_soul_block(crown_id: [*:0]const u8) void {
    const s = state.acquireShared();
    defer state.releaseShared();
    if (s.soul) |soul| c.divi_crown_soul_block(soul, crown_id);
}

/// Unblock an identity.
export fn orch_soul_unblock(crown_id: [*:0]const u8) void {
    const s = state.acquireShared();
    defer state.releaseShared();
    if (s.soul) |soul| c.divi_crown_soul_unblock(soul, crown_id);
}

/// Get the Soul's social graph as JSON.
/// Caller must free with divi_free_string.
export fn orch_soul_social_graph() ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const soul = s.soul orelse return null;
    return c.divi_crown_soul_social_graph(soul);
}

/// Estimate password strength. Returns JSON with entropy_bits, tier, hint,
/// and crack_time fields, or null on error.
///
/// Stateless — no init required. Caller must free the returned string with
/// `divi_free_string`.
///
/// Example return: `{"entropy_bits":45.2,"tier":"Fair","hint":"Add symbols...","crack_time":"3 years"}`
export fn orch_password_strength(password: [*:0]const u8) ?[*:0]u8 {
    return c.divi_sentinal_password_strength(password);
}

// ── Tests ─────────────────────────────────────────────────────

test "IdentityResult layout" {
    const result = IdentityResult{ .phrase = null, .crown_id = null, .status = 0 };
    try std.testing.expectEqual(@as(i32, 0), result.status);
}

test "create identity requires init" {
    // Without orch_init, should fail
    state.get().initialized = false;
    const result = orch_create_identity();
    try std.testing.expectEqual(@as(i32, -1), result.status);
}

test "create identity full chain" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();

    const result = orch_create_identity();
    try std.testing.expectEqual(@as(i32, 0), result.status);
    try std.testing.expect(result.phrase != null);
    try std.testing.expect(result.crown_id != null);
    try std.testing.expect(state.get().keyring != null);

    // Clean up returned strings
    if (result.phrase) |p| c.divi_free_string(p);
    if (result.crown_id) |p| c.divi_free_string(p);
}

test "import identity validates phrase" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();

    const result = orch_import_identity("not a valid phrase");
    try std.testing.expectEqual(@as(i32, -2), result.status);
}

test "public key requires identity" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();

    // No identity yet
    try std.testing.expect(orch_identity_public_key() == null);

    // Create identity
    const result = orch_create_identity();
    if (result.phrase) |p| c.divi_free_string(p);
    if (result.crown_id) |p| c.divi_free_string(p);

    // Now we should have a public key
    const pk = orch_identity_public_key();
    try std.testing.expect(pk != null);
    if (pk) |p| c.divi_free_string(p);
}

test "password strength returns JSON for valid input" {
    // Stateless — no init required
    const result = orch_password_strength("correcthorsebatterystaple");
    try std.testing.expect(result != null);
    if (result) |r| {
        defer c.divi_free_string(r);
        // Should contain expected JSON fields
        const slice = std.mem.span(r);
        try std.testing.expect(std.mem.indexOf(u8, slice, "entropy_bits") != null);
        try std.testing.expect(std.mem.indexOf(u8, slice, "tier") != null);
    }
}
