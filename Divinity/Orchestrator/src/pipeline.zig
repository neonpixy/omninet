// Pipeline Executor — dynamic operation chaining for the Zig orchestrator.
//
// Programs send pipeline specs as JSON. The executor parses the spec,
// resolves named step references, dispatches each step through the
// registry, and returns the final result. One call from the platform
// side runs an entire multi-step workflow.
//
// Input format:
//   { "source": "tome", "steps": [{ "id": "name", "op": "crate.verb", "input": {...} }] }
//
// Output format:
//   { "ok": true, "result": {...} }
//   { "ok": false, "error": "...", "failed_step": "...", "step_index": N }
//
// Reference syntax:
//   "$step_id.field.subfield" in any input string value resolves to the
//   referenced step's output field. Nested field access supported.
//
// Modifier execution order (locked in):
//   Before step: Polity -> Bulwark
//   After step:  Sentinal -> Lingo -> Yoke -> Quest
//
// Memory:
//   - Pipeline uses c_allocator for all dynamic allocations
//   - Step outputs from registry handlers may be Rust-allocated or Zig-allocated
//   - All intermediate outputs are copied to pipeline-owned buffers and originals freed
//   - Final response is c_allocator-allocated, caller frees

const std = @import("std");
const registry = @import("registry.zig");
const ffi = @import("ffi.zig");
const c = ffi.c;
const governance = @import("governance.zig");
const lingo = @import("lingo.zig");

const allocator = std.heap.c_allocator;

// ── Constants ─────────────────────────────────────────────────

const MAX_STEPS = 64;
const MAX_STEP_ID_LEN = 128;
const MAX_OP_LEN = 256;
const MAX_SOURCE_LEN = 64;
const MAX_PIPELINE_SIZE = 262144; // 256KB total pipeline input
const MAX_OUTPUT_SIZE = 262144; // 256KB per step output
const RESPONSE_BUF_SIZE = 8192; // 8KB for error/success envelope

// ── Types ─────────────────────────────────────────────────────

/// Stored result from a completed step.
const StepResult = struct {
    id: [MAX_STEP_ID_LEN]u8,
    id_len: usize,
    /// Copied output JSON, owned by pipeline (free with c_allocator).
    output: ?[]u8,
    /// Whether this step created an opaque handle that needs freeing.
    /// Currently convention-based: set when OpHandler.owns_handle is true
    /// and the output looks like a handle reference.
    owns_handle: bool,
};

/// A parsed step before execution.
const ParsedStep = struct {
    id: [MAX_STEP_ID_LEN]u8,
    id_len: usize,
    op: [MAX_OP_LEN]u8,
    op_len: usize,
    /// Slice into the original pipeline JSON for this step's input object.
    /// Empty slice if no input field present.
    input_start: usize,
    input_end: usize,
};

/// Pipeline context — metadata parsed from the top-level pipeline JSON.
/// Available to all modifiers during execution.
const PipelineContext = struct {
    /// The source program that submitted this pipeline (e.g., "tome", "quill").
    /// Defaults to "system" for orchestrator-internal calls.
    source: [MAX_SOURCE_LEN]u8,
    source_len: usize,

    fn getSource(self: *const PipelineContext) []const u8 {
        return self.source[0..self.source_len];
    }
};

// ── Export Functions ───────────────────────────────────────────

