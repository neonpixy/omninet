use std::ffi::c_char;
use std::sync::Mutex;

use crown::CrownKeypair;
use device_manager::{
    DeviceFleet, PairingProtocol, SyncPriority, SyncState, SyncTracker,
};
use globe::discovery::pairing::{PairingChallenge, PairingResponse};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// Internal helper: reconstruct a CrownKeypair from a hex secret key.
// ---------------------------------------------------------------------------

/// Decode a hex-encoded 32-byte secret key into a `CrownKeypair`.
fn keypair_from_hex(hex_str: &str) -> Result<CrownKeypair, String> {
    let bytes = hex::decode(hex_str)
        .map_err(|e| format!("invalid hex secret key: {e}"))?;
    CrownKeypair::from_private_key(&bytes)
        .map_err(|e| format!("invalid private key: {e}"))
}

// ===================================================================
// PairingProtocol — stateless (3 functions)
// ===================================================================

/// Initiate a pairing challenge.
///
/// - `secret_key_hex` is the initiator's 32-byte private key as a 64-char hex string.
/// - `device_name` is a human-readable device name.
/// - `relay_url` is the relay URL for the pairing response.
///
/// Returns JSON (PairingChallenge). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_pairing_initiate(
    secret_key_hex: *const c_char,
    device_name: *const c_char,
    relay_url: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(sk_hex) = c_str_to_str(secret_key_hex) else {
        set_last_error("divi_device_pairing_initiate: invalid secret_key_hex");
        return std::ptr::null_mut();
    };
    let Some(name) = c_str_to_str(device_name) else {
        set_last_error("divi_device_pairing_initiate: invalid device_name");
        return std::ptr::null_mut();
    };
    let Some(url) = c_str_to_str(relay_url) else {
        set_last_error("divi_device_pairing_initiate: invalid relay_url");
        return std::ptr::null_mut();
    };

    let keypair = match keypair_from_hex(sk_hex) {
        Ok(kp) => kp,
        Err(e) => {
            set_last_error(format!("divi_device_pairing_initiate: {e}"));
            return std::ptr::null_mut();
        }
    };

    let challenge = PairingProtocol::initiate(&keypair, name, url);
    json_to_c(&challenge)
}

/// Respond to a pairing challenge by signing the nonce.
///
/// - `challenge_json` is a JSON `PairingChallenge`.
/// - `secret_key_hex` is the responder's 32-byte private key as a 64-char hex string.
/// - `device_name` is the responder's human-readable device name.
///
/// Returns JSON (PairingResponse). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_pairing_respond(
    challenge_json: *const c_char,
    secret_key_hex: *const c_char,
    device_name: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(challenge_json) else {
        set_last_error("divi_device_pairing_respond: invalid challenge_json");
        return std::ptr::null_mut();
    };
    let Some(sk_hex) = c_str_to_str(secret_key_hex) else {
        set_last_error("divi_device_pairing_respond: invalid secret_key_hex");
        return std::ptr::null_mut();
    };
    let Some(name) = c_str_to_str(device_name) else {
        set_last_error("divi_device_pairing_respond: invalid device_name");
        return std::ptr::null_mut();
    };

    let challenge: PairingChallenge = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_device_pairing_respond: challenge: {e}"));
            return std::ptr::null_mut();
        }
    };

    let keypair = match keypair_from_hex(sk_hex) {
        Ok(kp) => kp,
        Err(e) => {
            set_last_error(format!("divi_device_pairing_respond: {e}"));
            return std::ptr::null_mut();
        }
    };

    match PairingProtocol::respond(&challenge, &keypair, name) {
        Ok(response) => json_to_c(&response),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Verify a pairing response and produce a DevicePair.
///
/// - `challenge_json` is a JSON `PairingChallenge`.
/// - `response_json` is a JSON `PairingResponse`.
///
/// Returns JSON (DevicePair). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// Both C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_pairing_verify(
    challenge_json: *const c_char,
    response_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(challenge_json) else {
        set_last_error("divi_device_pairing_verify: invalid challenge_json");
        return std::ptr::null_mut();
    };
    let Some(rj) = c_str_to_str(response_json) else {
        set_last_error("divi_device_pairing_verify: invalid response_json");
        return std::ptr::null_mut();
    };

    let challenge: PairingChallenge = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_device_pairing_verify: challenge: {e}"));
            return std::ptr::null_mut();
        }
    };

    let response: PairingResponse = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_device_pairing_verify: response: {e}"));
            return std::ptr::null_mut();
        }
    };

    match PairingProtocol::verify(&challenge, &response) {
        Ok(pair) => json_to_c(&pair),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// DeviceFleet — opaque pointer (device registry)
// ===================================================================

pub struct DiviDeviceFleet(pub(crate) Mutex<DeviceFleet>);

/// Create a new empty device fleet.
///
/// Free with `divi_device_fleet_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_device_fleet_new() -> *mut DiviDeviceFleet {
    Box::into_raw(Box::new(DiviDeviceFleet(Mutex::new(DeviceFleet::new()))))
}

