// Content orchestration — Ideas + Hall + Vault composition for .idea CRUD + media.
//
// Universal document lifecycle: create, save, load, delete, list, attach image.
// Composes divi_ideas_*, divi_hall_*, and divi_vault_* FFI calls
// into app-level operations that ANY program calls as single steps.
// NOT program-specific — Tome, Quill, Studio all use these same ops.
//
// Ownership rules:
//   - All returned ?[*:0]u8 strings must be freed by the caller via divi_free_string.
//   - Content keys are fetched, used, zeroed, and freed within each function.
//   - Intermediate FFI strings are freed before returning.
//
// Thread safety:
//   - No module-level mutable state — all buffers are stack-local.
//   - Global state accessed via acquireShared()/releaseShared() per MEMORY.md pattern.
//   - Stateless FFI calls (Ideas digit_new, etc.) need no lock.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Helpers ─────────────────────────────────────────────────────

/// Free a C string returned from an FFI call (null-safe).
inline fn freeStr(ptr: ?[*:0]u8) void {
    if (ptr) |p| c.divi_free_string(p);
}

/// Free a byte buffer returned from an FFI call, zeroing it first (null-safe).
inline fn freeKeyBytes(ptr: [*c]u8, len: usize) void {
    if (len > 0 and ptr != null) {
        const slice = ptr[0..len];
        @memset(slice, 0);
        c.divi_free_bytes(ptr, len);
    }
}

/// Convert a null-terminated C string pointer to a Zig slice.
inline fn cspan(ptr: [*:0]const u8) []const u8 {
    return std.mem.span(ptr);
}

// ── Public C API ──────────────────────────────────────────────