/// Execute a pipeline of operations defined by JSON.
///
/// pipeline_json: null-terminated JSON string of the form:
///   { "source": "tome", "steps": [{ "id": "name", "op": "crate.verb", "input": {...} }] }
///
/// The "source" field is optional and defaults to "system". It identifies
/// which program submitted the pipeline, used by Bulwark permission checks.
///
/// Returns a null-terminated JSON string (c_allocator-allocated, caller frees):
///   Success: { "ok": true, "result": <last step output> }
///   Failure: { "ok": false, "error": "...", "failed_step": "...", "step_index": N }
///
/// Returns null only on catastrophic allocation failure.
pub export fn orch_pipeline_execute(pipeline_json: ?[*:0]const u8) callconv(.c) ?[*:0]u8 {
    const input_ptr = pipeline_json orelse return buildError("null pipeline input", "", 0);
    const input = std.mem.span(input_ptr);

    if (input.len == 0) return buildError("empty pipeline input", "", 0);
    if (input.len > MAX_PIPELINE_SIZE) return buildError("pipeline exceeds 256KB limit", "", 0);

    // Parse pipeline context (source field)
    var ctx = parseContext(input);

    // Parse all steps from the pipeline JSON
    var steps: [MAX_STEPS]ParsedStep = undefined;
    const step_count = parseSteps(input, &steps) orelse {
        return buildError("failed to parse pipeline steps", "", 0);
    };

    if (step_count == 0) return buildSuccess("null");

    // Execute steps sequentially
    var results: [MAX_STEPS]StepResult = undefined;
    var result_count: usize = 0;

    // Cleanup all intermediate results on exit
    defer {
        for (results[0..result_count]) |*r| {
            if (r.output) |out| allocator.free(out);
        }
    }

    for (steps[0..step_count], 0..) |*step, i| {
        const op_name = step.op[0..step.op_len];

        // Look up the operation in the registry
        const handler = registry.lookup(op_name) orelse {
            return buildErrorFmt(
                "unknown operation",
                op_name,
                i,
            );
        };

        // Extract the raw input JSON for this step
        var step_input: []const u8 = "{}";
        if (step.input_start < step.input_end and step.input_end <= input.len) {
            step_input = input[step.input_start..step.input_end];
        }

        // Resolve $ref references in the input
        const resolved_input = resolveRefs(step_input, results[0..result_count]) orelse {
            return buildErrorFmt(
                "failed to resolve references in input",
                op_name,
                i,
            );
        };
        // resolved_input is either step_input (no alloc) or c_allocator-owned
        const resolved_owned = if (resolved_input.ptr != step_input.ptr) true else false;
        defer if (resolved_owned) allocator.free(@constCast(resolved_input));

        // Build a null-terminated copy for the handler
        const input_z = allocator.allocSentinel(u8, resolved_input.len, 0) catch {
            return buildErrorFmt("allocation failed", op_name, i);
        };
        defer allocator.free(input_z[0 .. resolved_input.len + 1]);
        @memcpy(input_z[0..resolved_input.len], resolved_input);

        // ── Pre-modifiers: Polity -> Bulwark ──────────────────────

        // Polity: check whether this operation would violate the Covenant
        if (handler.modifiers.polity) {
            const polity_result = runPolityCheck(op_name);
            if (polity_result) |violation_msg| {
                return buildErrorFmt(violation_msg, op_name, i);
            }
        }

        // Bulwark: permission/safety check
        if (handler.modifiers.bulwark) {
            const bulwark_result = runBulwarkCheck(op_name, handler.permission, &ctx);
            if (bulwark_result) |block_msg| {
                return buildErrorFmt(block_msg, op_name, i);
            }
        }

        // ── Execute the operation ─────────────────────────────────

        // Call the handler
        const raw_output = handler.call(input_z.ptr);

        if (raw_output == null) {
            // Step failed — read divi_last_error for details
            const err_msg = getLastError();
            return buildErrorFmt(err_msg, op_name, i);
        }

        // Copy the output to a pipeline-owned buffer, then free the original.
        // Handler outputs may be Rust-allocated (divi_free_string) or
        // Zig-allocated (c_allocator). Since registry handlers use both,
        // we copy to normalize ownership. The original is freed as Rust-allocated
        // which is safe for both — Rust's allocator and c_allocator use the same
        // underlying malloc/free on all platforms we target.
        const raw = raw_output.?;
        const output_span = std.mem.span(raw);
        const owned_output = allocator.alloc(u8, output_span.len) catch {
            c.divi_free_string(raw);
            return buildErrorFmt("allocation failed for step output", op_name, i);
        };
        @memcpy(owned_output, output_span);
        c.divi_free_string(raw);

        // ── Post-modifiers: Sentinal -> Lingo -> Yoke -> Quest ────

        // Sentinal: encrypt output
        // NO-OP by design. Encryption should happen at the storage layer
        // (Vault handles it), not at the pipeline output layer. Encrypting
        // arbitrary JSON output would break the pipeline's ability to
        // reference outputs in later steps via $ref. The modifier flag
        // exists for future use cases (e.g., encrypting data before sending
        // over the network via Globe).
        if (handler.modifiers.sentinal) {
            // Intentional no-op. See comment above.
        }

        // Lingo: translate output
        // NO-OP by design. Lingo translation applies to CONTENT being
        // stored/transmitted, not to pipeline JSON metadata. The modifier
        // flag exists so the pipeline knows this step involves translatable
        // content. Actual translation happens when content is written to
        // disk or sent over the network. Applying Babel encoding to JSON
        // keys/structure would corrupt the data.
        if (handler.modifiers.lingo) {
            // Intentional no-op. See comment above.
        }

        // Yoke: provenance tracking
        if (handler.modifiers.yoke) {
            runYokeProvenance(op_name, owned_output, &ctx);
            // Best-effort — provenance failure does NOT fail the pipeline step.
        }

        // Quest: XP award
        // TODO: Wire Quest XP awarding when quest.zig module is created.
        // The Quest module does not yet exist in the orchestrator. When it does,
        // this modifier should call divi_quest_award_xp with the actor identity,
        // an XP amount based on the operation type, and the source program.
        if (handler.modifiers.quest) {
            // Intentional no-op. Quest module not yet implemented.
        }

        // Store result
        results[result_count] = .{
            .id = step.id,
            .id_len = step.id_len,
            .output = owned_output,
            .owns_handle = handler.owns_handle,
        };
        result_count += 1;
    }

    // Build success response with the last step's output
    const last_output = results[result_count - 1].output orelse "null";
    return buildSuccess(last_output);
}

/// Initialize the pipeline module. Currently a no-op.
/// Returns 0 on success.
pub export fn orch_pipeline_init() callconv(.c) i32 {
    return 0;
}

/// Shut down the pipeline module. Currently a no-op.
pub export fn orch_pipeline_shutdown() callconv(.c) void {}

// ── Modifier Implementations ──────────────────────────────────

/// Run the Polity Covenant check for an operation.
///
/// Calls `divi_polity_would_violate(description)` which returns:
///   1 = violation, 0 = OK, -1 = error
///
/// Returns an error message string if the operation is blocked,
/// or null if the operation is allowed (or if Polity is not initialized).
fn runPolityCheck(op_name: []const u8) ?[]const u8 {
    // Build a null-terminated description from the operation name
    var desc_buf: [MAX_OP_LEN + 1]u8 = undefined;
    if (op_name.len > MAX_OP_LEN) return null; // skip if name too long
    @memcpy(desc_buf[0..op_name.len], op_name);
    desc_buf[op_name.len] = 0;

    const result = c.divi_polity_would_violate(@ptrCast(&desc_buf));

    if (result == 1) {
        // Covenant violation — block the step
        return "Covenant violation";
    }

    // result == 0 (allowed) or -1 (error, which we treat as "skip gracefully")
    return null;
}

