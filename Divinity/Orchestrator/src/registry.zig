// Operation Registry — comptime auto-dispatch from the C header.
//
// Replaces the manual HashMap registration with comptime iteration over
// all divi_* declarations imported via @cImport. At compile time, we
// inspect every function's signature and generate a pattern-matched
// calling adapter and metadata (permission, modifiers).
//
// Pipeline operations use dot notation: "vault.lock" maps to "divi_vault_lock".
// The dispatch function converts the dot name at runtime and matches it
// against the comptime-known set of divi_* function names.
//
// Third-party (runtime-registered) operations fall through to a small HashMap.
//
// Memory:
//   - Comptime dispatch has zero runtime allocation cost
//   - Third-party registry uses c_allocator
//   - Handler return strings follow the same ownership as the underlying FFI call:
//     Rust-allocated strings -> caller frees with divi_free_string
//     Zig-allocated wrappers (i32/bool/usize results) -> caller frees with c_allocator

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");
const governance = @import("governance.zig");
const commerce = @import("commerce.zig");
const discovery = @import("discovery.zig");
const ai_mod = @import("ai.zig");
const lingo_mod = @import("lingo.zig");

// ── Types ──────────────────────────────────────────────────────

/// The universal handler function signature.
/// Takes a null-terminated JSON input string.
/// Returns a null-terminated JSON output string (Rust-allocated, free with divi_free_string),
/// or null on error (check orch_last_error).
pub const HandlerFn = *const fn ([*:0]const u8) callconv(.c) ?[*:0]u8;

/// What handles an operation needs. Used by the pipeline for pre-flight validation.
pub const HandleReq = enum {
    // Global state handles (from state.zig)
    runtime,
    keyring,
    soul,
    vault,
    phone,
    email,
    contacts,
    pager,
    omnibus,
    theme,
    schema_registry,
    renderer_registry,
    tool_registry,
    // Governance handles (from governance.zig module state)
    rights,
    duties,
    protections,
    consent,
    trust_graph,
    breach_registry,
    enactment_registry,
    consent_validator,
    permission_checker,
    // Commerce handles (from commerce.zig module state)
    ledger,
    treasury,
    ubi,
    cart,
    // Discovery handles (from discovery.zig module state)
    tower_directory,
    query_router,
    result_cache,
    trend_tracker,
    health_history,
    // AI handles (from ai.zig module state)
    advisor_loop,
    advisor_store,
    advisor_router,
    advisor_skills,
    oracle_registry,
};

/// Permission level for pipeline pre-flight checks.
pub const PermissionLevel = enum(u8) {
    /// No permission needed. ~60% of ops (stateless transforms, queries).
    free = 0,
    /// Approved at install time (read access, non-destructive mutations).
    granted_once = 1,
    /// Prompts each time for sensitive operations (writes, key operations).
    per_action = 2,
    /// Always confirms. Nuclear operations (delete identity, wipe vault, etc.).
    always_ask = 3,
};

/// Bitfield for which cross-cutting modifiers apply to an operation.
pub const ModifierSet = packed struct(u8) {
    /// Polity: rights/consent validation before execution.
    polity: bool = false,
    /// Bulwark: safety/permission checks.
    bulwark: bool = false,
    /// Sentinal: encrypt/decrypt wrapping.
    sentinal: bool = false,
    /// Yoke: provenance tracking after execution.
    yoke: bool = false,
    /// Lingo: translation of string fields.
    lingo: bool = false,
    /// Quest: XP/progression events. Optional, can be disabled.
    quest: bool = false,
    _pad: u2 = 0,
};

/// A registered operation handler.
pub const OpHandler = struct {
    /// The handler function. Takes JSON input, returns JSON output (or null on error).
    call: HandlerFn,
    /// What handles this operation needs (for pre-flight validation).
    handles: []const HandleReq,
    /// Permission level for this operation.
    permission: PermissionLevel,
    /// Which mandatory modifiers apply.
    modifiers: ModifierSet,
    /// Whether this operation creates a handle that the pipeline should track
    /// and free on completion.
    owns_handle: bool = false,
};

// ── Allocator ──────────────────────────────────────────────────

const allocator = std.heap.c_allocator;

// ── Third-Party Registry ───────────────────────────────────────

const ThirdPartyMap = std.StringHashMap(OpHandler);

var third_party: ?ThirdPartyMap = null;
var tp_lock: std.Thread.RwLock = .{};

// ── Out-Param Thread-Local Storage ─────────────────────────────
//
// Functions with out-params (uint8_t**, uintptr_t*, etc.) write results
// through pointers. The universal dispatcher uses thread-local storage
// so that resolveArg can return pointers to writable locations.
// After the FFI call, callUniversal reads these to build the result.

threadlocal var out_ptrs: [4][*c]u8 = .{ null, null, null, null };
threadlocal var out_lens: [4]usize = .{ 0, 0, 0, 0 };
threadlocal var out_ptr_count: u8 = 0;
threadlocal var out_len_count: u8 = 0;

fn resetOutParams() void {
    out_ptrs = .{ null, null, null, null };
    out_lens = .{ 0, 0, 0, 0 };
    out_ptr_count = 0;
    out_len_count = 0;
}

/// Check at comptime if a function has any out-params.
fn hasOutParams(comptime name: []const u8) bool {
    const T = @TypeOf(@field(c, name));
    const params = @typeInfo(T).@"fn".params;
    for (params) |p| {
        const PT = p.type orelse continue;
        if (isOutParam(PT)) return true;
    }
    return false;
}

// ── Comptime Infrastructure ────────────────────────────────────

/// Maximum length for a C function name buffer.
const MAX_C_NAME = 261;

/// Check if a C declaration name should be skipped for dispatch.
/// Skips: memory management, lifecycle, runtime, error helpers.
fn shouldSkip(comptime name: []const u8) bool {
    // Global memory management
    if (std.mem.eql(u8, name, "divi_free_string")) return true;
    if (std.mem.eql(u8, name, "divi_free_bytes")) return true;
    if (std.mem.eql(u8, name, "divi_last_error")) return true;

    // Runtime lifecycle
    if (std.mem.eql(u8, name, "divi_runtime_new")) return true;
    if (std.mem.eql(u8, name, "divi_runtime_free")) return true;

    // All _free functions are lifecycle, not operations
    if (name.len > 5 and std.mem.endsWith(u8, name, "_free")) return true;

    return false;
}

/// Check if a decl is a dispatchable divi_* function.
fn isDispatchable(comptime name: []const u8) bool {
    if (!std.mem.startsWith(u8, name, "divi_")) return false;
    if (shouldSkip(name)) return false;
    const T = @TypeOf(@field(c, name));
    if (@typeInfo(T) != .@"fn") return false;
    if (classifyFn(name) == .unknown) return false;
    return true;
}

/// Convert a dot-notation op name to a C function name at runtime.
/// "vault.lock" -> "divi_vault_lock"
/// Returns the length of the C name written into buf, or null on overflow.
fn opNameToCName(op_name: []const u8, buf: *[MAX_C_NAME]u8) ?usize {
    const prefix = "divi_";
    if (op_name.len + prefix.len > MAX_C_NAME) return null;

    @memcpy(buf[0..prefix.len], prefix);
    var pos: usize = prefix.len;

    // Find the first dot and replace with underscore, copy the rest as-is
    var found_dot = false;
    for (op_name) |ch| {
        if (!found_dot and ch == '.') {
            buf[pos] = '_';
            found_dot = true;
        } else {
            buf[pos] = ch;
        }
        pos += 1;
    }

    return pos;
}

/// Convert a C function name to dot-notation op name at comptime.
/// "divi_vault_lock" -> "vault.lock"
/// Strips "divi_" prefix, replaces the FIRST underscore with a dot.
/// Returns a string literal (not a reference to a comptime var).
fn cNameToOpName(comptime c_name: []const u8) *const [cNameToOpNameLen(c_name)]u8 {
    comptime {
        const stripped = c_name[5..];
        var result: [stripped.len]u8 = undefined;
        var found_first = false;
        for (stripped, 0..) |ch, i| {
            if (!found_first and ch == '_') {
                result[i] = '.';
                found_first = true;
            } else {
                result[i] = ch;
            }
        }
        const final = result;
        return &final;
    }
}

fn cNameToOpNameLen(comptime c_name: []const u8) comptime_int {
    return c_name.len - 5;
}

/// Determine permission level from a function name at comptime.
fn permissionForOp(comptime name: []const u8) PermissionLevel {
    // Nuclear operations — always ask
    if (comptime containsAny(name, &.{ "delete_identity", "wipe", "export_keyring" }))
        return .always_ask;

    // Destructive operations — per action
    if (comptime containsAny(name, &.{ "delete", "remove", "revoke", "dissolve", "cancel", "dispute" }))
        return .per_action;

    // Sensitive crypto operations — per action
    if (comptime containsAny(name, &.{ "encrypt", "decrypt", "sign", "derive", "generate_salt", "onion_wrap", "onion_unwrap", "recovery_generate" }))
        return .per_action;

    // Write operations — granted once
    if (comptime containsAny(name, &.{ "create", "update", "set", "add", "register", "save", "record", "insert", "store", "import", "load", "grant", "approve", "enact", "amend", "post", "credit", "debit", "transfer", "claim", "publish" }))
        return .granted_once;

    // Read / query operations — granted once
    if (comptime containsAny(name, &.{ "read", "get", "list", "search", "query", "export" }))
        return .granted_once;

    // Everything else (pure functions, stateless transforms) — free
    return .free;
}

/// Determine modifier set from a function name at comptime.
fn modifiersForOp(comptime name: []const u8) ModifierSet {
    // Vault/Hall operations — sentinal + yoke (encrypted storage + provenance)
    if (comptime std.mem.startsWith(u8, name, "divi_vault_") or
        std.mem.startsWith(u8, name, "divi_hall_"))
        return .{ .sentinal = true, .yoke = true };

    // Sentinal operations — sentinal (crypto operations)
    if (comptime std.mem.startsWith(u8, name, "divi_sentinal_"))
        return .{ .sentinal = true };

    // Content creation — polity + yoke
    if (comptime containsAny(name, &.{ "create", "insert", "publish" }))
        return .{ .polity = true, .yoke = true };

    // Content mutation — yoke
    if (comptime containsAny(name, &.{ "update", "add", "remove", "delete" }))
        return .{ .yoke = true };

    // Governance operations — polity
    if (comptime std.mem.startsWith(u8, name, "divi_polity_") or
        std.mem.startsWith(u8, name, "divi_kingdom_"))
        return .{ .polity = true };

    // Safety operations — bulwark
    if (comptime std.mem.startsWith(u8, name, "divi_bulwark_"))
        return .{ .bulwark = true };

    // Default: no modifiers
    return .{};
}

