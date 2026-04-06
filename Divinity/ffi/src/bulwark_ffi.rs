use std::ffi::c_char;
use std::sync::Mutex;

use bulwark::{
    AgeTier, AgeTierConfig,
    ChildSafetyConcern, ChildSafetyFlag, ChildSafetyProtocol, RealWorldResources,
    ConsentRecord, ConsentScope, ConsentValidator,
    Delegation, FraudIndicator,
    KidsSphereConfig, ParentLink, ParentOversight, SiloedMinor,
    MinorDetectionReason, FamilyBond,
    PermissionChecker, PermissionDecision, PermissionSource, DenialReason,
    ActorContext, Action, ResourceScope, Role,
    Reputation, ReputationEvent, RiskScore, TrustLayer, UserHealthFactors, UserHealthPulse,
    CollectiveHealthFactors, CollectiveHealthPulse,
    LayerTransitionRequirements, LayerTransitionEvidence,
    VisibleBond, BondDepth,
};
use bulwark::trust::layer_transition::check_transition;

use serde::Serialize;

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// FFI-serializable wrappers for types that don't derive Serialize/Deserialize
// ---------------------------------------------------------------------------

/// Serializable representation of `PermissionDecision` for JSON round-trip.
#[derive(Debug, Serialize)]
struct FfiPermissionDecision {
    allowed: bool,
    source: Option<FfiPermissionSource>,
    denial_reason: Option<FfiDenialReason>,
}

#[derive(Debug, Serialize)]
enum FfiPermissionSource {
    Role(String),
    Conditional,
    Delegation { delegator: String },
}

#[derive(Debug, Serialize)]
enum FfiDenialReason {
    NoPermission,
    RolePrerequisitesNotMet { role: String, reason: String },
    ConditionsNotMet,
}

impl From<PermissionDecision> for FfiPermissionDecision {
    fn from(decision: PermissionDecision) -> Self {
        match decision {
            PermissionDecision::Allowed(source) => FfiPermissionDecision {
                allowed: true,
                source: Some(match source {
                    PermissionSource::Role(name) => FfiPermissionSource::Role(name),
                    PermissionSource::Conditional => FfiPermissionSource::Conditional,
                    PermissionSource::Delegation { delegator } => {
                        FfiPermissionSource::Delegation { delegator }
                    }
                }),
                denial_reason: None,
            },
            PermissionDecision::Denied(reason) => FfiPermissionDecision {
                allowed: false,
                source: None,
                denial_reason: Some(match reason {
                    DenialReason::NoPermission => FfiDenialReason::NoPermission,
                    DenialReason::RolePrerequisitesNotMet { role, reason } => {
                        FfiDenialReason::RolePrerequisitesNotMet { role, reason }
                    }
                    DenialReason::ConditionsNotMet => FfiDenialReason::ConditionsNotMet,
                }),
            },
        }
    }
}

/// Serializable representation of a layer transition result for JSON round-trip.
#[derive(Debug, Serialize)]
struct FfiLayerTransitionResult {
    allowed: bool,
    blockers: Vec<bulwark::LayerTransitionBlocker>,
}

// ===================================================================
// Trust Layer
// ===================================================================

/// Get capabilities for a trust layer.
///
/// `layer_json` is a JSON TrustLayer (e.g. `"Verified"`).
/// Returns JSON (LayerCapabilities). Caller must free via `divi_free_string`.
///
/// # Safety
/// `layer_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_trust_layer_capabilities(
    layer_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(lj) = c_str_to_str(layer_json) else {
        set_last_error("divi_bulwark_trust_layer_capabilities: invalid layer_json");
        return std::ptr::null_mut();
    };

    let layer: TrustLayer = match serde_json::from_str(lj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!("divi_bulwark_trust_layer_capabilities: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&layer.capabilities())
}

// ===================================================================
// Bonds
// ===================================================================

/// Create a new visible bond between two parties.
///
/// `depth_json` is a JSON BondDepth (e.g. `"Friend"`).
/// Returns JSON (VisibleBond). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_bond_new(
    party_a: *const c_char,
    party_b: *const c_char,
    depth_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(a) = c_str_to_str(party_a) else {
        set_last_error("divi_bulwark_bond_new: invalid party_a");
        return std::ptr::null_mut();
    };

    let Some(b) = c_str_to_str(party_b) else {
        set_last_error("divi_bulwark_bond_new: invalid party_b");
        return std::ptr::null_mut();
    };

    let Some(dj) = c_str_to_str(depth_json) else {
        set_last_error("divi_bulwark_bond_new: invalid depth_json");
        return std::ptr::null_mut();
    };

    let depth: BondDepth = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_bulwark_bond_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let bond = VisibleBond::new(a, b, depth);
    json_to_c(&bond)
}