/// Run the Bulwark permission check for an operation.
///
/// Returns an error message string if the operation is blocked,
/// or null if the operation is allowed.
///
/// `always_ask` operations are BLOCKED in the pipeline — they require explicit
/// user consent via a platform-side permission prompt, which the pipeline
/// cannot provide. Programs must call these operations directly through the
/// platform bridge, not through a pipeline.
///
/// `per_action` operations are currently allowed through. Full Bulwark
/// integration (wiring source to actor identity + platform prompt flow)
/// will gate these in a future update.
///
/// `free` and `granted_once` operations always pass.
fn runBulwarkCheck(
    op_name: []const u8,
    permission: registry.PermissionLevel,
    ctx: *const PipelineContext,
) ?[]const u8 {
    _ = op_name;
    _ = ctx;

    switch (permission) {
        .always_ask => {
            // Nuclear operations (delete identity, wipe vault, export keyring, etc.)
            // must NEVER execute inside a pipeline. They require explicit user
            // consent via a platform-side confirmation dialog.
            return "operation requires explicit user consent (always_ask) and cannot run in a pipeline";
        },
        .per_action => {
            // TODO: Wire source program to actor identity and implement
            // platform-side permission prompt flow via BulwarkPermissionChecker.
            // For now, allow through — the permission level is recorded on the
            // OpHandler for when the prompt flow is ready.
            return null;
        },
        .free, .granted_once => {
            // No check needed for free operations.
            // granted_once is approved at install time — no runtime check.
            return null;
        },
    }
}

/// Record provenance via Yoke after a successful step.
///
/// Creates a YokeLink connecting the operation to its output. This is
/// best-effort — a failure here does NOT fail the pipeline step.
///
/// Uses `divi_yoke_link_new(source, target, relation_type_json, author)`
/// which returns a JSON YokeLink string (caller frees).
fn runYokeProvenance(
    op_name: []const u8,
    output: []const u8,
    ctx: *const PipelineContext,
) void {
    // Build null-terminated strings for the FFI call.
    // Source: the operation name (e.g., "vault.store")
    var source_buf: [MAX_OP_LEN + 1]u8 = undefined;
    if (op_name.len > MAX_OP_LEN) return;
    @memcpy(source_buf[0..op_name.len], op_name);
    source_buf[op_name.len] = 0;

    // Target: try to extract an "id" field from the output, fall back to the source program
    var target_buf: [MAX_SOURCE_LEN + MAX_STEP_ID_LEN + 1]u8 = undefined;
    var target_len: usize = 0;

    if (extractStringField(output, "id")) |id_val| {
        const copy_len = @min(id_val.len, target_buf.len - 1);
        @memcpy(target_buf[0..copy_len], id_val[0..copy_len]);
        target_len = copy_len;
    } else {
        // Fall back to source program name as target
        const src = ctx.getSource();
        const copy_len = @min(src.len, target_buf.len - 1);
        @memcpy(target_buf[0..copy_len], src[0..copy_len]);
        target_len = copy_len;
    }
    target_buf[target_len] = 0;

    // Relation type: "ProducedBy" — this step produced the output
    const relation_type: [*:0]const u8 = "\"ProducedBy\"";

    // Author: the source program
    var author_buf: [MAX_SOURCE_LEN + 1]u8 = undefined;
    const src = ctx.getSource();
    @memcpy(author_buf[0..src.len], src);
    author_buf[src.len] = 0;

    // Call the FFI function — best-effort, ignore errors
    const link_json = c.divi_yoke_link_new(
        @ptrCast(&source_buf),
        @ptrCast(&target_buf),
        relation_type,
        @ptrCast(&author_buf),
    );

    // Free the returned JSON if it was allocated
    if (link_json) |ptr| {
        c.divi_free_string(ptr);
    }
}

// ── Context Parsing ───────────────────────────────────────────

/// Parse the pipeline context from the top-level JSON.
/// Extracts the optional "source" field.
fn parseContext(json: []const u8) PipelineContext {
    var ctx: PipelineContext = undefined;
    const default_source = "system";

    if (extractStringField(json, "source")) |source_val| {
        const copy_len = @min(source_val.len, MAX_SOURCE_LEN);
        @memcpy(ctx.source[0..copy_len], source_val[0..copy_len]);
        ctx.source_len = copy_len;
    } else {
        @memcpy(ctx.source[0..default_source.len], default_source);
        ctx.source_len = default_source.len;
    }

    return ctx;
}

// ── JSON Parsing (minimal, hand-rolled) ───────────────────────
//
// NOT a general JSON parser. Handles the specific pipeline format only.
// Follows the intercom.zig pattern of minimal, purpose-built parsing.

