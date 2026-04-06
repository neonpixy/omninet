use std::ffi::{c_char, CStr, CString};

use crate::LAST_ERROR;

// ---------------------------------------------------------------------------
// Error retrieval
// ---------------------------------------------------------------------------

/// Get the last error message, or null if no error.
///
/// The returned pointer is valid until the next FFI call on this thread.
/// Do NOT free it — it is owned by the thread-local storage.
#[unsafe(no_mangle)]
pub extern "C" fn divi_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

// ---------------------------------------------------------------------------
// Memory deallocation
// ---------------------------------------------------------------------------

/// Free a Rust-allocated C string.
///
/// # Safety
/// `ptr` must have been returned by a `divi_*` function that allocates strings,
/// or be null (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

/// Free a Rust-allocated byte buffer.
///
/// # Safety
/// `ptr` must have been returned by a `divi_*` function that allocates bytes,
/// with the matching `len`. Or null (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe {
            drop(Vec::from_raw_parts(ptr, len, len));
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers (not exported)
// ---------------------------------------------------------------------------

/// Lock a mutex, recovering from poisoning.
///
/// Poisoned mutexes indicate a prior panic on another thread; we recover the
/// inner value rather than propagating a panic across the FFI boundary (which
/// is undefined behavior).
pub(crate) fn lock_or_recover<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Convert a `*const c_char` to a Rust `&str`. Returns `None` if null or invalid UTF-8.
pub(crate) fn c_str_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Convert a Rust `String` to an owned `*mut c_char`. Caller must free via `divi_free_string`.
/// Returns null if the string contains interior null bytes (shouldn't happen with JSON).
pub(crate) fn string_to_c(s: String) -> *mut c_char {
    CString::new(s)
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Serialize a value to a JSON `*mut c_char`. Returns null on serialization failure.
pub(crate) fn json_to_c<T: serde::Serialize>(value: &T) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(s) => string_to_c(s),
        Err(e) => {
            crate::set_last_error(format!("JSON serialization failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Convert a raw byte slice to an owned `*mut u8` + length.
/// The caller must free via `divi_free_bytes`.
pub(crate) fn bytes_to_owned(data: Vec<u8>) -> (*mut u8, usize) {
    let len = data.len();
    let mut boxed = data.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (ptr, len)
}
