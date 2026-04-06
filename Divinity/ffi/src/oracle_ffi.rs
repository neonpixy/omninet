use std::ffi::c_char;
use std::sync::Mutex;

use oracle::disclosure::{
    DisclosureSignal, DisclosureTracker, FeatureVisibility, SovereigntyTier, TierDefaults,
};
use oracle::hints::StaticHint;
use oracle::{
    Condition, HintContext, Trigger, Workflow, WorkflowEvent, WorkflowMatch, WorkflowRegistry,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// DisclosureTracker — JSON round-trip (fully serializable)
// ===================================================================

/// Create a new `DisclosureTracker` at Citizen tier.
///
/// Returns JSON (DisclosureTracker). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_oracle_disclosure_tracker_new() -> *mut c_char {
    let tracker = DisclosureTracker::new();
    json_to_c(&tracker)
}

/// Create a `DisclosureTracker` with custom config.
///
/// `config_json` is a JSON `DisclosureConfig` (e.g. `{"enthusiast_threshold":5,"operator_threshold":3}`).
/// Returns JSON (DisclosureTracker). Caller must free via `divi_free_string`.
///
/// # Safety
/// `config_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_disclosure_tracker_with_config(
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(config_json) else {
        set_last_error("divi_oracle_disclosure_tracker_with_config: invalid config_json");
        return std::ptr::null_mut();
    };

    let config = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!(
                "divi_oracle_disclosure_tracker_with_config: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let tracker = DisclosureTracker::with_config(config);
    json_to_c(&tracker)
}

/// Record a disclosure signal on a tracker.
///
/// Takes tracker JSON and signal JSON, returns modified tracker JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_disclosure_tracker_record(
    tracker_json: *const c_char,
    signal_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tracker_json) else {
        set_last_error("divi_oracle_disclosure_tracker_record: invalid tracker_json");
        return std::ptr::null_mut();
    };

    let Some(sj) = c_str_to_str(signal_json) else {
        set_last_error("divi_oracle_disclosure_tracker_record: invalid signal_json");
        return std::ptr::null_mut();
    };

    let mut tracker: DisclosureTracker = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_oracle_disclosure_tracker_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    let signal: DisclosureSignal = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_oracle_disclosure_tracker_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    tracker.record(&signal);
    json_to_c(&tracker)
}

/// Get the current sovereignty tier from a tracker.
///
/// Returns JSON (SovereigntyTier). Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_disclosure_tracker_level(
    tracker_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tracker_json) else {
        set_last_error("divi_oracle_disclosure_tracker_level: invalid tracker_json");
        return std::ptr::null_mut();
    };

    let tracker: DisclosureTracker = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_oracle_disclosure_tracker_level: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&tracker.level())
}

/// Manually set the sovereignty tier on a tracker.
///
/// `level_json` is a JSON `SovereigntyTier` (e.g. `"Architect"`).
/// Returns modified tracker JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_disclosure_tracker_set_level(
    tracker_json: *const c_char,
    level_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tracker_json) else {
        set_last_error("divi_oracle_disclosure_tracker_set_level: invalid tracker_json");
        return std::ptr::null_mut();
    };

    let Some(lj) = c_str_to_str(level_json) else {
        set_last_error("divi_oracle_disclosure_tracker_set_level: invalid level_json");
        return std::ptr::null_mut();
    };

    let mut tracker: DisclosureTracker = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!(
                "divi_oracle_disclosure_tracker_set_level: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let level = match serde_json::from_str(lj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!(
                "divi_oracle_disclosure_tracker_set_level: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    tracker.set_level(level);
    json_to_c(&tracker)
}

/// Clear the manual override on a tracker (return to behavior-driven level).
///
/// Returns modified tracker JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_disclosure_tracker_clear_override(
    tracker_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tracker_json) else {
        set_last_error("divi_oracle_disclosure_tracker_clear_override: invalid tracker_json");
        return std::ptr::null_mut();
    };

    let mut tracker: DisclosureTracker = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!(
                "divi_oracle_disclosure_tracker_clear_override: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    tracker.clear_override();
    json_to_c(&tracker)
}

