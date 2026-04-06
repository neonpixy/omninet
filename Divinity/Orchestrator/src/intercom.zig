// Intercom — inter-program intent routing via Equipment's Phone FFI.
//
// Programs register capabilities (digit types + actions).
// Intents are fired and routed to the matching program.
// Equipment's Phone handles the delivery via registered handlers.
//
// Equipment is the telephone hardware. Intercom is the switchboard.
// Uses the shared Phone from OrchestratorState (created by orch_init).
//
// Delivery flow:
//   Platform registers handler via orch_intercom_register_handler()
//     → stores HandlerContext on heap, registers trampoline with Phone
//   orch_intercom_fire() finds matching program
//     → builds intent JSON
//     → calls divi_phone_call_raw_if_available() to deliver
//     → Phone invokes trampoline → trampoline calls platform handler

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

/// Maximum number of registered programs.
const MAX_PROGRAMS = 128;

/// Maximum number of capabilities per program.
const MAX_CAPS_PER_PROGRAM = 16;

/// Maximum length of a string field.
const MAX_STRING_LEN = 256;

/// Maximum length of intent JSON built by orch_intercom_fire.
const MAX_INTENT_JSON_LEN = 4096;

/// Platform-facing callback for intent delivery.
/// Receives a null-terminated JSON string with the intent details:
///   {"action":"...","digit_type":"...","payload":"...","source":"..."}
///
/// Returns 0 on success, -1 to reject the intent.
/// The handler is called on the thread that fired the intent (synchronous).
pub const IntentHandler = *const fn (
    intent_json: [*:0]const u8,
    context: ?*anyopaque,
) callconv(.c) i32;

/// Heap-allocated context passed to Phone's trampoline.
/// Stable pointer — never moves after allocation.
/// Freed when the program is unregistered or intercom shuts down.
const HandlerContext = struct {
    handler: IntentHandler,
    platform_context: ?*anyopaque,
};

/// A registered program's capability.
const Capability = struct {
    digit_type: [MAX_STRING_LEN]u8,
    digit_type_len: usize,
    action: [MAX_STRING_LEN]u8,
    action_len: usize,
};

/// A registered program.
const Program = struct {
    id: [MAX_STRING_LEN]u8,
    id_len: usize,
    capabilities: [MAX_CAPS_PER_PROGRAM]Capability,
    cap_count: usize,
    active: bool,
    /// Heap-allocated handler context. Owned by this program entry.
    /// Null if no handler registered.
    handler_ctx: ?*HandlerContext,
};

/// Global program registry. Intercom owns routing, platforms own views.
var programs: [MAX_PROGRAMS]Program = undefined;
var program_count: usize = 0;
var initialized: bool = false;

/// Mutex protecting module-level state (programs, program_count, initialized).
var mod_mutex: std.Thread.Mutex = .{};

// ── Trampoline ────────────────────────────────────────────────

/// Bridges DiviPhoneHandler signature to IntentHandler.
///
/// Phone calls this with raw bytes. The bytes are the intent JSON
/// (NOT null-terminated). We make a null-terminated copy on the stack,
/// call the platform handler, and write a minimal response.
///
/// No mutex needed — reads only from its own HeapContext (stable pointer).
fn trampoline(
    request_data: [*c]const u8,
    request_len: usize,
    response_data: [*c][*c]u8,
    response_len: [*c]usize,
    context: ?*anyopaque,
) callconv(.c) i32 {
    // Recover the HandlerContext
    const ctx: *HandlerContext = @ptrCast(@alignCast(context orelse return -1));

    // Build null-terminated copy of request data on stack
    if (request_len >= MAX_INTENT_JSON_LEN) return -1;

    var json_buf: [MAX_INTENT_JSON_LEN]u8 = undefined;
    if (request_len > 0 and request_data != null) {
        @memcpy(json_buf[0..request_len], request_data[0..request_len]);
    }
    json_buf[request_len] = 0;

    const json_ptr: [*:0]const u8 = @ptrCast(&json_buf);

    // Call the platform handler
    const result = ctx.handler(json_ptr, ctx.platform_context);

    // Write empty response (Phone expects valid out pointers)
    if (response_data != null) response_data.* = null;
    if (response_len != null) response_len.* = 0;

    return result;
}