/// Free a device fleet.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_free(ptr: *mut DiviDeviceFleet) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Add a device to the fleet.
///
/// `entry_json` is a JSON `FleetEntry`.
/// Returns 0 on success, -1 on error (e.g. already paired).
///
/// # Safety
/// `fleet` must be a valid pointer. `entry_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_add(
    fleet: *const DiviDeviceFleet,
    entry_json: *const c_char,
) -> i32 {
    clear_last_error();

    let fleet = unsafe { &*fleet };
    let Some(ej) = c_str_to_str(entry_json) else {
        set_last_error("divi_device_fleet_add: invalid entry_json");
        return -1;
    };

    let entry: device_manager::FleetEntry = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_device_fleet_add: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&fleet.0);
    match guard.add(entry) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Remove a device from the fleet.
///
/// Returns JSON (FleetEntry) of the removed device, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `fleet` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_remove(
    fleet: *const DiviDeviceFleet,
    crown_id: *const c_char,
) -> *mut c_char {
    let fleet = unsafe { &*fleet };
    let Some(n) = c_str_to_str(crown_id) else {
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&fleet.0);
    match guard.remove(n) {
        Some(entry) => json_to_c(&entry),
        None => std::ptr::null_mut(),
    }
}

/// Get a device by crown_id.
///
/// Returns JSON (FleetEntry) or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `fleet` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_get(
    fleet: *const DiviDeviceFleet,
    crown_id: *const c_char,
) -> *mut c_char {
    let fleet = unsafe { &*fleet };
    let Some(n) = c_str_to_str(crown_id) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&fleet.0);
    match guard.get(n) {
        Some(entry) => json_to_c(entry),
        None => std::ptr::null_mut(),
    }
}

/// List all devices in the fleet.
///
/// Returns JSON array of FleetEntry. Caller must free via `divi_free_string`.
///
/// # Safety
/// `fleet` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_list(
    fleet: *const DiviDeviceFleet,
) -> *mut c_char {
    let fleet = unsafe { &*fleet };
    let guard = lock_or_recover(&fleet.0);
    let entries: Vec<&device_manager::FleetEntry> = guard.list();
    json_to_c(&entries)
}

/// Update the status of a device.
///
/// `status_json` is a JSON `DeviceStatus`.
/// Returns 0 on success, -1 on error (device not found).
///
/// # Safety
/// `fleet` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_update_status(
    fleet: *const DiviDeviceFleet,
    crown_id: *const c_char,
    status_json: *const c_char,
) -> i32 {
    clear_last_error();

    let fleet = unsafe { &*fleet };
    let Some(n) = c_str_to_str(crown_id) else {
        set_last_error("divi_device_fleet_update_status: invalid crown_id");
        return -1;
    };
    let Some(sj) = c_str_to_str(status_json) else {
        set_last_error("divi_device_fleet_update_status: invalid status_json");
        return -1;
    };

    let status: device_manager::DeviceStatus = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_device_fleet_update_status: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&fleet.0);
    match guard.update_status(n, status) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get aggregate fleet health.
///
/// Returns JSON (FleetHealth). Caller must free via `divi_free_string`.
///
/// # Safety
/// `fleet` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_health(
    fleet: *const DiviDeviceFleet,
) -> *mut c_char {
    let fleet = unsafe { &*fleet };
    let guard = lock_or_recover(&fleet.0);
    json_to_c(&guard.health())
}

/// Get the number of devices in the fleet.
///
/// # Safety
/// `fleet` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_count(
    fleet: *const DiviDeviceFleet,
) -> usize {
    let fleet = unsafe { &*fleet };
    let guard = lock_or_recover(&fleet.0);
    guard.count()
}

/// Whether the fleet has no devices.
///
/// # Safety
/// `fleet` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_is_empty(
    fleet: *const DiviDeviceFleet,
) -> bool {
    let fleet = unsafe { &*fleet };
    let guard = lock_or_recover(&fleet.0);
    guard.is_empty()
}

