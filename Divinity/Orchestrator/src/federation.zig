// Federation orchestration — Kingdom federation agreements and registry.
//
// Cross-community federation: propose, accept, suspend, reactivate, withdraw.
// The registry tracks all agreements and supports path-finding between communities.
// Operations previously here are now served by the pipeline executor.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// -- Module State --

var federation_registry: ?*c.FederationRegistry = null;

/// Mutex protecting all module-level state.
var mod_mutex: std.Thread.Mutex = .{};

// -- Init ------------------------------------------------------------------

/// Initialize the federation registry.
/// Idempotent — returns 0 if already initialized.
/// Returns 0 on success, -1 on failure.
export fn orch_federation_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (federation_registry != null) return 0;
    federation_registry = c.divi_kingdom_federation_registry_new();
    return if (federation_registry != null) @as(i32, 0) else @as(i32, -1);
}

// -- Handle Accessor -------------------------------------------------------

/// Get the active FederationRegistry handle.
/// Returns null if not initialized.
/// Thread-safe — acquires and releases the module mutex.
pub fn getRegistry() ?*c.FederationRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return federation_registry;
}

// -- Core Operations -------------------------------------------------------

/// Propose a federation agreement between two communities.
///
/// Creates a new agreement in Proposed status, then registers it in the
/// module's registry so it is tracked for queries (is_federated, path_between, etc.).
///
/// Parameters:
///   community_a  — ID of the first community (null-terminated C string)
///   community_b  — ID of the second community (null-terminated C string)
///   proposed_by  — ID of the proposer (null-terminated C string)
///   scopes_json  — JSON array of FederationScope values (null-terminated C string)
///
/// Returns JSON (FederationAgreement) on success. Caller must free via divi_free_string.
/// Returns null on failure (check orch_last_error).
export fn orch_federation_propose(
    community_a: [*:0]const u8,
    community_b: [*:0]const u8,
    proposed_by: [*:0]const u8,
    scopes_json: [*:0]const u8,
) ?[*:0]u8 {
    // Propose the agreement (stateless FFI call — no module state needed).
    const proposal = c.divi_kingdom_federation_propose(community_a, community_b, proposed_by, scopes_json);
    if (proposal == null) return null;

    // Register it in the module registry.
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const reg = federation_registry orelse {
        // Registry not initialized — return the proposal as-is.
        return proposal;
    };

    const registered = c.divi_kingdom_federation_registry_register(reg, proposal);

    // Free the intermediate proposal string — we return the registered result.
    c.divi_free_string(proposal);

    return registered;
}

/// Accept a proposed federation agreement (Proposed -> Active).
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to accept (null-terminated C string)
///   accepted_by    — ID of the acceptor (null-terminated C string)
///
/// Returns updated JSON (FederationAgreement). Caller must free via divi_free_string.
/// Returns null on failure.
export fn orch_federation_accept(
    agreement_json: [*:0]const u8,
    accepted_by: [*:0]const u8,
) ?[*:0]u8 {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_accept(agreement_json, accepted_by);
}

/// Suspend an active federation agreement (Active -> Suspended).
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to suspend (null-terminated C string)
///   reason         — reason for suspension (null-terminated C string)
///
/// Returns updated JSON (FederationAgreement). Caller must free via divi_free_string.
/// Returns null on failure.
export fn orch_federation_suspend(
    agreement_json: [*:0]const u8,
    reason: [*:0]const u8,
) ?[*:0]u8 {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_suspend(agreement_json, reason);
}

/// Reactivate a suspended federation agreement (Suspended -> Active).
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to reactivate (null-terminated C string)
///
/// Returns updated JSON (FederationAgreement). Caller must free via divi_free_string.
/// Returns null on failure.
export fn orch_federation_reactivate(
    agreement_json: [*:0]const u8,
) ?[*:0]u8 {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_reactivate(agreement_json);
}

/// Withdraw from a federation agreement (Active|Suspended -> Withdrawn).
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to withdraw from (null-terminated C string)
///   withdrawn_by   — ID of the withdrawing party (null-terminated C string)
///   reason         — reason for withdrawal (null-terminated C string)
///
/// Returns updated JSON (FederationAgreement). Caller must free via divi_free_string.
/// Returns null on failure.
export fn orch_federation_withdraw(
    agreement_json: [*:0]const u8,
    withdrawn_by: [*:0]const u8,
    reason: [*:0]const u8,
) ?[*:0]u8 {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_withdraw(agreement_json, withdrawn_by, reason);
}

