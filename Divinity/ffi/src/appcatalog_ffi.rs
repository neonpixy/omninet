use std::ffi::c_char;
use std::sync::Mutex;

use app_catalog::{
    AppCatalog, AppManifest, CatalogEntry, InstallRequest, InstallStatus,
    resolve_install_action,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// AppCatalog — opaque pointer (in-memory app registry)
// ===================================================================

pub struct DiviAppCatalog(pub(crate) Mutex<AppCatalog>);

/// Create a new empty app catalog.
///
/// Free with `divi_appcatalog_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_appcatalog_new() -> *mut DiviAppCatalog {
    Box::into_raw(Box::new(DiviAppCatalog(Mutex::new(AppCatalog::new()))))
}

/// Free an app catalog.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_free(ptr: *mut DiviAppCatalog) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Add or update a manifest in the catalog.
///
/// `manifest_json` is a JSON `AppManifest`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `catalog` must be a valid pointer. `manifest_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_add_manifest(
    catalog: *const DiviAppCatalog,
    manifest_json: *const c_char,
) -> i32 {
    clear_last_error();

    let catalog = unsafe { &*catalog };
    let Some(mj) = c_str_to_str(manifest_json) else {
        set_last_error("divi_appcatalog_add_manifest: invalid manifest_json");
        return -1;
    };

    let manifest: AppManifest = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_appcatalog_add_manifest: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&catalog.0);
    guard.add_manifest(manifest);
    0
}

/// Look up a catalog entry by app ID.
///
/// Returns JSON (CatalogEntry) or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `catalog` must be a valid pointer. `app_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_get(
    catalog: *const DiviAppCatalog,
    app_id: *const c_char,
) -> *mut c_char {
    let catalog = unsafe { &*catalog };
    let Some(id) = c_str_to_str(app_id) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&catalog.0);
    match guard.get(id) {
        Some(entry) => json_to_c(entry),
        None => std::ptr::null_mut(),
    }
}

/// Search for entries by name or description (case-insensitive substring).
///
/// Returns JSON array of CatalogEntry. Caller must free via `divi_free_string`.
///
/// # Safety
/// `catalog` must be a valid pointer. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_search(
    catalog: *const DiviAppCatalog,
    query: *const c_char,
) -> *mut c_char {
    let catalog = unsafe { &*catalog };
    let Some(q) = c_str_to_str(query) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&catalog.0);
    let results: Vec<&CatalogEntry> = guard.search(q);
    json_to_c(&results)
}

/// Get all installed entries.
///
/// Returns JSON array of CatalogEntry. Caller must free via `divi_free_string`.
///
/// # Safety
/// `catalog` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_installed(
    catalog: *const DiviAppCatalog,
) -> *mut c_char {
    let catalog = unsafe { &*catalog };
    let guard = lock_or_recover(&catalog.0);
    let entries: Vec<&CatalogEntry> = guard.installed();
    json_to_c(&entries)
}

/// Get all entries with updates available.
///
/// Returns JSON array of CatalogEntry. Caller must free via `divi_free_string`.
///
/// # Safety
/// `catalog` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_updates_available(
    catalog: *const DiviAppCatalog,
) -> *mut c_char {
    let catalog = unsafe { &*catalog };
    let guard = lock_or_recover(&catalog.0);
    let entries: Vec<&CatalogEntry> = guard.updates_available();
    json_to_c(&entries)
}

/// Get all entries in the catalog.
///
/// Returns JSON array of CatalogEntry. Caller must free via `divi_free_string`.
///
/// # Safety
/// `catalog` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_all(
    catalog: *const DiviAppCatalog,
) -> *mut c_char {
    let catalog = unsafe { &*catalog };
    let guard = lock_or_recover(&catalog.0);
    let entries: Vec<&CatalogEntry> = guard.all();
    json_to_c(&entries)
}

/// Get the number of entries in the catalog.
///
/// # Safety
/// `catalog` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_count(
    catalog: *const DiviAppCatalog,
) -> usize {
    let catalog = unsafe { &*catalog };
    let guard = lock_or_recover(&catalog.0);
    guard.count()
}