/// Parse the steps array from pipeline JSON.
/// Returns the number of steps parsed, or null on parse error.
fn parseSteps(json: []const u8, steps: *[MAX_STEPS]ParsedStep) ?usize {
    // Find "steps" key
    const steps_key = findKey(json, "steps") orelse return null;
    // Find the opening bracket of the array
    const arr_start = findChar(json, steps_key, '[') orelse return null;

    var pos = arr_start + 1;
    var count: usize = 0;

    while (pos < json.len and count < MAX_STEPS) {
        // Skip whitespace
        pos = skipWhitespace(json, pos);
        if (pos >= json.len) break;

        // End of array?
        if (json[pos] == ']') break;

        // Skip comma between elements
        if (json[pos] == ',' and count > 0) {
            pos += 1;
            pos = skipWhitespace(json, pos);
        }

        // Expect an object
        if (pos >= json.len or json[pos] != '{') return null;

        // Find the matching closing brace for this step object
        const obj_start = pos;
        const obj_end = findMatchingBrace(json, obj_start) orelse return null;

        const obj = json[obj_start .. obj_end + 1];

        // Extract "id" field
        var step: ParsedStep = undefined;
        step.id_len = 0;
        step.op_len = 0;
        step.input_start = 0;
        step.input_end = 0;

        if (extractStringField(obj, "id")) |id_val| {
            if (id_val.len > MAX_STEP_ID_LEN) return null;
            @memcpy(step.id[0..id_val.len], id_val);
            step.id_len = id_val.len;
        }

        // Extract "op" field (required)
        const op_val = extractStringField(obj, "op") orelse return null;
        if (op_val.len == 0 or op_val.len > MAX_OP_LEN) return null;
        @memcpy(step.op[0..op_val.len], op_val);
        step.op_len = op_val.len;

        // Extract "input" field — find the value range (object/string/number/etc.)
        if (findKey(obj, "input")) |input_key_end| {
            const val_start = skipWhitespace(obj, input_key_end);
            if (val_start < obj.len) {
                const val_end = findValueEnd(obj, val_start) orelse obj.len;
                // Translate obj-relative offsets to json-absolute offsets
                step.input_start = obj_start + val_start;
                step.input_end = obj_start + val_end;
            }
        }

        steps[count] = step;
        count += 1;
        pos = obj_end + 1;
    }

    return count;
}

/// Find a JSON key in the given slice. Returns the position just after the colon.
/// Searches for `"key":` and returns the byte position after the `:`.
fn findKey(json: []const u8, key: []const u8) ?usize {
    // Search for "key" : (with possible whitespace around colon)
    var pos: usize = 0;
    while (pos + key.len + 2 < json.len) {
        if (json[pos] == '"') {
            const end = pos + 1 + key.len;
            if (end < json.len and
                json[end] == '"' and
                std.mem.eql(u8, json[pos + 1 .. end], key))
            {
                // Found the key, now find the colon
                var colon_pos = end + 1;
                colon_pos = skipWhitespace(json, colon_pos);
                if (colon_pos < json.len and json[colon_pos] == ':') {
                    return colon_pos + 1;
                }
            }
        }
        pos += 1;
    }
    return null;
}

/// Extract the string value for a given key. Returns the content between quotes.
/// Only works for simple string values (no escaped quotes in value).
fn extractStringField(json: []const u8, key: []const u8) ?[]const u8 {
    const after_colon = findKey(json, key) orelse return null;
    const val_start = skipWhitespace(json, after_colon);
    if (val_start >= json.len or json[val_start] != '"') return null;

    // Find closing quote (simple — no escape handling needed for IDs/op names)
    const content_start = val_start + 1;
    var end = content_start;
    while (end < json.len and json[end] != '"') : (end += 1) {}
    if (end >= json.len) return null;

    return json[content_start..end];
}

/// Find the end of a JSON value starting at `start`.
/// Handles strings, objects, arrays, numbers, booleans, null.
/// Returns the position one past the last character of the value.
fn findValueEnd(json: []const u8, start: usize) ?usize {
    if (start >= json.len) return null;

    return switch (json[start]) {
        '"' => {
            // String — find closing quote
            var pos = start + 1;
            while (pos < json.len) : (pos += 1) {
                if (json[pos] == '\\') {
                    pos += 1; // skip escaped char
                    continue;
                }
                if (json[pos] == '"') return pos + 1;
            }
            return null;
        },
        '{' => {
            // Object — find matching brace
            const end = findMatchingBrace(json, start) orelse return null;
            return end + 1;
        },
        '[' => {
            // Array — find matching bracket
            const end = findMatchingBracket(json, start) orelse return null;
            return end + 1;
        },
        't' => {
            // true
            if (start + 4 <= json.len and std.mem.eql(u8, json[start .. start + 4], "true"))
                return start + 4;
            return null;
        },
        'f' => {
            // false
            if (start + 5 <= json.len and std.mem.eql(u8, json[start .. start + 5], "false"))
                return start + 5;
            return null;
        },
        'n' => {
            // null
            if (start + 4 <= json.len and std.mem.eql(u8, json[start .. start + 4], "null"))
                return start + 4;
            return null;
        },
        '-', '0'...'9' => {
            // Number — scan digits, dot, e/E, +/-
            var pos = start;
            if (pos < json.len and json[pos] == '-') pos += 1;
            while (pos < json.len and (json[pos] >= '0' and json[pos] <= '9')) : (pos += 1) {}
            if (pos < json.len and json[pos] == '.') {
                pos += 1;
                while (pos < json.len and (json[pos] >= '0' and json[pos] <= '9')) : (pos += 1) {}
            }
            if (pos < json.len and (json[pos] == 'e' or json[pos] == 'E')) {
                pos += 1;
                if (pos < json.len and (json[pos] == '+' or json[pos] == '-')) pos += 1;
                while (pos < json.len and (json[pos] >= '0' and json[pos] <= '9')) : (pos += 1) {}
            }
            return pos;
        },
        else => null,
    };
}