/// Update a bond's depth from one party's perspective.
///
/// Takes bond JSON, returns modified bond JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_bond_update_depth(
    bond_json: *const c_char,
    pubkey: *const c_char,
    new_depth_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(bj) = c_str_to_str(bond_json) else {
        set_last_error("divi_bulwark_bond_update_depth: invalid bond_json");
        return std::ptr::null_mut();
    };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_bulwark_bond_update_depth: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(dj) = c_str_to_str(new_depth_json) else {
        set_last_error("divi_bulwark_bond_update_depth: invalid new_depth_json");
        return std::ptr::null_mut();
    };

    let depth: BondDepth = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_bulwark_bond_update_depth: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut bond: VisibleBond = match serde_json::from_str(bj) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(format!("divi_bulwark_bond_update_depth: {e}"));
            return std::ptr::null_mut();
        }
    };

    bond.update_depth(pk, depth);
    json_to_c(&bond)
}

// ===================================================================
// Health
// ===================================================================

/// Compute a user health pulse from factors.
///
/// `factors_json` is a JSON UserHealthFactors.
/// Returns JSON (UserHealthPulse). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_user_health_compute(
    pubkey: *const c_char,
    factors_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_bulwark_user_health_compute: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(fj) = c_str_to_str(factors_json) else {
        set_last_error("divi_bulwark_user_health_compute: invalid factors_json");
        return std::ptr::null_mut();
    };

    let factors: UserHealthFactors = match serde_json::from_str(fj) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_bulwark_user_health_compute: {e}"));
            return std::ptr::null_mut();
        }
    };

    let pulse = UserHealthPulse::compute(pk, factors);
    json_to_c(&pulse)
}

/// Compute a collective health pulse from factors.
///
/// `collective_id` is a UUID string. `factors_json` is a JSON CollectiveHealthFactors.
/// Returns JSON (CollectiveHealthPulse). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_collective_health_compute(
    collective_id: *const c_char,
    factors_json: *const c_char,
    contributing_members: u32,
) -> *mut c_char {
    clear_last_error();

    let Some(cid) = c_str_to_str(collective_id) else {
        set_last_error("divi_bulwark_collective_health_compute: invalid collective_id");
        return std::ptr::null_mut();
    };

    let Some(fj) = c_str_to_str(factors_json) else {
        set_last_error("divi_bulwark_collective_health_compute: invalid factors_json");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(cid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_bulwark_collective_health_compute: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let factors: CollectiveHealthFactors = match serde_json::from_str(fj) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_bulwark_collective_health_compute: {e}"));
            return std::ptr::null_mut();
        }
    };

    let pulse = CollectiveHealthPulse::compute(uuid, factors, contributing_members);
    json_to_c(&pulse)
}

// ===================================================================
// Reputation
// ===================================================================

/// Create a new reputation for a pubkey (starts at default scores).
///
/// Returns JSON (Reputation). Caller must free via `divi_free_string`.
///
/// # Safety
/// `pubkey` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_reputation_new(
    pubkey: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_bulwark_reputation_new: invalid pubkey");
        return std::ptr::null_mut();
    };

    let rep = Reputation::new(pk);
    json_to_c(&rep)
}