/// Create an .idea end-to-end. Universal — any program can create any idea type.
///
/// Composes:
///   1. Crown keyring -> get pubkey (author)
///   2. Ideas digit_new(digit_type, content, author) -> root digit JSON
///   3. Ideas digit_with_property(digit, "title", title) -> titled digit JSON
///   4. Ideas digit_id(digit) -> root UUID string
///   5. Ideas header_create(pubkey, signature, root_id, key_slot) -> header JSON
///   6. Vault personal_path -> ideas directory
///   7. Build package JSON with header + digit
///   8. Vault content_key(idea_id) -> encryption key
///   9. Hall write(package, path, key) -> bytes written
///  10. Vault register_idea(manifest entry) -> manifest updated
///
/// Parameters:
///   - digit_type: null-terminated digit type string (e.g., "text", "section", "canvas").
///   - title: null-terminated title string.
///   - content_json: null-terminated JSON value for the root digit content.
///     Pass `"null"` or `"{}"` for empty content.
///
/// Returns: JSON ManifestEntry on success, null on error.
///          Caller must free with divi_free_string.
///          Check orch_last_error() for details on failure.
export fn orch_idea_create(digit_type: [*:0]const u8, title: [*:0]const u8, content_json: [*:0]const u8) ?[*:0]u8 {
    // ── Step 1: Get the author's public key from the keyring ──
    const pubkey: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const keyring = s.keyring orelse return null;
        break :blk c.divi_crown_keyring_public_key(keyring);
    };
    if (pubkey == null) return null;
    defer freeStr(pubkey);

    // ── Step 2: Create a root digit of the requested type ──
    // Stateless FFI call — no lock needed.
    const digit_raw: ?[*:0]u8 = c.divi_ideas_digit_new(digit_type, content_json, pubkey.?);
    if (digit_raw == null) return null;
    defer freeStr(digit_raw);

    // ── Step 3: Set the title property on the digit ──
    // The property value must be an X Value: {"string":"..."}
    // Build a JSON Value wrapping the title string.
    const title_slice = cspan(title);
    var title_buf: [4096]u8 = undefined;
    var tp: usize = 0;

    // Write prefix: {"string":"
    const prefix = "{\"string\":\"";
    for (prefix) |ch| {
        title_buf[tp] = ch;
        tp += 1;
    }

    for (title_slice) |ch| {
        if (tp + 4 >= title_buf.len) return null; // title too long
        switch (ch) {
            '"' => {
                title_buf[tp] = '\\';
                title_buf[tp + 1] = '"';
                tp += 2;
            },
            '\\' => {
                title_buf[tp] = '\\';
                title_buf[tp + 1] = '\\';
                tp += 2;
            },
            '\n' => {
                title_buf[tp] = '\\';
                title_buf[tp + 1] = 'n';
                tp += 2;
            },
            '\r' => {
                title_buf[tp] = '\\';
                title_buf[tp + 1] = 'r';
                tp += 2;
            },
            '\t' => {
                title_buf[tp] = '\\';
                title_buf[tp + 1] = 't';
                tp += 2;
            },
            else => {
                title_buf[tp] = ch;
                tp += 1;
            },
        }
    }

    // Write suffix: "}
    const suffix = "\"}";
    for (suffix) |ch| {
        title_buf[tp] = ch;
        tp += 1;
    }
    title_buf[tp] = 0;

    const title_json_z: [*:0]const u8 = @ptrCast(title_buf[0..tp :0]);

    // Stateless FFI call.
    const digit_titled: ?[*:0]u8 = c.divi_ideas_digit_with_property(
        digit_raw.?,
        "title",
        title_json_z,
        pubkey.?,
    );
    if (digit_titled == null) return null;
    defer freeStr(digit_titled);

    // ── Step 4: Extract the root digit's UUID ──
    // Stateless FFI call.
    const root_id: ?[*:0]u8 = c.divi_ideas_digit_id(digit_titled.?);
    if (root_id == null) return null;
    defer freeStr(root_id);

    // ── Step 5: Create the header ──
    // Stateless FFI call.
    const header: ?[*:0]u8 = c.divi_ideas_header_create(
        pubkey.?,
        "", // signature — empty for now, sign later if needed
        root_id.?,
        "{\"type\":\"internal\",\"key_id\":\"local\",\"wrapped_key\":\"\"}", // local vault key slot
    );
    if (header == null) return null;
    defer freeStr(header);

    // ── Step 6: Get the vault personal path and build idea directory path ──
    const personal_path: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_personal_path(vault);
    };
    if (personal_path == null) return null;
    defer freeStr(personal_path);

    // Build: {personal_path}/{root_id}.idea
    const personal_slice = cspan(personal_path.?);
    const root_id_slice = cspan(root_id.?);
    var path_buf: [4096]u8 = undefined;
    const idea_path_len = personal_slice.len + 1 + root_id_slice.len + 5;
    if (idea_path_len >= path_buf.len) return null;
    @memcpy(path_buf[0..personal_slice.len], personal_slice);
    path_buf[personal_slice.len] = '/';
    @memcpy(path_buf[personal_slice.len + 1 ..][0..root_id_slice.len], root_id_slice);
    @memcpy(path_buf[personal_slice.len + 1 + root_id_slice.len ..][0..5], ".idea");
    path_buf[idea_path_len] = 0;
    const idea_path_z: [*:0]const u8 = @ptrCast(path_buf[0..idea_path_len :0]);

    // ── Step 7: Build the package JSON ──
    // IdeaPackage JSON: {"header":<header>,"digits":{"<uuid>":<digit>}}
    // The path field is skipped by serde, so we omit it.
    const header_slice = cspan(header.?);
    const digit_slice = cspan(digit_titled.?);

    const pkg_overhead = 30 + root_id_slice.len;
    const pkg_needed = pkg_overhead + header_slice.len + digit_slice.len + 1;

    const allocator = std.heap.page_allocator;
    const pkg_buf = allocator.alloc(u8, pkg_needed) catch return null;
    defer allocator.free(pkg_buf);

    var pp: usize = 0;

    // Assemble: {"header":<header>,"digits":{"<root_id>":<digit>}}
    const parts = [_][]const u8{
        "{\"header\":",
        header_slice,
        ",\"digits\":{\"",
        root_id_slice,
        "\":",
        digit_slice,
        "}}",
    };
    for (parts) |part| {
        @memcpy(pkg_buf[pp..][0..part.len], part);
        pp += part.len;
    }
    pkg_buf[pp] = 0;

    const pkg_json_z: [*:0]const u8 = @ptrCast(pkg_buf[0..pp :0]);

    // ── Step 8: Get the content key ──
    // Extract the header's UUID (the idea ID) — it's different from the root digit ID.
    var id_buf: [64]u8 = undefined;
    const idea_id = extractIdFromHeaderJson(header_slice, &id_buf) orelse return null;

    var content_key_ptr: [*c]u8 = undefined;
    var content_key_len: usize = undefined;
    const key_rc = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_content_key(vault, idea_id.ptr, &content_key_ptr, &content_key_len);
    };
    if (key_rc != 0) return null;
    defer freeKeyBytes(content_key_ptr, content_key_len);

    // ── Step 9: Write to disk via Hall ──
    const bytes_written = c.divi_hall_write(
        pkg_json_z,
        idea_path_z,
        content_key_ptr,
        content_key_len,
    );
    if (bytes_written < 0) return null;

    // ── Step 10: Register in the vault manifest ──
    var ts_buf: [64]u8 = undefined;
    const manifest_entry = buildManifestEntryJson(
        idea_id,
        root_id_slice,
        title_slice,
        cspan(digit_type),
        cspan(pubkey.?),
        header_slice,
        &ts_buf,
    ) orelse return null;
    defer allocator.free(manifest_entry.buf);

    const reg_rc = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_register_idea(vault, manifest_entry.ptr);
    };
    if (reg_rc != 0) return null;

    // Return the canonical manifest entry from the vault.
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_get_idea(vault, idea_id.ptr);
}

