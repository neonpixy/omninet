use std::ffi::c_char;

use equipment::notification::{Notification, NotificationPriority, PagerState};
use equipment::Pager;
use uuid::Uuid;

use crate::helpers::{c_str_to_str, json_to_c, string_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Create a new Pager. Returns an opaque pointer.
/// Free with `divi_pager_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_pager_new() -> *mut Pager {
    Box::into_raw(Box::new(Pager::new()))
}

/// Free a Pager created by `divi_pager_new`.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_pager_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_free(ptr: *mut Pager) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Notification
// ---------------------------------------------------------------------------

/// Push a notification from a JSON string.
///
/// The JSON must deserialize to a `Notification`. Returns the UUID of the
/// notification as a string (`*mut c_char`), or null on error.
///
/// The returned string must be freed via `divi_free_string`.
///
/// # Safety
/// `pager` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_notify(
    pager: *const Pager,
    json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let pager = unsafe { &*pager };
    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_pager_notify: invalid json string");
        return std::ptr::null_mut();
    };

    let notification: Notification = match serde_json::from_str(json_str) {
        Ok(n) => n,
        Err(e) => {
            set_last_error(format!("divi_pager_notify: JSON parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let id = pager.notify(notification);
    string_to_c(id.to_string())
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Get pending notifications as a JSON array.
///
/// If `priority_json` is non-null, it should be a JSON string like `"high"` or `"urgent"`
/// to filter by priority. Pass null for all priorities.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pager` must be a valid pointer. `priority_json` can be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_get_pending(
    pager: *const Pager,
    priority_json: *const c_char,
) -> *mut c_char {
    let pager = unsafe { &*pager };

    let priority: Option<NotificationPriority> = if let Some(json_str) = c_str_to_str(priority_json)
    {
        serde_json::from_str(json_str).ok()
    } else {
        None
    };

    let notifications = pager.get_pending(priority);
    json_to_c(&notifications)
}

/// Get unread notifications as a JSON array.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pager` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_get_unread(pager: *const Pager) -> *mut c_char {
    let pager = unsafe { &*pager };
    let notifications = pager.get_unread();
    json_to_c(&notifications)
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

/// Mark a notification as read. Returns true if found, false otherwise.
///
/// # Safety
/// `pager` must be a valid pointer. `uuid_str` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_mark_read(
    pager: *const Pager,
    uuid_str: *const c_char,
) -> bool {
    let pager = unsafe { &*pager };
    let Some(uuid_str) = c_str_to_str(uuid_str) else {
        return false;
    };

    let Ok(uuid) = Uuid::parse_str(uuid_str) else {
        return false;
    };

    pager.mark_read(uuid)
}

/// Dismiss a notification. Returns true if found, false otherwise.
///
/// # Safety
/// `pager` must be a valid pointer. `uuid_str` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_dismiss(
    pager: *const Pager,
    uuid_str: *const c_char,
) -> bool {
    let pager = unsafe { &*pager };
    let Some(uuid_str) = c_str_to_str(uuid_str) else {
        return false;
    };

    let Ok(uuid) = Uuid::parse_str(uuid_str) else {
        return false;
    };

    pager.dismiss(uuid)
}

/// Get the count of unread notifications.
///
/// # Safety
/// `pager` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_badge_count(pager: *const Pager) -> usize {
    let pager = unsafe { &*pager };
    pager.badge_count()
}

/// Prune expired notifications.
///
/// # Safety
/// `pager` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_prune_expired(pager: *const Pager) {
    let pager = unsafe { &*pager };
    pager.prune_expired();
}

// ---------------------------------------------------------------------------
// State persistence
// ---------------------------------------------------------------------------

/// Export pager state as a JSON array of notifications.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pager` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_export_state(pager: *const Pager) -> *mut c_char {
    let pager = unsafe { &*pager };
    let state = pager.export_state();
    json_to_c(&state)
}

/// Restore pager state from a JSON array of notifications.
///
/// # Safety
/// `pager` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_pager_restore_state(
    pager: *const Pager,
    json: *const c_char,
) {
    let pager = unsafe { &*pager };
    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_pager_restore_state: invalid json string");
        return;
    };

    let notifications: Vec<Notification> = match serde_json::from_str(json_str) {
        Ok(n) => n,
        Err(e) => {
            set_last_error(format!("divi_pager_restore_state: JSON parse error: {e}"));
            return;
        }
    };

    pager.restore_state(PagerState { notifications });
}