// ── Public C API ──────────────────────────────────────────────

/// Initialize Intercom. Uses the shared Phone from OrchestratorState.
/// Must be called after orch_init().
export fn orch_intercom_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (initialized) return 0;

    // Require shared state to be initialized (Phone lives there)
    // Read shared state to check phone — safe to hold mod_mutex while
    // acquiring shared lock (we never call state setters from inside).
    const s = state.acquireShared();
    const has_phone = s.phone != null;
    const is_init = s.initialized;
    state.releaseShared();

    if (!is_init or !has_phone) {
        // Fallback: create a standalone Phone (backwards compat with old tests)
        if (!has_phone) {
            const new_phone = c.divi_phone_new();
            if (new_phone == null) return -1;
            state.setPhone(new_phone);
        }
    }

    program_count = 0;
    initialized = true;
    return 0;
}

/// Shut down Intercom. Unregisters all handlers from Phone,
/// frees all handler contexts, and clears the registry.
/// Does NOT free the shared Phone — that's owned by orch_shutdown().
pub export fn orch_intercom_shutdown() void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    // Read phone handle from shared state
    const s = state.acquireShared();
    const phone = s.phone;
    state.releaseShared();

    // Unregister from Phone BEFORE freeing handler contexts to prevent
    // use-after-free if Phone tries to call the trampoline.
    for (programs[0..program_count]) |*p| {
        if (p.handler_ctx) |ctx| {
            if (phone) |ph| {
                // Build null-terminated program ID
                var pid_buf: [MAX_STRING_LEN + 1]u8 = undefined;
                @memcpy(pid_buf[0..p.id_len], p.id[0..p.id_len]);
                pid_buf[p.id_len] = 0;
                const pid_z: [*:0]const u8 = @ptrCast(&pid_buf);
                c.divi_phone_unregister(ph, pid_z);
            }
            std.heap.c_allocator.destroy(ctx);
            p.handler_ctx = null;
        }
    }

    program_count = 0;
    initialized = false;
}

/// Register a program with a capability (digit type + action).
/// Call multiple times to register multiple capabilities for the same program.
///
/// Returns 0 on success, negative on error.
export fn orch_intercom_register(
    program_id: [*:0]const u8,
    digit_type: [*:0]const u8,
    action: [*:0]const u8,
) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (!initialized) return -1;

    const pid = std.mem.span(program_id);
    const dtype = std.mem.span(digit_type);
    const act = std.mem.span(action);

    if (pid.len >= MAX_STRING_LEN or dtype.len >= MAX_STRING_LEN or act.len >= MAX_STRING_LEN) {
        return -2; // String too long
    }

    // Find existing program or create new one
    var prog: ?*Program = null;
    for (programs[0..program_count]) |*p| {
        if (p.active and std.mem.eql(u8, p.id[0..p.id_len], pid)) {
            prog = p;
            break;
        }
    }

    if (prog == null) {
        // Try to reuse an inactive slot first (from prior unregister)
        for (programs[0..program_count]) |*p| {
            if (!p.active) {
                prog = p;
                break;
            }
        }

        if (prog == null) {
            // No inactive slot — append if room
            if (program_count >= MAX_PROGRAMS) return -3; // Registry full
            prog = &programs[program_count];
            program_count += 1;
        }

        prog.?.active = true;
        prog.?.cap_count = 0;
        prog.?.id_len = pid.len;
        prog.?.handler_ctx = null;
        @memcpy(prog.?.id[0..pid.len], pid);
    }

    const p = prog.?;
    if (p.cap_count >= MAX_CAPS_PER_PROGRAM) return -4; // Too many capabilities

    var cap = &p.capabilities[p.cap_count];
    cap.digit_type_len = dtype.len;
    @memcpy(cap.digit_type[0..dtype.len], dtype);
    cap.action_len = act.len;
    @memcpy(cap.action[0..act.len], act);
    p.cap_count += 1;

    return 0;
}

