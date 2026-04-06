// Governance orchestration — Kingdom + Polity + Bulwark + Jail.
//
// Communities, rights, consent, trust. The Covenant made executable.
// Each registry is created on first use and lives in module-level state.
// Operations previously here are now served by the pipeline executor.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");

// ── Module State ──

var rights: ?*c.PolityRightsRegistry = null;
var duties: ?*c.PolityDutiesRegistry = null;
var protections: ?*c.PolityProtectionsRegistry = null;
var consent: ?*c.PolityConsentRegistry = null;
var breaches: ?*c.PolityBreachRegistry = null;
var enactments: ?*c.PolityEnactmentRegistry = null;
var trust_graph: ?*c.JailTrustGraph = null;
var permission_checker: ?*c.BulwarkPermissionChecker = null;
var consent_validator: ?*c.BulwarkConsentValidator = null;

/// Mutex protecting all module-level state.
var mod_mutex: std.Thread.Mutex = .{};

// ── Init Functions ──────────────────────────────────────────────

/// Initialize the rights registry with Covenant defaults.
/// Returns 0 on success.
export fn orch_rights_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (rights != null) return 0;
    rights = c.divi_polity_rights_new_with_covenant();
    return if (rights != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the duties registry with Covenant defaults.
/// Returns 0 on success.
export fn orch_duties_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (duties != null) return 0;
    duties = c.divi_polity_duties_new_with_covenant();
    return if (duties != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the protections registry with Covenant defaults.
/// Returns 0 on success.
export fn orch_protections_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (protections != null) return 0;
    protections = c.divi_polity_protections_new_with_covenant();
    return if (protections != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the consent registry.
/// Returns 0 on success.
export fn orch_consent_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (consent != null) return 0;
    consent = c.divi_polity_consent_registry_new();
    return if (consent != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the breach registry.
/// Returns 0 on success.
export fn orch_breach_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (breaches != null) return 0;
    breaches = c.divi_polity_breach_new();
    return if (breaches != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the trust graph.
/// Returns 0 on success.
export fn orch_trust_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (trust_graph != null) return 0;
    trust_graph = c.divi_jail_trust_graph_new();
    return if (trust_graph != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the permission checker.
/// Returns 0 on success.
export fn orch_permission_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (permission_checker != null) return 0;
    permission_checker = c.divi_bulwark_permission_checker_new();
    return if (permission_checker != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the enactment registry.
/// Returns 0 on success.
export fn orch_enactment_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (enactments != null) return 0;
    enactments = c.divi_polity_enactment_registry_new();
    return if (enactments != null) @as(i32, 0) else @as(i32, -1);
}

/// Initialize the consent validator.
/// Returns 0 on success.
export fn orch_consent_validator_init() i32 {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (consent_validator != null) return 0;
    consent_validator = c.divi_bulwark_consent_validator_new();
    return if (consent_validator != null) @as(i32, 0) else @as(i32, -1);
}

// ── Handle Accessors ──────────────────────────────────────────────

/// Get the active BulwarkPermissionChecker handle.
/// Returns null if the permission checker has not been initialized.
/// Thread-safe — acquires and releases the module mutex.
pub fn getPermissionChecker() ?*c.BulwarkPermissionChecker {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return permission_checker;
}

/// Get the active PolityRightsRegistry handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getRights() ?*c.PolityRightsRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return rights;
}

/// Get the active PolityProtectionsRegistry handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getProtections() ?*c.PolityProtectionsRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return protections;
}

/// Get the active PolityDutiesRegistry handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getDuties() ?*c.PolityDutiesRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return duties;
}

/// Get the active PolityConsentRegistry handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getConsent() ?*c.PolityConsentRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return consent;
}

/// Get the active PolityBreachRegistry handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getBreaches() ?*c.PolityBreachRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return breaches;
}

/// Get the active PolityEnactmentRegistry handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getEnactments() ?*c.PolityEnactmentRegistry {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return enactments;
}

/// Get the active JailTrustGraph handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getTrustGraph() ?*c.JailTrustGraph {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return trust_graph;
}

/// Get the active BulwarkConsentValidator handle.
/// Returns null if not initialized.
/// Thread-safe.
pub fn getConsentValidator() ?*c.BulwarkConsentValidator {
    mod_mutex.lock();
    defer mod_mutex.unlock();
    return consent_validator;
}

// ── Shutdown ──

/// Free all governance module state. Called by orch_shutdown.
pub export fn orch_governance_shutdown() void {
    mod_mutex.lock();
    defer mod_mutex.unlock();

    if (consent_validator) |p| c.divi_bulwark_consent_validator_free(p);
    if (permission_checker) |p| c.divi_bulwark_permission_checker_free(p);
    if (trust_graph) |p| c.divi_jail_trust_graph_free(p);
    if (enactments) |p| c.divi_polity_enactment_registry_free(p);
    if (breaches) |p| c.divi_polity_breach_free(p);
    if (consent) |p| c.divi_polity_consent_registry_free(p);
    if (protections) |p| c.divi_polity_protections_free(p);
    if (duties) |p| c.divi_polity_duties_free(p);
    if (rights) |p| c.divi_polity_rights_free(p);

    rights = null;
    duties = null;
    protections = null;
    consent = null;
    breaches = null;
    enactments = null;
    trust_graph = null;
    permission_checker = null;
    consent_validator = null;
}

// ── Tests ─────────────────────────────────────────────────────

test "rights init with covenant" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_governance_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_rights_init());
}

test "duties init with covenant" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_governance_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_duties_init());
}

test "trust graph init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_governance_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_trust_init());
}

test "consent registry init" {
    try std.testing.expectEqual(@as(i32, 0), state.orch_init());
    defer {
        orch_governance_shutdown();
        state.orch_shutdown();
    }

    try std.testing.expectEqual(@as(i32, 0), orch_consent_init());
}