/// Get signal counts from a tracker.
///
/// Returns JSON `{"steward":<n>,"architect":<n>}`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_disclosure_tracker_signal_counts(
    tracker_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tracker_json) else {
        set_last_error("divi_oracle_disclosure_tracker_signal_counts: invalid tracker_json");
        return std::ptr::null_mut();
    };

    let tracker: DisclosureTracker = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!(
                "divi_oracle_disclosure_tracker_signal_counts: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let (steward, architect) = tracker.signal_counts();
    let counts = serde_json::json!({
        "steward": steward,
        "architect": architect,
    });
    json_to_c(&counts)
}

/// Get the default settings for a sovereignty tier.
///
/// `tier_json` is a JSON `SovereigntyTier` (e.g. `"Citizen"`).
/// Returns JSON (TierDefaults). Caller must free via `divi_free_string`.
///
/// # Safety
/// `tier_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_tier_defaults(
    tier_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tier_json) else {
        set_last_error("divi_oracle_tier_defaults: invalid tier_json");
        return std::ptr::null_mut();
    };

    let tier: SovereigntyTier = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_oracle_tier_defaults: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&TierDefaults::for_tier(tier))
}

/// Get all tier defaults (one per sovereignty tier).
///
/// Returns JSON array of TierDefaults. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_oracle_tier_defaults_all() -> *mut c_char {
    json_to_c(&TierDefaults::all())
}

/// Get the feature visibility for a sovereignty tier.
///
/// `tier_json` is a JSON `SovereigntyTier` (e.g. `"Steward"`).
/// Returns JSON (FeatureVisibility). Caller must free via `divi_free_string`.
///
/// # Safety
/// `tier_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_feature_visibility(
    tier_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tier_json) else {
        set_last_error("divi_oracle_feature_visibility: invalid tier_json");
        return std::ptr::null_mut();
    };

    let tier: SovereigntyTier = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_oracle_feature_visibility: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&FeatureVisibility::for_tier(tier))
}

// ===================================================================
// Workflow helpers — JSON round-trip (stateless)
// ===================================================================

/// Create and validate a workflow from JSON.
///
/// `workflow_json` is a JSON `Workflow`. This function deserializes and
/// re-serializes to normalize the data (e.g., default fields).
/// Returns JSON (Workflow). Caller must free via `divi_free_string`.
///
/// # Safety
/// `workflow_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_workflow_new(
    workflow_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(wj) = c_str_to_str(workflow_json) else {
        set_last_error("divi_oracle_workflow_new: invalid workflow_json");
        return std::ptr::null_mut();
    };

    let workflow: Workflow = match serde_json::from_str(wj) {
        Ok(w) => w,
        Err(e) => {
            set_last_error(format!("divi_oracle_workflow_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&workflow)
}

/// Check if a trigger matches an event.
///
/// Returns 1 if the trigger matches the event, 0 if not, -1 on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_trigger_matches(
    trigger_json: *const c_char,
    event_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(tj) = c_str_to_str(trigger_json) else {
        set_last_error("divi_oracle_trigger_matches: invalid trigger_json");
        return -1;
    };

    let Some(ej) = c_str_to_str(event_json) else {
        set_last_error("divi_oracle_trigger_matches: invalid event_json");
        return -1;
    };

    let trigger: Trigger = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_oracle_trigger_matches: {e}"));
            return -1;
        }
    };

    let event: WorkflowEvent = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_oracle_trigger_matches: {e}"));
            return -1;
        }
    };

    i32::from(trigger.matches(&event))
}

/// Evaluate a condition against a set of fields.
///
/// `condition_json` is a JSON `Condition`.
/// `fields_json` is a JSON `HashMap<String, String>`.
/// Returns 1 if the condition evaluates to true, 0 if false, -1 on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_condition_evaluate(
    condition_json: *const c_char,
    fields_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(cj) = c_str_to_str(condition_json) else {
        set_last_error("divi_oracle_condition_evaluate: invalid condition_json");
        return -1;
    };

    let Some(fj) = c_str_to_str(fields_json) else {
        set_last_error("divi_oracle_condition_evaluate: invalid fields_json");
        return -1;
    };

    let condition: Condition = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_oracle_condition_evaluate: {e}"));
            return -1;
        }
    };

    let fields: std::collections::HashMap<String, String> = match serde_json::from_str(fj) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_oracle_condition_evaluate: {e}"));
            return -1;
        }
    };

    i32::from(condition.evaluate(&fields))
}