/// Register an intent handler for a program.
///
/// The handler will be called when an intent is delivered to this program
/// via orch_intercom_fire(). The handler receives a null-terminated JSON
/// string: {"action":"...","digit_type":"...","payload":"...","source":"..."}.
///
/// The program must already be registered via orch_intercom_register().
/// Replaces any previously registered handler for this program.
///
/// Parameters:
///   program_id: null-terminated program identifier (must match a registered program)
///   handler: callback function pointer (must remain valid until unregister/shutdown)
///   context: opaque pointer passed through to every handler invocation (may be null)
///
/// Returns 0 on success, -1 if program not found, -2 if not initialized,
/// -3 if allocation failed.
export fn orch_intercom_register_handler(
    program_id: [*:0]const u8,
    handler: ?IntentHandler,
    context: ?*anyopaque,
) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (!initialized) return -2;

    const h = handler orelse return -1;
    const pid = std.mem.span(program_id);

    // Find the program
    var prog: ?*Program = null;
    for (programs[0..program_count]) |*p| {
        if (p.active and std.mem.eql(u8, p.id[0..p.id_len], pid)) {
            prog = p;
            break;
        }
    }

    const p = prog orelse return -1; // Program not found

    // Free old handler context if replacing
    if (p.handler_ctx) |old| {
        std.heap.c_allocator.destroy(old);
    }

    // Allocate new handler context on the heap (stable pointer)
    const ctx = std.heap.c_allocator.create(HandlerContext) catch return -3;
    ctx.* = .{
        .handler = h,
        .platform_context = context,
    };
    p.handler_ctx = ctx;

    // Register trampoline with Phone
    const s = state.acquireShared();
    const phone = s.phone;
    state.releaseShared();

    if (phone) |ph| {
        c.divi_phone_register_raw(ph, program_id, trampoline, @ptrCast(ctx));
    }

    return 0;
}

/// Unregister a program and all its capabilities.
/// Frees the handler context and unregisters from Phone.
/// Returns 0 on success, -1 if not found.
export fn orch_intercom_unregister(program_id: [*:0]const u8) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (!initialized) return -1;

    const pid = std.mem.span(program_id);

    for (programs[0..program_count]) |*p| {
        if (p.active and std.mem.eql(u8, p.id[0..p.id_len], pid)) {
            p.active = false;

            // Free handler context
            if (p.handler_ctx) |ctx| {
                std.heap.c_allocator.destroy(ctx);
                p.handler_ctx = null;
            }

            // Unregister from Phone — read handle from shared state
            const s = state.acquireShared();
            const phone = s.phone;
            state.releaseShared();

            if (phone) |ph| {
                c.divi_phone_unregister(ph, program_id);
            }
            return 0;
        }
    }

    return -1; // Not found
}

