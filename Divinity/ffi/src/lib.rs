//! Divinity FFI — C bindings for the Omnidea Rust core.
//!
//! Exposes Equipment (Phone, Email, Contacts, Pager) to any language
//! that speaks C ABI. All functions use the `divi_` prefix.
//!
//! ## Conventions
//!
//! - Stateful types are opaque pointers (`*mut T`). Create with `_new`, destroy with `_free`.
//! - Data types cross the boundary as JSON strings (`*const c_char` / `*mut c_char`).
//! - Raw bytes cross as pointer + length (`*const u8` + `usize`).
//! - Fallible functions return `i32`: 0 = success, -1 = error.
//! - Error details: call `divi_last_error()` after a failure.
//! - Every `*mut c_char` returned by Rust must be freed via `divi_free_string()`.
//! - Every `*mut u8` returned by Rust must be freed via `divi_free_bytes()`.

mod advisor_ffi;
mod bulwark_ffi;
mod commerce_ffi;
mod contacts_ffi;
mod discovery_ffi;
mod crown_ffi;
mod email_ffi;
mod formula_ffi;
mod fortune_ffi;
mod globe_ffi;
mod hall_ffi;
mod helpers;
mod ideas_ffi;
mod jail_ffi;
mod kingdom_ffi;
mod lingo_ffi;
mod magic_ffi;
mod omnibus_ffi;
mod oracle_ffi;
mod pager_ffi;
mod physical_ffi;
mod polity_ffi;
mod phone_ffi;
mod pulse_ffi;
mod quest_ffi;
mod regalia_ffi;
mod runtime_ffi;
mod sentinal_ffi;
mod server_ffi;
mod vault_ffi;
mod yoke_ffi;

mod nexus_ffi;
mod undercroft_ffi;
mod appcatalog_ffi;
mod device_ffi;
mod zeitgeist_ffi;

use std::cell::RefCell;
use std::ffi::CString;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Store an error message in thread-local storage.
fn set_last_error(msg: impl Into<String>) {
    let msg = msg.into();
    log::error!("FFI error: {msg}");
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Clear the thread-local error.
fn clear_last_error() {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = None;
    });
}