/// Apply a reputation event to a reputation.
///
/// Takes reputation JSON + event JSON, returns modified reputation JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_reputation_apply_event(
    reputation_json: *const c_char,
    event_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(reputation_json) else {
        set_last_error("divi_bulwark_reputation_apply_event: invalid reputation_json");
        return std::ptr::null_mut();
    };

    let Some(ej) = c_str_to_str(event_json) else {
        set_last_error("divi_bulwark_reputation_apply_event: invalid event_json");
        return std::ptr::null_mut();
    };

    let mut rep: Reputation = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_bulwark_reputation_apply_event: {e}"));
            return std::ptr::null_mut();
        }
    };

    let event: ReputationEvent = match serde_json::from_str(ej) {
        Ok(ev) => ev,
        Err(e) => {
            set_last_error(format!("divi_bulwark_reputation_apply_event: {e}"));
            return std::ptr::null_mut();
        }
    };

    rep.apply_event(event);
    json_to_c(&rep)
}

// ===================================================================
// Child Safety (Covenant-mandated: must be present from day 1)
// ===================================================================

/// File a child safety flag.
///
/// `concern_json` is a JSON ChildSafetyConcern (e.g. `"Grooming"`).
/// Returns JSON (ChildSafetyFlag). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_child_safety_flag_file(
    reporter_pubkey: *const c_char,
    concern_json: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(reporter) = c_str_to_str(reporter_pubkey) else {
        set_last_error("divi_bulwark_child_safety_flag_file: invalid reporter_pubkey");
        return std::ptr::null_mut();
    };

    let Some(cj) = c_str_to_str(concern_json) else {
        set_last_error("divi_bulwark_child_safety_flag_file: invalid concern_json");
        return std::ptr::null_mut();
    };

    let Some(desc) = c_str_to_str(description) else {
        set_last_error("divi_bulwark_child_safety_flag_file: invalid description");
        return std::ptr::null_mut();
    };

    let concern: ChildSafetyConcern = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_bulwark_child_safety_flag_file: {e}"));
            return std::ptr::null_mut();
        }
    };

    let flag = ChildSafetyFlag::file(reporter, concern, desc);
    json_to_c(&flag)
}

/// Get the default child safety protocol (all protections enabled).
///
/// Returns JSON (ChildSafetyProtocol). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bulwark_child_safety_protocol_default() -> *mut c_char {
    json_to_c(&ChildSafetyProtocol::default())
}

/// Get US default real-world resources (911, 988, Childhelp).
///
/// Returns JSON (RealWorldResources). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bulwark_real_world_resources() -> *mut c_char {
    json_to_c(&RealWorldResources::us_defaults())
}

// ===================================================================
// Permissions — opaque pointer (PermissionChecker)
// ===================================================================

pub struct BulwarkPermissionChecker(pub(crate) Mutex<PermissionChecker>);

/// Create a new empty permission checker.
/// Free with `divi_bulwark_permission_checker_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bulwark_permission_checker_new() -> *mut BulwarkPermissionChecker {
    Box::into_raw(Box::new(BulwarkPermissionChecker(Mutex::new(
        PermissionChecker::new(),
    ))))
}