/// Serialize the fleet to JSON.
///
/// Returns JSON (DeviceFleet). Caller must free via `divi_free_string`.
///
/// # Safety
/// `fleet` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_to_json(
    fleet: *const DiviDeviceFleet,
) -> *mut c_char {
    let fleet = unsafe { &*fleet };
    let guard = lock_or_recover(&fleet.0);
    json_to_c(&*guard)
}

/// Deserialize a fleet from JSON.
///
/// `json` is a JSON `DeviceFleet`.
/// Returns a new fleet pointer. Free with `divi_device_fleet_free`.
/// Returns null on error.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_fleet_from_json(
    json: *const c_char,
) -> *mut DiviDeviceFleet {
    clear_last_error();

    let Some(j) = c_str_to_str(json) else {
        set_last_error("divi_device_fleet_from_json: invalid json");
        return std::ptr::null_mut();
    };

    let fleet: DeviceFleet = match serde_json::from_str(j) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_device_fleet_from_json: {e}"));
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(DiviDeviceFleet(Mutex::new(fleet))))
}

// ===================================================================
// SyncPriority — opaque pointer (data type -> home device)
// ===================================================================

pub struct DiviSyncPriority(pub(crate) Mutex<SyncPriority>);

/// Create a new empty sync priority map.
///
/// Free with `divi_device_sync_priority_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_device_sync_priority_new() -> *mut DiviSyncPriority {
    Box::into_raw(Box::new(DiviSyncPriority(Mutex::new(SyncPriority::new()))))
}

/// Free a sync priority map.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_free(ptr: *mut DiviSyncPriority) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Set which device is home for a data type.
///
/// # Safety
/// `priority` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_set_home(
    priority: *const DiviSyncPriority,
    data_type: *const c_char,
    device_crown_id: *const c_char,
) {
    let priority = unsafe { &*priority };
    let Some(dt) = c_str_to_str(data_type) else {
        return;
    };
    let Some(crown_id) = c_str_to_str(device_crown_id) else {
        return;
    };

    let mut guard = lock_or_recover(&priority.0);
    guard.set_home(dt, crown_id);
}

/// Get the home device for a data type.
///
/// Returns a C string or null if no home is set.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `priority` must be a valid pointer. `data_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_home_for(
    priority: *const DiviSyncPriority,
    data_type: *const c_char,
) -> *mut c_char {
    let priority = unsafe { &*priority };
    let Some(dt) = c_str_to_str(data_type) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&priority.0);
    match guard.home_for(dt) {
        Some(crown_id) => string_to_c(crown_id.to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Remove the home assignment for a data type.
///
/// # Safety
/// `priority` must be a valid pointer. `data_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_remove(
    priority: *const DiviSyncPriority,
    data_type: *const c_char,
) {
    let priority = unsafe { &*priority };
    let Some(dt) = c_str_to_str(data_type) else {
        return;
    };

    let mut guard = lock_or_recover(&priority.0);
    guard.remove(dt);
}

/// Get all home assignments.
///
/// Returns JSON object mapping data_type -> device_crown_id.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `priority` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_all(
    priority: *const DiviSyncPriority,
) -> *mut c_char {
    let priority = unsafe { &*priority };
    let guard = lock_or_recover(&priority.0);
    json_to_c(guard.all())
}

/// Serialize the sync priority map to JSON.
///
/// Returns JSON (SyncPriority). Caller must free via `divi_free_string`.
///
/// # Safety
/// `priority` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_to_json(
    priority: *const DiviSyncPriority,
) -> *mut c_char {
    let priority = unsafe { &*priority };
    let guard = lock_or_recover(&priority.0);
    json_to_c(&*guard)
}

/// Deserialize a sync priority map from JSON.
///
/// `json` is a JSON `SyncPriority`.
/// Returns a new pointer. Free with `divi_device_sync_priority_free`.
/// Returns null on error.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_priority_from_json(
    json: *const c_char,
) -> *mut DiviSyncPriority {
    clear_last_error();

    let Some(j) = c_str_to_str(json) else {
        set_last_error("divi_device_sync_priority_from_json: invalid json");
        return std::ptr::null_mut();
    };

    let priority: SyncPriority = match serde_json::from_str(j) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_device_sync_priority_from_json: {e}"));
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(DiviSyncPriority(Mutex::new(priority))))
}

// ===================================================================
// SyncTracker — opaque pointer (device -> data_type -> SyncState)
// ===================================================================

pub struct DiviSyncTracker(pub(crate) Mutex<SyncTracker>);