/// Save (write) a note's package to disk.
///
/// Composes:
///   1. Vault get_idea(id) -> manifest entry (validates existence, gets path)
///   2. Vault resolve_path(relative) -> absolute path
///   3. Vault content_key(id) -> encryption key
///   4. Hall write(package_json, path, key) -> bytes written
///
/// Parameters:
///   - idea_id: null-terminated UUID string of the idea.
///   - package_json: null-terminated JSON IdeaPackage to write.
///
/// Returns: bytes written (i64) on success, negative on error.
///   -1 = not initialized / no vault
///   -2 = idea not found in manifest
///   -3 = content key derivation failed
///   -4 = hall write failed
export fn orch_idea_save(idea_id: [*:0]const u8, package_json: [*:0]const u8) i64 {
    // ── Step 1: Get the manifest entry to find the path ──
    const entry_json: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return -1;
        break :blk c.divi_vault_get_idea(vault, idea_id);
    };
    if (entry_json == null) return -2;
    defer freeStr(entry_json);

    // Extract the path from the manifest entry JSON.
    var field_buf: [4096]u8 = undefined;
    const idea_path = extractFieldFromJson(cspan(entry_json.?), "path", &field_buf) orelse return -2;

    // Resolve the relative path to absolute.
    const resolved_path: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return -1;
        break :blk c.divi_vault_resolve_path(vault, idea_path.ptr);
    };
    if (resolved_path == null) return -2;
    defer freeStr(resolved_path);

    // ── Step 2: Get the content key ──
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    const key_rc = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return -1;
        break :blk c.divi_vault_content_key(vault, idea_id, &key_ptr, &key_len);
    };
    if (key_rc != 0) return -3;
    defer freeKeyBytes(key_ptr, key_len);

    // ── Step 3: Write to disk ──
    const bytes_written = c.divi_hall_write(
        package_json,
        resolved_path.?,
        key_ptr,
        key_len,
    );
    if (bytes_written < 0) return -4;

    return bytes_written;
}

/// Load a note's package from disk.
///
/// Composes:
///   1. Vault get_idea(id) -> manifest entry (path)
///   2. Vault resolve_path(relative) -> absolute path
///   3. Vault content_key(id) -> decryption key
///   4. Hall read(path, key) -> package JSON
///
/// Parameters:
///   - idea_id: null-terminated UUID string.
///
/// Returns: JSON IdeaPackage on success, null on error.
///          Caller must free with divi_free_string.
///          Check orch_last_error() for details on failure.
export fn orch_idea_load(idea_id: [*:0]const u8) ?[*:0]u8 {
    // ── Step 1: Get the manifest entry ──
    const entry_json: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_get_idea(vault, idea_id);
    };
    if (entry_json == null) return null;
    defer freeStr(entry_json);

    // Extract the path from the entry.
    var field_buf: [4096]u8 = undefined;
    const idea_path = extractFieldFromJson(cspan(entry_json.?), "path", &field_buf) orelse return null;

    // ── Step 2: Resolve the relative path ──
    const resolved_path: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_resolve_path(vault, idea_path.ptr);
    };
    if (resolved_path == null) return null;
    defer freeStr(resolved_path);

    // ── Step 3: Get the content key ──
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    const key_rc = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_content_key(vault, idea_id, &key_ptr, &key_len);
    };
    if (key_rc != 0) return null;
    defer freeKeyBytes(key_ptr, key_len);

    // ── Step 4: Read from disk ──
    // divi_hall_read takes a `char**` out-param for warnings.
    var warnings_raw: ?[*:0]u8 = null;
    const package_json: ?[*:0]u8 = c.divi_hall_read(
        resolved_path.?,
        key_ptr,
        key_len,
        @ptrCast(&warnings_raw),
    );

    // Free warnings if returned (we don't surface them yet).
    freeStr(warnings_raw);

    return package_json;
}

/// Delete a note.
///
/// Composes:
///   1. Vault unregister_idea(id) -> removes from manifest
///
/// Note: This does NOT delete the .idea directory from disk. That requires
/// filesystem operations not currently exposed via Hall FFI. The manifest
/// entry is removed so the note no longer appears in listings. A future
/// garbage collection pass can clean up orphaned directories.
///
/// Parameters:
///   - idea_id: null-terminated UUID string.
///
/// Returns: 0 on success, negative on error.
///   -1 = not initialized / no vault
///   -2 = unregister failed (idea not found or vault error)
export fn orch_idea_delete(idea_id: [*:0]const u8) i32 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return -1;
    const rc = c.divi_vault_unregister_idea(vault, idea_id);
    if (rc != 0) return -2;
    return 0;
}

/// List .ideas matching a filter. Universal — no type injection.
///
/// Wraps: Vault list_ideas. The caller provides the complete filter.
/// Programs should include their own extended_type filter if needed
/// (e.g., Tome passes {"extended_type":"note"}).
///
/// Parameters:
///   - filter_json: null-terminated JSON IdeaFilter string.
///     Pass "{}" to list all .ideas.
///
/// Returns: JSON array of ManifestEntry on success, null on error.
///          Caller must free with divi_free_string.
export fn orch_idea_list(filter_json: [*:0]const u8) ?[*:0]u8 {
    const s = state.acquireShared();
    defer state.releaseShared();
    const vault = s.vault orelse return null;
    return c.divi_vault_list_ideas(vault, filter_json);
}