/// Fire an intent. Routes to the matching program and delivers via Phone.
///
/// Returns the program ID that handled it (caller-owned, free with
/// std.heap.c_allocator), or null if no match / delivery failed.
///
/// When exactly one program matches:
///   - Builds intent JSON: {"action":"...","digit_type":"...","payload":"...","source":"..."}
///   - Delivers via divi_phone_call_raw_if_available to avoid error on missing handler
///   - Returns the matched program ID on successful delivery
///   - Returns null if delivery was rejected by the handler
///
/// When multiple match, returns null (caller should disambiguate).
/// When none match, returns null.
export fn orch_intercom_fire(
    action: [*:0]const u8,
    digit_type: [*:0]const u8,
    payload: [*:0]const u8,
    source: [*:0]const u8,
) ?[*:0]u8 {
    mod_mutex.lock();

    if (!initialized) {
        mod_mutex.unlock();
        return null;
    }

    const act = std.mem.span(action);
    const dtype = std.mem.span(digit_type);

    // Find matching programs
    var match_count: usize = 0;
    var matched_id_buf: [MAX_STRING_LEN]u8 = undefined;
    var matched_id_len: usize = 0;

    for (programs[0..program_count]) |*p| {
        if (!p.active) continue;
        for (p.capabilities[0..p.cap_count]) |*cap| {
            if (std.mem.eql(u8, cap.digit_type[0..cap.digit_type_len], dtype) and
                std.mem.eql(u8, cap.action[0..cap.action_len], act))
            {
                match_count += 1;
                matched_id_len = p.id_len;
                @memcpy(matched_id_buf[0..p.id_len], p.id[0..p.id_len]);
                break; // Don't double-count same program
            }
        }
    }

    // Release mutex BEFORE making Phone call to avoid deadlock.
    // The trampoline is called synchronously inside divi_phone_call_raw
    // and doesn't need mod_mutex (it reads from its own HandlerContext).
    mod_mutex.unlock();

    if (match_count != 1) return null;

    const mid = matched_id_buf[0..matched_id_len];

    // Build intent JSON into stack buffer
    var json_buf: [MAX_INTENT_JSON_LEN]u8 = undefined;
    var pos: usize = 0;

    const p1 = "{\"action\":\"";
    const p2 = "\",\"digit_type\":\"";
    const p3 = "\",\"payload\":\"";
    const p4 = "\",\"source\":\"";
    const p5 = "\"}";

    const pay = std.mem.span(payload);
    const src = std.mem.span(source);

    // Bounds check: ensure total JSON fits in the stack buffer
    const total = p1.len + act.len + p2.len + dtype.len + p3.len + pay.len + p4.len + src.len + p5.len;
    if (total >= MAX_INTENT_JSON_LEN) return null;

    @memcpy(json_buf[pos .. pos + p1.len], p1);
    pos += p1.len;

    @memcpy(json_buf[pos .. pos + act.len], act);
    pos += act.len;

    @memcpy(json_buf[pos .. pos + p2.len], p2);
    pos += p2.len;

    @memcpy(json_buf[pos .. pos + dtype.len], dtype);
    pos += dtype.len;

    @memcpy(json_buf[pos .. pos + p3.len], p3);
    pos += p3.len;

    @memcpy(json_buf[pos .. pos + pay.len], pay);
    pos += pay.len;

    @memcpy(json_buf[pos .. pos + p4.len], p4);
    pos += p4.len;

    @memcpy(json_buf[pos .. pos + src.len], src);
    pos += src.len;

    @memcpy(json_buf[pos .. pos + p5.len], p5);
    pos += p5.len;

    // Read phone handle from shared state
    const s = state.acquireShared();
    const phone = s.phone;
    state.releaseShared();

    if (phone) |ph| {
        // Build null-terminated program ID for Phone call
        var pid_buf: [MAX_STRING_LEN + 1]u8 = undefined;
        @memcpy(pid_buf[0..mid.len], mid);
        pid_buf[mid.len] = 0;
        const pid_z: [*:0]const u8 = @ptrCast(&pid_buf);

        var out_data: [*c]u8 = null;
        var out_len: usize = 0;

        // Use call_raw_if_available — returns 1 if no handler (not error)
        const call_result = c.divi_phone_call_raw_if_available(
            ph,
            pid_z,
            @ptrCast(&json_buf),
            pos,
            @ptrCast(&out_data),
            @ptrCast(&out_len),
        );

        // Free response bytes if any were returned
        if (out_data != null and out_len > 0) {
            c.divi_free_bytes(out_data, out_len);
        }

        if (call_result == 0) {
            // Success — return matched program ID
            const result = std.heap.c_allocator.allocSentinel(u8, mid.len, 0) catch return null;
            @memcpy(result[0..mid.len], mid);
            return result.ptr;
        }
    }

    // No phone, no handler (1), or handler rejected (-1)
    return null;
}