/// Infer handle requirements from function name at comptime.
fn handlesForOp(comptime name: []const u8) []const HandleReq {
    // Global state handles
    if (comptime std.mem.startsWith(u8, name, "divi_vault_")) return &.{.vault};
    if (comptime containsSeq(name, "keyring")) return &.{.keyring};
    if (comptime containsSeq(name, "soul")) return &.{.soul};
    if (comptime std.mem.startsWith(u8, name, "divi_omnibus_")) return &.{.omnibus};
    if (comptime std.mem.startsWith(u8, name, "divi_phone_")) return &.{.phone};
    if (comptime std.mem.startsWith(u8, name, "divi_email_")) return &.{.email};
    if (comptime std.mem.startsWith(u8, name, "divi_contacts_")) return &.{.contacts};
    if (comptime std.mem.startsWith(u8, name, "divi_pager_")) return &.{.pager};

    // Module handles (governance)
    if (comptime std.mem.startsWith(u8, name, "divi_polity_rights_")) return &.{.rights};
    if (comptime std.mem.startsWith(u8, name, "divi_polity_duties_")) return &.{.duties};
    if (comptime std.mem.startsWith(u8, name, "divi_polity_protections_")) return &.{.protections};
    if (comptime std.mem.startsWith(u8, name, "divi_polity_consent_registry_")) return &.{.consent};
    if (comptime std.mem.startsWith(u8, name, "divi_polity_breach_")) return &.{.breach_registry};
    if (comptime std.mem.startsWith(u8, name, "divi_polity_enactment_registry_")) return &.{.enactment_registry};
    if (comptime std.mem.startsWith(u8, name, "divi_bulwark_consent_validator_")) return &.{.consent_validator};
    if (comptime std.mem.startsWith(u8, name, "divi_bulwark_permission_checker_")) return &.{.permission_checker};
    if (comptime std.mem.startsWith(u8, name, "divi_jail_trust_graph_")) return &.{.trust_graph};

    // Module handles (commerce)
    if (comptime std.mem.startsWith(u8, name, "divi_fortune_ledger_")) return &.{.ledger};
    if (comptime std.mem.startsWith(u8, name, "divi_fortune_treasury_")) return &.{.treasury};
    if (comptime std.mem.startsWith(u8, name, "divi_fortune_ubi_")) return &.{.ubi};
    if (comptime std.mem.startsWith(u8, name, "divi_commerce_cart_")) return &.{.cart};

    // Module handles (discovery)
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_directory_")) return &.{.tower_directory};
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_router_")) return &.{.query_router};
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_cache_")) return &.{.result_cache};
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_trends_")) return &.{.trend_tracker};
    if (comptime std.mem.startsWith(u8, name, "divi_undercroft_history_")) return &.{.health_history};

    // Module handles (AI)
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_loop_")) return &.{.advisor_loop};
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_store_")) return &.{.advisor_store};
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_router_")) return &.{.advisor_router};
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_skills_")) return &.{.advisor_skills};
    if (comptime std.mem.startsWith(u8, name, "divi_oracle_registry_")) return &.{.oracle_registry};

    // Stateless — no handles
    return &.{};
}

/// Comptime helper: check if name contains any of the given substrings.
fn containsAny(comptime name: []const u8, comptime needles: []const []const u8) bool {
    for (needles) |needle| {
        if (comptime std.mem.indexOf(u8, name, needle) != null) return true;
    }
    return false;
}

/// Comptime helper: check if name contains a substring.
fn containsSeq(comptime name: []const u8, comptime needle: []const u8) bool {
    return comptime std.mem.indexOf(u8, name, needle) != null;
}

/// Check if a parameter type is an opaque handle pointer (not a string or numeric type).
fn isHandleParam(comptime T: type) bool {
    // Exclude string types first
    if (T == [*c]const u8 or T == ?[*c]const u8 or T == ?[*:0]const u8) return false;
    if (T == [*c]u8 or T == ?[*c]u8 or T == ?[*:0]u8) return false;
    // Check for pointer or optional pointer
    const info = @typeInfo(T);
    return switch (info) {
        .pointer => true,
        .optional => @typeInfo(info.optional.child) == .pointer,
        else => false,
    };
}

/// Check if a parameter type is a C string.
fn isStrParam(comptime T: type) bool {
    return T == [*c]const u8 or T == ?[*c]const u8 or T == ?[*:0]const u8;
}

/// Check if a parameter type is a numeric integer (any C integer type).
fn isNumericParam(comptime T: type) bool {
    return T == i32 or T == c_int or T == u32 or T == c_uint or
        T == i64 or T == c_longlong or T == u64 or T == c_ulonglong or
        T == usize or T == isize or T == u16 or T == i16 or T == u8 or T == i8;
}

/// Check if a parameter type is a floating-point number (C double/float).
fn isFloatParam(comptime T: type) bool {
    return T == f64 or T == f32;
}

/// Check if a parameter type is bool.
fn isBoolParam(comptime T: type) bool {
    return T == bool;
}

/// Check if a parameter type is a function pointer (callback).
fn isFnPtrParam(comptime T: type) bool {
    const info = @typeInfo(T);
    if (info == .optional) {
        const child = info.optional.child;
        if (@typeInfo(child) == .pointer and @typeInfo(@typeInfo(child).pointer.child) == .@"fn")
            return true;
    }
    if (info == .pointer and @typeInfo(info.pointer.child) == .@"fn") return true;
    return false;
}

/// Check if a parameter type is an out-param (mutable pointer for output).
fn isOutParam(comptime T: type) bool {
    return T == [*c][*c]u8 or T == [*c]usize or T == [*c]u32 or T == [*c]i32;
}

/// Check if a parameter type is marshallable from JSON.
/// Marshallable types: strings, numbers, bools, floats. NOT: callbacks, out-params.
fn isMarshallableParam(comptime T: type) bool {
    return isStrParam(T) or isNumericParam(T) or isFloatParam(T) or isBoolParam(T);
}

/// Check if a function can be universally dispatched.
/// True if: every parameter is either a resolvable handle (first param only),
/// a marshallable type (string/number/bool), a secondary handle (from JSON address),
/// or an out-param (resolved via thread-local storage). No callbacks.
/// Return type must also be wrappable (str/i32/bool/usize/i64/void/u32/handle).
fn isUniversallyCallable(comptime name: []const u8) bool {
    const T = @TypeOf(@field(c, name));
    const info = @typeInfo(T).@"fn";
    const params = info.params;
    const Ret = info.return_type orelse void;

    // Check return type is wrappable
    const ret_ok = (Ret == ?[*:0]u8 or Ret == [*c]u8 or
        Ret == i32 or Ret == c_int or Ret == bool or
        Ret == usize or Ret == i64 or Ret == c_longlong or
        Ret == u32 or Ret == c_uint or Ret == u64 or Ret == c_ulonglong or
        Ret == u16 or Ret == c_ushort or Ret == i16 or Ret == c_short or
        Ret == f64 or Ret == f32 or
        Ret == void or isHandleParam(Ret));
    if (!ret_ok) return false;

    for (params, 0..) |p, i| {
        const PT = p.type orelse return false;
        if (isFnPtrParam(PT)) return false;
        if (i == 0 and isHandleParam(PT)) continue; // handle resolved from state or JSON
        if (isMarshallableParam(PT)) continue;
        // Secondary handles (e.g., divi_magic_history_undo(history, doc)):
        // resolved from JSON array as integer addresses (from prior pipeline step outputs).
        if (isHandleParam(PT)) continue;
        // Out-params (e.g., uint8_t**, uintptr_t*): resolved via thread-local storage.
        // The FFI function writes to these pointers; results are read after the call.
        if (isOutParam(PT)) continue;
        return false;
    }
    return true;
}

/// Map a C function name to the OrchestratorState field that holds its handle.
/// Returns null for functions whose handles live in module state (use moduleHandleForFn).
fn stateFieldForFn(comptime name: []const u8) ?[]const u8 {
    // Order matters — more specific prefixes first
    if (comptime std.mem.startsWith(u8, name, "divi_crown_keyring_")) return "keyring";
    if (comptime std.mem.startsWith(u8, name, "divi_crown_soul_")) return "soul";
    if (comptime std.mem.startsWith(u8, name, "divi_crown_")) return "keyring"; // blinding, ECDH, etc.
    if (comptime std.mem.startsWith(u8, name, "divi_vault_")) return "vault";
    if (comptime std.mem.startsWith(u8, name, "divi_omnibus_")) return "omnibus";
    if (comptime std.mem.startsWith(u8, name, "divi_phone_")) return "phone";
    if (comptime std.mem.startsWith(u8, name, "divi_email_")) return "email";
    if (comptime std.mem.startsWith(u8, name, "divi_contacts_")) return "contacts";
    if (comptime std.mem.startsWith(u8, name, "divi_pager_")) return "pager";
    if (comptime std.mem.startsWith(u8, name, "divi_regalia_theme_collection_")) return "theme";
    if (comptime std.mem.startsWith(u8, name, "divi_ideas_schema_registry_")) return "schema_registry";
    if (comptime std.mem.startsWith(u8, name, "divi_magic_renderer_registry_")) return "renderer_registry";
    if (comptime std.mem.startsWith(u8, name, "divi_magic_tool_registry_")) return "tool_registry";
    return null;
}

/// Check if a function's handle lives in a module (governance, commerce, discovery, ai, lingo).
fn moduleHandleForFn(comptime name: []const u8) bool {
    // Governance
    if (comptime std.mem.startsWith(u8, name, "divi_polity_rights_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_polity_duties_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_polity_protections_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_polity_consent_registry_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_polity_breach_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_polity_enactment_registry_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_jail_trust_graph_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_bulwark_permission_checker_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_bulwark_consent_validator_")) return true;
    // Commerce
    if (comptime std.mem.startsWith(u8, name, "divi_fortune_ledger_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_fortune_treasury_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_fortune_ubi_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_commerce_cart_")) return true;
    // Discovery
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_directory_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_router_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_cache_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_zeitgeist_trends_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_undercroft_history_")) return true;
    // AI
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_loop_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_store_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_router_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_advisor_skills_")) return true;
    if (comptime std.mem.startsWith(u8, name, "divi_oracle_registry_")) return true;
    // Lingo
    if (comptime std.mem.startsWith(u8, name, "divi_lingo_babel_")) return true;
    return false;
}

/// Check if a function has any resolvable handle (global state or module).
fn hasResolvableHandle(comptime name: []const u8) bool {
    return stateFieldForFn(name) != null or moduleHandleForFn(name);
}

/// Check if all params from `start` onward are C string types.
fn allRemainingAreStr(comptime params: anytype, comptime start: usize) bool {
    for (start..params.len) |i| {
        const T = params[i].type orelse return false;
        if (!isStrParam(T)) return false;
    }
    return true;
}

// ── JSON Array Extraction ──────────────────────────────────────
//
// For multi-string functions, the pipeline input is a JSON array:
//   ["arg1", "arg2", "arg3"]
// We extract each element as a null-terminated string into a fixed buffer.

const MAX_ARRAY_ARGS = 7;
const MAX_ARG_LEN = 4096;

/// Extract string elements from a JSON array into null-terminated buffers.
/// Returns the number of elements extracted, or null on parse error.
fn extractJsonArray(input: [*:0]const u8, bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8, lens: *[MAX_ARRAY_ARGS]usize) ?usize {
    const str = std.mem.span(input);
    if (str.len < 2) return null;

    // Find opening bracket
    var pos: usize = 0;
    while (pos < str.len and str[pos] != '[') : (pos += 1) {}
    if (pos >= str.len) return null;
    pos += 1; // skip [

    var n_elems: usize = 0;
    while (pos < str.len and n_elems < MAX_ARRAY_ARGS) {
        // Skip whitespace and commas
        while (pos < str.len and (str[pos] == ' ' or str[pos] == ',' or str[pos] == '\n' or str[pos] == '\r' or str[pos] == '\t')) : (pos += 1) {}
        if (pos >= str.len or str[pos] == ']') break;

        if (str[pos] == '"') {
            // Quoted string — extract content
            pos += 1; // skip opening quote
            var out_len: usize = 0;
            while (pos < str.len and str[pos] != '"') {
                if (str[pos] == '\\' and pos + 1 < str.len) {
                    // Escape sequence
                    pos += 1;
                    if (out_len < MAX_ARG_LEN - 1) {
                        bufs[n_elems][out_len] = str[pos];
                        out_len += 1;
                    }
                } else {
                    if (out_len < MAX_ARG_LEN - 1) {
                        bufs[n_elems][out_len] = str[pos];
                        out_len += 1;
                    }
                }
                pos += 1;
            }
            if (pos < str.len) pos += 1; // skip closing quote
            bufs[n_elems][out_len] = 0; // null terminate
            lens[n_elems] = out_len;
            n_elems += 1;
        } else {
            // Non-string value (number, bool, null, object) — copy raw until , or ]
            var out_len: usize = 0;
            while (pos < str.len and str[pos] != ',' and str[pos] != ']') {
                if (out_len < MAX_ARG_LEN - 1) {
                    bufs[n_elems][out_len] = str[pos];
                    out_len += 1;
                }
                pos += 1;
            }
            // Trim trailing whitespace
            while (out_len > 0 and (bufs[n_elems][out_len - 1] == ' ' or bufs[n_elems][out_len - 1] == '\n')) {
                out_len -= 1;
            }
            bufs[n_elems][out_len] = 0;
            lens[n_elems] = out_len;
            n_elems += 1;
        }
    }
    return n_elems;
}