/// Attach an image to an existing .idea.
///
/// Composes:
///   1. Vault get_idea(id) -> manifest entry (validates existence, gets path)
///   2. Vault resolve_path(relative) -> absolute .idea directory path
///   3. Vault content_key(id) -> encryption key
///   4. Vault vocabulary_seed() -> Babel obfuscation seed
///   5. Hall asset_import(data, idea_path, key, seed) -> SHA-256 hex hash
///   6. Hall extract_image_metadata(data, len) -> width/height/mime/blurhash (best-effort)
///   7. Build and return JSON result
///
/// Parameters:
///   - idea_id: null-terminated UUID string of the note to attach the image to.
///   - image_data: pointer to raw image bytes (PNG, JPEG, GIF, WebP).
///   - image_data_len: length of the image data in bytes.
///
/// Returns: JSON string on success with the shape:
///   `{ "hash": "<sha256hex>", "size": <N>, "metadata": { "width": <W>, "height": <H>, "mime": "<type>", "blurhash": "<str>" } }`
///   The `metadata` field is omitted if image metadata extraction fails (non-fatal).
///   Returns null on error — check orch_last_error().
///   Caller must free the returned string via divi_free_string.
export fn orch_idea_attach_image(
    idea_id: [*:0]const u8,
    image_data: [*c]const u8,
    image_data_len: usize,
) ?[*:0]u8 {
    // ── Validate input ──
    if (image_data == null or image_data_len == 0) return null;

    // ── Step 1: Get the manifest entry to find the path ──
    const entry_json: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_get_idea(vault, idea_id);
    };
    if (entry_json == null) return null;
    defer freeStr(entry_json);

    // Extract the relative path from the manifest entry JSON.
    var field_buf: [4096]u8 = undefined;
    const idea_rel_path = extractFieldFromJson(cspan(entry_json.?), "path", &field_buf) orelse return null;

    // ── Step 2: Resolve the relative path to absolute ──
    const resolved_path: ?[*:0]u8 = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_resolve_path(vault, idea_rel_path.ptr);
    };
    if (resolved_path == null) return null;
    defer freeStr(resolved_path);

    // ── Step 3: Get the content key ──
    var key_ptr: [*c]u8 = undefined;
    var key_len: usize = undefined;
    const key_rc = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_content_key(vault, idea_id, &key_ptr, &key_len);
    };
    if (key_rc != 0) return null;
    defer freeKeyBytes(key_ptr, key_len);

    // ── Step 4: Get the vocabulary seed ──
    var seed_ptr: [*c]u8 = undefined;
    var seed_len: usize = undefined;
    const seed_rc = blk: {
        const s = state.acquireShared();
        defer state.releaseShared();
        const vault = s.vault orelse return null;
        break :blk c.divi_vault_vocabulary_seed(vault, &seed_ptr, &seed_len);
    };
    if (seed_rc != 0) return null;
    defer freeKeyBytes(seed_ptr, seed_len);

    // ── Step 5: Import the image as an encrypted asset ──
    // divi_hall_asset_import returns the SHA-256 hex hash string.
    const hash: ?[*:0]u8 = c.divi_hall_asset_import(
        image_data,
        image_data_len,
        resolved_path.?,
        key_ptr,
        key_len,
        seed_ptr,
        seed_len,
    );
    if (hash == null) return null;
    defer freeStr(hash);

    // ── Step 6: Extract image metadata (best-effort) ──
    // This is stateless — no lock needed. If it fails, we still return the hash.
    const metadata: ?[*:0]u8 = c.divi_hall_extract_image_metadata(image_data, image_data_len);
    defer freeStr(metadata);

    // ── Step 7: Build the result JSON ──
    const hash_slice = cspan(hash.?);
    const allocator = std.heap.page_allocator;

    if (metadata) |meta_ptr| {
        // Full result with metadata embedded.
        // Shape: {"hash":"<hex>","size":<N>,"metadata":<meta_json>}
        const meta_slice = cspan(meta_ptr);

        // Estimate: {"hash":"..hash..","size":..len..,"metadata":..meta..}
        //   overhead ~40 + hash(64) + size_digits(~10) + meta_json
        const needed = 50 + hash_slice.len + 20 + meta_slice.len;
        const buf = allocator.alloc(u8, needed) catch return null;
        defer allocator.free(buf);

        var pos: usize = 0;

        const prefix = "{\"hash\":\"";
        @memcpy(buf[pos..][0..prefix.len], prefix);
        pos += prefix.len;

        @memcpy(buf[pos..][0..hash_slice.len], hash_slice);
        pos += hash_slice.len;

        const mid = "\",\"size\":";
        @memcpy(buf[pos..][0..mid.len], mid);
        pos += mid.len;

        // Format the size as a decimal number.
        var size_buf: [20]u8 = undefined;
        const size_str = std.fmt.bufPrint(&size_buf, "{d}", .{image_data_len}) catch return null;
        @memcpy(buf[pos..][0..size_str.len], size_str);
        pos += size_str.len;

        const meta_key = ",\"metadata\":";
        @memcpy(buf[pos..][0..meta_key.len], meta_key);
        pos += meta_key.len;

        @memcpy(buf[pos..][0..meta_slice.len], meta_slice);
        pos += meta_slice.len;

        buf[pos] = '}';
        pos += 1;
        buf[pos] = 0;

        // Allocate via the FFI string allocator so the caller can free with divi_free_string.
        // We use divi_free_string-compatible allocation: duplicate into a C string.
        return dupeAsFFIString(buf[0..pos]);
    } else {
        // Metadata extraction failed — return just hash and size.
        // Shape: {"hash":"<hex>","size":<N>}
        const needed = 40 + hash_slice.len + 20;
        const buf = allocator.alloc(u8, needed) catch return null;
        defer allocator.free(buf);

        var pos: usize = 0;

        const prefix = "{\"hash\":\"";
        @memcpy(buf[pos..][0..prefix.len], prefix);
        pos += prefix.len;

        @memcpy(buf[pos..][0..hash_slice.len], hash_slice);
        pos += hash_slice.len;

        const mid = "\",\"size\":";
        @memcpy(buf[pos..][0..mid.len], mid);
        pos += mid.len;

        var size_buf: [20]u8 = undefined;
        const size_str = std.fmt.bufPrint(&size_buf, "{d}", .{image_data_len}) catch return null;
        @memcpy(buf[pos..][0..size_str.len], size_str);
        pos += size_str.len;

        buf[pos] = '}';
        pos += 1;
        buf[pos] = 0;

        return dupeAsFFIString(buf[0..pos]);
    }
}

