use std::ffi::c_char;
use std::os::raw::c_void;

use equipment::Contacts;
use equipment::ModuleInfo;

use crate::helpers::{c_str_to_str, json_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// Callback type
// ---------------------------------------------------------------------------

/// C function pointer for a shutdown callback.
pub type DiviShutdownCallback = extern "C" fn(context: *mut c_void);

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Create a new Contacts registry. Returns an opaque pointer.
/// Free with `divi_contacts_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_contacts_new() -> *mut Contacts {
    Box::into_raw(Box::new(Contacts::new()))
}

/// Free a Contacts created by `divi_contacts_new`.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_contacts_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_free(ptr: *mut Contacts) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register a module from a JSON string.
///
/// The JSON must deserialize to a `ModuleInfo`:
/// ```json
/// {"id": "vault", "name": "Vault", "module_type": "source", "depends_on": ["sentinal"]}
/// ```
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `contacts` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_register(
    contacts: *const Contacts,
    json: *const c_char,
) -> i32 {
    clear_last_error();

    let contacts = unsafe { &*contacts };
    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_contacts_register: invalid json string");
        return -1;
    };

    let info: ModuleInfo = match serde_json::from_str(json_str) {
        Ok(i) => i,
        Err(e) => {
            set_last_error(format!("divi_contacts_register: JSON parse error: {e}"));
            return -1;
        }
    };

    match contacts.register(info) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Register a module with a shutdown callback.
///
/// Same as `divi_contacts_register`, but the `callback` is invoked when the
/// module is unregistered or during `divi_contacts_shutdown_all`.
///
/// # Safety
/// `contacts` must be a valid pointer. `json` must be a valid C string.
/// `context` must be valid until the callback fires, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_register_with_shutdown(
    contacts: *const Contacts,
    json: *const c_char,
    callback: DiviShutdownCallback,
    context: *mut c_void,
) -> i32 {
    clear_last_error();

    let contacts = unsafe { &*contacts };
    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_contacts_register_with_shutdown: invalid json string");
        return -1;
    };

    let info: ModuleInfo = match serde_json::from_str(json_str) {
        Ok(i) => i,
        Err(e) => {
            set_last_error(format!(
                "divi_contacts_register_with_shutdown: JSON parse error: {e}"
            ));
            return -1;
        }
    };

    // Cast context to usize so the closure is Send.
    let ctx = context as usize;
    let cb = callback;

    match contacts.register_with_shutdown(info, move || {
        cb(ctx as *mut c_void);
    }) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ---------------------------------------------------------------------------
// Unregistration / shutdown
// ---------------------------------------------------------------------------

/// Unregister a module by ID. Dependents are shut down first.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `contacts` must be a valid pointer. `module_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_unregister(
    contacts: *const Contacts,
    module_id: *const c_char,
) -> i32 {
    clear_last_error();

    let contacts = unsafe { &*contacts };
    let Some(module_id) = c_str_to_str(module_id) else {
        set_last_error("divi_contacts_unregister: invalid module_id");
        return -1;
    };

    match contacts.unregister(module_id) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Shut down all modules in dependency order.
///
/// # Safety
/// `contacts` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_shutdown_all(contacts: *const Contacts) {
    let contacts = unsafe { &*contacts };
    contacts.shutdown_all();
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Look up a module by ID. Returns JSON string or null if not found.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `contacts` must be a valid pointer. `module_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_lookup(
    contacts: *const Contacts,
    module_id: *const c_char,
) -> *mut c_char {
    let contacts = unsafe { &*contacts };
    let Some(module_id) = c_str_to_str(module_id) else {
        return std::ptr::null_mut();
    };

    match contacts.lookup(module_id) {
        Some(info) => json_to_c(&info),
        None => std::ptr::null_mut(),
    }
}

/// Get all registered modules as a JSON array string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `contacts` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_all_modules(contacts: *const Contacts) -> *mut c_char {
    let contacts = unsafe { &*contacts };
    let modules = contacts.all_modules();
    json_to_c(&modules)
}

/// Get all registered module IDs as a JSON array string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `contacts` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_registered_module_ids(
    contacts: *const Contacts,
) -> *mut c_char {
    let contacts = unsafe { &*contacts };
    let ids = contacts.registered_module_ids();
    json_to_c(&ids)
}

/// Get all modules that depend on the given module, as a JSON array.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `contacts` must be a valid pointer. `module_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_contacts_dependents_of(
    contacts: *const Contacts,
    module_id: *const c_char,
) -> *mut c_char {
    let contacts = unsafe { &*contacts };
    let Some(module_id) = c_str_to_str(module_id) else {
        return json_to_c(&Vec::<ModuleInfo>::new());
    };

    let deps = contacts.dependents_of(module_id);
    json_to_c(&deps)
}