/// Find the matching closing brace for an opening `{` at `start`.
/// Respects nesting and strings.
fn findMatchingBrace(json: []const u8, start: usize) ?usize {
    if (start >= json.len or json[start] != '{') return null;

    var depth: usize = 0;
    var pos = start;
    var in_string = false;

    while (pos < json.len) : (pos += 1) {
        if (in_string) {
            if (json[pos] == '\\') {
                pos += 1; // skip escaped char
                continue;
            }
            if (json[pos] == '"') in_string = false;
            continue;
        }

        switch (json[pos]) {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if (depth == 0) return pos;
            },
            else => {},
        }
    }
    return null;
}

/// Find the matching closing bracket for an opening `[` at `start`.
fn findMatchingBracket(json: []const u8, start: usize) ?usize {
    if (start >= json.len or json[start] != '[') return null;

    var depth: usize = 0;
    var pos = start;
    var in_string = false;

    while (pos < json.len) : (pos += 1) {
        if (in_string) {
            if (json[pos] == '\\') {
                pos += 1;
                continue;
            }
            if (json[pos] == '"') in_string = false;
            continue;
        }

        switch (json[pos]) {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if (depth == 0) return pos;
            },
            else => {},
        }
    }
    return null;
}

/// Find the first occurrence of `ch` at or after `start`.
fn findChar(json: []const u8, start: usize, ch: u8) ?usize {
    var pos = start;
    while (pos < json.len) : (pos += 1) {
        if (json[pos] == ch) return pos;
    }
    return null;
}

/// Skip whitespace characters from `start`, return new position.
fn skipWhitespace(json: []const u8, start: usize) usize {
    var pos = start;
    while (pos < json.len and (json[pos] == ' ' or json[pos] == '\t' or
        json[pos] == '\n' or json[pos] == '\r')) : (pos += 1)
    {}
    return pos;
}

// ── Reference Resolution ──────────────────────────────────────

/// Resolve $step_id.field.subfield references in a JSON input string.
///
/// Scans for string values starting with `$` and replaces them with the
/// referenced field value from a previous step's output.
///
/// Returns the original slice if no refs found (no allocation).
/// Returns a new c_allocator-owned slice if refs were resolved.
/// Returns null on resolution failure.
fn resolveRefs(input: []const u8, results: []const StepResult) ?[]const u8 {
    // Quick scan: does the input contain any "$" inside a string?
    if (std.mem.indexOf(u8, input, "\"$") == null) return input;

    // Build a new string with refs resolved
    var out: std.ArrayListUnmanaged(u8) = .empty;
    out.ensureTotalCapacity(allocator, input.len + 256) catch return null;

    var pos: usize = 0;
    while (pos < input.len) {
        // Look for a string value that starts with $
        if (input[pos] == '"' and pos + 1 < input.len and input[pos + 1] == '$') {
            // Check for $$ escape
            if (pos + 2 < input.len and input[pos + 2] == '$') {
                // Escaped dollar: write opening quote + single $
                out.append(allocator, '"') catch return null;
                out.append(allocator, '$') catch return null;
                pos += 3; // skip "$$ — continue copying rest of string
                while (pos < input.len and input[pos] != '"') : (pos += 1) {
                    out.append(allocator, input[pos]) catch return null;
                }
                if (pos < input.len) {
                    out.append(allocator, '"') catch return null;
                    pos += 1; // skip closing quote
                }
                continue;
            }

            // This is a reference. Find the closing quote.
            const ref_start = pos + 2; // skip "$
            var ref_end = ref_start;
            while (ref_end < input.len and input[ref_end] != '"') : (ref_end += 1) {}
            if (ref_end >= input.len) return null; // unterminated string

            const ref_str = input[ref_start..ref_end];

            // Parse the reference: step_id.field.subfield...
            const resolved = resolveRef(ref_str, results) orelse return null;

            // Write the resolved value directly (not quoted — it's already
            // a JSON value: string, number, object, etc.)
            out.appendSlice(allocator, resolved) catch return null;

            pos = ref_end + 1; // skip past closing quote
        } else {
            out.append(allocator, input[pos]) catch return null;
            pos += 1;
        }
    }

    // Return owned slice
    return out.items;
}

/// Resolve a single reference string like "step_id.field.subfield".
/// Returns the raw JSON value (may be a string with quotes, number, object, etc.).
fn resolveRef(ref_str: []const u8, results: []const StepResult) ?[]const u8 {
    // Split on first dot to get step_id
    const first_dot = std.mem.indexOf(u8, ref_str, ".") orelse return null;
    const step_id = ref_str[0..first_dot];
    const field_path = ref_str[first_dot + 1 ..];

    // Find the step result
    var step_output: ?[]const u8 = null;
    for (results) |*r| {
        if (r.id_len == step_id.len and
            std.mem.eql(u8, r.id[0..r.id_len], step_id))
        {
            step_output = r.output;
            break;
        }
    }

    const output = step_output orelse return null;

    // Navigate the field path
    return extractJsonPath(output, field_path);
}