/// Get the status of a federation agreement.
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to query (null-terminated C string)
///
/// Returns JSON (FederationStatus). Caller must free via divi_free_string.
/// Returns null on failure.
export fn orch_federation_status(
    agreement_json: [*:0]const u8,
) ?[*:0]u8 {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_status(agreement_json);
}

/// Check whether a federation agreement involves a specific community.
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to check (null-terminated C string)
///   community_id   — community ID to look for (null-terminated C string)
///
/// Returns true if the community is party to the agreement, false otherwise.
export fn orch_federation_involves(
    agreement_json: [*:0]const u8,
    community_id: [*:0]const u8,
) bool {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_involves(agreement_json, community_id);
}

/// Get the partner community ID from a federation agreement.
///
/// Parameters:
///   agreement_json — JSON (FederationAgreement) to query (null-terminated C string)
///   community_id   — community ID whose partner to look up (null-terminated C string)
///
/// Returns the partner's community ID string, or null if community_id is not a party.
/// Caller must free via divi_free_string.
export fn orch_federation_partner_of(
    agreement_json: [*:0]const u8,
    community_id: [*:0]const u8,
) ?[*:0]u8 {
    // Stateless FFI call — no module state needed.
    return c.divi_kingdom_federation_partner_of(agreement_json, community_id);
}

/// Check whether two communities are actively federated.
///
/// Requires the federation registry to be initialized (orch_federation_init).
///
/// Parameters:
///   community_a — ID of the first community (null-terminated C string)
///   community_b — ID of the second community (null-terminated C string)
///
/// Returns 1 = yes, 0 = no, -1 = error (registry not initialized or invalid input).
export fn orch_federation_is_federated(
    community_a: [*:0]const u8,
    community_b: [*:0]const u8,
) i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const reg = federation_registry orelse return -1;
    return c.divi_kingdom_federation_registry_is_federated(reg, community_a, community_b);
}

/// List all communities actively federated with a given community.
///
/// Requires the federation registry to be initialized.
///
/// Parameters:
///   community_id — ID of the community to query (null-terminated C string)
///
/// Returns JSON array of community ID strings. Caller must free via divi_free_string.
/// Returns null on failure (registry not initialized or error).
export fn orch_federation_federated_with(
    community_id: [*:0]const u8,
) ?[*:0]u8 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const reg = federation_registry orelse return null;
    return c.divi_kingdom_federation_registry_federated_with(reg, community_id);
}

/// List all active federation agreements in the registry.
///
/// Requires the federation registry to be initialized.
///
/// Returns JSON array of FederationAgreement objects.
/// Caller must free via divi_free_string.
/// Returns null on failure.
export fn orch_federation_list_active() ?[*:0]u8 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const reg = federation_registry orelse return null;
    return c.divi_kingdom_federation_registry_all_active(reg);
}

/// Find a federation path between two communities via active links (BFS).
///
/// Requires the federation registry to be initialized.
///
/// Parameters:
///   from — source community ID (null-terminated C string)
///   to   — target community ID (null-terminated C string)
///
/// Returns JSON array of community ID strings (the path), or JSON null
/// if no path exists. Caller must free via divi_free_string.
/// Returns null on error.
export fn orch_federation_path_between(
    from: [*:0]const u8,
    to: [*:0]const u8,
) ?[*:0]u8 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const reg = federation_registry orelse return null;
    return c.divi_kingdom_federation_registry_path_between(reg, from, to);
}

/// Get the total number of agreements in the registry (any status).
///
/// Requires the federation registry to be initialized.
///
/// Returns the count, or -1 if registry not initialized.
export fn orch_federation_count() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    const reg = federation_registry orelse return -1;
    return c.divi_kingdom_federation_registry_count(reg);
}

// -- Shutdown --------------------------------------------------------------

/// Free all federation module state. Called by orch_shutdown.
pub export fn orch_federation_shutdown() void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (federation_registry) |p| c.divi_kingdom_federation_registry_free(p);
    federation_registry = null;
}

// -- Tests -----------------------------------------------------------------

test "federation init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_federation_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_federation_init());
    try std.testing.expect(getRegistry() != null);
}

test "federation double init is idempotent" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_federation_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_federation_init());
    try std.testing.expectEqual(@as(i32, 0), orch_federation_init());
}

test "federation shutdown without init is safe" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();

    // Should not crash — registry is null, shutdown is a no-op.
    orch_federation_shutdown();
}

test "federation count returns -1 before init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer state.orch_shutdown();

    // Registry not initialized — count should return -1.
    try std.testing.expectEqual(@as(i32, -1), orch_federation_count());
}

test "federation count returns 0 after init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_federation_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_federation_init());
    try std.testing.expectEqual(@as(i32, 0), orch_federation_count());
}