/// Get the number of registered programs.
export fn orch_intercom_program_count() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    return @intCast(program_count);
}

/// Check if a specific program is registered.
export fn orch_intercom_has_program(program_id: [*:0]const u8) bool {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const pid = std.mem.span(program_id);
    for (programs[0..program_count]) |*p| {
        if (p.active and std.mem.eql(u8, p.id[0..p.id_len], pid)) {
            return true;
        }
    }
    return false;
}

/// Check if a program has a handler registered.
export fn orch_intercom_has_handler(program_id: [*:0]const u8) bool {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const pid = std.mem.span(program_id);
    for (programs[0..program_count]) |*p| {
        if (p.active and std.mem.eql(u8, p.id[0..p.id_len], pid)) {
            return p.handler_ctx != null;
        }
    }
    return false;
}

/// Get the list of all registered programs as JSON array of objects.
/// Each object has: id, capabilities (array of {digit_type, action}).
/// Caller must free with std.heap.c_allocator.
export fn orch_intercom_list_programs() ?[*:0]u8 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (!initialized) return null;

    // Build a simple JSON array manually
    const LIST_BUF_SIZE = 8192;
    var buf: [LIST_BUF_SIZE]u8 = undefined;
    var pos: usize = 0;

    buf[pos] = '[';
    pos += 1;

    var first = true;
    for (programs[0..program_count]) |*p| {
        if (!p.active) continue;

        if (!first) {
            if (pos + 1 > LIST_BUF_SIZE) return null;
            buf[pos] = ',';
            pos += 1;
        }
        first = false;

        // {"id":"...","capabilities":[...]}
        const prefix = "{\"id\":\"";
        if (pos + prefix.len > LIST_BUF_SIZE) return null;
        @memcpy(buf[pos .. pos + prefix.len], prefix);
        pos += prefix.len;

        if (pos + p.id_len > LIST_BUF_SIZE) return null;
        @memcpy(buf[pos .. pos + p.id_len], p.id[0..p.id_len]);
        pos += p.id_len;

        const caps_prefix = "\",\"capabilities\":[";
        if (pos + caps_prefix.len > LIST_BUF_SIZE) return null;
        @memcpy(buf[pos .. pos + caps_prefix.len], caps_prefix);
        pos += caps_prefix.len;

        var first_cap = true;
        for (p.capabilities[0..p.cap_count]) |*cap_item| {
            if (!first_cap) {
                if (pos + 1 > LIST_BUF_SIZE) return null;
                buf[pos] = ',';
                pos += 1;
            }
            first_cap = false;

            const dt_prefix = "{\"digit_type\":\"";
            if (pos + dt_prefix.len > LIST_BUF_SIZE) return null;
            @memcpy(buf[pos .. pos + dt_prefix.len], dt_prefix);
            pos += dt_prefix.len;

            if (pos + cap_item.digit_type_len > LIST_BUF_SIZE) return null;
            @memcpy(buf[pos .. pos + cap_item.digit_type_len], cap_item.digit_type[0..cap_item.digit_type_len]);
            pos += cap_item.digit_type_len;

            const act_prefix = "\",\"action\":\"";
            if (pos + act_prefix.len > LIST_BUF_SIZE) return null;
            @memcpy(buf[pos .. pos + act_prefix.len], act_prefix);
            pos += act_prefix.len;

            if (pos + cap_item.action_len > LIST_BUF_SIZE) return null;
            @memcpy(buf[pos .. pos + cap_item.action_len], cap_item.action[0..cap_item.action_len]);
            pos += cap_item.action_len;

            const suffix = "\"}";
            if (pos + suffix.len > LIST_BUF_SIZE) return null;
            @memcpy(buf[pos .. pos + suffix.len], suffix);
            pos += suffix.len;
        }

        const obj_suffix = "]}";
        if (pos + obj_suffix.len > LIST_BUF_SIZE) return null;
        @memcpy(buf[pos .. pos + obj_suffix.len], obj_suffix);
        pos += obj_suffix.len;
    }

    if (pos + 1 > LIST_BUF_SIZE) return null;
    buf[pos] = ']';
    pos += 1;

    // Allocate C string
    const result = std.heap.c_allocator.allocSentinel(u8, pos, 0) catch return null;
    @memcpy(result[0..pos], buf[0..pos]);
    return result.ptr;
}