/// Free a permission checker.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_permission_checker_free(
    ptr: *mut BulwarkPermissionChecker,
) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register an app-defined role in the permission checker.
///
/// `role_json` is a JSON Role (name, description, permissions, etc.).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `checker` must be a valid pointer. `role_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_permission_checker_register_role(
    checker: *const BulwarkPermissionChecker,
    role_json: *const c_char,
) -> i32 {
    clear_last_error();

    let checker = unsafe { &*checker };
    let Some(rj) = c_str_to_str(role_json) else {
        set_last_error("divi_bulwark_permission_checker_register_role: invalid role_json");
        return -1;
    };

    let role: Role = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!(
                "divi_bulwark_permission_checker_register_role: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&checker.0);
    match guard.register_role(role) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Check whether an actor can perform an action on a resource (no context).
///
/// `actor_json` is a JSON ActorContext. `action` and `resource` are plain strings.
/// Returns JSON `{"allowed": true/false}`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `checker` must be a valid pointer. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_permission_checker_can(
    checker: *const BulwarkPermissionChecker,
    actor_json: *const c_char,
    action: *const c_char,
    resource: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let checker = unsafe { &*checker };
    let Some(aj) = c_str_to_str(actor_json) else {
        set_last_error("divi_bulwark_permission_checker_can: invalid actor_json");
        return std::ptr::null_mut();
    };

    let Some(act_str) = c_str_to_str(action) else {
        set_last_error("divi_bulwark_permission_checker_can: invalid action");
        return std::ptr::null_mut();
    };

    let Some(res_str) = c_str_to_str(resource) else {
        set_last_error("divi_bulwark_permission_checker_can: invalid resource");
        return std::ptr::null_mut();
    };

    let actor: ActorContext = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_bulwark_permission_checker_can: {e}"));
            return std::ptr::null_mut();
        }
    };

    let act = Action::new(act_str);
    let res = ResourceScope::new(res_str);

    let guard = lock_or_recover(&checker.0);
    let allowed = guard.can(&actor, &act, &res);

    #[derive(Serialize)]
    struct CanResult {
        allowed: bool,
    }

    json_to_c(&CanResult { allowed })
}

/// Full permission check with context — returns decision with source/denial details.
///
/// `actor_json` is a JSON ActorContext. `action` and `resource` are plain strings.
/// `context_json` is a JSON PermissionContext (may be null for empty context).
/// Returns JSON (FfiPermissionDecision). Caller must free via `divi_free_string`.
///
/// # Safety
/// `checker` must be a valid pointer. C strings must be valid. `context_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_permission_checker_check(
    checker: *const BulwarkPermissionChecker,
    actor_json: *const c_char,
    action: *const c_char,
    resource: *const c_char,
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let checker = unsafe { &*checker };
    let Some(aj) = c_str_to_str(actor_json) else {
        set_last_error("divi_bulwark_permission_checker_check: invalid actor_json");
        return std::ptr::null_mut();
    };

    let Some(act_str) = c_str_to_str(action) else {
        set_last_error("divi_bulwark_permission_checker_check: invalid action");
        return std::ptr::null_mut();
    };

    let Some(res_str) = c_str_to_str(resource) else {
        set_last_error("divi_bulwark_permission_checker_check: invalid resource");
        return std::ptr::null_mut();
    };

    let actor: ActorContext = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_bulwark_permission_checker_check: {e}"));
            return std::ptr::null_mut();
        }
    };

    let context = if context_json.is_null() {
        bulwark::PermissionContext::new()
    } else if let Some(cj) = c_str_to_str(context_json) {
        match serde_json::from_str(cj) {
            Ok(c) => c,
            Err(e) => {
                set_last_error(format!("divi_bulwark_permission_checker_check: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        bulwark::PermissionContext::new()
    };

    let act = Action::new(act_str);
    let res = ResourceScope::new(res_str);

    let guard = lock_or_recover(&checker.0);
    let decision = guard.check(&actor, &act, &res, &context);
    let ffi_decision: FfiPermissionDecision = decision.into();

    json_to_c(&ffi_decision)
}

/// Grant a delegation in the permission checker.
///
/// `delegation_json` is a JSON Delegation.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `checker` must be a valid pointer. `delegation_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_permission_checker_grant_delegation(
    checker: *const BulwarkPermissionChecker,
    delegation_json: *const c_char,
) -> i32 {
    clear_last_error();

    let checker = unsafe { &*checker };
    let Some(dj) = c_str_to_str(delegation_json) else {
        set_last_error(
            "divi_bulwark_permission_checker_grant_delegation: invalid delegation_json",
        );
        return -1;
    };

    let delegation: Delegation = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!(
                "divi_bulwark_permission_checker_grant_delegation: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&checker.0);
    guard.delegations.grant(delegation);
    0
}

/// Revoke a delegation by UUID string.
///
/// `delegation_id` is a UUID string.
/// Returns 0 on success (revoked), -1 on error (not found or invalid UUID).
///
/// # Safety
/// `checker` must be a valid pointer. `delegation_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_permission_checker_revoke_delegation(
    checker: *const BulwarkPermissionChecker,
    delegation_id: *const c_char,
) -> i32 {
    clear_last_error();

    let checker = unsafe { &*checker };
    let Some(id_str) = c_str_to_str(delegation_id) else {
        set_last_error(
            "divi_bulwark_permission_checker_revoke_delegation: invalid delegation_id",
        );
        return -1;
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_bulwark_permission_checker_revoke_delegation: invalid UUID: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&checker.0);
    if guard.delegations.revoke_by_id(uuid) {
        0
    } else {
        set_last_error(
            "divi_bulwark_permission_checker_revoke_delegation: delegation not found",
        );
        -1
    }
}

// ===================================================================
// Kids Sphere — JSON round-trip
// ===================================================================

/// Get the default Kids Sphere config.
///
/// Returns JSON (KidsSphereConfig). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bulwark_kids_sphere_config_default() -> *mut c_char {
    json_to_c(&KidsSphereConfig::default())
}

/// Get parent oversight settings appropriate for a given age.
///
/// Returns JSON (ParentOversight). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bulwark_parent_oversight_for_age(age: u8) -> *mut c_char {
    json_to_c(&ParentOversight::for_age(age))
}