/// Mark an app as installed at the given version.
///
/// Returns 0 on success, -1 on error (not found or already installed).
///
/// # Safety
/// `catalog` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_mark_installed(
    catalog: *const DiviAppCatalog,
    app_id: *const c_char,
    version: *const c_char,
) -> i32 {
    clear_last_error();

    let catalog = unsafe { &*catalog };
    let Some(id) = c_str_to_str(app_id) else {
        set_last_error("divi_appcatalog_mark_installed: invalid app_id");
        return -1;
    };
    let Some(ver) = c_str_to_str(version) else {
        set_last_error("divi_appcatalog_mark_installed: invalid version");
        return -1;
    };

    let mut guard = lock_or_recover(&catalog.0);
    match guard.mark_installed(id, ver) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Mark an app as uninstalled (revert to Available).
///
/// Returns 0 on success, -1 on error (not found or not installed).
///
/// # Safety
/// `catalog` must be a valid pointer. `app_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_mark_uninstalled(
    catalog: *const DiviAppCatalog,
    app_id: *const c_char,
) -> i32 {
    clear_last_error();

    let catalog = unsafe { &*catalog };
    let Some(id) = c_str_to_str(app_id) else {
        set_last_error("divi_appcatalog_mark_uninstalled: invalid app_id");
        return -1;
    };

    let mut guard = lock_or_recover(&catalog.0);
    match guard.mark_uninstalled(id) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Set an arbitrary install status on an entry.
///
/// `status_json` is a JSON `InstallStatus`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `catalog` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_mark_status(
    catalog: *const DiviAppCatalog,
    app_id: *const c_char,
    status_json: *const c_char,
) -> i32 {
    clear_last_error();

    let catalog = unsafe { &*catalog };
    let Some(id) = c_str_to_str(app_id) else {
        set_last_error("divi_appcatalog_mark_status: invalid app_id");
        return -1;
    };
    let Some(sj) = c_str_to_str(status_json) else {
        set_last_error("divi_appcatalog_mark_status: invalid status_json");
        return -1;
    };

    let status: InstallStatus = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_appcatalog_mark_status: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&catalog.0);
    match guard.mark_status(id, status) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Scan installed entries and mark those with newer manifest versions.
///
/// # Safety
/// `catalog` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_check_updates(
    catalog: *const DiviAppCatalog,
) {
    let catalog = unsafe { &*catalog };
    let mut guard = lock_or_recover(&catalog.0);
    guard.check_updates();
}

/// Remove an entry from the catalog entirely.
///
/// Returns JSON (CatalogEntry) of the removed entry, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `catalog` must be a valid pointer. `app_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_remove(
    catalog: *const DiviAppCatalog,
    app_id: *const c_char,
) -> *mut c_char {
    let catalog = unsafe { &*catalog };
    let Some(id) = c_str_to_str(app_id) else {
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&catalog.0);
    match guard.remove(id) {
        Some(entry) => json_to_c(&entry),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// Lifecycle — stateless
// ===================================================================

/// Resolve the platform-appropriate install action for a request.
///
/// - `request_json` is a JSON `InstallRequest`.
/// - `manifest_json` is a JSON `AppManifest`.
///
/// Returns JSON (InstallAction). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// Both C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_resolve_install(
    request_json: *const c_char,
    manifest_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(request_json) else {
        set_last_error("divi_appcatalog_resolve_install: invalid request_json");
        return std::ptr::null_mut();
    };
    let Some(mj) = c_str_to_str(manifest_json) else {
        set_last_error("divi_appcatalog_resolve_install: invalid manifest_json");
        return std::ptr::null_mut();
    };

    let request: InstallRequest = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_appcatalog_resolve_install: request: {e}"));
            return std::ptr::null_mut();
        }
    };

    let manifest: AppManifest = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_appcatalog_resolve_install: manifest: {e}"));
            return std::ptr::null_mut();
        }
    };

    match resolve_install_action(&request, &manifest) {
        Ok(action) => json_to_c(&action),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Manifest helpers — stateless
// ===================================================================

/// Verify a manifest's BIP-340 signature.
///
/// `manifest_json` is a JSON `AppManifest`.
/// Returns 0 if the signature is valid, -1 if invalid or absent.
///
/// # Safety
/// `manifest_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_manifest_verify(
    manifest_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(mj) = c_str_to_str(manifest_json) else {
        set_last_error("divi_appcatalog_manifest_verify: invalid manifest_json");
        return -1;
    };

    let manifest: AppManifest = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_appcatalog_manifest_verify: {e}"));
            return -1;
        }
    };

    match manifest.verify_signature() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get the latest version entry from a manifest.
///
/// `manifest_json` is a JSON `AppManifest`.
/// Returns JSON (AppVersion) or null if no versions exist.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `manifest_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_appcatalog_manifest_latest_version(
    manifest_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(mj) = c_str_to_str(manifest_json) else {
        set_last_error("divi_appcatalog_manifest_latest_version: invalid manifest_json");
        return std::ptr::null_mut();
    };

    let manifest: AppManifest = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!(
                "divi_appcatalog_manifest_latest_version: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    match manifest.latest_version() {
        Some(version) => json_to_c(version),
        None => std::ptr::null_mut(),
    }
}