/// Check if a workflow should fire for the given event.
///
/// Returns 1 if the workflow should fire, 0 if not, -1 on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_workflow_should_fire(
    workflow_json: *const c_char,
    event_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(wj) = c_str_to_str(workflow_json) else {
        set_last_error("divi_oracle_workflow_should_fire: invalid workflow_json");
        return -1;
    };

    let Some(ej) = c_str_to_str(event_json) else {
        set_last_error("divi_oracle_workflow_should_fire: invalid event_json");
        return -1;
    };

    let workflow: Workflow = match serde_json::from_str(wj) {
        Ok(w) => w,
        Err(e) => {
            set_last_error(format!("divi_oracle_workflow_should_fire: {e}"));
            return -1;
        }
    };

    let event: WorkflowEvent = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_oracle_workflow_should_fire: {e}"));
            return -1;
        }
    };

    i32::from(workflow.should_fire(&event))
}

// ===================================================================
// StaticHint + HintContext — JSON round-trip
// ===================================================================

/// Create and validate a `StaticHint` from JSON.
///
/// Deserializes and re-serializes to normalize. Returns JSON (StaticHint).
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `hint_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_static_hint_new(
    hint_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(hj) = c_str_to_str(hint_json) else {
        set_last_error("divi_oracle_static_hint_new: invalid hint_json");
        return std::ptr::null_mut();
    };

    let hint: StaticHint = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_oracle_static_hint_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&hint)
}

/// Check if a static hint should be shown given a context.
///
/// Returns 1 if the hint should be shown, 0 if not, -1 on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_static_hint_should_show(
    hint_json: *const c_char,
    context_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(hj) = c_str_to_str(hint_json) else {
        set_last_error("divi_oracle_static_hint_should_show: invalid hint_json");
        return -1;
    };

    let Some(cj) = c_str_to_str(context_json) else {
        set_last_error("divi_oracle_static_hint_should_show: invalid context_json");
        return -1;
    };

    let hint: StaticHint = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_oracle_static_hint_should_show: {e}"));
            return -1;
        }
    };

    let context: HintContext = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_oracle_static_hint_should_show: {e}"));
            return -1;
        }
    };

    // Use OracleHint trait method via StaticHint's implementation.
    use oracle::OracleHint;
    i32::from(hint.should_show(&context))
}

/// Create an empty `HintContext`.
///
/// Returns JSON (HintContext). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_oracle_hint_context_new() -> *mut c_char {
    let ctx = HintContext::new();
    json_to_c(&ctx)
}

// ===================================================================
// WorkflowRegistry — opaque pointer (stateful container)
// ===================================================================

/// Opaque handle wrapping `oracle::WorkflowRegistry` behind a `Mutex`.
pub struct OracleWorkflowRegistry(pub(crate) Mutex<WorkflowRegistry>);

/// Create a new empty `WorkflowRegistry`.
///
/// Free with `divi_oracle_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_oracle_registry_new() -> *mut OracleWorkflowRegistry {
    Box::into_raw(Box::new(OracleWorkflowRegistry(Mutex::new(
        WorkflowRegistry::new(),
    ))))
}

/// Free a `WorkflowRegistry`.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_free(ptr: *mut OracleWorkflowRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register a workflow in the registry.
///
/// `workflow_json` is a JSON `Workflow`. Replaces any existing workflow with the same ID.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `workflow_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_register(
    registry: *const OracleWorkflowRegistry,
    workflow_json: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(wj) = c_str_to_str(workflow_json) else {
        set_last_error("divi_oracle_registry_register: invalid workflow_json");
        return -1;
    };

    let workflow: Workflow = match serde_json::from_str(wj) {
        Ok(w) => w,
        Err(e) => {
            set_last_error(format!("divi_oracle_registry_register: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    guard.register(workflow);
    0
}

/// Unregister a workflow by ID.
///
/// Returns JSON (Workflow) of the removed workflow, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_unregister(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_oracle_registry_unregister: invalid id");
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.unregister(id_str) {
        Some(wf) => json_to_c(&wf),
        None => {
            set_last_error(format!(
                "divi_oracle_registry_unregister: workflow '{id_str}' not found"
            ));
            std::ptr::null_mut()
        }
    }
}

/// Get a workflow by ID.
///
/// Returns JSON (Workflow) or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_get(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(id_str) {
        Some(wf) => json_to_c(wf),
        None => std::ptr::null_mut(),
    }
}