// ── Internal helpers ─────────────────────────────────────────────

/// Duplicate a byte slice into a null-terminated C string allocated via c_allocator.
/// The caller must free the returned string via divi_free_string (libc-compatible).
/// Returns null on allocation failure.
///
/// This follows the same pattern as intercom.zig: c_allocator.allocSentinel
/// produces libc-malloc memory, which is compatible with Rust's CString::from_raw
/// (used by divi_free_string) on all supported platforms.
fn dupeAsFFIString(data: []const u8) ?[*:0]u8 {
    const result = std.heap.c_allocator.allocSentinel(u8, data.len, 0) catch return null;
    @memcpy(result[0..data.len], data);
    return result.ptr;
}

/// Result of extracting a null-terminated field from JSON.
const ExtractedField = struct {
    /// Pointer to a null-terminated string in a caller-owned buffer.
    ptr: [*:0]const u8,
    /// The raw bytes (not including null terminator).
    bytes: []const u8,
};

/// Minimal JSON field extraction. Finds "key":"value" and returns value.
/// Only works for simple string fields (no nested objects).
/// Handles escaped quotes within values.
///
/// Thread-safe: uses caller-provided buffer, no mutable globals.
fn extractFieldFromJson(json: []const u8, key: []const u8, out_buf: []u8) ?ExtractedField {
    // Build the search needle: "key":"
    var needle_buf: [256]u8 = undefined;
    if (key.len + 4 > needle_buf.len) return null;

    needle_buf[0] = '"';
    @memcpy(needle_buf[1..][0..key.len], key);
    needle_buf[key.len + 1] = '"';
    needle_buf[key.len + 2] = ':';
    needle_buf[key.len + 3] = '"';
    const needle = needle_buf[0 .. key.len + 4];

    const start_idx = std.mem.indexOf(u8, json, needle) orelse return null;
    const value_start = start_idx + needle.len;

    // Find the closing quote (handle escaped quotes).
    var i: usize = value_start;
    while (i < json.len) {
        if (json[i] == '\\') {
            i += 2;
            continue;
        }
        if (json[i] == '"') break;
        i += 1;
    }
    if (i >= json.len) return null;

    const value = json[value_start..i];
    if (value.len >= out_buf.len) return null;

    @memcpy(out_buf[0..value.len], value);
    out_buf[value.len] = 0;

    return ExtractedField{
        .ptr = @ptrCast(out_buf[0..value.len :0]),
        .bytes = out_buf[0..value.len],
    };
}

/// Extract the "id" field from a header JSON string.
/// Returns a null-terminated UUID string suitable for FFI calls.
///
/// Thread-safe: uses caller-provided buffer, no mutable globals.
fn extractIdFromHeaderJson(header_json: []const u8, out_buf: *[64]u8) ?ExtractedField {
    const needle = "\"id\":\"";
    const start_idx = std.mem.indexOf(u8, header_json, needle) orelse return null;
    const value_start = start_idx + needle.len;

    // UUID is 36 chars: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    if (value_start + 36 > header_json.len) return null;

    const uuid_bytes = header_json[value_start .. value_start + 36];

    // Validate it looks like a UUID (basic dash positions).
    if (uuid_bytes[8] != '-' or uuid_bytes[13] != '-' or
        uuid_bytes[18] != '-' or uuid_bytes[23] != '-') return null;

    @memcpy(out_buf[0..36], uuid_bytes);
    out_buf[36] = 0;

    return ExtractedField{
        .ptr = @ptrCast(out_buf[0..36 :0]),
        .bytes = out_buf[0..36],
    };
}

/// Result of building a ManifestEntry JSON.
const ManifestEntryJson = struct {
    ptr: [*:0]const u8,
    buf: []u8,
};