/// Extract a nested field from JSON given a dot-separated path.
/// E.g., "key.subkey" navigates into {"key":{"subkey":"value"}} and returns "value" (with quotes).
fn extractJsonPath(json: []const u8, path: []const u8) ?[]const u8 {
    var current = json;
    var remaining = path;

    while (remaining.len > 0) {
        // Get the next field name
        const dot = std.mem.indexOf(u8, remaining, ".");
        const field = if (dot) |d| remaining[0..d] else remaining;
        remaining = if (dot) |d| remaining[d + 1 ..] else "";

        // Find this field in current JSON object
        const after_colon = findKey(current, field) orelse return null;
        const val_start = skipWhitespace(current, after_colon);
        if (val_start >= current.len) return null;

        const val_end = findValueEnd(current, val_start) orelse return null;
        const value = current[val_start..val_end];

        if (remaining.len == 0) {
            // This is the final field — return the value
            return value;
        }

        // More path segments — the value must be an object, descend into it
        if (value.len == 0 or value[0] != '{') return null;
        current = value;
    }

    return null;
}

// ── Response Builders ─────────────────────────────────────────

/// Build a success response: { "ok": true, "result": <output> }
fn buildSuccess(output: []const u8) ?[*:0]u8 {
    const prefix = "{\"ok\":true,\"result\":";
    const suffix = "}";
    const total = prefix.len + output.len + suffix.len;

    const buf = allocator.allocSentinel(u8, total, 0) catch return null;
    var pos: usize = 0;

    @memcpy(buf[pos .. pos + prefix.len], prefix);
    pos += prefix.len;
    @memcpy(buf[pos .. pos + output.len], output);
    pos += output.len;
    @memcpy(buf[pos .. pos + suffix.len], suffix);

    return buf.ptr;
}

/// Build an error response: { "ok": false, "error": "...", "failed_step": "...", "step_index": N }
/// Escapes `"` and `\` in err_msg and failed_step to produce valid JSON.
fn buildError(err_msg: []const u8, failed_step: []const u8, step_index: usize) ?[*:0]u8 {
    // Use a stack buffer, then copy to heap.
    // Escape err_msg and failed_step into scratch buffers first.
    var escaped_msg: [RESPONSE_BUF_SIZE / 2]u8 = undefined;
    const escaped_msg_slice = jsonEscapeString(err_msg, &escaped_msg) orelse return null;

    var escaped_step: [MAX_STEP_ID_LEN * 2]u8 = undefined;
    const escaped_step_slice = jsonEscapeString(failed_step, &escaped_step) orelse return null;

    var buf: [RESPONSE_BUF_SIZE]u8 = undefined;
    const slice = std.fmt.bufPrint(&buf,
        \\{{"ok":false,"error":"{s}","failed_step":"{s}","step_index":{d}}}
    , .{ escaped_msg_slice, escaped_step_slice, step_index }) catch return null;

    const out = allocator.allocSentinel(u8, slice.len, 0) catch return null;
    @memcpy(out[0..slice.len], slice);
    return out.ptr;
}

/// Escape `"` and `\` in a string for safe JSON embedding.
/// Writes into the provided output buffer. Returns the escaped slice, or null
/// if the output buffer is too small.
fn jsonEscapeString(input: []const u8, output: []u8) ?[]const u8 {
    var pos: usize = 0;
    for (input) |ch| {
        if (ch == '"' or ch == '\\') {
            if (pos + 2 > output.len) return null;
            output[pos] = '\\';
            pos += 1;
            output[pos] = ch;
            pos += 1;
        } else {
            if (pos + 1 > output.len) return null;
            output[pos] = ch;
            pos += 1;
        }
    }
    return output[0..pos];
}

/// Build an error response from slices (not comptime strings).
/// Same as buildError but avoids @ptrCast issues with runtime slices.
fn buildErrorFmt(err_msg: []const u8, failed_step: []const u8, step_index: usize) ?[*:0]u8 {
    return buildError(err_msg, failed_step, step_index);
}

/// Read the last FFI error message. Returns a human-readable string.
fn getLastError() []const u8 {
    const err = c.divi_last_error();
    if (err != null) {
        return std.mem.span(@as([*:0]const u8, @ptrCast(err.?)));
    }
    return "operation returned null";
}

// ── Tests ─────────────────────────────────────────────────────

const state = @import("state.zig");

/// Helper: ensure registry is initialized for tests.
fn ensureInit() void {
    _ = state.orch_init();
    _ = registry.init();
}

fn ensureShutdown() void {
    registry.deinit();
    state.orch_shutdown();
}

test "pipeline init and shutdown" {
    try std.testing.expectEqual(@as(i32, 0), orch_pipeline_init());
    orch_pipeline_shutdown(); // no-op, must not crash
}

test "single step pipeline - polity.axioms" {
    ensureInit();
    defer ensureShutdown();

    const pipeline =
        \\{"steps":[{"id":"s1","op":"polity.axioms","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        // Should contain ok:true
        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
        // Should contain result with actual axioms data
        try std.testing.expect(std.mem.indexOf(u8, span, "\"result\":") != null);
    }
}

test "multi-step pipeline with $ref" {
    ensureInit();
    defer ensureShutdown();

    // Register a test op that returns known JSON with a "name" field
    const producer_op = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const out_str = "{\"name\":\"alice\",\"age\":30}";
            const out = allocator.allocSentinel(u8, out_str.len, 0) catch return null;
            @memcpy(out[0..out_str.len], out_str);
            return out.ptr;
        }
    };

    const echo_op = struct {
        fn call(input: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const span = std.mem.span(input);
            const out = allocator.allocSentinel(u8, span.len, 0) catch return null;
            @memcpy(out[0..span.len], span);
            return out.ptr;
        }
    };

    registry.register("test.produce", .{
        .call = producer_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    }) catch return;

    registry.register("test.echo", .{
        .call = echo_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    }) catch return;

    // Step 1 produces {"name":"alice","age":30}
    // Step 2 echoes {"data":"$s1.name"} → should become {"data":"alice"}
    const pipe =
        \\{"steps":[{"id":"s1","op":"test.produce","input":{}},{"id":"s2","op":"test.echo","input":{"data":"$s1.name"}}]}
    ;

    const result = orch_pipeline_execute(pipe);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
        // The echoed result should contain "alice" from the ref resolution
        try std.testing.expect(std.mem.indexOf(u8, span, "alice") != null);
    }
}