// ── Comptime Pattern Detection & Calling ───────────────────────

/// Categorize a function's calling pattern at comptime.
const CallPattern = enum {
    /// () -> ?[*:0]u8
    no_arg_str_out,
    /// ([*c]const u8) -> ?[*:0]u8
    json_in_str_out,
    /// ([*c]const u8) -> i32
    json_in_i32_out,
    /// ([*c]const u8) -> bool
    json_in_bool_out,
    /// () -> i32
    no_arg_i32_out,
    /// () -> bool
    no_arg_bool_out,
    /// () -> usize
    no_arg_usize_out,
    /// ([*c]const u8) -> usize
    json_in_usize_out,
    // ── Handle-bearing patterns (opaque pointer first arg, resolved from state) ──
    /// (handle) -> str
    handle_str_out,
    /// (handle) -> i32
    handle_i32_out,
    /// (handle) -> bool
    handle_bool_out,
    /// (handle) -> i64
    handle_i64_out,
    /// (handle) -> void
    handle_void,
    /// (handle, str) -> str
    handle_json_in_str_out,
    /// (handle, str) -> i32
    handle_json_in_i32_out,
    /// (handle, str) -> bool
    handle_json_in_bool_out,
    /// (handle, str) -> void
    handle_json_in_void,
    // ── Array-input patterns (JSON array parsed into N string args) ──
    /// (handle, str...) -> str — input is JSON array
    handle_arr_str_out,
    /// (handle, str...) -> i32
    handle_arr_i32_out,
    /// (handle, str...) -> bool
    handle_arr_bool_out,
    /// (handle, str...) -> void
    handle_arr_void,
    /// (str, str...) -> str — no handle, all strings, input is JSON array
    arr_str_out,
    /// (str, str...) -> i32
    arr_i32_out,
    /// (str, str...) -> bool
    arr_bool_out,
    /// (str, str...) -> void
    arr_void,
    // ── Universal patterns (mixed param types, parsed from JSON array) ──
    /// Mixed params (handle + str/num/bool) — handle resolved from state, rest from JSON array
    universal,
    /// Multi-arg or complex — recognized but not auto-callable
    multi_arg,
    /// Unrecognized pattern
    unknown,
};

/// Classify a divi_* function's pattern at comptime.
fn classifyFn(comptime name: []const u8) CallPattern {
    const T = @TypeOf(@field(c, name));
    const info = @typeInfo(T).@"fn";
    const params = info.params;
    const Ret = info.return_type orelse void;

    // Check return type categories.
    // C's `char *` maps to `[*c]u8` in Zig @cImport. Our handler returns
    // `?[*:0]u8`, so we need to handle the coercion in callByPattern.
    const ret_is_opt_str = (Ret == ?[*:0]u8 or Ret == [*c]u8);
    const ret_is_i32 = (Ret == i32 or Ret == c_int);
    const ret_is_bool = (Ret == bool);
    const ret_is_usize = (Ret == usize);
    const ret_is_i64 = (Ret == i64 or Ret == c_longlong);
    const ret_is_void = (Ret == void);

    if (params.len == 0) {
        if (ret_is_opt_str) return .no_arg_str_out;
        if (ret_is_i32) return .no_arg_i32_out;
        if (ret_is_bool) return .no_arg_bool_out;
        if (ret_is_usize) return .no_arg_usize_out;
        // Additional 0-arg return types handled by universal dispatcher
        if (comptime isUniversallyCallable(name)) return .universal;
        return .unknown;
    }

    if (params.len == 1) {
        const P0 = params[0].type orelse return .unknown;

        if (isStrParam(P0)) {
            if (ret_is_opt_str) return .json_in_str_out;
            if (ret_is_i32) return .json_in_i32_out;
            if (ret_is_bool) return .json_in_bool_out;
            if (ret_is_usize) return .json_in_usize_out;
        }

        // Handle-bearing: opaque pointer arg, resolvable from global state
        if (isHandleParam(P0) and hasResolvableHandle(name)) {
            if (ret_is_opt_str) return .handle_str_out;
            if (ret_is_i32) return .handle_i32_out;
            if (ret_is_bool) return .handle_bool_out;
            if (ret_is_i64) return .handle_i64_out;
            if (ret_is_void) return .handle_void;
        }

        // 1-arg with non-standard return types (u32, u64, f64, handle returns)
        // or numeric/bool/float params — fall through to universal check below.
    }

    if (params.len == 2) {
        const P0 = params[0].type orelse return .unknown;
        const P1 = params[1].type orelse return .unknown;

        // Handle + string arg
        if (isHandleParam(P0) and isStrParam(P1) and hasResolvableHandle(name)) {
            if (ret_is_opt_str) return .handle_json_in_str_out;
            if (ret_is_i32) return .handle_json_in_i32_out;
            if (ret_is_bool) return .handle_json_in_bool_out;
            if (ret_is_void) return .handle_json_in_void;
        }
    }

    // Handle + N string args: input is a JSON array ["arg1", "arg2", ...]
    if (params.len >= 3 and params.len <= 7) {
        const P0 = params[0].type orelse return .unknown;
        if (isHandleParam(P0) and hasResolvableHandle(name) and allRemainingAreStr(params, 1)) {
            if (ret_is_opt_str) return .handle_arr_str_out;
            if (ret_is_i32) return .handle_arr_i32_out;
            if (ret_is_bool) return .handle_arr_bool_out;
            if (ret_is_void) return .handle_arr_void;
        }
    }

    // Multi-string without handle (2+ string args, no handle)
    if (params.len >= 2 and params.len <= 7) {
        if (allRemainingAreStr(params, 0)) {
            if (ret_is_opt_str) return .arr_str_out;
            if (ret_is_i32) return .arr_i32_out;
            if (ret_is_bool) return .arr_bool_out;
            if (ret_is_void) return .arr_void;
        }
    }

    // Universal: mixed param types (str/num/bool/handle) that can be marshalled from JSON.
    // This catches everything the specialized patterns above missed:
    //   - handle + numeric params (e.g., cache_put(cache, query, results, now_i64))
    //   - handle + str + num (e.g., trends_record_query(trends, query, now))
    //   - pure str + num (e.g., encrypt functions without out-params)
    //   - functions returning handles (e.g., _new() constructors)
    // Functions with callbacks, out-params, or unsupported types stay as multi_arg.
    if (params.len >= 1 and params.len <= 8) {
        if (comptime isUniversallyCallable(name)) {
            return .universal;
        }
    }

    return .multi_arg;
}

// ── Comptime Op Count ──────────────────────────────────────────

/// Count all dispatchable ops at comptime.
fn comptimeOpCount() comptime_int {
    @setEvalBranchQuota(20_000_000);
    comptime {
        var n: usize = 0;
        for (@typeInfo(c).@"struct".decls) |decl| {
            if (isDispatchable(decl.name)) n += 1;
        }
        return n;
    }
}

/// Total number of comptime-dispatchable operations.
const comptime_op_count: usize = comptimeOpCount();

// ── Inline Iteration Helpers ───────────────────────────────────
//
// Instead of building a table, we iterate @typeInfo(c).@"struct".decls
// directly with inline for. Each function that needs to match an op name
// does its own inline iteration. The compiler optimizes this into a
// series of string comparisons.

/// Dispatch an operation by name. Returns the result JSON string, or null on error.
fn comptimeDispatch(op_name: []const u8, input: ?[*:0]const u8) ?[*:0]u8 {
    var c_name_buf: [MAX_C_NAME]u8 = undefined;
    const c_name_len = opNameToCName(op_name, &c_name_buf) orelse return null;
    const c_name = c_name_buf[0..c_name_len];

    @setEvalBranchQuota(20_000_000);

    inline for (@typeInfo(c).@"struct".decls) |decl| {
        if (comptime isDispatchable(decl.name)) {
            if (decl.name.len == c_name.len and
                std.mem.eql(u8, decl.name, c_name))
            {
                return callByPattern(decl.name, comptime classifyFn(decl.name), input);
            }
        }
    }

    return null;
}

/// Check if an op name exists in the comptime declarations.
fn comptimeHasOp(op_name: []const u8) bool {
    var c_name_buf: [MAX_C_NAME]u8 = undefined;
    const c_name_len = opNameToCName(op_name, &c_name_buf) orelse return false;
    const c_name = c_name_buf[0..c_name_len];

    @setEvalBranchQuota(20_000_000);

    inline for (@typeInfo(c).@"struct".decls) |decl| {
        if (comptime isDispatchable(decl.name)) {
            if (decl.name.len == c_name.len and
                std.mem.eql(u8, decl.name, c_name))
            {
                return true;
            }
        }
    }

    return false;
}

/// Look up an OpHandler for an op by name from the comptime declarations.
fn comptimeLookup(op_name: []const u8) ?OpHandler {
    var c_name_buf: [MAX_C_NAME]u8 = undefined;
    const c_name_len = opNameToCName(op_name, &c_name_buf) orelse return null;
    const c_name = c_name_buf[0..c_name_len];

    @setEvalBranchQuota(20_000_000);

    inline for (@typeInfo(c).@"struct".decls) |decl| {
        if (comptime isDispatchable(decl.name)) {
            if (decl.name.len == c_name.len and
                std.mem.eql(u8, decl.name, c_name))
            {
                return OpHandler{
                    .call = makeTrampoline(decl.name, comptime classifyFn(decl.name)),
                    .handles = comptime handlesForOp(decl.name),
                    .permission = comptime permissionForOp(decl.name),
                    .modifiers = comptime modifiersForOp(decl.name),
                    .owns_handle = comptime containsAny(decl.name, &.{"_new"}),
                };
            }
        }
    }

    return null;
}

/// Call an FFI function by its comptime-known name and pattern.
/// Handles the coercion from C's `[*c]u8` to our `?[*:0]u8` return type.
fn callByPattern(comptime c_name: []const u8, comptime pattern: CallPattern, input: ?[*:0]const u8) ?[*:0]u8 {
    return switch (pattern) {
        .no_arg_str_out => coerceStrReturn(@field(c, c_name)()),
        .json_in_str_out => coerceStrReturn(@field(c, c_name)(input orelse return null)),
        .json_in_i32_out => wrapI32(@field(c, c_name)(input orelse return null)),
        .json_in_bool_out => wrapBool(@field(c, c_name)(input orelse return null)),
        .no_arg_i32_out => wrapI32(@field(c, c_name)()),
        .no_arg_bool_out => wrapBool(@field(c, c_name)()),
        .no_arg_usize_out => wrapUsize(@field(c, c_name)()),
        .json_in_usize_out => wrapUsize(@field(c, c_name)(input orelse return null)),
        // Handle-bearing patterns — resolve handle from global state
        .handle_str_out, .handle_i32_out, .handle_bool_out, .handle_i64_out, .handle_void,
        .handle_json_in_str_out, .handle_json_in_i32_out, .handle_json_in_bool_out, .handle_json_in_void,
        => callWithHandle(c_name, pattern, input),
        // Array-input patterns — parse JSON array into N string args
        .handle_arr_str_out, .handle_arr_i32_out, .handle_arr_bool_out, .handle_arr_void,
        => callWithHandleAndArray(c_name, pattern, input),
        .arr_str_out, .arr_i32_out, .arr_bool_out, .arr_void,
        => callWithArray(c_name, pattern, input),
        // Universal: mixed param types marshalled from JSON array
        .universal => callUniversal(c_name, input),
        .multi_arg, .unknown => null,
    };
}

/// Call a handle + N-string function. Input is a JSON array: ["arg1", "arg2", ...].
/// The handle is resolved from state. Each array element becomes a separate const char* arg.
fn callWithHandleAndArray(comptime c_name: []const u8, comptime pattern: CallPattern, input: ?[*:0]const u8) ?[*:0]u8 {
    const in = input orelse return null;

    var bufs: [MAX_ARRAY_ARGS][MAX_ARG_LEN]u8 = undefined;
    var lens: [MAX_ARRAY_ARGS]usize = undefined;
    _ = extractJsonArray(in, &bufs, &lens) orelse return null;

    // Resolve handle
    if (comptime stateFieldForFn(c_name)) |field_name| {
        const s = state.acquireShared();
        defer state.releaseShared();
        const handle = @field(s.*, field_name) orelse return null;
        return dispatchHandleArray(c_name, pattern, handle, &bufs);
    }
    // Module handles — same comptime if-chain as callWithHandle
    return callModuleHandleArray(c_name, pattern, &bufs);
}

