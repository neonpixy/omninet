// Storage orchestration — Vault lifecycle and idea manifest management.
//
// The Vault is the encrypted filing cabinet. Everything persists here.
// Modules that need to store/retrieve data go through storage.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Public C API ──────────────────────────────────────────────

/// Create and unlock a vault. This is the one-step setup for first use.
///
/// Composes: vault_new → vault_unlock
///
/// Returns 0 on success, negative on error.
export fn orch_vault_setup(password: [*:0]const u8, root_path: [*:0]const u8) i32 {
    if (!state.isInitialized()) return -1;

    const vault = c.divi_vault_new();
    if (vault == null) return -2;

    if (c.divi_vault_unlock(vault, password, root_path) != 0) {
        c.divi_vault_free(vault);
        return -3;
    }

    // Setter frees any existing vault
    state.setVault(vault);
    return 0;
}

/// Lock the vault. Zeroes all keys from memory.
/// Returns 0 on success.
export fn orch_vault_lock() i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_lock(vault);
}

/// Check if the vault is currently unlocked.
export fn orch_vault_is_unlocked() bool {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return false;
    return c.divi_vault_is_unlocked(vault);
}

/// Register an .idea entry in the vault manifest.
/// entry_json is a JSON ManifestEntry.
/// Returns 0 on success.
export fn orch_vault_register_idea(entry_json: [*:0]const u8) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_register_idea(vault, entry_json);
}

/// Remove an .idea entry from the manifest by ID.
/// Returns 0 on success.
export fn orch_vault_unregister_idea(id: [*:0]const u8) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_unregister_idea(vault, id);
}

/// Get a manifest entry by ID.
/// Returns JSON ManifestEntry. Caller must free with divi_free_string.
export fn orch_vault_get_idea(id: [*:0]const u8) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_get_idea(vault, id);
}

/// Get a manifest entry by relative path.
/// Caller must free with divi_free_string.
export fn orch_vault_get_idea_by_path(path: [*:0]const u8) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_get_idea_by_path(vault, path);
}

/// List ideas matching a filter. filter_json is a JSON IdeaFilter.
/// Returns JSON array of ManifestEntry. Caller must free with divi_free_string.
export fn orch_vault_list_ideas(filter_json: [*:0]const u8) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_list_ideas(vault, filter_json);
}

/// List ideas in a folder (path prefix match).
/// Returns JSON array. Caller must free with divi_free_string.
export fn orch_vault_list_ideas_in_folder(folder: [*:0]const u8) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_list_ideas_in_folder(vault, folder);
}

/// Get the number of registered ideas. Returns -1 on error.
export fn orch_vault_idea_count() i64 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_idea_count(vault);
}

/// Encrypt data for a specific idea using its content key.
/// Returns 0 on success. Caller must free out_data with divi_free_bytes.
export fn orch_vault_encrypt(
    data: [*]const u8,
    data_len: usize,
    idea_id: [*:0]const u8,
    out_data: [*c][*c]u8,
    out_len: [*c]usize,
) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_encrypt_for_idea(vault, data, data_len, idea_id, out_data, out_len);
}

/// Decrypt data for a specific idea.
/// Returns 0 on success. Caller must free out_data with divi_free_bytes.
export fn orch_vault_decrypt(
    data: [*]const u8,
    data_len: usize,
    idea_id: [*:0]const u8,
    out_data: [*c][*c]u8,
    out_len: [*c]usize,
) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_decrypt_for_idea(vault, data, data_len, idea_id, out_data, out_len);
}

/// Save module state (key-value store per module).
/// Returns 0 on success.
export fn orch_vault_save_state(
    module_id: [*:0]const u8,
    state_key: [*:0]const u8,
    data: [*:0]const u8,
) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_save_module_state(vault, module_id, state_key, data);
}

/// Load module state. Returns raw string (not necessarily JSON).
/// Caller must free with divi_free_string.
export fn orch_vault_load_state(
    module_id: [*:0]const u8,
    state_key: [*:0]const u8,
) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_load_module_state(vault, module_id, state_key);
}

/// Delete module state entry. Returns 0 on success.
export fn orch_vault_delete_state(
    module_id: [*:0]const u8,
    state_key: [*:0]const u8,
) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_delete_module_state(vault, module_id, state_key);
}

/// Get the vault root path. Caller must free with divi_free_string.
export fn orch_vault_root_path() ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_root_path(vault);
}

/// Get the personal ideas directory path. Caller must free with divi_free_string.
export fn orch_vault_personal_path() ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_personal_path(vault);
}

/// Resolve a relative path within the vault root.
/// Caller must free with divi_free_string.
export fn orch_vault_resolve_path(relative: [*:0]const u8) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_resolve_path(vault, relative);
}

/// Create a new collective vault. Returns JSON Collective.
/// Caller must free with divi_free_string.
export fn orch_vault_create_collective(
    name: [*:0]const u8,
    owner_pubkey: [*:0]const u8,
) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_create_collective(vault, name, owner_pubkey);
}

/// List all collectives. Returns JSON array.
/// Caller must free with divi_free_string.
export fn orch_vault_list_collectives() ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_list_collectives(vault);
}

// ── Search ────────────────────────────────────────────────────

/// Search vault ideas by text query (FTS5).
/// Returns JSON array of SearchHit objects. Caller must free with divi_free_string.
/// `limit` defaults to 20 if <= 0.
export fn orch_vault_search(query: [*:0]const u8, limit: i32) callconv(.c) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_search(vault, query, limit);
}

/// Index an idea for full-text search.
/// tags_json is a JSON array of strings, or null for no tags.
/// Returns 0 on success.
export fn orch_vault_index_idea(
    idea_id: [*:0]const u8,
    title: [*:0]const u8,
    content_text: [*:0]const u8,
    tags_json: ?[*:0]const u8,
) callconv(.c) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_index_idea(vault, idea_id, title, content_text, tags_json);
}

/// Remove an idea from the search index.
/// Returns 0 on success.
export fn orch_vault_remove_search_index(idea_id: [*:0]const u8) callconv(.c) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_remove_search_index(vault, idea_id);
}

/// Rebuild the entire search index from the manifest.
/// Returns the number of indexed entries, or -1 on error.
export fn orch_vault_rebuild_search_index() callconv(.c) i64 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    return c.divi_vault_rebuild_search_index(vault);
}

// ── Tests ─────────────────────────────────────────────────────

test "vault requires init" {
    state.get().initialized = false;
    try std.testing.expectEqual(@as(i32, -1), orch_vault_setup("pass", "/tmp"));
}

test "vault not unlocked before setup" {
    try std.testing.expect(!orch_vault_is_unlocked());
}

test "vault idea count without vault" {
    try std.testing.expectEqual(@as(i64, -1), orch_vault_idea_count());
}