/// Create a new empty sync tracker.
///
/// Free with `divi_device_sync_tracker_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_device_sync_tracker_new() -> *mut DiviSyncTracker {
    Box::into_raw(Box::new(DiviSyncTracker(Mutex::new(SyncTracker::new()))))
}

/// Free a sync tracker.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_free(ptr: *mut DiviSyncTracker) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Set the sync state for a device + data type pair.
///
/// `state_json` is a JSON `SyncState`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `tracker` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_set_state(
    tracker: *const DiviSyncTracker,
    crown_id: *const c_char,
    data_type: *const c_char,
    state_json: *const c_char,
) -> i32 {
    clear_last_error();

    let tracker = unsafe { &*tracker };
    let Some(n) = c_str_to_str(crown_id) else {
        set_last_error("divi_device_sync_tracker_set_state: invalid crown_id");
        return -1;
    };
    let Some(dt) = c_str_to_str(data_type) else {
        set_last_error("divi_device_sync_tracker_set_state: invalid data_type");
        return -1;
    };
    let Some(sj) = c_str_to_str(state_json) else {
        set_last_error("divi_device_sync_tracker_set_state: invalid state_json");
        return -1;
    };

    let state: SyncState = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_device_sync_tracker_set_state: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&tracker.0);
    guard.set_state(n, dt, state);
    0
}

/// Get the sync state for a device + data type pair.
///
/// Returns JSON (SyncState). Returns "Unknown" if not tracked.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_get_state(
    tracker: *const DiviSyncTracker,
    crown_id: *const c_char,
    data_type: *const c_char,
) -> *mut c_char {
    let tracker = unsafe { &*tracker };
    let Some(n) = c_str_to_str(crown_id) else {
        return std::ptr::null_mut();
    };
    let Some(dt) = c_str_to_str(data_type) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&tracker.0);
    let state = guard.get_state(n, dt);
    json_to_c(state)
}

/// Get all sync states for a specific device.
///
/// Returns JSON object mapping data_type -> SyncState, or null if device not tracked.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_states_for_device(
    tracker: *const DiviSyncTracker,
    crown_id: *const c_char,
) -> *mut c_char {
    let tracker = unsafe { &*tracker };
    let Some(n) = c_str_to_str(crown_id) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&tracker.0);
    match guard.states_for_device(n) {
        Some(states) => json_to_c(states),
        None => std::ptr::null_mut(),
    }
}

/// Check if all tracked states are Synced.
///
/// Returns true if all synced (or empty).
///
/// # Safety
/// `tracker` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_all_synced(
    tracker: *const DiviSyncTracker,
) -> bool {
    let tracker = unsafe { &*tracker };
    let guard = lock_or_recover(&tracker.0);
    guard.all_synced()
}

/// Get all conflict states.
///
/// Returns JSON array of objects with `device_crown_id`, `data_type`, and `state` fields.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_conflicts(
    tracker: *const DiviSyncTracker,
) -> *mut c_char {
    let tracker = unsafe { &*tracker };
    let guard = lock_or_recover(&tracker.0);
    let conflicts = guard.conflicts();

    // Serialize as an array of {device_crown_id, data_type, state} objects.
    let serializable: Vec<serde_json::Value> = conflicts
        .into_iter()
        .map(|(crown_id, dt, state)| {
            serde_json::json!({
                "device_crown_id": crown_id,
                "data_type": dt,
                "state": state,
            })
        })
        .collect();

    json_to_c(&serializable)
}

/// Serialize the sync tracker to JSON.
///
/// Returns JSON (SyncTracker). Caller must free via `divi_free_string`.
///
/// # Safety
/// `tracker` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_to_json(
    tracker: *const DiviSyncTracker,
) -> *mut c_char {
    let tracker = unsafe { &*tracker };
    let guard = lock_or_recover(&tracker.0);
    json_to_c(&*guard)
}

/// Deserialize a sync tracker from JSON.
///
/// `json` is a JSON `SyncTracker`.
/// Returns a new pointer. Free with `divi_device_sync_tracker_free`.
/// Returns null on error.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_device_sync_tracker_from_json(
    json: *const c_char,
) -> *mut DiviSyncTracker {
    clear_last_error();

    let Some(j) = c_str_to_str(json) else {
        set_last_error("divi_device_sync_tracker_from_json: invalid json");
        return std::ptr::null_mut();
    };

    let tracker: SyncTracker = match serde_json::from_str(j) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_device_sync_tracker_from_json: {e}"));
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(DiviSyncTracker(Mutex::new(tracker))))
}
