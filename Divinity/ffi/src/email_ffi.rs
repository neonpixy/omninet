use std::ffi::c_char;
use std::os::raw::c_void;

use equipment::Email;
use uuid::Uuid;

use crate::helpers::{c_str_to_str, json_to_c};
use crate::set_last_error;

// ---------------------------------------------------------------------------
// Callback type
// ---------------------------------------------------------------------------

/// C function pointer for an Email subscriber.
///
/// - Receives event bytes (`data` + `len`).
/// - Returns nothing (fire-and-forget).
/// - `context` is the opaque pointer passed at subscription time.
pub type DiviEmailHandler =
    extern "C" fn(data: *const u8, len: usize, context: *mut c_void);

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Create a new Email hub. Returns an opaque pointer.
/// Free with `divi_email_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_email_new() -> *mut Email {
    Box::into_raw(Box::new(Email::new()))
}

/// Free an Email created by `divi_email_new`.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_email_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_free(ptr: *mut Email) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

/// Subscribe a raw handler for an email ID.
///
/// On success, writes the 16-byte subscriber UUID into `out_uuid` and returns 0.
/// On error, returns -1.
///
/// # Safety
/// `email` must be a valid pointer from `divi_email_new`.
/// `email_id` must be a valid null-terminated C string.
/// `out_uuid` must point to at least 16 writable bytes.
/// `context` must be valid for the lifetime of the subscription, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_subscribe_raw(
    email: *const Email,
    email_id: *const c_char,
    handler: DiviEmailHandler,
    context: *mut c_void,
    out_uuid: *mut u8,
) -> i32 {
    let email = unsafe { &*email };
    let Some(email_id) = c_str_to_str(email_id) else {
        set_last_error("divi_email_subscribe_raw: invalid email_id");
        return -1;
    };

    // Cast context to usize so the closure is Send + Sync.
    // Safety: the caller guarantees the context pointer is thread-safe.
    let ctx = context as usize;
    let cb = handler;

    let subscriber_id = email.subscribe_raw(email_id, move |data: &[u8]| {
        cb(data.as_ptr(), data.len(), ctx as *mut c_void);
    });

    // Write UUID bytes to out_uuid.
    if !out_uuid.is_null() {
        let bytes = subscriber_id.as_bytes();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_uuid, 16);
        }
    }

    0
}

// ---------------------------------------------------------------------------
// Sending
// ---------------------------------------------------------------------------

/// Send raw bytes to all subscribers of an email ID. Fire-and-forget.
///
/// # Safety
/// `email` must be a valid pointer from `divi_email_new`.
/// `email_id` must be a valid null-terminated C string.
/// `data` must point to `data_len` valid bytes (or be null if `data_len` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_send_raw(
    email: *const Email,
    email_id: *const c_char,
    data: *const u8,
    data_len: usize,
) {
    let email = unsafe { &*email };
    let Some(email_id) = c_str_to_str(email_id) else {
        return;
    };

    let input = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    email.send_raw(email_id, input);
}

// ---------------------------------------------------------------------------
// Unsubscription
// ---------------------------------------------------------------------------

/// Unsubscribe a specific subscriber by UUID (16 bytes).
///
/// # Safety
/// `email` must be a valid pointer. `uuid_bytes` must point to 16 valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_unsubscribe(
    email: *const Email,
    uuid_bytes: *const u8,
) {
    let email = unsafe { &*email };
    if uuid_bytes.is_null() {
        return;
    }

    let bytes: [u8; 16] = unsafe {
        let mut buf = [0u8; 16];
        std::ptr::copy_nonoverlapping(uuid_bytes, buf.as_mut_ptr(), 16);
        buf
    };

    let uuid = Uuid::from_bytes(bytes);
    email.unsubscribe(uuid);
}

/// Unsubscribe all subscribers for an email ID.
///
/// # Safety
/// `email` must be a valid pointer. `email_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_unsubscribe_all(
    email: *const Email,
    email_id: *const c_char,
) {
    let email = unsafe { &*email };
    let Some(email_id) = c_str_to_str(email_id) else {
        return;
    };
    email.unsubscribe_all(email_id);
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Check if any subscribers exist for an email ID.
///
/// # Safety
/// `email` must be a valid pointer. `email_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_has_subscribers(
    email: *const Email,
    email_id: *const c_char,
) -> bool {
    let email = unsafe { &*email };
    let Some(email_id) = c_str_to_str(email_id) else {
        return false;
    };
    email.has_subscribers(email_id)
}

/// Get all active email IDs as a JSON array string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `email` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_email_active_email_ids(email: *const Email) -> *mut c_char {
    let email = unsafe { &*email };
    let ids = email.active_email_ids();
    json_to_c(&ids)
}
