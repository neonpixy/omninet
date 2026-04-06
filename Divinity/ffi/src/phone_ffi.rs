use std::ffi::c_char;
use std::os::raw::c_void;

use equipment::Phone;
use equipment::error::PhoneError;

use crate::helpers::{bytes_to_owned, c_str_to_str, json_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// Callback type
// ---------------------------------------------------------------------------

/// C function pointer for a Phone handler.
///
/// - Receives request bytes (`request_data` + `request_len`).
/// - Must write response bytes into `*response_data` and `*response_len`.
///   The response buffer should be allocated with `malloc` (Swift/C side).
/// - Returns 0 on success, -1 on error.
/// - `context` is the opaque pointer passed at registration time.
pub type DiviPhoneHandler = extern "C" fn(
    request_data: *const u8,
    request_len: usize,
    response_data: *mut *mut u8,
    response_len: *mut usize,
    context: *mut c_void,
) -> i32;

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Create a new Phone. Returns an opaque pointer.
/// Free with `divi_phone_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_phone_new() -> *mut Phone {
    Box::into_raw(Box::new(Phone::new()))
}

/// Free a Phone created by `divi_phone_new`.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_phone_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_free(ptr: *mut Phone) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register a raw handler for a call ID.
///
/// The `handler` function will be called when `divi_phone_call_raw` is invoked
/// with the matching `call_id`. The handler receives request bytes and must
/// write response bytes into the out-parameters.
///
/// The `context` pointer is passed through to every handler invocation.
/// Pass null if no context is needed.
///
/// # Safety
/// `phone` must be a valid pointer from `divi_phone_new`.
/// `call_id` must be a valid null-terminated C string.
/// `context` must be valid for the lifetime of the registration, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_register_raw(
    phone: *const Phone,
    call_id: *const c_char,
    handler: DiviPhoneHandler,
    context: *mut c_void,
) {
    let phone = unsafe { &*phone };
    let Some(call_id) = c_str_to_str(call_id) else {
        set_last_error("divi_phone_register_raw: invalid call_id");
        return;
    };

    // Cast context to usize so the closure is Send + Sync.
    // Safety: the caller guarantees the context pointer is thread-safe.
    let ctx = context as usize;
    let cb = handler;

    phone.register_raw(call_id, move |data: &[u8]| {
        let mut response_data: *mut u8 = std::ptr::null_mut();
        let mut response_len: usize = 0;

        let result = cb(
            data.as_ptr(),
            data.len(),
            &mut response_data,
            &mut response_len,
            ctx as *mut c_void,
        );

        if result != 0 {
            return Err(PhoneError::HandlerFailed {
                call_id: "ffi".to_string(),
                message: "FFI handler returned error".to_string(),
            });
        }

        if response_data.is_null() || response_len == 0 {
            return Ok(Vec::new());
        }

        // Copy the response and let the caller free the original.
        let response = unsafe {
            std::slice::from_raw_parts(response_data, response_len).to_vec()
        };

        Ok(response)
    });
}

// ---------------------------------------------------------------------------
// Calling
// ---------------------------------------------------------------------------

/// Make a raw call. Returns 0 on success, -1 on error.
///
/// On success, `*out_data` and `*out_len` are set to the response bytes.
/// The caller must free `*out_data` via `divi_free_bytes(*out_data, *out_len)`.
///
/// On error, call `divi_last_error()` for details.
///
/// # Safety
/// `phone` must be a valid pointer from `divi_phone_new`.
/// `call_id` must be a valid null-terminated C string.
/// `data` must point to `data_len` valid bytes (or be null if `data_len` is 0).
/// `out_data` and `out_len` must be valid writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_call_raw(
    phone: *const Phone,
    call_id: *const c_char,
    data: *const u8,
    data_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();

    let phone = unsafe { &*phone };
    let Some(call_id) = c_str_to_str(call_id) else {
        set_last_error("divi_phone_call_raw: invalid call_id");
        return -1;
    };

    let input = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    match phone.call_raw(call_id, input) {
        Ok(response) => {
            let (ptr, len) = bytes_to_owned(response);
            unsafe {
                *out_data = ptr;
                *out_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Make a raw call, returning 1 if no handler is registered (not an error).
///
/// Returns: 0 = success, 1 = no handler (not an error), -1 = error.
///
/// # Safety
/// Same as `divi_phone_call_raw`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_call_raw_if_available(
    phone: *const Phone,
    call_id: *const c_char,
    data: *const u8,
    data_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    let phone = unsafe { &*phone };
    let Some(call_id_str) = c_str_to_str(call_id) else {
        set_last_error("divi_phone_call_raw_if_available: invalid call_id");
        return -1;
    };

    if !phone.has_handler(call_id_str) {
        return 1;
    }

    unsafe { divi_phone_call_raw(phone as *const Phone, call_id, data, data_len, out_data, out_len) }
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Check if a handler is registered for a call ID.
///
/// # Safety
/// `phone` must be a valid pointer. `call_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_has_handler(
    phone: *const Phone,
    call_id: *const c_char,
) -> bool {
    let phone = unsafe { &*phone };
    let Some(call_id) = c_str_to_str(call_id) else {
        return false;
    };
    phone.has_handler(call_id)
}

/// Get all registered call IDs as a JSON array string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// `phone` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_registered_call_ids(phone: *const Phone) -> *mut c_char {
    let phone = unsafe { &*phone };
    let ids = phone.registered_call_ids();
    json_to_c(&ids)
}

/// Unregister a handler by call ID.
///
/// # Safety
/// `phone` must be a valid pointer. `call_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_phone_unregister(
    phone: *const Phone,
    call_id: *const c_char,
) {
    let phone = unsafe { &*phone };
    let Some(call_id) = c_str_to_str(call_id) else {
        return;
    };
    phone.unregister(call_id);
}