/// Build a ManifestEntry JSON for vault registration.
/// `ts_buf` is a scratch buffer for timestamp extraction.
fn buildManifestEntryJson(
    idea_id: ExtractedField,
    root_id: []const u8,
    title: []const u8,
    extended_type: []const u8,
    creator: []const u8,
    header_json: []const u8,
    ts_buf: *[64]u8,
) ?ManifestEntryJson {
    // Extract "created" timestamp from the header JSON.
    const created = extractFieldFromJson(header_json, "created", ts_buf) orelse return null;
    // Copy created bytes before ts_buf could be reused.
    var created_copy: [64]u8 = undefined;
    if (created.bytes.len >= created_copy.len) return null;
    @memcpy(created_copy[0..created.bytes.len], created.bytes);
    const created_bytes = created_copy[0..created.bytes.len];

    // Build the relative path: personal/{root_id}.idea
    const path_prefix = "personal/";
    const path_suffix = ".idea";

    // JSON template:
    // {"id":"<id>","path":"personal/<root_id>.idea","title":"<title>",
    //  "extended_type":"<type>","creator":"<creator>",
    //  "created_at":"<ts>","modified_at":"<ts>"}
    const overhead = 150;
    const needed = overhead + idea_id.bytes.len + path_prefix.len + root_id.len +
        path_suffix.len + title.len + extended_type.len + creator.len + (created_bytes.len * 2);

    const allocator = std.heap.page_allocator;
    const buf = allocator.alloc(u8, needed + 1) catch return null;

    var pos: usize = 0;
    const parts = [_][]const u8{
        "{\"id\":\"",          idea_id.bytes,
        "\",\"path\":\"",     path_prefix,
        root_id,               path_suffix,
        "\",\"title\":\"",    title,
        "\",\"extended_type\":\"",  extended_type,
        "\",\"creator\":\"",       creator,
        "\",\"created_at\":\"", created_bytes,
        "\",\"modified_at\":\"", created_bytes,
        "\"}",
    };

    for (parts) |part| {
        @memcpy(buf[pos..][0..part.len], part);
        pos += part.len;
    }
    buf[pos] = 0;

    return ManifestEntryJson{
        .ptr = @ptrCast(buf[0..pos :0]),
        .buf = buf,
    };
}

// ── Tests ─────────────────────────────────────────────────────

test "extract id from header json" {
    const json = "{\"version\":\"1.0\",\"id\":\"550e8400-e29b-41d4-a716-446655440000\",\"created\":\"2024-01-01\"}";
    var buf: [64]u8 = undefined;
    const result = extractIdFromHeaderJson(json, &buf);
    try std.testing.expect(result != null);
    try std.testing.expectEqualStrings("550e8400-e29b-41d4-a716-446655440000", result.?.bytes);
}

test "extract id from header json - invalid uuid" {
    const json = "{\"id\":\"not-a-uuid-at-all-no-way-not-36chars\"}";
    var buf: [64]u8 = undefined;
    const result = extractIdFromHeaderJson(json, &buf);
    try std.testing.expect(result == null);
}

test "extract field from json" {
    const json = "{\"path\":\"personal/test.idea\",\"title\":\"My Note\"}";
    var buf1: [4096]u8 = undefined;
    const path = extractFieldFromJson(json, "path", &buf1);
    try std.testing.expect(path != null);
    try std.testing.expectEqualStrings("personal/test.idea", path.?.bytes);

    var buf2: [4096]u8 = undefined;
    const title_field = extractFieldFromJson(json, "title", &buf2);
    try std.testing.expect(title_field != null);
    try std.testing.expectEqualStrings("My Note", title_field.?.bytes);
}

test "extract field missing returns null" {
    const json = "{\"path\":\"test.idea\"}";
    var buf: [4096]u8 = undefined;
    const result = extractFieldFromJson(json, "missing", &buf);
    try std.testing.expect(result == null);
}

test "extract field with escaped quotes" {
    const json = "{\"title\":\"A \\\"quoted\\\" title\"}";
    var buf: [4096]u8 = undefined;
    const result = extractFieldFromJson(json, "title", &buf);
    try std.testing.expect(result != null);
    try std.testing.expectEqualStrings("A \\\"quoted\\\" title", result.?.bytes);
}

test "note create requires init" {
    state.get().initialized = false;
    state.get().keyring = null;
    const result = orch_idea_create("text", "Test", "\"hello\"");
    try std.testing.expect(result == null);
}

test "note save requires vault" {
    state.get().vault = null;
    const result = orch_idea_save("some-id", "{}");
    try std.testing.expectEqual(@as(i64, -1), result);
}

test "note load requires vault" {
    state.get().vault = null;
    const result = orch_idea_load("some-id");
    try std.testing.expect(result == null);
}

test "note delete requires vault" {
    state.get().vault = null;
    const result = orch_idea_delete("some-id");
    try std.testing.expectEqual(@as(i32, -1), result);
}

test "note list requires vault" {
    state.get().vault = null;
    const result = orch_idea_list("{}");
    try std.testing.expect(result == null);
}

test "note list filter augmentation" {
    // With no vault, each call returns null — but we verify the filter
    // building logic doesn't crash for various inputs.
    state.get().vault = null;
    _ = orch_idea_list("{}");
    _ = orch_idea_list("null");
    _ = orch_idea_list("{\"creator\":\"abc\"}");
    _ = orch_idea_list("{\"extended_type\":\"custom\"}");
}