/// Call an all-strings function. Input is a JSON array: ["arg1", "arg2", ...].
fn callWithArray(comptime c_name: []const u8, comptime pattern: CallPattern, input: ?[*:0]const u8) ?[*:0]u8 {
    const in = input orelse return null;

    var bufs: [MAX_ARRAY_ARGS][MAX_ARG_LEN]u8 = undefined;
    var lens: [MAX_ARRAY_ARGS]usize = undefined;
    _ = extractJsonArray(in, &bufs, &lens) orelse return null;

    const T = @TypeOf(@field(c, c_name));
    const nparams = @typeInfo(T).@"fn".params.len;
    const raw = callWithNStrings(c_name, nparams, &bufs);
    return wrapRawResult(pattern, raw);
}

/// Dispatch a handle + array call to the right FFI function based on comptime param count.
fn dispatchHandleArray(comptime c_name: []const u8, comptime pattern: CallPattern, handle: anytype, bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8) ?[*:0]u8 {
    const T = @TypeOf(@field(c, c_name));
    const nparams = @typeInfo(T).@"fn".params.len;
    // nparams includes the handle (param 0), so string args = nparams - 1
    const raw = switch (nparams) {
        3 => @field(c, c_name)(handle, @ptrCast(&bufs[0]), @ptrCast(&bufs[1])),
        4 => @field(c, c_name)(handle, @ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2])),
        5 => @field(c, c_name)(handle, @ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2]), @ptrCast(&bufs[3])),
        6 => @field(c, c_name)(handle, @ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2]), @ptrCast(&bufs[3]), @ptrCast(&bufs[4])),
        7 => @field(c, c_name)(handle, @ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2]), @ptrCast(&bufs[3]), @ptrCast(&bufs[4]), @ptrCast(&bufs[5])),
        else => return null,
    };
    return wrapRawResult(pattern, raw);
}

/// Call a pure-string function with N string args from array buffers.
fn callWithNStrings(comptime c_name: []const u8, comptime nparams: usize, bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8) @typeInfo(@TypeOf(@field(c, c_name))).@"fn".return_type.? {
    return switch (nparams) {
        2 => @field(c, c_name)(@ptrCast(&bufs[0]), @ptrCast(&bufs[1])),
        3 => @field(c, c_name)(@ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2])),
        4 => @field(c, c_name)(@ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2]), @ptrCast(&bufs[3])),
        5 => @field(c, c_name)(@ptrCast(&bufs[0]), @ptrCast(&bufs[1]), @ptrCast(&bufs[2]), @ptrCast(&bufs[3]), @ptrCast(&bufs[4])),
        else => unreachable,
    };
}

/// Wrap a raw FFI return value based on the CallPattern.
fn wrapRawResult(comptime pattern: CallPattern, raw: anytype) ?[*:0]u8 {
    return switch (pattern) {
        .handle_arr_str_out, .arr_str_out => coerceStrReturn(raw),
        .handle_arr_i32_out, .arr_i32_out => wrapI32(raw),
        .handle_arr_bool_out, .arr_bool_out => wrapBool(raw),
        .handle_arr_void, .arr_void => wrapVoid(),
        else => null,
    };
}