// ── Tests ─────────────────────────────────────────────────────

test "init and shutdown" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    try std.testing.expect(initialized);
    orch_intercom_shutdown();
    try std.testing.expect(!initialized);
}

test "register program" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    const result = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    try std.testing.expectEqual(@as(i32, 0), result);
    try std.testing.expectEqual(@as(i32, 1), orch_intercom_program_count());
    try std.testing.expect(orch_intercom_has_program("com.omnidea.tome"));
}

test "register multiple capabilities" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    _ = orch_intercom_register("com.omnidea.tome", "note", "open");
    _ = orch_intercom_register("com.omnidea.tome", "richtext", "share");

    try std.testing.expectEqual(@as(i32, 1), orch_intercom_program_count());
}

test "register multiple programs" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    _ = orch_intercom_register("com.omnidea.courier", "message", "open");

    try std.testing.expectEqual(@as(i32, 2), orch_intercom_program_count());
    try std.testing.expect(orch_intercom_has_program("com.omnidea.tome"));
    try std.testing.expect(orch_intercom_has_program("com.omnidea.courier"));
}

test "unregister program" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    try std.testing.expect(orch_intercom_has_program("com.omnidea.tome"));

    try std.testing.expectEqual(@as(i32, 0), orch_intercom_unregister("com.omnidea.tome"));
    try std.testing.expect(!orch_intercom_has_program("com.omnidea.tome"));
}

test "has_program returns false for unknown" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    try std.testing.expect(!orch_intercom_has_program("com.unknown.app"));
}

test "fire routes to single match" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");

    const result = orch_intercom_fire("open", "richtext", "{}", "library");
    // Without a handler registered, fire returns null (no delivery)
    // This is correct — fire now requires actual delivery, not just routing.
    try std.testing.expect(result == null);
}

test "fire returns null for no match" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");

    const result = orch_intercom_fire("open", "audio.track", "{}", "library");
    try std.testing.expect(result == null);
}

test "fire returns null for ambiguous match" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    _ = orch_intercom_register("com.omnidea.quill", "richtext", "open");

    const result = orch_intercom_fire("open", "richtext", "{}", "library");
    try std.testing.expect(result == null);
}

test "list programs JSON" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");

    const json = orch_intercom_list_programs();
    try std.testing.expect(json != null);
    if (json) |j| {
        const str = std.mem.span(j);
        try std.testing.expect(str.len > 2); // More than "[]"
        std.heap.c_allocator.free(j[0 .. str.len + 1]);
    }
}

// ── Handler + Delivery Tests ──────────────────────────────────

/// Test state shared between test handler and test body.
/// Module-level so the handler callback can access it.
var test_received_json: [MAX_INTENT_JSON_LEN]u8 = undefined;
var test_received_json_len: usize = 0;
var test_handler_call_count: usize = 0;

/// Test handler that captures the intent JSON.
fn testHandler(intent_json: [*:0]const u8, _: ?*anyopaque) callconv(.c) i32 {
    const span = std.mem.span(intent_json);
    @memcpy(test_received_json[0..span.len], span);
    test_received_json_len = span.len;
    test_handler_call_count += 1;
    return 0; // success
}

/// Test handler that rejects intents.
fn testRejectHandler(_: [*:0]const u8, _: ?*anyopaque) callconv(.c) i32 {
    test_handler_call_count += 1;
    return -1; // reject
}