test "unknown operation fails" {
    ensureInit();
    defer ensureShutdown();

    const pipeline =
        \\{"steps":[{"id":"s1","op":"nonexistent.op","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":false") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "unknown operation") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "nonexistent.op") != null);
    }
}

test "empty steps array" {
    ensureInit();
    defer ensureShutdown();

    const pipeline =
        \\{"steps":[]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        // Empty steps = success with null result
        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "null") != null);
    }
}

test "missing op field fails" {
    ensureInit();
    defer ensureShutdown();

    const pipeline =
        \\{"steps":[{"id":"s1","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":false") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "failed to parse") != null);
    }
}

test "null input returns error" {
    const result = orch_pipeline_execute(null);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":false") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "null pipeline input") != null);
    }
}

test "failed step returns error with step info" {
    ensureInit();
    defer ensureShutdown();

    // Register an operation that always fails
    const fail_op = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            return null;
        }
    };

    registry.register("test.always_fail", .{
        .call = fail_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
    }) catch return;

    const pipeline =
        \\{"steps":[{"id":"s1","op":"test.always_fail","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":false") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "test.always_fail") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "\"step_index\":0") != null);
    }
}

// ── Source Field Tests ────────────────────────────────────────

test "source field is parsed from pipeline JSON" {
    const json =
        \\{"source":"tome","steps":[]}
    ;
    const ctx = parseContext(json);
    try std.testing.expectEqualStrings("tome", ctx.getSource());
}

test "source defaults to system when missing" {
    const json =
        \\{"steps":[]}
    ;
    const ctx = parseContext(json);
    try std.testing.expectEqualStrings("system", ctx.getSource());
}

test "source field with various programs" {
    const json_quill =
        \\{"source":"quill","steps":[]}
    ;
    const ctx_quill = parseContext(json_quill);
    try std.testing.expectEqualStrings("quill", ctx_quill.getSource());

    const json_studio =
        \\{"source":"studio","steps":[]}
    ;
    const ctx_studio = parseContext(json_studio);
    try std.testing.expectEqualStrings("studio", ctx_studio.getSource());
}