/// Create a new siloed minor (local-only, no network access).
///
/// `pubkey` is the minor's public key. `age` is their claimed age (0 if unknown).
/// `reason_json` is a JSON MinorDetectionReason (e.g. `"SelfDeclared"`).
/// Returns JSON (SiloedMinor). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_siloed_minor_new(
    pubkey: *const c_char,
    age: u8,
    reason_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_bulwark_siloed_minor_new: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(rj) = c_str_to_str(reason_json) else {
        set_last_error("divi_bulwark_siloed_minor_new: invalid reason_json");
        return std::ptr::null_mut();
    };

    let reason: MinorDetectionReason = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_bulwark_siloed_minor_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let claimed_age = if age == 0 { None } else { Some(age) };
    let minor = SiloedMinor::new(pk, claimed_age, reason);
    json_to_c(&minor)
}

/// Link a parent to a siloed minor. Transitions state to ParentLinked.
///
/// `minor_json` is a JSON SiloedMinor. `parent_link_json` is a JSON ParentLink.
/// Returns modified JSON (SiloedMinor). Caller must free via `divi_free_string`.
/// Returns null on error (e.g. already linked).
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_siloed_minor_link_parent(
    minor_json: *const c_char,
    parent_link_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(mj) = c_str_to_str(minor_json) else {
        set_last_error("divi_bulwark_siloed_minor_link_parent: invalid minor_json");
        return std::ptr::null_mut();
    };

    let Some(pj) = c_str_to_str(parent_link_json) else {
        set_last_error("divi_bulwark_siloed_minor_link_parent: invalid parent_link_json");
        return std::ptr::null_mut();
    };

    let mut minor: SiloedMinor = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!(
                "divi_bulwark_siloed_minor_link_parent: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let link: ParentLink = match serde_json::from_str(pj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!(
                "divi_bulwark_siloed_minor_link_parent: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = minor.link_parent(link) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&minor)
}

/// Authorize a parent-linked minor for Kids Sphere access.
///
/// `minor_json` is a JSON SiloedMinor (must be in ParentLinked state).
/// Returns modified JSON (SiloedMinor). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// `minor_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_siloed_minor_authorize(
    minor_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(mj) = c_str_to_str(minor_json) else {
        set_last_error("divi_bulwark_siloed_minor_authorize: invalid minor_json");
        return std::ptr::null_mut();
    };

    let mut minor: SiloedMinor = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_bulwark_siloed_minor_authorize: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = minor.authorize() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&minor)
}

