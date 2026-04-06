//! Hall FFI — C bindings for encrypted .idea file I/O.
//!
//! All functions are stateless — Hall has no long-lived state.
//! Keys cross the boundary as raw bytes (`*const u8` + `usize`).
//! Data types cross as JSON strings (`*mut c_char`).

use std::ffi::c_char;
use std::path::PathBuf;

use crate::helpers::{c_str_to_str, json_to_c, string_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Scholar — read operations
// ===================================================================

/// Check whether a path is a .idea package (directory with Header.json).
///
/// Returns `true` if it is, `false` otherwise (including on null path).
///
/// # Safety
/// `path` must be a valid null-terminated C string, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_is_idea_package(path: *const c_char) -> bool {
    let Some(path_str) = c_str_to_str(path) else {
        return false;
    };
    hall::scholar::is_idea_package(path_str.as_ref())
}

/// Read just the header from an .idea package (no key needed).
///
/// Returns a JSON-encoded `Header` string, or null on error.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `path` must be a valid null-terminated C string, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_read_header(path: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_hall_read_header: null path");
        return std::ptr::null_mut();
    };

    match hall::scholar::read_header(path_str.as_ref()) {
        Ok(header) => json_to_c(&header),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Read a full .idea package with decryption and graceful degradation.
///
/// Returns a JSON-encoded `IdeaPackage` string (with `path` field omitted),
/// or null on error. Warnings (non-fatal issues) are written to
/// `out_warnings_json` as a JSON array of `HallWarning` objects.
/// Both the returned string and the warnings string must be freed
/// via `divi_free_string`.
///
/// # Safety
/// - `path` must be a valid null-terminated C string.
/// - `content_key` must point to `key_len` valid bytes.
/// - `out_warnings_json` must be a valid pointer to a `*mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_read(
    path: *const c_char,
    content_key: *const u8,
    key_len: usize,
    out_warnings_json: *mut *mut c_char,
) -> *mut c_char {
    clear_last_error();

    // Initialize out param to null in case of early return.
    if !out_warnings_json.is_null() {
        unsafe { *out_warnings_json = std::ptr::null_mut() };
    }

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_hall_read: null path");
        return std::ptr::null_mut();
    };

    if content_key.is_null() || key_len == 0 {
        set_last_error("divi_hall_read: null content_key");
        return std::ptr::null_mut();
    }
    let key = unsafe { std::slice::from_raw_parts(content_key, key_len) };

    match hall::scholar::read(path_str.as_ref(), key, None) {
        Ok(result) => {
            // Serialize warnings to the out-parameter.
            if !out_warnings_json.is_null() {
                unsafe { *out_warnings_json = json_to_c(&result.warnings) };
            }
            // Serialize the package.
            json_to_c(&result.value)
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Scribe — write operations
// ===================================================================

/// Write an IdeaPackage to disk with encryption.
///
/// `package_json` is a JSON-encoded `IdeaPackage`. The `path` parameter
/// overrides the package's path field (which is skipped during serde).
///
/// Returns the number of bytes written on success, or -1 on error.
///
/// # Safety
/// - `package_json` and `path` must be valid null-terminated C strings.
/// - `content_key` must point to `key_len` valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_write(
    package_json: *const c_char,
    path: *const c_char,
    content_key: *const u8,
    key_len: usize,
) -> i64 {
    clear_last_error();

    let Some(json_str) = c_str_to_str(package_json) else {
        set_last_error("divi_hall_write: null package_json");
        return -1;
    };

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_hall_write: null path");
        return -1;
    };

    if content_key.is_null() || key_len == 0 {
        set_last_error("divi_hall_write: null content_key");
        return -1;
    }
    let key = unsafe { std::slice::from_raw_parts(content_key, key_len) };

    let mut package: ideas::IdeaPackage = match serde_json::from_str(json_str) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_hall_write: invalid package JSON: {e}"));
            return -1;
        }
    };

    // Set the path from the explicit parameter (serde skips it).
    package.path = PathBuf::from(path_str);

    match hall::scribe::write(&package, key, None) {
        Ok(bytes) => bytes as i64,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ===================================================================
// Archivist — asset operations
// ===================================================================

/// Import raw bytes as an encrypted asset. Returns the SHA-256 hex hash.
///
/// Caller must free the returned string via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// - `data` must point to `data_len` valid bytes.
/// - `idea_path` must be a valid null-terminated C string.
/// - `content_key` must point to `key_len` valid bytes.
/// - `vocab_seed` must point to `seed_len` valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_import(
    data: *const u8,
    data_len: usize,
    idea_path: *const c_char,
    content_key: *const u8,
    key_len: usize,
    vocab_seed: *const u8,
    seed_len: usize,
) -> *mut c_char {
    clear_last_error();

    if data.is_null() || data_len == 0 {
        set_last_error("divi_hall_asset_import: null data");
        return std::ptr::null_mut();
    }
    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len) };

    let Some(idea_path_str) = c_str_to_str(idea_path) else {
        set_last_error("divi_hall_asset_import: null idea_path");
        return std::ptr::null_mut();
    };

    if content_key.is_null() || key_len == 0 {
        set_last_error("divi_hall_asset_import: null content_key");
        return std::ptr::null_mut();
    }
    let key = unsafe { std::slice::from_raw_parts(content_key, key_len) };

    if vocab_seed.is_null() || seed_len == 0 {
        set_last_error("divi_hall_asset_import: null vocab_seed");
        return std::ptr::null_mut();
    }
    let seed = unsafe { std::slice::from_raw_parts(vocab_seed, seed_len) };

    match hall::archivist::import(data_slice, idea_path_str.as_ref(), key, seed) {
        Ok(hash) => string_to_c(hash),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Import a file as an encrypted asset. Returns the SHA-256 hex hash.
///
/// Caller must free the returned string via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// - `source_path` and `idea_path` must be valid null-terminated C strings.
/// - `content_key` must point to `key_len` valid bytes.
/// - `vocab_seed` must point to `seed_len` valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_import_file(
    source_path: *const c_char,
    idea_path: *const c_char,
    content_key: *const u8,
    key_len: usize,
    vocab_seed: *const u8,
    seed_len: usize,
) -> *mut c_char {
    clear_last_error();

    let Some(source_str) = c_str_to_str(source_path) else {
        set_last_error("divi_hall_asset_import_file: null source_path");
        return std::ptr::null_mut();
    };

    let Some(idea_str) = c_str_to_str(idea_path) else {
        set_last_error("divi_hall_asset_import_file: null idea_path");
        return std::ptr::null_mut();
    };

    if content_key.is_null() || key_len == 0 {
        set_last_error("divi_hall_asset_import_file: null content_key");
        return std::ptr::null_mut();
    }
    let key = unsafe { std::slice::from_raw_parts(content_key, key_len) };

    if vocab_seed.is_null() || seed_len == 0 {
        set_last_error("divi_hall_asset_import_file: null vocab_seed");
        return std::ptr::null_mut();
    }
    let seed = unsafe { std::slice::from_raw_parts(vocab_seed, seed_len) };

    match hall::archivist::import_file(source_str.as_ref(), idea_str.as_ref(), key, seed) {
        Ok(hash) => string_to_c(hash),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Read an asset by its hash. Decrypts, deobfuscates, and verifies integrity.
///
/// Output bytes are written to `out_data`/`out_len`. Caller must free
/// via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// - `hash` and `idea_path` must be valid null-terminated C strings.
/// - `content_key` must point to `key_len` valid bytes.
/// - `vocab_seed` must point to `seed_len` valid bytes.
/// - `out_data` and `out_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_read(
    hash: *const c_char,
    idea_path: *const c_char,
    content_key: *const u8,
    key_len: usize,
    vocab_seed: *const u8,
    seed_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();

    let Some(hash_str) = c_str_to_str(hash) else {
        set_last_error("divi_hall_asset_read: null hash");
        return -1;
    };

    let Some(idea_str) = c_str_to_str(idea_path) else {
        set_last_error("divi_hall_asset_read: null idea_path");
        return -1;
    };

    if content_key.is_null() || key_len == 0 {
        set_last_error("divi_hall_asset_read: null content_key");
        return -1;
    }
    let key = unsafe { std::slice::from_raw_parts(content_key, key_len) };

    if vocab_seed.is_null() || seed_len == 0 {
        set_last_error("divi_hall_asset_read: null vocab_seed");
        return -1;
    }
    let seed = unsafe { std::slice::from_raw_parts(vocab_seed, seed_len) };

    match hall::archivist::read(hash_str, idea_str.as_ref(), key, seed) {
        Ok(data) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(data);
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

/// Export an asset to a destination file (decrypted).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// - `hash`, `idea_path`, and `dest_path` must be valid null-terminated C strings.
/// - `content_key` must point to `key_len` valid bytes.
/// - `vocab_seed` must point to `seed_len` valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_export(
    hash: *const c_char,
    idea_path: *const c_char,
    dest_path: *const c_char,
    content_key: *const u8,
    key_len: usize,
    vocab_seed: *const u8,
    seed_len: usize,
) -> i32 {
    clear_last_error();

    let Some(hash_str) = c_str_to_str(hash) else {
        set_last_error("divi_hall_asset_export: null hash");
        return -1;
    };

    let Some(idea_str) = c_str_to_str(idea_path) else {
        set_last_error("divi_hall_asset_export: null idea_path");
        return -1;
    };

    let Some(dest_str) = c_str_to_str(dest_path) else {
        set_last_error("divi_hall_asset_export: null dest_path");
        return -1;
    };

    if content_key.is_null() || key_len == 0 {
        set_last_error("divi_hall_asset_export: null content_key");
        return -1;
    }
    let key = unsafe { std::slice::from_raw_parts(content_key, key_len) };

    if vocab_seed.is_null() || seed_len == 0 {
        set_last_error("divi_hall_asset_export: null vocab_seed");
        return -1;
    }
    let seed = unsafe { std::slice::from_raw_parts(vocab_seed, seed_len) };

    match hall::archivist::export(hash_str, idea_str.as_ref(), dest_str.as_ref(), key, seed) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// List all asset hashes in the Assets/ directory.
///
/// Returns a JSON array of hex hash strings. Caller must free via
/// `divi_free_string`. Returns null on error.
///
/// # Safety
/// `idea_path` must be a valid null-terminated C string, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_list(idea_path: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(idea_str) = c_str_to_str(idea_path) else {
        set_last_error("divi_hall_asset_list: null idea_path");
        return std::ptr::null_mut();
    };

    match hall::archivist::list(idea_str.as_ref()) {
        Ok(hashes) => json_to_c(&hashes),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Check if an asset exists by its hash.
///
/// Returns `true` if the asset file exists, `false` otherwise.
///
/// # Safety
/// `hash` and `idea_path` must be valid null-terminated C strings, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_exists(
    hash: *const c_char,
    idea_path: *const c_char,
) -> bool {
    let Some(hash_str) = c_str_to_str(hash) else {
        return false;
    };
    let Some(idea_str) = c_str_to_str(idea_path) else {
        return false;
    };
    hall::archivist::exists(hash_str, idea_str.as_ref())
}

/// Delete an asset by its hash.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `hash` and `idea_path` must be valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_asset_delete(
    hash: *const c_char,
    idea_path: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(hash_str) = c_str_to_str(hash) else {
        set_last_error("divi_hall_asset_delete: null hash");
        return -1;
    };

    let Some(idea_str) = c_str_to_str(idea_path) else {
        set_last_error("divi_hall_asset_delete: null idea_path");
        return -1;
    };

    match hall::archivist::delete(hash_str, idea_str.as_ref()) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ===================================================================
// Media utilities
// ===================================================================

/// Extract image metadata from raw bytes.
///
/// Detects image format, reads dimensions, and generates a blurhash
/// placeholder. Returns a JSON-encoded `ImageMetadata` string with
/// `width`, `height`, `mime`, `size`, and `blurhash` fields.
///
/// Caller must free the returned string via `divi_free_string`.
/// Returns null on error (e.g. unrecognized image format).
///
/// # Safety
/// - `data` must point to `data_len` valid bytes, or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_hall_extract_image_metadata(
    data: *const u8,
    data_len: usize,
) -> *mut c_char {
    clear_last_error();

    if data.is_null() || data_len == 0 {
        set_last_error("divi_hall_extract_image_metadata: null or empty data");
        return std::ptr::null_mut();
    }
    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len) };

    match hall::extract_image_metadata(data_slice) {
        Ok(meta) => json_to_c(&meta),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}