test "build manifest entry json" {
    const id = ExtractedField{
        .ptr = "550e8400-e29b-41d4-a716-446655440000",
        .bytes = "550e8400-e29b-41d4-a716-446655440000",
    };
    const header = "{\"version\":\"1.0\",\"id\":\"550e8400-e29b-41d4-a716-446655440000\",\"created\":\"2024-01-01T00:00:00Z\",\"modified\":\"2024-01-01T00:00:00Z\"}";

    var ts_buf: [64]u8 = undefined;
    const result = buildManifestEntryJson(
        id,
        "test-root-id",
        "My Note",
        "text",
        "pubkey123",
        header,
        &ts_buf,
    );
    try std.testing.expect(result != null);
    defer std.heap.page_allocator.free(result.?.buf);

    const json = cspan(result.?.ptr);
    try std.testing.expect(std.mem.indexOf(u8, json, "550e8400") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "My Note") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "\"text\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "pubkey123") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "personal/test-root-id.idea") != null);
    try std.testing.expect(std.mem.indexOf(u8, json, "2024-01-01T00:00:00Z") != null);
}

test "attach image requires vault" {
    state.get().vault = null;
    const data = [_]u8{ 0x89, 0x50, 0x4E, 0x47 }; // PNG magic bytes (incomplete, but we'll fail at vault check first)
    const result = orch_idea_attach_image("some-idea-id", &data, data.len);
    try std.testing.expect(result == null);
}

test "attach image rejects null data" {
    const result = orch_idea_attach_image("some-idea-id", null, 0);
    try std.testing.expect(result == null);
}

test "attach image rejects zero-length data" {
    const data = [_]u8{0x00};
    const result = orch_idea_attach_image("some-idea-id", &data, 0);
    try std.testing.expect(result == null);
}

// ── Pipeline Registry Wrappers ──────────────────────────────────
//
// The pipeline executor dispatches by string key → HandlerFn.
// Comptime auto-dispatch only covers divi_* (Rust FFI) functions.
// orch_note_* are Zig-native, so we register them manually as
// third-party operations with proper permissions and modifiers.
//
// Input convention: JSON array of string arguments.
//   ["arg0", "arg1", ...]
// The JS store must JSON.stringify before including objects.
//
// Thread safety: wrappers use stack buffers only. No module-level
// mutable state. The underlying orch_note_* functions are already
// thread-safe (they acquire shared state locks internally).

const reg = @import("registry.zig");

/// Extract the N-th string element from a JSON array, with unescape.
/// Returns the length of the extracted (unescaped) value, or null on error.
/// The output buffer is null-terminated.
fn extractArrayArg(json: []const u8, index: usize, out: []u8) ?usize {
    var pos: usize = 0;
    // Find '['
    while (pos < json.len and json[pos] != '[') : (pos += 1) {}
    if (pos >= json.len) return null;
    pos += 1;

    // Skip to the desired element
    var current: usize = 0;
    while (current < index) {
        // Skip whitespace/commas
        while (pos < json.len and (json[pos] == ' ' or json[pos] == ',' or
            json[pos] == '\n' or json[pos] == '\r' or json[pos] == '\t')) : (pos += 1)
        {}
        if (pos >= json.len or json[pos] == ']') return null;
        if (json[pos] != '"') return null;
        pos += 1; // skip opening "
        // Walk past this string value
        while (pos < json.len) {
            if (json[pos] == '\\') {
                pos += 2;
                continue;
            }
            if (json[pos] == '"') break;
            pos += 1;
        }
        if (pos >= json.len) return null;
        pos += 1; // skip closing "
        current += 1;
    }

    // Skip whitespace/commas to find our element
    while (pos < json.len and (json[pos] == ' ' or json[pos] == ',' or
        json[pos] == '\n' or json[pos] == '\r' or json[pos] == '\t')) : (pos += 1)
    {}
    if (pos >= json.len or json[pos] == ']') return null;
    if (json[pos] != '"') return null;
    pos += 1; // skip opening "

    // Copy with unescape
    var oi: usize = 0;
    while (pos < json.len and oi + 1 < out.len) {
        if (json[pos] == '\\' and pos + 1 < json.len) {
            const next = json[pos + 1];
            switch (next) {
                '"' => {
                    out[oi] = '"';
                    oi += 1;
                    pos += 2;
                },
                '\\' => {
                    out[oi] = '\\';
                    oi += 1;
                    pos += 2;
                },
                'n' => {
                    out[oi] = '\n';
                    oi += 1;
                    pos += 2;
                },
                'r' => {
                    out[oi] = '\r';
                    oi += 1;
                    pos += 2;
                },
                't' => {
                    out[oi] = '\t';
                    oi += 1;
                    pos += 2;
                },
                '/' => {
                    out[oi] = '/';
                    oi += 1;
                    pos += 2;
                },
                else => {
                    out[oi] = json[pos];
                    oi += 1;
                    pos += 1;
                },
            }
        } else if (json[pos] == '"') {
            break;
        } else {
            out[oi] = json[pos];
            oi += 1;
            pos += 1;
        }
    }
    if (oi >= out.len) return null;
    out[oi] = 0;
    return oi;
}