/// Create a new family bond between two parents. REQUIRES proximity proof.
///
/// `parent_a` and `parent_b` are pubkeys. `proof_json` is a JSON ProximityProof.
/// Returns JSON (FamilyBond). Caller must free via `divi_free_string`.
/// Returns null if proximity proof is invalid.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_family_bond_new(
    parent_a: *const c_char,
    parent_b: *const c_char,
    proof_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pa) = c_str_to_str(parent_a) else {
        set_last_error("divi_bulwark_family_bond_new: invalid parent_a");
        return std::ptr::null_mut();
    };

    let Some(pb) = c_str_to_str(parent_b) else {
        set_last_error("divi_bulwark_family_bond_new: invalid parent_b");
        return std::ptr::null_mut();
    };

    let Some(pj) = c_str_to_str(proof_json) else {
        set_last_error("divi_bulwark_family_bond_new: invalid proof_json");
        return std::ptr::null_mut();
    };

    let proof: bulwark::ProximityProof = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_bulwark_family_bond_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    match FamilyBond::new(pa, pb, proof) {
        Ok(bond) => json_to_c(&bond),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Consent — opaque pointer (ConsentValidator)
// ===================================================================

pub struct BulwarkConsentValidator(pub(crate) Mutex<ConsentValidator>);

/// Create a new empty consent validator.
/// Free with `divi_bulwark_consent_validator_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bulwark_consent_validator_new() -> *mut BulwarkConsentValidator {
    Box::into_raw(Box::new(BulwarkConsentValidator(Mutex::new(
        ConsentValidator::new(),
    ))))
}

/// Free a consent validator.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_consent_validator_free(
    ptr: *mut BulwarkConsentValidator,
) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Grant a consent record.
///
/// `record_json` is a JSON ConsentRecord.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `validator` must be a valid pointer. `record_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_consent_validator_grant(
    validator: *const BulwarkConsentValidator,
    record_json: *const c_char,
) -> i32 {
    clear_last_error();

    let validator = unsafe { &*validator };
    let Some(rj) = c_str_to_str(record_json) else {
        set_last_error("divi_bulwark_consent_validator_grant: invalid record_json");
        return -1;
    };

    let record: ConsentRecord = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_bulwark_consent_validator_grant: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&validator.0);
    guard.grant(record);
    0
}

/// Check if active consent exists from grantor to recipient for a scope.
///
/// `grantor` and `recipient` are pubkey strings. `scope_json` is a JSON ConsentScope.
/// Returns true if active consent exists.
///
/// # Safety
/// `validator` must be a valid pointer. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_consent_validator_has_consent(
    validator: *const BulwarkConsentValidator,
    grantor: *const c_char,
    recipient: *const c_char,
    scope_json: *const c_char,
) -> bool {
    let validator = unsafe { &*validator };
    let Some(g) = c_str_to_str(grantor) else {
        return false;
    };

    let Some(r) = c_str_to_str(recipient) else {
        return false;
    };

    let Some(sj) = c_str_to_str(scope_json) else {
        return false;
    };

    let scope: ConsentScope = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let guard = lock_or_recover(&validator.0);
    guard.has_consent(g, r, scope)
}

/// Revoke all consent from grantor to recipient.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `validator` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_consent_validator_revoke_all(
    validator: *const BulwarkConsentValidator,
    grantor: *const c_char,
    recipient: *const c_char,
) -> i32 {
    clear_last_error();

    let validator = unsafe { &*validator };
    let Some(g) = c_str_to_str(grantor) else {
        set_last_error("divi_bulwark_consent_validator_revoke_all: invalid grantor");
        return -1;
    };

    let Some(r) = c_str_to_str(recipient) else {
        set_last_error("divi_bulwark_consent_validator_revoke_all: invalid recipient");
        return -1;
    };

    let mut guard = lock_or_recover(&validator.0);
    guard.revoke_all(g, r);
    0
}

// ===================================================================
// Age Tier — JSON round-trip
// ===================================================================