/// Enable a workflow by ID.
///
/// Returns 0 on success, -1 on error (workflow not found).
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_enable(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_oracle_registry_enable: invalid id");
        return -1;
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.enable(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Disable a workflow by ID.
///
/// Returns 0 on success, -1 on error (workflow not found).
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_disable(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_oracle_registry_disable: invalid id");
        return -1;
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.disable(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Grant consent for a workflow by ID.
///
/// Returns 0 on success, -1 on error (workflow not found).
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_grant_consent(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_oracle_registry_grant_consent: invalid id");
        return -1;
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.grant_consent(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Revoke consent for a workflow by ID.
///
/// Returns 0 on success, -1 on error (workflow not found).
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_revoke_consent(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_oracle_registry_revoke_consent: invalid id");
        return -1;
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.revoke_consent(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// List all workflows in the registry.
///
/// Returns JSON array of Workflows. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_list(
    registry: *const OracleWorkflowRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    let workflows: Vec<&Workflow> = guard.list();
    json_to_c(&workflows)
}

/// List workflows for a specific actor.
///
/// Returns JSON array of Workflows. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `actor` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_list_for_actor(
    registry: *const OracleWorkflowRegistry,
    actor: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };

    let Some(actor_str) = c_str_to_str(actor) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    let workflows: Vec<&Workflow> = guard.list_for_actor(actor_str);
    json_to_c(&workflows)
}

/// Evaluate an event against all workflows in the registry.
///
/// `event_json` is a JSON `WorkflowEvent`. `timestamp` is a Unix timestamp (seconds).
/// Returns JSON array of `WorkflowMatch`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_evaluate(
    registry: *const OracleWorkflowRegistry,
    event_json: *const c_char,
    timestamp: u64,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(ej) = c_str_to_str(event_json) else {
        set_last_error("divi_oracle_registry_evaluate: invalid event_json");
        return std::ptr::null_mut();
    };

    let event: WorkflowEvent = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_oracle_registry_evaluate: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    let matches: Vec<WorkflowMatch> = guard.evaluate(&event, timestamp);
    json_to_c(&matches)
}

/// Get the count of registered workflows.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_count(
    registry: *const OracleWorkflowRegistry,
) -> usize {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.count()
}

/// Export all workflows as JSON for persistence.
///
/// Returns JSON array of Workflows. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_export(
    registry: *const OracleWorkflowRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    let workflows: Vec<Workflow> = guard.export_workflows();
    json_to_c(&workflows)
}

/// Import workflows from JSON (e.g., from persistence).
///
/// `workflows_json` is a JSON array of Workflows. Replaces existing workflows
/// with the same IDs.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `workflows_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_import(
    registry: *const OracleWorkflowRegistry,
    workflows_json: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };

    let Some(wj) = c_str_to_str(workflows_json) else {
        set_last_error("divi_oracle_registry_import: invalid workflows_json");
        return -1;
    };

    let workflows: Vec<Workflow> = match serde_json::from_str(wj) {
        Ok(w) => w,
        Err(e) => {
            set_last_error(format!("divi_oracle_registry_import: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    guard.import_workflows(workflows);
    0
}

/// Get the full audit log.
///
/// Returns JSON array of `AuditEntry`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_audit_log(
    registry: *const OracleWorkflowRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.audit_log())
}

/// Get audit entries for a specific workflow.
///
/// Returns JSON array of `AuditEntry`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_audit_for_workflow(
    registry: *const OracleWorkflowRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };

    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    let entries: Vec<_> = guard.audit_for_workflow(id_str);
    json_to_c(&entries)
}

/// Clear the audit log.
///
/// Returns 0 on success.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_oracle_registry_clear_audit(
    registry: *const OracleWorkflowRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let mut guard = lock_or_recover(&registry.0);
    guard.clear_audit_log();
    0
}