/// idea.create: ["digit_type", "title", "content_json"]
fn handleIdeaCreate(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
    const json = std.mem.span(input);
    var type_buf: [256]u8 = undefined;
    var title_buf: [4096]u8 = undefined;
    var content_buf: [4096]u8 = undefined;
    _ = extractArrayArg(json, 0, &type_buf) orelse return null;
    _ = extractArrayArg(json, 1, &title_buf) orelse return null;
    if (extractArrayArg(json, 2, &content_buf)) |_| {
        return orch_idea_create(@ptrCast(&type_buf), @ptrCast(&title_buf), @ptrCast(&content_buf));
    }
    return orch_idea_create(@ptrCast(&type_buf), @ptrCast(&title_buf), "null");
}

/// idea.load: ["idea_id"]
fn handleIdeaLoad(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
    const json = std.mem.span(input);
    var id_buf: [128]u8 = undefined;
    _ = extractArrayArg(json, 0, &id_buf) orelse return null;
    return orch_idea_load(@ptrCast(&id_buf));
}

/// idea.save: ["idea_id", "package_json_string"]
/// The package_json must be pre-stringified by the caller.
fn handleIdeaSave(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
    const json = std.mem.span(input);
    var id_buf: [128]u8 = undefined;
    _ = extractArrayArg(json, 0, &id_buf) orelse return null;

    // Heap-allocate for potentially large package JSON
    const alloc = std.heap.c_allocator;
    const pkg_buf = alloc.alloc(u8, json.len + 1) catch return null;
    defer alloc.free(pkg_buf);
    _ = extractArrayArg(json, 1, pkg_buf) orelse return null;

    const bytes = orch_idea_save(@ptrCast(&id_buf), @ptrCast(pkg_buf.ptr));
    if (bytes < 0) return null;

    // Return bytes written as a JSON number string
    var num_buf: [32]u8 = undefined;
    const num_slice = std.fmt.bufPrint(&num_buf, "{d}", .{bytes}) catch return null;
    return dupeAsFFIString(num_slice);
}

/// idea.delete: ["idea_id"]
fn handleIdeaDelete(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
    const json = std.mem.span(input);
    var id_buf: [128]u8 = undefined;
    _ = extractArrayArg(json, 0, &id_buf) orelse return null;
    const rc = orch_idea_delete(@ptrCast(&id_buf));
    if (rc < 0) return null;
    return dupeAsFFIString("0");
}

/// idea.list: ["filter_json"] or just the filter object directly
fn handleIdeaList(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
    const json = std.mem.span(input);
    // Try array extraction first
    var filter_buf: [4096]u8 = undefined;
    if (extractArrayArg(json, 0, &filter_buf)) |_| {
        return orch_idea_list(@ptrCast(&filter_buf));
    }
    // Fall back to using the input directly as filter JSON
    return orch_idea_list(input);
}

/// Register universal .idea CRUD operations in the pipeline registry.
/// Called from orch_init() after reg.init().
pub fn registerContentOps() void {
    reg.register("idea.create", .{
        .call = &handleIdeaCreate,
        .handles = &.{},
        .permission = .granted_once,
        .modifiers = .{ .polity = true, .yoke = true },
    }) catch {};
    reg.register("idea.load", .{
        .call = &handleIdeaLoad,
        .handles = &.{},
        .permission = .granted_once,
        .modifiers = .{},
    }) catch {};
    reg.register("idea.save", .{
        .call = &handleIdeaSave,
        .handles = &.{},
        .permission = .granted_once,
        .modifiers = .{ .sentinal = true, .yoke = true },
    }) catch {};
    reg.register("idea.delete", .{
        .call = &handleIdeaDelete,
        .handles = &.{},
        .permission = .per_action,
        .modifiers = .{ .yoke = true },
    }) catch {};
    reg.register("idea.list", .{
        .call = &handleIdeaList,
        .handles = &.{},
        .permission = .granted_once,
        .modifiers = .{},
    }) catch {};
}

// ── Tests ────────────────────────────────────────────────────────

test "extractArrayArg basic" {
    var buf: [128]u8 = undefined;
    const len = extractArrayArg("[\"hello\",\"world\"]", 0, &buf);
    try std.testing.expect(len != null);
    try std.testing.expectEqualStrings("hello", buf[0..len.?]);

    const len2 = extractArrayArg("[\"hello\",\"world\"]", 1, &buf);
    try std.testing.expect(len2 != null);
    try std.testing.expectEqualStrings("world", buf[0..len2.?]);
}

test "extractArrayArg unescape" {
    var buf: [256]u8 = undefined;
    const input = "[\"first\",\"{\\\"header\\\":{},\\\"digits\\\":{}}\"]";
    const len = extractArrayArg(input, 1, &buf);
    try std.testing.expect(len != null);
    try std.testing.expectEqualStrings("{\"header\":{},\"digits\":{}}", buf[0..len.?]);
}

test "extractArrayArg out of bounds returns null" {
    var buf: [128]u8 = undefined;
    const result = extractArrayArg("[\"only\"]", 1, &buf);
    try std.testing.expect(result == null);
}

test "dupeAsFFIString round trip" {
    const input = "hello, orchestrator";
    const result = dupeAsFFIString(input);
    try std.testing.expect(result != null);
    defer std.heap.c_allocator.free(result.?[0..input.len :0]);
    try std.testing.expectEqualStrings(input, std.mem.span(result.?));
}