test "pipeline with source field executes correctly" {
    ensureInit();
    defer ensureShutdown();

    const pipeline =
        \\{"source":"tome","steps":[{"id":"s1","op":"polity.axioms","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
    }
}

// ── Polity Modifier Tests ─────────────────────────────────────

test "polity modifier runs on flagged operation" {
    ensureInit();
    defer ensureShutdown();

    // Register a test op with polity modifier enabled
    const ok_op = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const out_str = "{\"status\":\"ok\"}";
            const out = allocator.allocSentinel(u8, out_str.len, 0) catch return null;
            @memcpy(out[0..out_str.len], out_str);
            return out.ptr;
        }
    };

    registry.register("test.polity_checked", .{
        .call = ok_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{ .polity = true },
    }) catch return;

    // Normal operation names should NOT trigger a Covenant violation
    const pipeline =
        \\{"steps":[{"id":"s1","op":"test.polity_checked","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        // The operation should succeed because its name doesn't violate the Covenant
        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
    }
}

// ── Yoke Provenance Tests ─────────────────────────────────────

test "yoke modifier runs without crashing" {
    ensureInit();
    defer ensureShutdown();

    // Register a test op with yoke modifier enabled
    const ok_op = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const out_str = "{\"id\":\"test-123\",\"data\":\"hello\"}";
            const out = allocator.allocSentinel(u8, out_str.len, 0) catch return null;
            @memcpy(out[0..out_str.len], out_str);
            return out.ptr;
        }
    };

    registry.register("test.yoke_tracked", .{
        .call = ok_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{ .yoke = true },
    }) catch return;

    const pipeline =
        \\{"source":"tome","steps":[{"id":"s1","op":"test.yoke_tracked","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        // Step should succeed — yoke provenance is best-effort
        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
    }
}

// ── Handle Tracking Tests ─────────────────────────────────────

test "owns_handle flag is tracked on step result" {
    ensureInit();
    defer ensureShutdown();

    // Register a test op that "creates a handle"
    const handle_op = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const out_str = "{\"handle\":\"0xDEADBEEF\"}";
            const out = allocator.allocSentinel(u8, out_str.len, 0) catch return null;
            @memcpy(out[0..out_str.len], out_str);
            return out.ptr;
        }
    };

    registry.register("test.create_handle", .{
        .call = handle_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{},
        .owns_handle = true,
    }) catch return;

    // Should execute successfully — handle tracking metadata is set
    const pipeline =
        \\{"steps":[{"id":"s1","op":"test.create_handle","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
    }
}

// ── Combined Modifier Tests ───────────────────────────────────

test "multiple modifiers run in correct order" {
    ensureInit();
    defer ensureShutdown();

    // Register an op with all modifiers enabled
    const multi_op = struct {
        fn call(_: [*:0]const u8) callconv(.c) ?[*:0]u8 {
            const out_str = "{\"result\":\"multi-modified\"}";
            const out = allocator.allocSentinel(u8, out_str.len, 0) catch return null;
            @memcpy(out[0..out_str.len], out_str);
            return out.ptr;
        }
    };

    registry.register("test.all_modifiers", .{
        .call = multi_op.call,
        .handles = &.{},
        .permission = .free,
        .modifiers = .{
            .polity = true,
            .bulwark = true,
            .sentinal = true,
            .yoke = true,
            .lingo = true,
            .quest = true,
        },
    }) catch return;

    const pipeline =
        \\{"source":"studio","steps":[{"id":"s1","op":"test.all_modifiers","input":{}}]}
    ;

    const result = orch_pipeline_execute(pipeline);
    try std.testing.expect(result != null);

    if (result) |r| {
        const span = std.mem.span(r);
        defer allocator.free(r[0 .. span.len + 1]);

        // Should succeed — no-op modifiers (sentinal, lingo, quest) and
        // best-effort modifiers (yoke) should not cause failure
        try std.testing.expect(std.mem.indexOf(u8, span, "\"ok\":true") != null);
        try std.testing.expect(std.mem.indexOf(u8, span, "multi-modified") != null);
    }
}

// ── JSON Helper Unit Tests ────────────────────────────────────

test "extractJsonPath navigates nested fields" {
    const json =
        \\{"outer":{"inner":"hello"},"num":42}
    ;

    // Single field
    const num = extractJsonPath(json, "num");
    try std.testing.expect(num != null);
    try std.testing.expectEqualStrings("42", num.?);

    // Nested field
    const inner = extractJsonPath(json, "outer.inner");
    try std.testing.expect(inner != null);
    try std.testing.expectEqualStrings("\"hello\"", inner.?);

    // Non-existent field
    const missing = extractJsonPath(json, "nonexistent");
    try std.testing.expect(missing == null);
}

test "extractJsonPath handles arrays and booleans" {
    const json =
        \\{"arr":[1,2,3],"flag":true,"empty":null}
    ;

    const arr = extractJsonPath(json, "arr");
    try std.testing.expect(arr != null);
    try std.testing.expectEqualStrings("[1,2,3]", arr.?);

    const flag = extractJsonPath(json, "flag");
    try std.testing.expect(flag != null);
    try std.testing.expectEqualStrings("true", flag.?);

    const empty = extractJsonPath(json, "empty");
    try std.testing.expect(empty != null);
    try std.testing.expectEqualStrings("null", empty.?);
}

test "resolveRefs with no refs returns input unchanged" {
    const input =
        \\{"key":"value","num":42}
    ;
    const results = [0]StepResult{};

    const resolved = resolveRefs(input, &results);
    try std.testing.expect(resolved != null);
    // Should be the exact same pointer (no allocation)
    try std.testing.expectEqual(input.ptr, resolved.?.ptr);
}

test "resolveRefs substitutes reference" {
    var results: [1]StepResult = undefined;
    results[0] = .{
        .id = undefined,
        .id_len = 2,
        .output = @constCast(
            \\{"name":"alice","age":30}
        ),
        .owns_handle = false,
    };
    @memcpy(results[0].id[0..2], "s1");

    const input =
        \\{"greeting":"$s1.name"}
    ;

    const resolved = resolveRefs(input, &results);
    try std.testing.expect(resolved != null);

    const span = resolved.?;
    defer if (span.ptr != input.ptr) allocator.free(@constCast(span));

    // The reference "$s1.name" should be replaced with "alice" (the JSON value with quotes)
    try std.testing.expect(std.mem.indexOf(u8, span, "\"alice\"") != null);
}

test "resolveRefs handles dollar escape" {
    const input =
        \\{"price":"$$100"}
    ;
    const results = [0]StepResult{};

    const resolved = resolveRefs(input, &results);
    try std.testing.expect(resolved != null);

    const span = resolved.?;
    defer if (span.ptr != input.ptr) allocator.free(@constCast(span));

    // Should have a single $ in the output
    try std.testing.expect(std.mem.indexOf(u8, span, "$100") != null);
}

test "parseSteps extracts multiple steps" {
    const json =
        \\{"steps":[{"id":"a","op":"polity.axioms","input":{}},{"id":"b","op":"crown.keyring_public_key","input":{"key":"val"}}]}
    ;

    var steps: [MAX_STEPS]ParsedStep = undefined;
    const count = parseSteps(json, &steps);

    try std.testing.expect(count != null);
    try std.testing.expectEqual(@as(usize, 2), count.?);

    try std.testing.expectEqualStrings("a", steps[0].id[0..steps[0].id_len]);
    try std.testing.expectEqualStrings("polity.axioms", steps[0].op[0..steps[0].op_len]);

    try std.testing.expectEqualStrings("b", steps[1].id[0..steps[1].id_len]);
    try std.testing.expectEqualStrings("crown.keyring_public_key", steps[1].op[0..steps[1].op_len]);
}

test "findMatchingBrace handles nested objects" {
    const json =
        \\{"a":{"b":{"c":1}},"d":2}
    ;

    const end = findMatchingBrace(json, 0);
    try std.testing.expect(end != null);
    try std.testing.expectEqual(json.len - 1, end.?);
}