/// Module handle resolution for array dispatch — mirrors callWithHandle's module branches.
fn callModuleHandleArray(comptime c_name: []const u8, comptime pattern: CallPattern, bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8) ?[*:0]u8 {
    // Governance
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_rights_"))
        return dispatchHandleArray(c_name, pattern, governance.getRights() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_duties_"))
        return dispatchHandleArray(c_name, pattern, governance.getDuties() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_protections_"))
        return dispatchHandleArray(c_name, pattern, governance.getProtections() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_consent_registry_"))
        return dispatchHandleArray(c_name, pattern, governance.getConsent() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_breach_"))
        return dispatchHandleArray(c_name, pattern, governance.getBreaches() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_enactment_registry_"))
        return dispatchHandleArray(c_name, pattern, governance.getEnactments() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_jail_trust_graph_"))
        return dispatchHandleArray(c_name, pattern, governance.getTrustGraph() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_bulwark_permission_checker_"))
        return dispatchHandleArray(c_name, pattern, governance.getPermissionChecker() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_bulwark_consent_validator_"))
        return dispatchHandleArray(c_name, pattern, governance.getConsentValidator() orelse return null, bufs);
    // Commerce
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_ledger_"))
        return dispatchHandleArray(c_name, pattern, commerce.getLedger() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_treasury_"))
        return dispatchHandleArray(c_name, pattern, commerce.getTreasury() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_ubi_"))
        return dispatchHandleArray(c_name, pattern, commerce.getUbi() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_commerce_cart_"))
        return dispatchHandleArray(c_name, pattern, commerce.getCart() orelse return null, bufs);
    // Discovery
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_directory_"))
        return dispatchHandleArray(c_name, pattern, discovery.getDirectory() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_router_"))
        return dispatchHandleArray(c_name, pattern, discovery.getRouter() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_cache_"))
        return dispatchHandleArray(c_name, pattern, discovery.getCache() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_trends_"))
        return dispatchHandleArray(c_name, pattern, discovery.getTrends() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_undercroft_history_"))
        return dispatchHandleArray(c_name, pattern, discovery.getHealthHistory() orelse return null, bufs);
    // AI
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_loop_"))
        return dispatchHandleArray(c_name, pattern, ai_mod.getAdvisorLoop() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_store_"))
        return dispatchHandleArray(c_name, pattern, ai_mod.getAdvisorStore() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_router_"))
        return dispatchHandleArray(c_name, pattern, ai_mod.getAdvisorRouter() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_skills_"))
        return dispatchHandleArray(c_name, pattern, ai_mod.getAdvisorSkills() orelse return null, bufs);
    if (comptime std.mem.startsWith(u8, c_name, "divi_oracle_registry_"))
        return dispatchHandleArray(c_name, pattern, ai_mod.getOracleRegistry() orelse return null, bufs);
    // Lingo
    if (comptime std.mem.startsWith(u8, c_name, "divi_lingo_babel_"))
        return dispatchHandleArray(c_name, pattern, lingo_mod.getBabel() orelse return null, bufs);
    // Crown (global state — handled above in callWithHandleAndArray)
    return null;
}

/// Generic dispatch — calls FFI function with a resolved handle of any type.
fn dispatchWithHandle(comptime c_name: []const u8, comptime pattern: CallPattern, handle: anytype, input: ?[*:0]const u8) ?[*:0]u8 {
    return switch (pattern) {
        .handle_str_out => coerceStrReturn(@field(c, c_name)(handle)),
        .handle_i32_out => wrapI32(@field(c, c_name)(handle)),
        .handle_bool_out => wrapBool(@field(c, c_name)(handle)),
        .handle_i64_out => wrapI64(@field(c, c_name)(handle)),
        .handle_void => blk: {
            @field(c, c_name)(handle);
            break :blk wrapVoid();
        },
        .handle_json_in_str_out => coerceStrReturn(@field(c, c_name)(handle, input orelse return null)),
        .handle_json_in_i32_out => wrapI32(@field(c, c_name)(handle, input orelse return null)),
        .handle_json_in_bool_out => wrapBool(@field(c, c_name)(handle, input orelse return null)),
        .handle_json_in_void => blk: {
            @field(c, c_name)(handle, input orelse return null);
            break :blk wrapVoid();
        },
        else => null,
    };
}

/// Resolve handle from global state or module state, then dispatch.
/// Comptime if-chains ensure only the matching branch compiles per function.
fn callWithHandle(comptime c_name: []const u8, comptime pattern: CallPattern, input: ?[*:0]const u8) ?[*:0]u8 {
    // ── Global state handles ──
    if (comptime stateFieldForFn(c_name)) |field_name| {
        const s = state.acquireShared();
        defer state.releaseShared();
        const handle = @field(s.*, field_name) orelse return null;
        return dispatchWithHandle(c_name, pattern, handle, input);
    }

    // ── Governance handles ──
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_rights_"))
        return dispatchWithHandle(c_name, pattern, governance.getRights() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_duties_"))
        return dispatchWithHandle(c_name, pattern, governance.getDuties() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_protections_"))
        return dispatchWithHandle(c_name, pattern, governance.getProtections() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_consent_registry_"))
        return dispatchWithHandle(c_name, pattern, governance.getConsent() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_breach_"))
        return dispatchWithHandle(c_name, pattern, governance.getBreaches() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_enactment_registry_"))
        return dispatchWithHandle(c_name, pattern, governance.getEnactments() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_jail_trust_graph_"))
        return dispatchWithHandle(c_name, pattern, governance.getTrustGraph() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_bulwark_permission_checker_"))
        return dispatchWithHandle(c_name, pattern, governance.getPermissionChecker() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_bulwark_consent_validator_"))
        return dispatchWithHandle(c_name, pattern, governance.getConsentValidator() orelse return null, input);

    // ── Commerce handles ──
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_ledger_"))
        return dispatchWithHandle(c_name, pattern, commerce.getLedger() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_treasury_"))
        return dispatchWithHandle(c_name, pattern, commerce.getTreasury() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_ubi_"))
        return dispatchWithHandle(c_name, pattern, commerce.getUbi() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_commerce_cart_"))
        return dispatchWithHandle(c_name, pattern, commerce.getCart() orelse return null, input);

    // ── Discovery handles ──
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_directory_"))
        return dispatchWithHandle(c_name, pattern, discovery.getDirectory() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_router_"))
        return dispatchWithHandle(c_name, pattern, discovery.getRouter() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_cache_"))
        return dispatchWithHandle(c_name, pattern, discovery.getCache() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_trends_"))
        return dispatchWithHandle(c_name, pattern, discovery.getTrends() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_undercroft_history_"))
        return dispatchWithHandle(c_name, pattern, discovery.getHealthHistory() orelse return null, input);

    // ── AI handles ──
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_loop_"))
        return dispatchWithHandle(c_name, pattern, ai_mod.getAdvisorLoop() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_store_"))
        return dispatchWithHandle(c_name, pattern, ai_mod.getAdvisorStore() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_router_"))
        return dispatchWithHandle(c_name, pattern, ai_mod.getAdvisorRouter() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_skills_"))
        return dispatchWithHandle(c_name, pattern, ai_mod.getAdvisorSkills() orelse return null, input);
    if (comptime std.mem.startsWith(u8, c_name, "divi_oracle_registry_"))
        return dispatchWithHandle(c_name, pattern, ai_mod.getOracleRegistry() orelse return null, input);

    // ── Lingo handle ──
    if (comptime std.mem.startsWith(u8, c_name, "divi_lingo_babel_"))
        return dispatchWithHandle(c_name, pattern, lingo_mod.getBabel() orelse return null, input);

    return null;
}

/// Coerce various C string return types to ?[*:0]u8.
/// C's `char *` maps to `[*c]u8` in Zig. We cast through `?*anyopaque`
/// to avoid pointer attribute mismatch, then reinterpret as sentinel-terminated.
fn coerceStrReturn(raw: anytype) ?[*:0]u8 {
    const T = @TypeOf(raw);
    if (T == ?[*:0]u8) return raw;
    if (T == [*c]u8) {
        // [*c]u8 is nullable — check for null
        const ptr: ?*anyopaque = @ptrCast(raw);
        if (ptr == null) return null;
        return @ptrCast(raw);
    }
    return null;
}

/// Generate a trampoline HandlerFn for a specific FFI function at comptime.
fn makeTrampoline(comptime c_name: []const u8, comptime pattern: CallPattern) HandlerFn {
    return struct {
        fn call(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            return callByPattern(c_name, pattern, input);
        }
    }.call;
}

// ── JSON Wrapping Helpers ──────────────────────────────────────

/// Helper: wrap an i32 as a JSON string {"result": N}.
fn wrapI32(val: i32) ?[*:0]u8 {
    var buf: [64]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{val}) catch return null;
    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Helper: wrap a bool as a JSON string {"result": true/false}.
fn wrapBool(val: bool) ?[*:0]u8 {
    const s = if (val) "{\"result\":true}" else "{\"result\":false}";
    const out = allocator.allocSentinel(u8, s.len, 0) catch return null;
    @memcpy(out[0..s.len], s);
    return out.ptr;
}

/// Helper: wrap a usize as a JSON string {"result": N}.
fn wrapUsize(val: usize) ?[*:0]u8 {
    var buf: [64]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{val}) catch return null;
    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Helper: wrap an i64 as a JSON string {"result": N}.
fn wrapI64(val: i64) ?[*:0]u8 {
    var buf: [64]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{val}) catch return null;
    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Helper: wrap void operations as {"result": null}.
fn wrapVoid() ?[*:0]u8 {
    const s = "{\"result\":null}";
    const out = allocator.allocSentinel(u8, s.len, 0) catch return null;
    @memcpy(out[0..s.len], s);
    return out.ptr;
}

/// Helper: wrap a u32 as a JSON string {"result": N}.
fn wrapU32(val: u32) ?[*:0]u8 {
    var buf: [64]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{val}) catch return null;
    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Helper: wrap a u64 as a JSON string {"result": N}.
fn wrapU64(val: u64) ?[*:0]u8 {
    var buf: [64]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{val}) catch return null;
    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Helper: wrap a f64 as a JSON string {"result": N}.
fn wrapF64(val: f64) ?[*:0]u8 {
    var buf: [128]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{val}) catch return null;
    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Wrap any supported return type as JSON based on its comptime type.
fn wrapReturnValue(comptime Ret: type, val: Ret) ?[*:0]u8 {
    if (Ret == ?[*:0]u8 or Ret == [*c]u8) return coerceStrReturn(val);
    if (Ret == i32 or Ret == c_int) return wrapI32(val);
    if (Ret == bool) return wrapBool(val);
    if (Ret == usize) return wrapUsize(val);
    if (Ret == i64 or Ret == c_longlong) return wrapI64(val);
    if (Ret == u32 or Ret == c_uint) return wrapU32(val);
    if (Ret == u16 or Ret == c_ushort) return wrapU32(@intCast(val));
    if (Ret == i16 or Ret == c_short) return wrapI32(@intCast(val));
    if (Ret == u64 or Ret == c_ulonglong) return wrapU64(val);
    if (Ret == f64 or Ret == f32) return wrapF64(val);
    if (Ret == void) return wrapVoid();
    // Handle return (opaque pointer from _new constructors) — wrap as integer address
    if (comptime isHandleParam(Ret)) {
        const ptr_val: ?*anyopaque = @ptrCast(val);
        if (ptr_val == null) return null;
        const addr = @intFromPtr(ptr_val.?);
        var buf: [64]u8 = undefined;
        const slice = std.fmt.bufPrint(&buf, "{{\"result\":{d}}}", .{addr}) catch return null;
        const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
        @memcpy(out[0..slice.len], slice);
        return out.ptr;
    }
    return null;
}

// ── Universal Dispatch ─────────────────────────────────────────
//
// Handles mixed-type parameter functions that don't fit the specialized
// patterns. At comptime, inspects each parameter's type and generates
// type-specific extraction from the JSON array input.
//
// Input format: JSON array with positional args.
//   - First-param handles are resolved from state/modules (not in array)
//   - Strings: "value" (quoted)
//   - Numbers: 42, -1, 3.14 (unquoted)
//   - Bools: true/false (unquoted)
//
// Examples:
//   divi_zeitgeist_cache_put(cache, query, results, now_i64)
//     Input: ["search term", "{}", 1234567890]
//     Handle resolved from state, 3 JSON array elements
//
//   divi_regalia_contrast_ratio(r1, g1, b1, r2, g2, b2)
//     Input: [0.5, 0.3, 0.1, 1.0, 1.0, 1.0]
//     No handle, 6 JSON array elements

/// Parse a JSON array element buffer into the target comptime type.
/// The buffer is null-terminated by extractJsonArray. We find the null
/// terminator first to avoid reading undefined memory beyond it.
fn parseJsonArg(comptime T: type, buf: []const u8) ?T {
    if (comptime isStrParam(T)) {
        // String params: the buffer IS the string content (already null-terminated by extractJsonArray).
        // Explicitly type the intermediate to avoid double-optional issues with [*c] types
        // ([*c]const u8 is inherently nullable, so ?[*c]const u8 would be a double-optional).
        const ptr: T = @ptrCast(buf.ptr);
        return ptr;
    }

    // Find the null terminator — extractJsonArray always null-terminates.
    // This is critical: the buffer beyond the null may be undefined memory.
    const end = std.mem.indexOfScalar(u8, buf, 0) orelse buf.len;
    const content = std.mem.trim(u8, buf[0..end], &.{ ' ', '\t', '\n', '\r' });
    if (content.len == 0) return null;

    if (comptime isFloatParam(T)) {
        const val = std.fmt.parseFloat(f64, content) catch return null;
        return if (T == f32) @floatCast(val) else val;
    }
    if (comptime isNumericParam(T)) {
        return std.fmt.parseInt(T, content, 10) catch return null;
    }
    if (comptime isBoolParam(T)) {
        if (std.mem.eql(u8, content, "true")) return true;
        if (std.mem.eql(u8, content, "false")) return false;
        return null;
    }
    // Handle params: parse integer address from JSON (from prior pipeline step outputs).
    // Pipeline steps that return handles (e.g., _new constructors) emit {"result": <addr>}.
    // Subsequent steps reference these via $ref, which resolves to the integer address.
    if (comptime isHandleParam(T)) {
        const addr = std.fmt.parseInt(usize, content, 10) catch return null;
        if (addr == 0) return null;
        const info = @typeInfo(T);
        if (info == .optional) {
            const Inner = info.optional.child;
            const ptr: Inner = @ptrFromInt(addr);
            return ptr;
        }
        return @ptrFromInt(addr);
    }
    return null;
}

/// Count how many JSON array elements a universally-callable function needs.
/// Handles are resolved from state (not from array), so they don't consume an element.
fn universalJsonArgCount(comptime name: []const u8) comptime_int {
    const T = @TypeOf(@field(c, name));
    const params = @typeInfo(T).@"fn".params;
    var n: comptime_int = 0;
    for (params, 0..) |p, i| {
        const PT = p.type orelse continue;
        if (i == 0 and isHandleParam(PT) and hasResolvableHandle(name)) continue;
        if (isOutParam(PT)) continue; // out-params don't consume JSON slots
        n += 1;
    }
    return n;
}

/// Check if the first parameter of a universally-callable function is a resolvable handle.
fn universalHasHandle(comptime name: []const u8) bool {
    const T = @TypeOf(@field(c, name));
    const params = @typeInfo(T).@"fn".params;
    if (params.len == 0) return false;
    const P0 = params[0].type orelse return false;
    return isHandleParam(P0) and hasResolvableHandle(name);
}

/// Resolve a handle for a universally-callable function from module state.
/// Module handles have their own mutexes; getters acquire/release internally.
/// The returned handles remain valid because module shutdown is coordinated
/// with orch_shutdown.
///
/// NOTE: Global state handles are resolved directly in callUniversal (which
/// holds the shared lock through the FFI call). This function is only called
/// for module handles.
fn resolveUniversalHandle(comptime c_name: []const u8) ?*anyopaque {
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_rights_"))
        return if (governance.getRights()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_duties_"))
        return if (governance.getDuties()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_protections_"))
        return if (governance.getProtections()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_consent_registry_"))
        return if (governance.getConsent()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_breach_"))
        return if (governance.getBreaches()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_polity_enactment_registry_"))
        return if (governance.getEnactments()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_jail_trust_graph_"))
        return if (governance.getTrustGraph()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_bulwark_permission_checker_"))
        return if (governance.getPermissionChecker()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_bulwark_consent_validator_"))
        return if (governance.getConsentValidator()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_ledger_"))
        return if (commerce.getLedger()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_treasury_"))
        return if (commerce.getTreasury()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_fortune_ubi_"))
        return if (commerce.getUbi()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_commerce_cart_"))
        return if (commerce.getCart()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_directory_"))
        return if (discovery.getDirectory()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_router_"))
        return if (discovery.getRouter()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_cache_"))
        return if (discovery.getCache()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_zeitgeist_trends_"))
        return if (discovery.getTrends()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_undercroft_history_"))
        return if (discovery.getHealthHistory()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_loop_"))
        return if (ai_mod.getAdvisorLoop()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_store_"))
        return if (ai_mod.getAdvisorStore()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_router_"))
        return if (ai_mod.getAdvisorRouter()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_advisor_skills_"))
        return if (ai_mod.getAdvisorSkills()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_oracle_registry_"))
        return if (ai_mod.getOracleRegistry()) |h| @ptrCast(@constCast(h)) else null;
    if (comptime std.mem.startsWith(u8, c_name, "divi_lingo_babel_"))
        return if (lingo_mod.getBabel()) |h| @ptrCast(@constCast(h)) else null;
    return null;
}

/// Universal dispatch for mixed-parameter functions.
/// Parses JSON array elements into the correct types at comptime, resolves
/// handles from state, calls the FFI function, and wraps the return value.
fn callUniversal(comptime c_name: []const u8, input: ?[*:0]const u8) ?[*:0]u8 {
    const T = @TypeOf(@field(c, c_name));
    const fn_info = @typeInfo(T).@"fn";
    const params = fn_info.params;
    const has_handle = comptime universalHasHandle(c_name);
    const json_arg_count = comptime universalJsonArgCount(c_name);

    // Parse JSON array from input (if we need any args from it)
    var bufs: [MAX_ARRAY_ARGS][MAX_ARG_LEN]u8 = undefined;
    _ = &bufs; // may be unused for 0-arg functions

    if (json_arg_count > 0) {
        const in = input orelse return null;
        var lens: [MAX_ARRAY_ARGS]usize = undefined;
        const n_parsed = extractJsonArray(in, &bufs, &lens) orelse return null;
        if (n_parsed < json_arg_count) return null;
    }

    // Resolve handle if needed.
    // For global state handles, we hold the shared lock during the FFI call
    // to prevent use-after-free from concurrent shutdown.
    const is_global_handle = comptime (has_handle and stateFieldForFn(c_name) != null);
    var handle_ptr: ?*anyopaque = null;
    var holding_state_lock = false;
    if (has_handle) {
        if (is_global_handle) {
            // Acquire lock for global state handles — held through FFI call
            const s = state.acquireShared();
            const handle = @field(s.*, stateFieldForFn(c_name).?);
            if (handle == null) {
                state.releaseShared();
                return null;
            }
            handle_ptr = @ptrCast(@constCast(handle.?));
            holding_state_lock = true;
        } else {
            // Module handles — getters acquire/release their own locks
            handle_ptr = resolveUniversalHandle(c_name);
            if (handle_ptr == null) return null;
        }
    }
    // Release global state lock after FFI call (if we acquired it)
    defer {
        if (holding_state_lock) state.releaseShared();
    }

    // Reset out-param thread-local storage before dispatch
    const has_out = comptime hasOutParams(c_name);
    if (has_out) resetOutParams();

    // Dispatch to the correct arity handler.
    // Each handler resolves args, calls the FFI function, and wraps the result.
    const raw_result = switch (params.len) {
        0 => wrapUniversalResult(c_name, @field(c, c_name)()),
        1 => callUniversal1(c_name, params, has_handle, handle_ptr, &bufs),
        2 => callUniversal2(c_name, params, has_handle, handle_ptr, &bufs),
        3 => callUniversal3(c_name, params, has_handle, handle_ptr, &bufs),
        4 => callUniversal4(c_name, params, has_handle, handle_ptr, &bufs),
        5 => callUniversal5(c_name, params, has_handle, handle_ptr, &bufs),
        6 => callUniversal6(c_name, params, has_handle, handle_ptr, &bufs),
        7 => callUniversal7(c_name, params, has_handle, handle_ptr, &bufs),
        8 => callUniversal8(c_name, params, has_handle, handle_ptr, &bufs),
        else => null,
    };

    // If this function has out-params, post-process to include the output data.
    // Binary out-params (uint8_t** + uintptr_t*) are hex-encoded in the result.
    if (has_out and out_ptr_count > 0) {
        return wrapWithOutParams(raw_result);
    }
    return raw_result;
}

/// Resolve a single argument at a given param index.
/// If it's the handle (index 0 and has_handle), cast from handle_ptr.
/// Otherwise, parse from the JSON array buffer at the appropriate index.
fn resolveArg(
    comptime PT: type,
    comptime param_idx: usize,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
    comptime json_idx: usize,
) ?PT {
    if (param_idx == 0 and has_handle) {
        // Cast the anyopaque handle pointer to the expected type.
        // PT may be ?*const T (C pointers are nullable in @cImport).
        // @ptrCast produces the inner pointer type; optional wrapping is automatic.
        const info = @typeInfo(PT);
        if (info == .optional) {
            const Inner = info.optional.child;
            const ptr: Inner = @ptrCast(@alignCast(handle_ptr.?));
            return ptr;
        }
        return @ptrCast(@alignCast(handle_ptr.?));
    }
    // Out-param types: return pointers to thread-local storage.
    // The FFI function writes to these; results are read after the call.
    if (comptime isOutParam(PT)) {
        return resolveOutParam(PT);
    }
    // Parse from JSON array buffer
    return parseJsonArg(PT, bufs[json_idx][0..MAX_ARG_LEN]);
}

/// Resolve an out-param by returning a pointer to thread-local storage.
/// Each call advances the slot counter for the relevant type.
fn resolveOutParam(comptime PT: type) ?PT {
    if (PT == [*c][*c]u8) {
        if (out_ptr_count >= 4) return null;
        const idx = out_ptr_count;
        out_ptr_count += 1;
        out_ptrs[idx] = null;
        return @ptrCast(&out_ptrs[idx]);
    }
    if (PT == [*c]usize) {
        if (out_len_count >= 4) return null;
        const idx = out_len_count;
        out_len_count += 1;
        out_lens[idx] = 0;
        return @ptrCast(&out_lens[idx]);
    }
    // [*c]u32 and [*c]i32 out-params: use the len slots (cast on read)
    if (PT == [*c]u32 or PT == [*c]i32) {
        if (out_len_count >= 4) return null;
        const idx = out_len_count;
        out_len_count += 1;
        out_lens[idx] = 0;
        return @ptrCast(&out_lens[idx]);
    }
    return null;
}

/// Compute the JSON array index for a given param position.
/// When has_handle is true, param 0 doesn't consume a JSON slot.
/// For param 0 with handle, returns 0 (unused — resolveArg short-circuits).
fn jsonIdx(comptime param_idx: usize, comptime has_handle: bool) usize {
    if (!has_handle) return param_idx;
    if (param_idx == 0) return 0; // handle case — index not used by resolveArg
    return param_idx - 1;
}

fn callUniversal1(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0));
}

fn callUniversal2(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1));
}

fn callUniversal3(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const P2 = params[2].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    const a2 = resolveArg(P2, 2, has_handle, handle_ptr, bufs, comptime jsonIdx(2, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1, a2));
}

fn callUniversal4(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const P2 = params[2].type.?;
    const P3 = params[3].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    const a2 = resolveArg(P2, 2, has_handle, handle_ptr, bufs, comptime jsonIdx(2, has_handle)) orelse return null;
    const a3 = resolveArg(P3, 3, has_handle, handle_ptr, bufs, comptime jsonIdx(3, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1, a2, a3));
}

fn callUniversal5(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const P2 = params[2].type.?;
    const P3 = params[3].type.?;
    const P4 = params[4].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    const a2 = resolveArg(P2, 2, has_handle, handle_ptr, bufs, comptime jsonIdx(2, has_handle)) orelse return null;
    const a3 = resolveArg(P3, 3, has_handle, handle_ptr, bufs, comptime jsonIdx(3, has_handle)) orelse return null;
    const a4 = resolveArg(P4, 4, has_handle, handle_ptr, bufs, comptime jsonIdx(4, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1, a2, a3, a4));
}

fn callUniversal6(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const P2 = params[2].type.?;
    const P3 = params[3].type.?;
    const P4 = params[4].type.?;
    const P5 = params[5].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    const a2 = resolveArg(P2, 2, has_handle, handle_ptr, bufs, comptime jsonIdx(2, has_handle)) orelse return null;
    const a3 = resolveArg(P3, 3, has_handle, handle_ptr, bufs, comptime jsonIdx(3, has_handle)) orelse return null;
    const a4 = resolveArg(P4, 4, has_handle, handle_ptr, bufs, comptime jsonIdx(4, has_handle)) orelse return null;
    const a5 = resolveArg(P5, 5, has_handle, handle_ptr, bufs, comptime jsonIdx(5, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1, a2, a3, a4, a5));
}

fn callUniversal7(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const P2 = params[2].type.?;
    const P3 = params[3].type.?;
    const P4 = params[4].type.?;
    const P5 = params[5].type.?;
    const P6 = params[6].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    const a2 = resolveArg(P2, 2, has_handle, handle_ptr, bufs, comptime jsonIdx(2, has_handle)) orelse return null;
    const a3 = resolveArg(P3, 3, has_handle, handle_ptr, bufs, comptime jsonIdx(3, has_handle)) orelse return null;
    const a4 = resolveArg(P4, 4, has_handle, handle_ptr, bufs, comptime jsonIdx(4, has_handle)) orelse return null;
    const a5 = resolveArg(P5, 5, has_handle, handle_ptr, bufs, comptime jsonIdx(5, has_handle)) orelse return null;
    const a6 = resolveArg(P6, 6, has_handle, handle_ptr, bufs, comptime jsonIdx(6, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1, a2, a3, a4, a5, a6));
}

fn callUniversal8(
    comptime c_name: []const u8,
    comptime params: anytype,
    comptime has_handle: bool,
    handle_ptr: ?*anyopaque,
    bufs: *[MAX_ARRAY_ARGS][MAX_ARG_LEN]u8,
) ?[*:0]u8 {
    const P0 = params[0].type.?;
    const P1 = params[1].type.?;
    const P2 = params[2].type.?;
    const P3 = params[3].type.?;
    const P4 = params[4].type.?;
    const P5 = params[5].type.?;
    const P6 = params[6].type.?;
    const P7 = params[7].type.?;
    const a0 = resolveArg(P0, 0, has_handle, handle_ptr, bufs, comptime jsonIdx(0, has_handle)) orelse return null;
    const a1 = resolveArg(P1, 1, has_handle, handle_ptr, bufs, comptime jsonIdx(1, has_handle)) orelse return null;
    const a2 = resolveArg(P2, 2, has_handle, handle_ptr, bufs, comptime jsonIdx(2, has_handle)) orelse return null;
    const a3 = resolveArg(P3, 3, has_handle, handle_ptr, bufs, comptime jsonIdx(3, has_handle)) orelse return null;
    const a4 = resolveArg(P4, 4, has_handle, handle_ptr, bufs, comptime jsonIdx(4, has_handle)) orelse return null;
    const a5 = resolveArg(P5, 5, has_handle, handle_ptr, bufs, comptime jsonIdx(5, has_handle)) orelse return null;
    const a6 = resolveArg(P6, 6, has_handle, handle_ptr, bufs, comptime jsonIdx(6, has_handle)) orelse return null;
    const a7 = resolveArg(P7, 7, has_handle, handle_ptr, bufs, comptime jsonIdx(7, has_handle)) orelse return null;
    return wrapUniversalResult(c_name, @field(c, c_name)(a0, a1, a2, a3, a4, a5, a6, a7));
}

/// Post-process a result to include out-param binary data.
/// Hex-encodes each binary out-param (ptr + len pair) and builds a combined JSON result.
/// Frees FFI-allocated out-param data with divi_free_bytes.
fn wrapWithOutParams(raw_result: ?[*:0]u8) ?[*:0]u8 {
    // Build result with out-param data
    var result_buf: [16384]u8 = undefined;
    var pos: usize = 0;

    // Start with the raw result (status code etc.)
    const prefix = "{\"status\":";
    @memcpy(result_buf[pos .. pos + prefix.len], prefix);
    pos += prefix.len;

    // Extract status from raw_result if available
    if (raw_result) |r| {
        const rstr = std.mem.span(r);
        // Find the value after "result":
        if (std.mem.indexOf(u8, rstr, "\"result\":")) |idx| {
            const val_start = idx + 9;
            const val_end = std.mem.indexOf(u8, rstr[val_start..], "}") orelse rstr.len - val_start;
            const val = rstr[val_start .. val_start + val_end];
            @memcpy(result_buf[pos .. pos + val.len], val);
            pos += val.len;
        } else {
            result_buf[pos] = '0';
            pos += 1;
        }
        allocator.free(r[0 .. rstr.len + 1]);
    } else {
        result_buf[pos] = '0';
        pos += 1;
    }

    // Add hex-encoded out-param data for each ptr+len pair
    var i: u8 = 0;
    while (i < out_ptr_count) : (i += 1) {
        const data_ptr = out_ptrs[i];
        const data_len = if (i < out_len_count) out_lens[i] else 0;

        const key = if (i == 0) ",\"data\":\"" else ",\"data2\":\"";
        @memcpy(result_buf[pos .. pos + key.len], key);
        pos += key.len;

        if (data_ptr != null and data_len > 0) {
            const data = @as([*]const u8, @ptrCast(data_ptr))[0..data_len];
            // Hex-encode
            for (data) |byte| {
                if (pos + 2 >= result_buf.len) break;
                const hex = "0123456789abcdef";
                result_buf[pos] = hex[byte >> 4];
                result_buf[pos + 1] = hex[byte & 0x0f];
                pos += 2;
            }
            // Free FFI-allocated data
            c.divi_free_bytes(data_ptr, data_len);
        }

        result_buf[pos] = '"';
        pos += 1;
    }

    result_buf[pos] = '}';
    pos += 1;

    // Allocate and return the result string
    const out = allocator.allocSentinel(u8, pos, 0) catch return null;
    @memcpy(out[0..pos], result_buf[0..pos]);
    return out.ptr;
}

/// Wrap the raw return value of a universal FFI call as JSON.
/// Handles void, string, numeric, bool, float, and handle return types.
fn wrapUniversalResult(comptime c_name: []const u8, raw: anytype) ?[*:0]u8 {
    _ = c_name;
    const Ret = @TypeOf(raw);
    if (Ret == void) return wrapVoid();
    return wrapReturnValue(Ret, raw);
}

// ── Public API ─────────────────────────────────────────────────

/// Initialize the registry. For the comptime registry this just sets up
/// the third-party HashMap. Returns 0 on success, -1 on error.
/// Idempotent: calling multiple times is safe.
pub fn init() i32 {
    tp_lock.lock();
    defer tp_lock.unlock();

    if (third_party != null) return 0;
    third_party = ThirdPartyMap.init(allocator);
    return 0;
}

/// Shut down the registry. Frees the third-party HashMap.
/// Idempotent: calling multiple times is safe.
pub fn deinit() void {
    tp_lock.lock();
    defer tp_lock.unlock();

    if (third_party) |*m| {
        m.deinit();
        third_party = null;
    }
}

/// Register or replace a third-party operation.
/// Thread-safe (acquires exclusive lock).
pub fn register(key: []const u8, handler: OpHandler) !void {
    tp_lock.lock();
    defer tp_lock.unlock();

    if (third_party) |*m| {
        try m.put(key, handler);
    }
}

/// Look up an operation by key. Checks comptime dispatch first,
/// then falls through to third-party registry.
/// Thread-safe.
pub fn lookup(key: []const u8) ?OpHandler {
    // Check comptime table first
    if (comptimeLookup(key)) |handler| return handler;

    // Fall through to third-party
    tp_lock.lockShared();
    defer tp_lock.unlockShared();

    if (third_party) |m| {
        return m.get(key);
    }
    return null;
}

/// Execute an operation by name. Dispatches through comptime first,
/// then falls through to third-party.
pub fn execute(op_name: []const u8, input: ?[*:0]const u8) ?[*:0]u8 {
    // Try comptime dispatch first
    if (comptimeDispatch(op_name, input)) |result| return result;

    // Fall through to third-party
    tp_lock.lockShared();
    defer tp_lock.unlockShared();

    if (third_party) |m| {
        if (m.get(op_name)) |handler| {
            return handler.call(input orelse return null);
        }
    }
    return null;
}

/// Check if an operation is registered (comptime or third-party).
pub fn has(key: []const u8) bool {
    if (comptimeHasOp(key)) return true;

    tp_lock.lockShared();
    defer tp_lock.unlockShared();

    if (third_party) |m| {
        return m.contains(key);
    }
    return false;
}

/// Get the total number of operations (comptime + third-party).
pub fn count() usize {
    var n: usize = comptime_op_count;

    tp_lock.lockShared();
    defer tp_lock.unlockShared();

    if (third_party) |m| {
        n += m.count();
    }
    return n;
}

/// Returns a JSON array of all registered operation names.
/// Caller must free the returned string with c_allocator.
/// Returns null on error.
pub fn listOps() ?[*:0]u8 {
    @setEvalBranchQuota(20_000_000);

    var result: std.ArrayListUnmanaged(u8) = .empty;
    defer result.deinit(allocator);

    result.ensureTotalCapacity(allocator, 32768) catch return null;

    result.append(allocator, '[') catch return null;

    var first = true;

    // Comptime ops — inline iterate over C declarations
    inline for (@typeInfo(c).@"struct".decls) |decl| {
        if (comptime isDispatchable(decl.name)) {
            if (!first) {
                result.append(allocator, ',') catch return null;
            }
            first = false;
            result.append(allocator, '"') catch return null;
            result.appendSlice(allocator, comptime cNameToOpName(decl.name)) catch return null;
            result.append(allocator, '"') catch return null;
        }
    }

    // Third-party ops
    {
        tp_lock.lockShared();
        defer tp_lock.unlockShared();

        if (third_party) |m| {
            var iter = m.iterator();
            while (iter.next()) |entry| {
                if (!first) {
                    result.append(allocator, ',') catch return null;
                }
                first = false;
                result.append(allocator, '"') catch return null;
                result.appendSlice(allocator, entry.key_ptr.*) catch return null;
                result.append(allocator, '"') catch return null;
            }
        }
    }

    result.append(allocator, ']') catch return null;

    // Copy to sentinel-terminated C string
    const len = result.items.len;
    const out = allocator.allocSentinel(u8, len, 0) catch return null;
    @memcpy(out[0..len], result.items);
    return out.ptr;
}

// ── Exported C API ─────────────────────────────────────────────

/// Initialize the operation registry.
/// For the comptime-based registry, this only initializes the third-party HashMap.
/// Must be called after orch_init(). Idempotent.
/// Returns 0 on success.
pub export fn orch_registry_init() callconv(.c) i32 {
    return init();
}

/// Shut down the operation registry and free third-party memory.
/// Safe to call multiple times.
pub export fn orch_registry_shutdown() callconv(.c) void {
    deinit();
}

/// Check if an operation is registered (comptime or third-party).
pub export fn orch_registry_has_op(key: [*:0]const u8) callconv(.c) bool {
    return has(std.mem.span(key));
}

/// Get a JSON array of all registered operation names.
/// Caller must free with c_allocator (NOT divi_free_string — this is Zig-allocated).
pub export fn orch_registry_list_ops() callconv(.c) ?[*:0]u8 {
    return listOps();
}

/// Get the number of registered operations.
pub export fn orch_registry_count() callconv(.c) i32 {
    return @intCast(count());
}

/// Register a custom operation handler from the platform side.
/// key: null-terminated operation name (e.g., "myapp.custom_op")
/// handler: function pointer matching HandlerFn signature
/// Returns 0 on success, -1 on error.
pub export fn orch_registry_register(
    key: [*:0]const u8,
    handler: ?*const fn ([*:0]const u8) callconv(.c) ?[*:0]u8,
) callconv(.c) i32 {
    const h = handler orelse return -1;
    register(std.mem.span(key), .{
        .call = h,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    }) catch return -1;
    return 0;
}

// ── Tests ──────────────────────────────────────────────────────

test "init and shutdown" {
    const rc = init();
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expect(count() > 0);
    deinit();
}

test "double init is idempotent" {
    try std.testing.expectEqual(@as(i32, 0), init());
    const c1 = count();
    try std.testing.expectEqual(@as(i32, 0), init());
    try std.testing.expectEqual(c1, count());
    deinit();
}

test "double shutdown is safe" {
    try std.testing.expectEqual(@as(i32, 0), init());
    deinit();
    deinit(); // should not crash
}

test "comptime ops are available without init" {
    // Comptime ops don't need init — they're always available
    try std.testing.expect(count() > 50);
}

test "lookup existing operation" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const handler = lookup("polity.axioms");
    try std.testing.expect(handler != null);
    try std.testing.expectEqual(PermissionLevel.free, handler.?.permission);
}

test "lookup non-existent returns null" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    try std.testing.expect(lookup("nonexistent.op") == null);
}

test "has returns true for registered ops" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    try std.testing.expect(has("polity.axioms"));
    try std.testing.expect(has("regalia.default_reign"));
    try std.testing.expect(!has("fake.operation"));
}

test "count is nonzero after init" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const n = count();
    try std.testing.expect(n > 50);
}

test "register custom operation" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const before = count();

    try register("test.custom_op", .{
        .call = struct {
            fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
                return null;
            }
        }.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    });

    try std.testing.expect(has("test.custom_op"));
    try std.testing.expectEqual(before + 1, count());
}

test "overwrite existing registration" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const before = count();

    // Register a custom op
    try register("test.overwrite_me", .{
        .call = struct {
            fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
                return null;
            }
        }.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    });

    try std.testing.expectEqual(before + 1, count());

    // Overwrite it — count should not change
    try register("test.overwrite_me", .{
        .call = struct {
            fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
                return null;
            }
        }.call,
        .handles = &.{},
        .permission = .per_action,
        .modifiers = .{},
    });

    try std.testing.expectEqual(before + 1, count());
    const h = lookup("test.overwrite_me");
    try std.testing.expect(h != null);
    try std.testing.expectEqual(PermissionLevel.per_action, h.?.permission);
}

test "listOps returns valid JSON" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const json = listOps();
    try std.testing.expect(json != null);

    if (json) |j| {
        const str = std.mem.span(j);
        // Must start with [ and end with ]
        try std.testing.expect(str.len >= 2);
        try std.testing.expectEqual(@as(u8, '['), str[0]);
        try std.testing.expectEqual(@as(u8, ']'), str[str.len - 1]);
        // Must contain at least one known operation
        try std.testing.expect(std.mem.indexOf(u8, str, "polity.axioms") != null);
        allocator.free(j[0 .. str.len + 1]);
    }
}

test "export functions work" {
    try std.testing.expectEqual(@as(i32, 0), orch_registry_init());
    defer orch_registry_shutdown();

    try std.testing.expect(orch_registry_count() > 0);
    try std.testing.expect(orch_registry_has_op("polity.axioms"));
    try std.testing.expect(!orch_registry_has_op("fake.nothing"));
}

test "stateless handler invocation" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Call polity.axioms — a no-arg stateless function
    const handler = lookup("polity.axioms");
    try std.testing.expect(handler != null);

    const result = handler.?.call("");
    // divi_polity_axioms() should return non-null JSON
    try std.testing.expect(result != null);

    if (result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(str.len > 0);
        c.divi_free_string(r);
    }
}

test "sentinal recovery_generate handler" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const handler = lookup("sentinal.recovery_generate");
    try std.testing.expect(handler != null);
    try std.testing.expectEqual(PermissionLevel.per_action, handler.?.permission);
    try std.testing.expect(handler.?.modifiers.sentinal);

    const result = handler.?.call("");
    try std.testing.expect(result != null);
    if (result) |r| {
        c.divi_free_string(r);
    }
}

test "regalia.default_reign handler returns JSON" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const handler = lookup("regalia.default_reign");
    try std.testing.expect(handler != null);

    const result = handler.?.call("");
    try std.testing.expect(result != null);
    if (result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(str.len > 0);
        try std.testing.expectEqual(@as(u8, '{'), str[0]);
        c.divi_free_string(r);
    }
}

test "modifier sets work" {
    const m1 = ModifierSet{ .polity = true, .yoke = true };
    try std.testing.expect(m1.polity);
    try std.testing.expect(m1.yoke);
    try std.testing.expect(!m1.bulwark);
    try std.testing.expect(!m1.sentinal);
    try std.testing.expect(!m1.lingo);
    try std.testing.expect(!m1.quest);

    const m2: ModifierSet = .{};
    try std.testing.expect(!m2.polity);
}

test "handle requirements on ops" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Stateless ops should have no handle requirements
    const axioms = lookup("polity.axioms");
    try std.testing.expect(axioms != null);
    try std.testing.expectEqual(@as(usize, 0), axioms.?.handles.len);

    // Recovery generate should have no handle requirements (stateless)
    const recovery = lookup("sentinal.recovery_generate");
    try std.testing.expect(recovery != null);
    try std.testing.expectEqual(@as(usize, 0), recovery.?.handles.len);
}

test "execute dispatches correctly" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const result = execute("polity.axioms", "");
    try std.testing.expect(result != null);
    if (result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(str.len > 0);
        c.divi_free_string(r);
    }
}

test "execute third-party op" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    const test_handler = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const out_str = "{\"custom\":true}";
            const out = allocator.allocSentinel(u8, out_str.len, 0) catch return null;
            @memcpy(out[0..out_str.len], out_str);
            return out.ptr;
        }
    };

    try register("test.tp_exec", .{
        .call = test_handler.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    });

    const result = execute("test.tp_exec", "{}");
    try std.testing.expect(result != null);
    if (result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(std.mem.indexOf(u8, str, "custom") != null);
        allocator.free(r[0 .. str.len + 1]);
    }
}

test "op name conversion" {
    var buf: [MAX_C_NAME]u8 = undefined;
    const len1 = opNameToCName("vault.lock", &buf).?;
    try std.testing.expectEqualStrings("divi_vault_lock", buf[0..len1]);

    const len2 = opNameToCName("crown.keyring_public_key", &buf).?;
    try std.testing.expectEqualStrings("divi_crown_keyring_public_key", buf[0..len2]);

    const len3 = opNameToCName("polity.axioms", &buf).?;
    try std.testing.expectEqualStrings("divi_polity_axioms", buf[0..len3]);
}

// ── Handle Pattern Tests ──────────────────────────────────────

test "handle ops are dispatchable (not multi_arg)" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Crown soul operations should now be handle patterns
    try std.testing.expect(has("crown.soul_profile"));
    try std.testing.expect(has("crown.soul_is_dirty"));

    // Crown keyring operations
    try std.testing.expect(has("crown.keyring_public_key"));
    try std.testing.expect(has("crown.keyring_is_unlocked"));
    try std.testing.expect(has("crown.keyring_generate_primary"));

    // Vault operations
    try std.testing.expect(has("vault.idea_count"));
    try std.testing.expect(has("vault.is_unlocked"));
    try std.testing.expect(has("vault.root_path"));
}

test "handle lookup returns handle pattern metadata" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Soul profile should have soul handle requirement
    const soul_prof = lookup("crown.soul_profile");
    try std.testing.expect(soul_prof != null);
    try std.testing.expect(soul_prof.?.handles.len > 0);
    try std.testing.expectEqual(HandleReq.soul, soul_prof.?.handles[0]);

    // Vault idea_count should have vault handle requirement
    const vault_count = lookup("vault.idea_count");
    try std.testing.expect(vault_count != null);
    try std.testing.expect(vault_count.?.handles.len > 0);
    try std.testing.expectEqual(HandleReq.vault, vault_count.?.handles[0]);
}

test "handle dispatch returns null when no handle set" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // No keyring set — should return null (not crash)
    const result = execute("crown.keyring_public_key", "");
    try std.testing.expect(result == null);

    // No vault set — should return null
    const vault_result = execute("vault.idea_count", "");
    try std.testing.expect(vault_result == null);
}

test "handle dispatch with keyring" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Create and store a keyring
    const kr = c.divi_crown_keyring_new() orelse return error.SkipZigTest;
    state.setKeyring(kr);

    // Generate a primary key through the pipeline
    const gen_result = execute("crown.keyring_generate_primary", "");
    try std.testing.expect(gen_result != null);
    if (gen_result) |r| c.divi_free_string(r);

    // Get the public key through the pipeline
    const pk_result = execute("crown.keyring_public_key", "");
    try std.testing.expect(pk_result != null);
    if (pk_result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(str.len > 0);
        c.divi_free_string(r);
    }
}

test "handle dispatch with soul" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Create and store a soul
    const soul = c.divi_crown_soul_new() orelse return error.SkipZigTest;
    state.setSoul(soul);

    // Get profile through the pipeline
    const prof_result = execute("crown.soul_profile", "");
    try std.testing.expect(prof_result != null);
    if (prof_result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(str.len > 0);
        c.divi_free_string(r);
    }

    // Check is_dirty (bool return)
    const dirty_result = execute("crown.soul_is_dirty", "");
    try std.testing.expect(dirty_result != null);
    if (dirty_result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(std.mem.indexOf(u8, str, "result") != null);
        allocator.free(r[0 .. str.len + 1]);
    }
}

test "handle dispatch with handle + string arg" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // Create and store a soul
    const soul = c.divi_crown_soul_new() orelse return error.SkipZigTest;
    state.setSoul(soul);

    // Update profile through the pipeline (handle + json arg)
    const update_json = "{\"display_name\":\"Test\"}";
    const update_result = execute("crown.soul_update_profile", update_json);
    try std.testing.expect(update_result != null);
    if (update_result) |r| {
        const str = std.mem.span(r);
        try std.testing.expect(std.mem.indexOf(u8, str, "result") != null);
        allocator.free(r[0 .. str.len + 1]);
    }
}

test "wrapI64 produces valid JSON" {
    const r = wrapI64(42) orelse return error.SkipZigTest;
    const str = std.mem.span(r);
    try std.testing.expectEqualStrings("{\"result\":42}", str);
    allocator.free(r[0 .. str.len + 1]);

    const neg = wrapI64(-1) orelse return error.SkipZigTest;
    const neg_str = std.mem.span(neg);
    try std.testing.expectEqualStrings("{\"result\":-1}", neg_str);
    allocator.free(neg[0 .. neg_str.len + 1]);
}

test "wrapVoid produces valid JSON" {
    const r = wrapVoid() orelse return error.SkipZigTest;
    const str = std.mem.span(r);
    try std.testing.expectEqualStrings("{\"result\":null}", str);
    allocator.free(r[0 .. str.len + 1]);
}

test "wrapU32 produces valid JSON" {
    const r = wrapU32(42) orelse return error.SkipZigTest;
    const str = std.mem.span(r);
    try std.testing.expectEqualStrings("{\"result\":42}", str);
    allocator.free(r[0 .. str.len + 1]);
}

test "wrapU64 produces valid JSON" {
    const r = wrapU64(123456789) orelse return error.SkipZigTest;
    const str = std.mem.span(r);
    try std.testing.expectEqualStrings("{\"result\":123456789}", str);
    allocator.free(r[0 .. str.len + 1]);
}

test "wrapF64 produces valid JSON" {
    // Integer-valued float should format cleanly
    const r = wrapF64(1.0) orelse return error.SkipZigTest;
    const str = std.mem.span(r);
    try std.testing.expect(str.len > 0);
    try std.testing.expect(std.mem.startsWith(u8, str, "{\"result\":"));
    allocator.free(r[0 .. str.len + 1]);
}

test "parseJsonArg: integers" {
    var buf: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf[0..2], "42");
    buf[2] = 0;
    const val = parseJsonArg(i32, buf[0..MAX_ARG_LEN]);
    try std.testing.expect(val != null);
    try std.testing.expectEqual(@as(i32, 42), val.?);
}

test "parseJsonArg: negative integers" {
    var buf: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf[0..3], "-99");
    buf[3] = 0;
    const val = parseJsonArg(i64, buf[0..MAX_ARG_LEN]);
    try std.testing.expect(val != null);
    try std.testing.expectEqual(@as(i64, -99), val.?);
}

test "parseJsonArg: unsigned integers" {
    var buf: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf[0..3], "255");
    buf[3] = 0;
    const val = parseJsonArg(u32, buf[0..MAX_ARG_LEN]);
    try std.testing.expect(val != null);
    try std.testing.expectEqual(@as(u32, 255), val.?);
}

test "parseJsonArg: floats" {
    var buf: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf[0..4], "3.14");
    buf[4] = 0;
    const val = parseJsonArg(f64, buf[0..MAX_ARG_LEN]);
    try std.testing.expect(val != null);
    try std.testing.expect(val.? > 3.13 and val.? < 3.15);
}

test "parseJsonArg: bools" {
    var buf_true: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf_true[0..4], "true");
    buf_true[4] = 0;
    try std.testing.expectEqual(@as(?bool, true), parseJsonArg(bool, buf_true[0..MAX_ARG_LEN]));

    var buf_false: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf_false[0..5], "false");
    buf_false[5] = 0;
    try std.testing.expectEqual(@as(?bool, false), parseJsonArg(bool, buf_false[0..MAX_ARG_LEN]));
}

test "parseJsonArg: strings" {
    var buf: [MAX_ARG_LEN]u8 = @splat(0);
    @memcpy(buf[0..5], "hello");
    const val = parseJsonArg([*c]const u8, buf[0..MAX_ARG_LEN]);
    // [*c]const u8 is inherently nullable (C pointer). The optional wrapper
    // gives us ?[*c]const u8. Unwrap the optional, then verify the pointer.
    try std.testing.expect(val != null);
    const ptr: [*c]const u8 = val.?;
    try std.testing.expect(ptr != null);
    try std.testing.expectEqual(@as(u8, 'h'), ptr[0]);
    try std.testing.expectEqual(@as(u8, 'e'), ptr[1]);
}

// ── Universal Dispatch Tests ──────────────────────────────────

test "universal: stateless multi-float function" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // divi_regalia_contrast_ratio(r1, g1, b1, r2, g2, b2) -> f64
    try std.testing.expect(has("regalia.contrast_ratio"));

    // White vs black should give contrast ratio of 21.0
    const result = execute("regalia.contrast_ratio", "[1.0, 1.0, 1.0, 0.0, 0.0, 0.0]");
    try std.testing.expect(result != null);
    const str = std.mem.span(result.?);
    try std.testing.expect(std.mem.startsWith(u8, str, "{\"result\":"));
    allocator.free(result.?[0 .. str.len + 1]);
}

test "universal: mixed float+string function" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // divi_regalia_resolve_layout(x, y, w, h, sanctums_json) -> str
    const handler = lookup("regalia.resolve_layout");
    try std.testing.expect(handler != null);
    // Note: may return null if sanctums_json is invalid, but handler itself should exist
}

test "universal: float param + str return (surge_evaluate)" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // divi_regalia_surge_evaluate(preset_name, t) -> f64
    try std.testing.expect(has("regalia.surge_evaluate"));

    // "smooth" is a valid Surge preset, t=0.5 should return a value
    const result = execute("regalia.surge_evaluate", "[\"smooth\", 0.5]");
    try std.testing.expect(result != null);
    const str = std.mem.span(result.?);
    try std.testing.expect(std.mem.startsWith(u8, str, "{\"result\":"));
    allocator.free(result.?[0 .. str.len + 1]);
}

test "universal: u32 return type (ideas_bonds_count)" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // divi_ideas_bonds_count(json) -> u32
    // This was previously multi_arg because u32 return wasn't handled.
    // Now it's universal. Input is a JSON array with one string element.
    const handler = lookup("ideas.bonds_count");
    try std.testing.expect(handler != null);
}

test "universal: 0-arg handle return (_new constructors)" {
    try std.testing.expectEqual(@as(i32, 0), init());
    defer deinit();

    // divi_formula_evaluator_new() -> *FormulaEvaluatorHandle
    const handler = lookup("formula.evaluator_new");
    try std.testing.expect(handler != null);
    try std.testing.expect(handler.?.owns_handle);

    const result = handler.?.call("");
    try std.testing.expect(result != null);
    if (result) |r| {
        const str = std.mem.span(r);
        // Should be {"result": <integer_address>}
        try std.testing.expect(std.mem.startsWith(u8, str, "{\"result\":"));
        // Extract the address and free the handle
        const addr_start = std.mem.indexOf(u8, str, ":").? + 1;
        const addr_end = std.mem.indexOf(u8, str, "}").?;
        const addr_str = str[addr_start..addr_end];
        const addr = std.fmt.parseInt(usize, addr_str, 10) catch 0;
        if (addr != 0) {
            // Free the created handle
            const ptr: *c.FormulaEvaluatorHandle = @ptrFromInt(addr);
            c.divi_formula_evaluator_free(ptr);
        }
        allocator.free(r[0 .. str.len + 1]);
    }
}

test "coverage: callable vs multi_arg" {
    @setEvalBranchQuota(20_000_000);
    const counts = comptime blk: {
        var callable: usize = 0;
        var multi: usize = 0;
        var cb: usize = 0;
        var out: usize = 0;
        var sec: usize = 0;
        var oth: usize = 0;
        for (@typeInfo(c).@"struct".decls) |decl| {
            if (isDispatchable(decl.name)) {
                if (classifyFn(decl.name) == .multi_arg) {
                    multi += 1;
                    // Classify why it's multi_arg
                    const T = @TypeOf(@field(c, decl.name));
                    const info = @typeInfo(T).@"fn";
                    const params = info.params;
                    var has_cb = false;
                    var has_out = false;
                    var has_sec = false;
                    for (params, 0..) |p, i| {
                        const PT = p.type orelse continue;
                        if (isFnPtrParam(PT)) has_cb = true;
                        if (isOutParam(PT)) has_out = true;
                        if (i > 0 and isHandleParam(PT)) has_sec = true;
                    }
                    if (has_cb) { cb += 1; } else if (has_out) { out += 1; } else if (has_sec) { sec += 1; } else { oth += 1; }
                } else {
                    callable += 1;
                }
            }
        }
        break :blk .{ .callable = callable, .multi = multi, .cb = cb, .out = out, .sec = sec, .oth = oth };
    };
    const total = counts.callable + counts.multi;
    const pct = (counts.callable * 100) / total;
    std.debug.print("\n  Pipeline coverage: {d}/{d} callable ({d}%), {d} multi_arg\n", .{ counts.callable, total, pct, counts.multi });
    std.debug.print("  Breakdown: {d} callback, {d} out-param, {d} secondary-handle, {d} other\n", .{ counts.cb, counts.out, counts.sec, counts.oth });
    // Fail if coverage drops below 95% (only callbacks should remain as multi_arg)
    try std.testing.expect(pct > 95);
}

test "remaining multi_arg are all callbacks" {
    @setEvalBranchQuota(20_000_000);
    // All remaining multi_arg functions should be callback-based (function pointer params).
    // These are handled by Intercom/Equipment registration, not the pipeline.
    const counts = comptime blk: {
        var cb: usize = 0;
        var other: usize = 0;
        for (@typeInfo(c).@"struct".decls) |decl| {
            if (isDispatchable(decl.name) and classifyFn(decl.name) == .multi_arg) {
                const T = @TypeOf(@field(c, decl.name));
                const params = @typeInfo(T).@"fn".params;
                var has_cb = false;
                for (params) |p| {
                    const PT = p.type orelse continue;
                    if (isFnPtrParam(PT)) has_cb = true;
                }
                if (has_cb) cb += 1 else other += 1;
            }
        }
        break :blk .{ .cb = cb, .other = other };
    };
    // All multi_arg should be callbacks — nothing else should remain
    try std.testing.expectEqual(@as(usize, 0), counts.other);
    try std.testing.expect(counts.cb > 0);
}