test "register handler and fire delivers intent" {
    // Reset test state
    test_handler_call_count = 0;
    test_received_json_len = 0;

    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    // Register program + capability
    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");

    // Register handler
    const reg_result = orch_intercom_register_handler("com.omnidea.tome", testHandler, null);
    try std.testing.expectEqual(@as(i32, 0), reg_result);
    try std.testing.expect(orch_intercom_has_handler("com.omnidea.tome"));

    // Fire intent
    const result = orch_intercom_fire("open", "richtext", "{}", "library");
    try std.testing.expect(result != null);

    if (result) |r| {
        const matched = std.mem.span(r);
        try std.testing.expectEqualStrings("com.omnidea.tome", matched);
        std.heap.c_allocator.free(r[0 .. matched.len + 1]);
    }

    // Verify handler was called with correct JSON
    try std.testing.expectEqual(@as(usize, 1), test_handler_call_count);
    const received = test_received_json[0..test_received_json_len];
    // Check that JSON contains expected fields
    try std.testing.expect(std.mem.indexOf(u8, received, "\"action\":\"open\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, received, "\"digit_type\":\"richtext\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, received, "\"payload\":\"{}\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, received, "\"source\":\"library\"") != null);
}

test "fire without handler returns null" {
    test_handler_call_count = 0;

    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    // Register program + capability but NO handler
    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    try std.testing.expect(!orch_intercom_has_handler("com.omnidea.tome"));

    // Fire — should return null (no handler to deliver to)
    const result = orch_intercom_fire("open", "richtext", "{}", "library");
    try std.testing.expect(result == null);

    // Handler was never called
    try std.testing.expectEqual(@as(usize, 0), test_handler_call_count);
}

test "fire delivers to correct program among multiple" {
    test_handler_call_count = 0;
    test_received_json_len = 0;

    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    // Register two programs with DIFFERENT capabilities
    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    _ = orch_intercom_register("com.omnidea.courier", "message", "send");

    // Register handlers for both
    _ = orch_intercom_register_handler("com.omnidea.tome", testHandler, null);
    _ = orch_intercom_register_handler("com.omnidea.courier", testHandler, null);

    // Fire intent matching Tome
    const result = orch_intercom_fire("open", "richtext", "{\"id\":\"123\"}", "library");
    try std.testing.expect(result != null);

    if (result) |r| {
        const matched = std.mem.span(r);
        try std.testing.expectEqualStrings("com.omnidea.tome", matched);
        std.heap.c_allocator.free(r[0 .. matched.len + 1]);
    }

    // Verify handler received the right intent
    try std.testing.expectEqual(@as(usize, 1), test_handler_call_count);
    const received = test_received_json[0..test_received_json_len];
    try std.testing.expect(std.mem.indexOf(u8, received, "\"action\":\"open\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, received, "\"digit_type\":\"richtext\"") != null);
}

test "fire with rejecting handler returns null" {
    test_handler_call_count = 0;

    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    _ = orch_intercom_register_handler("com.omnidea.tome", testRejectHandler, null);

    // Fire — handler rejects, so fire returns null
    const result = orch_intercom_fire("open", "richtext", "{}", "library");
    try std.testing.expect(result == null);

    // Handler was still called
    try std.testing.expectEqual(@as(usize, 1), test_handler_call_count);
}

test "register handler for unknown program fails" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    // No program registered — handler registration should fail
    const result = orch_intercom_register_handler("com.unknown.app", testHandler, null);
    try std.testing.expectEqual(@as(i32, -1), result);
}

test "unregister cleans up handler" {
    try std.testing.expectEqual(@as(i32, 0), orch_intercom_init());
    defer orch_intercom_shutdown();

    _ = orch_intercom_register("com.omnidea.tome", "richtext", "open");
    _ = orch_intercom_register_handler("com.omnidea.tome", testHandler, null);
    try std.testing.expect(orch_intercom_has_handler("com.omnidea.tome"));

    _ = orch_intercom_unregister("com.omnidea.tome");
    try std.testing.expect(!orch_intercom_has_handler("com.omnidea.tome"));
}