/// Get the age tier for a given age.
///
/// `config_json` may be null (uses default AgeTierConfig).
/// Returns JSON (AgeTier, e.g. `"Kid"`). Caller must free via `divi_free_string`.
///
/// # Safety
/// `config_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_age_tier_from_age(
    age: u8,
    config_json: *const c_char,
) -> *mut c_char {
    let config = if config_json.is_null() {
        AgeTierConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        AgeTierConfig::default()
    };

    let tier = AgeTier::from_age(age, &config);
    json_to_c(&tier)
}

// ===================================================================
// Layer Transitions — JSON round-trip
// ===================================================================

/// Check eligibility for a trust layer transition.
///
/// `current_json` and `target_json` are JSON TrustLayer values.
/// `requirements_json` is a JSON LayerTransitionRequirements (null for defaults).
/// `evidence_json` is a JSON LayerTransitionEvidence.
/// Returns JSON with `allowed: bool` and `blockers: [string]`.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `requirements_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_check_layer_transition(
    current_json: *const c_char,
    target_json: *const c_char,
    requirements_json: *const c_char,
    evidence_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(current_json) else {
        set_last_error("divi_bulwark_check_layer_transition: invalid current_json");
        return std::ptr::null_mut();
    };

    let Some(tj) = c_str_to_str(target_json) else {
        set_last_error("divi_bulwark_check_layer_transition: invalid target_json");
        return std::ptr::null_mut();
    };

    let Some(ej) = c_str_to_str(evidence_json) else {
        set_last_error("divi_bulwark_check_layer_transition: invalid evidence_json");
        return std::ptr::null_mut();
    };

    let current: TrustLayer = match serde_json::from_str(cj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!("divi_bulwark_check_layer_transition: {e}"));
            return std::ptr::null_mut();
        }
    };

    let target: TrustLayer = match serde_json::from_str(tj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!("divi_bulwark_check_layer_transition: {e}"));
            return std::ptr::null_mut();
        }
    };

    let requirements = if requirements_json.is_null() {
        LayerTransitionRequirements::default()
    } else if let Some(rj) = c_str_to_str(requirements_json) {
        match serde_json::from_str(rj) {
            Ok(r) => r,
            Err(e) => {
                set_last_error(format!("divi_bulwark_check_layer_transition: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        LayerTransitionRequirements::default()
    };

    let evidence: LayerTransitionEvidence = match serde_json::from_str(ej) {
        Ok(ev) => ev,
        Err(e) => {
            set_last_error(format!("divi_bulwark_check_layer_transition: {e}"));
            return std::ptr::null_mut();
        }
    };

    let result = match check_transition(current, target, &requirements, &evidence) {
        Ok(()) => FfiLayerTransitionResult {
            allowed: true,
            blockers: Vec::new(),
        },
        Err(blockers) => FfiLayerTransitionResult {
            allowed: false,
            blockers,
        },
    };

    json_to_c(&result)
}

// ===================================================================
// Risk Score — JSON round-trip
// ===================================================================

/// Compute a fraud risk score from indicators.
///
/// `pubkey` is the subject's public key. `indicators_json` is a JSON array
/// of FraudIndicator.
/// Returns JSON (RiskScore). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bulwark_risk_score_compute(
    pubkey: *const c_char,
    indicators_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_bulwark_risk_score_compute: invalid pubkey");
        return std::ptr::null_mut();
    };

    let Some(ij) = c_str_to_str(indicators_json) else {
        set_last_error("divi_bulwark_risk_score_compute: invalid indicators_json");
        return std::ptr::null_mut();
    };

    let indicators: Vec<FraudIndicator> = match serde_json::from_str(ij) {
        Ok(i) => i,
        Err(e) => {
            set_last_error(format!("divi_bulwark_risk_score_compute: {e}"));
            return std::ptr::null_mut();
        }
    };

    let risk = RiskScore::compute(pk, indicators);
    json_to_c(&risk)
}
