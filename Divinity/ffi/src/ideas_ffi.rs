//! Ideas FFI — C bindings for the Ideas crate (.idea universal content format).
//!
//! Exposes Digit, Header, IdeaPackage, SchemaRegistry, accessibility,
//! bindings, bonds, CRDT operations, and all domain digit helpers
//! (media, sheet, slide, form, richtext, interactive, commerce).
//!
//! ## Patterns
//!
//! - **IdeaPackage** and **SchemaRegistry** are opaque pointer + Mutex (stateful).
//! - **Digit**, **Header**, **Bonds**, **DataBinding**, etc. use JSON round-trip (pure data).
//! - Domain helpers follow `create_digit(meta_json, author) -> JSON` and
//!   `parse_meta(digit_json) -> JSON` pairs.

use std::ffi::c_char;
use std::path::PathBuf;
use std::sync::Mutex;

use ideas::accessibility::{self, AccessibilityMetadata};
use ideas::binding::{self, DataBinding};
use ideas::bonds::Bonds;
use ideas::crdt::DigitOperation;
use ideas::digit::Digit;
use ideas::header::{Header, KeySlot};
use ideas::package::IdeaPackage;
use ideas::schema::{self, DigitSchema, SchemaRegistry};
use ideas::textspan;
use ideas::validation;
use x::{Value, VectorClock};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Opaque pointer types
// ===================================================================

/// Thread-safe wrapper for IdeaPackage (filesystem I/O, mutable state).
pub struct IdeasPackage(pub(crate) Mutex<IdeaPackage>);

/// Thread-safe wrapper for SchemaRegistry (accumulates schemas, queried repeatedly).
pub struct IdeasSchemaRegistry(pub(crate) Mutex<SchemaRegistry>);

// ===================================================================
// Wave 1: IdeaPackage (17 functions)
// ===================================================================

/// Create a new IdeaPackage in memory.
///
/// `path` is the filesystem path for the .idea directory.
/// `header_json` is a JSON Header. `root_digit_json` is a JSON Digit.
/// Returns an opaque pointer, or null on error. Free with `divi_ideas_package_free`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_new(
    path: *const c_char,
    header_json: *const c_char,
    root_digit_json: *const c_char,
) -> *mut IdeasPackage {
    clear_last_error();

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_ideas_package_new: invalid path");
        return std::ptr::null_mut();
    };
    let Some(header_str) = c_str_to_str(header_json) else {
        set_last_error("divi_ideas_package_new: invalid header_json");
        return std::ptr::null_mut();
    };
    let Some(digit_str) = c_str_to_str(root_digit_json) else {
        set_last_error("divi_ideas_package_new: invalid root_digit_json");
        return std::ptr::null_mut();
    };

    let header: Header = match serde_json::from_str(header_str) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_ideas_package_new: header parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let root_digit: Digit = match serde_json::from_str(digit_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_package_new: digit parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let pkg = IdeaPackage::new(PathBuf::from(path_str), header, root_digit);
    Box::into_raw(Box::new(IdeasPackage(Mutex::new(pkg))))
}

/// Load an IdeaPackage from a .idea directory on disk.
///
/// Returns an opaque pointer, or null on error. Free with `divi_ideas_package_free`.
///
/// # Safety
/// `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_load(path: *const c_char) -> *mut IdeasPackage {
    clear_last_error();

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_ideas_package_load: invalid path");
        return std::ptr::null_mut();
    };

    match IdeaPackage::load(std::path::Path::new(path_str)) {
        Ok(pkg) => Box::into_raw(Box::new(IdeasPackage(Mutex::new(pkg)))),
        Err(e) => {
            set_last_error(format!("divi_ideas_package_load: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Free an IdeaPackage.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_ideas_package_*` constructor, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_free(ptr: *mut IdeasPackage) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Save the package to disk.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_save(pkg: *const IdeasPackage) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };
    let guard = lock_or_recover(&pkg.0);

    match guard.save() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_package_save: {e}"));
            -1
        }
    }
}

/// Get the package header as JSON.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pkg` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_header(pkg: *const IdeasPackage) -> *mut c_char {
    let pkg = unsafe { &*pkg };
    let guard = lock_or_recover(&pkg.0);
    json_to_c(&guard.header)
}

/// Get the root digit as JSON, or null if not found.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pkg` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_root_digit(pkg: *const IdeasPackage) -> *mut c_char {
    let pkg = unsafe { &*pkg };
    let guard = lock_or_recover(&pkg.0);

    match guard.root_digit() {
        Some(d) => json_to_c(d),
        None => std::ptr::null_mut(),
    }
}

/// Get a digit by ID (UUID string) as JSON, or null if not found.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pkg` and `id` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_digit(
    pkg: *const IdeasPackage,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_ideas_package_digit: invalid id");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_ideas_package_digit: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&pkg.0);
    match guard.digits.get(&uuid) {
        Some(d) => json_to_c(d),
        None => std::ptr::null_mut(),
    }
}

/// Get the number of digits in the package.
///
/// # Safety
/// `pkg` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_digit_count(pkg: *const IdeasPackage) -> u32 {
    let pkg = unsafe { &*pkg };
    let guard = lock_or_recover(&pkg.0);
    guard.digits.len() as u32
}

/// Get all digits as a JSON array.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `pkg` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_all_digits(pkg: *const IdeasPackage) -> *mut c_char {
    let pkg = unsafe { &*pkg };
    let guard = lock_or_recover(&pkg.0);
    let digits: Vec<&Digit> = guard.digits.values().collect();
    json_to_c(&digits)
}

/// Add a digit to the package from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_add_digit(
    pkg: *const IdeasPackage,
    digit_json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_package_add_digit: invalid digit_json");
        return -1;
    };

    let digit: Digit = match serde_json::from_str(json_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_package_add_digit: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&pkg.0);
    guard.digits.insert(digit.id(), digit);
    0
}

/// Set the package's book (authority) from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_set_book(
    pkg: *const IdeasPackage,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_ideas_package_set_book: invalid json");
        return -1;
    };

    match serde_json::from_str(json_str) {
        Ok(book) => {
            let mut guard = lock_or_recover(&pkg.0);
            guard.book = Some(book);
            0
        }
        Err(e) => {
            set_last_error(format!("divi_ideas_package_set_book: {e}"));
            -1
        }
    }
}

/// Set the package's tree (provenance) from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_set_tree(
    pkg: *const IdeasPackage,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_ideas_package_set_tree: invalid json");
        return -1;
    };

    match serde_json::from_str(json_str) {
        Ok(tree) => {
            let mut guard = lock_or_recover(&pkg.0);
            guard.tree = Some(tree);
            0
        }
        Err(e) => {
            set_last_error(format!("divi_ideas_package_set_tree: {e}"));
            -1
        }
    }
}

/// Set the package's cool (currency value) from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_set_cool(
    pkg: *const IdeasPackage,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_ideas_package_set_cool: invalid json");
        return -1;
    };

    match serde_json::from_str(json_str) {
        Ok(cool) => {
            let mut guard = lock_or_recover(&pkg.0);
            guard.cool = Some(cool);
            0
        }
        Err(e) => {
            set_last_error(format!("divi_ideas_package_set_cool: {e}"));
            -1
        }
    }
}

/// Set the package's redemption from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_set_redemption(
    pkg: *const IdeasPackage,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_ideas_package_set_redemption: invalid json");
        return -1;
    };

    match serde_json::from_str(json_str) {
        Ok(redemption) => {
            let mut guard = lock_or_recover(&pkg.0);
            guard.redemption = Some(redemption);
            0
        }
        Err(e) => {
            set_last_error(format!("divi_ideas_package_set_redemption: {e}"));
            -1
        }
    }
}

/// Set the package's bonds from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_set_bonds(
    pkg: *const IdeasPackage,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_ideas_package_set_bonds: invalid json");
        return -1;
    };

    match serde_json::from_str(json_str) {
        Ok(bonds) => {
            let mut guard = lock_or_recover(&pkg.0);
            guard.bonds = Some(bonds);
            0
        }
        Err(e) => {
            set_last_error(format!("divi_ideas_package_set_bonds: {e}"));
            -1
        }
    }
}

/// Set the package's position from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pkg` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_set_position(
    pkg: *const IdeasPackage,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let pkg = unsafe { &*pkg };

    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_ideas_package_set_position: invalid json");
        return -1;
    };

    match serde_json::from_str(json_str) {
        Ok(position) => {
            let mut guard = lock_or_recover(&pkg.0);
            guard.position = Some(position);
            0
        }
        Err(e) => {
            set_last_error(format!("divi_ideas_package_set_position: {e}"));
            -1
        }
    }
}

/// Read only the header from a .idea directory (static, no package needed).
///
/// Returns JSON Header, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_package_read_header(path: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_ideas_package_read_header: invalid path");
        return std::ptr::null_mut();
    };

    match IdeaPackage::read_header(std::path::Path::new(path_str)) {
        Ok(header) => json_to_c(&header),
        Err(e) => {
            set_last_error(format!("divi_ideas_package_read_header: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Wave 1: Digit CRUD (19 functions)
// ===================================================================

/// Create a new digit.
///
/// `digit_type` is the type string (e.g. "text", "media.image").
/// `content_json` is a JSON Value for the content.
/// `author` is the creator's crown_id.
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_new(
    digit_type: *const c_char,
    content_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(type_str) = c_str_to_str(digit_type) else {
        set_last_error("divi_ideas_digit_new: invalid digit_type");
        return std::ptr::null_mut();
    };
    let Some(content_str) = c_str_to_str(content_json) else {
        set_last_error("divi_ideas_digit_new: invalid content_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_digit_new: invalid author");
        return std::ptr::null_mut();
    };

    let content: Value = match serde_json::from_str(content_str) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_new: content parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    match Digit::new(type_str.into(), content, author_str.into()) {
        Ok(d) => json_to_c(&d),
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_new: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Return a new digit with updated content (copy-on-write).
///
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_content(
    digit_json: *const c_char,
    content_json: *const c_char,
    by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_content: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(cj) = c_str_to_str(content_json) else {
        set_last_error("divi_ideas_digit_with_content: invalid content_json");
        return std::ptr::null_mut();
    };
    let Some(by_str) = c_str_to_str(by) else {
        set_last_error("divi_ideas_digit_with_content: invalid by");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_content: {e}"));
            return std::ptr::null_mut();
        }
    };
    let content: Value = match serde_json::from_str(cj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_content: content parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let updated = digit.with_content(content, by_str);
    json_to_c(&updated)
}

/// Return a new digit with a property set (copy-on-write).
///
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_property(
    digit_json: *const c_char,
    key: *const c_char,
    value_json: *const c_char,
    by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_property: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(key_str) = c_str_to_str(key) else {
        set_last_error("divi_ideas_digit_with_property: invalid key");
        return std::ptr::null_mut();
    };
    let Some(vj) = c_str_to_str(value_json) else {
        set_last_error("divi_ideas_digit_with_property: invalid value_json");
        return std::ptr::null_mut();
    };
    let Some(by_str) = c_str_to_str(by) else {
        set_last_error("divi_ideas_digit_with_property: invalid by");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_property: {e}"));
            return std::ptr::null_mut();
        }
    };
    let value: Value = match serde_json::from_str(vj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_property: value parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let updated = digit.with_property(key_str.into(), value, by_str);
    json_to_c(&updated)
}

/// Return a new digit with a child added (copy-on-write).
///
/// `child_id` is a UUID string.
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_child(
    digit_json: *const c_char,
    child_id: *const c_char,
    by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_child: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(child_id) else {
        set_last_error("divi_ideas_digit_with_child: invalid child_id");
        return std::ptr::null_mut();
    };
    let Some(by_str) = c_str_to_str(by) else {
        set_last_error("divi_ideas_digit_with_child: invalid by");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_child: {e}"));
            return std::ptr::null_mut();
        }
    };
    let uuid = match uuid::Uuid::parse_str(cid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_child: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let updated = digit.with_child(uuid, by_str);
    json_to_c(&updated)
}

/// Return a tombstoned copy of this digit.
///
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_deleted(
    digit_json: *const c_char,
    by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_deleted: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(by_str) = c_str_to_str(by) else {
        set_last_error("divi_ideas_digit_deleted: invalid by");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_deleted: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&digit.deleted(by_str))
}

/// Return a restored (un-tombstoned) copy of this digit.
///
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_restored(
    digit_json: *const c_char,
    by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_restored: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(by_str) = c_str_to_str(by) else {
        set_last_error("divi_ideas_digit_restored: invalid by");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_restored: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&digit.restored(by_str))
}

/// Get the digit's UUID as a string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`, or null on error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_id(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_id: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_id: {e}"));
            return std::ptr::null_mut();
        }
    };

    string_to_c(digit.id().to_string())
}

/// Get the digit's type string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`, or null on error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_type(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_type: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_type: {e}"));
            return std::ptr::null_mut();
        }
    };

    string_to_c(digit.digit_type().to_string())
}

/// Get the digit's author string.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`, or null on error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_author(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_author: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_author: {e}"));
            return std::ptr::null_mut();
        }
    };

    string_to_c(digit.author().to_string())
}

/// Check if the digit is tombstoned (deleted).
///
/// Returns false on parse error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_is_deleted(digit_json: *const c_char) -> bool {
    let Some(dj) = c_str_to_str(digit_json) else {
        return false;
    };
    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(_) => return false,
    };
    digit.is_deleted()
}

/// Check if the digit has children.
///
/// Returns false on parse error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_has_children(digit_json: *const c_char) -> bool {
    let Some(dj) = c_str_to_str(digit_json) else {
        return false;
    };
    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(_) => return false,
    };
    digit.has_children()
}

/// Extract all text content from a digit for search indexing.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`, or null on error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_extract_text(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_extract_text: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_extract_text: {e}"));
            return std::ptr::null_mut();
        }
    };

    string_to_c(digit.extract_text())
}

/// Validate a digit (checks type and property keys).
///
/// Returns 0 on success (valid), -1 on validation error.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_validate(digit_json: *const c_char) -> i32 {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_validate: invalid digit_json");
        return -1;
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_validate: {e}"));
            return -1;
        }
    };

    match digit.validate() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_validate: {e}"));
            -1
        }
    }
}

/// Return a new digit with TextSpan data added/replaced on the "spans" property.
///
/// `spans_json` is a JSON array of `TextSpan` objects (see `textspan.rs`).
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_spans(
    digit_json: *const c_char,
    spans_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_spans: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(spans_json) else {
        set_last_error("divi_ideas_digit_with_spans: invalid spans_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_digit_with_spans: invalid author");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_spans: {e}"));
            return std::ptr::null_mut();
        }
    };
    let spans: Vec<textspan::TextSpan> = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!(
                "divi_ideas_digit_with_spans: spans parse error: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let value = textspan::spans_to_value(&spans);
    let updated = digit.with_property("spans".into(), value, author_str);
    json_to_c(&updated)
}

/// Extract TextSpan data from a digit's "spans" property.
///
/// Returns JSON array of `TextSpan` objects, or null if no spans are present.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_spans(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_spans: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_spans: {e}"));
            return std::ptr::null_mut();
        }
    };

    match digit.properties.get("spans") {
        Some(val) => match textspan::spans_from_value(val) {
            Some(spans) => json_to_c(&spans),
            None => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Return a new digit with a child removed by UUID string.
///
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_without_child(
    digit_json: *const c_char,
    child_id_str: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_without_child: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(child_id_str) else {
        set_last_error("divi_ideas_digit_without_child: invalid child_id_str");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_without_child: {e}"));
            return std::ptr::null_mut();
        }
    };
    let uuid = match uuid::Uuid::parse_str(cid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_ideas_digit_without_child: invalid UUID: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let author = digit.author().to_string();
    let updated = digit.without_child(uuid, &author);
    json_to_c(&updated)
}

/// Return a new digit with a child inserted at a specific index.
///
/// `index` is the 0-based insertion position. If >= current child count, appends.
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_child_at(
    digit_json: *const c_char,
    index: usize,
    child_id_str: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_child_at: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(child_id_str) else {
        set_last_error("divi_ideas_digit_with_child_at: invalid child_id_str");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_digit_with_child_at: invalid author");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_child_at: {e}"));
            return std::ptr::null_mut();
        }
    };
    let uuid = match uuid::Uuid::parse_str(cid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_ideas_digit_with_child_at: invalid UUID: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let updated = digit.with_child_at(index, uuid, author_str);
    json_to_c(&updated)
}

/// Return a new digit with children reordered.
///
/// `children_json` is a JSON array of UUID strings representing the new order.
/// Returns JSON Digit, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_reorder_children(
    digit_json: *const c_char,
    children_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_reorder_children: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(cj) = c_str_to_str(children_json) else {
        set_last_error("divi_ideas_digit_reorder_children: invalid children_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_digit_reorder_children: invalid author");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_reorder_children: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Parse the JSON array of UUID strings.
    let uuid_strings: Vec<String> = match serde_json::from_str(cj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!(
                "divi_ideas_digit_reorder_children: children parse error: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let mut uuids = Vec::with_capacity(uuid_strings.len());
    for s in &uuid_strings {
        match uuid::Uuid::parse_str(s) {
            Ok(u) => uuids.push(u),
            Err(e) => {
                set_last_error(format!(
                    "divi_ideas_digit_reorder_children: invalid UUID '{s}': {e}"
                ));
                return std::ptr::null_mut();
            }
        }
    }

    let updated = digit.with_children_reordered(uuids, author_str);
    json_to_c(&updated)
}

// ===================================================================
// Wave 1: Header (8 functions)
// ===================================================================

/// Create a new header.
///
/// `pubkey` and `signature` are creator identity strings.
/// `root_id` is a UUID string. `key_slot_json` is a JSON KeySlot.
/// Returns JSON Header. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_create(
    pubkey: *const c_char,
    signature: *const c_char,
    root_id: *const c_char,
    key_slot_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_ideas_header_create: invalid pubkey");
        return std::ptr::null_mut();
    };
    let Some(sig) = c_str_to_str(signature) else {
        set_last_error("divi_ideas_header_create: invalid signature");
        return std::ptr::null_mut();
    };
    let Some(rid) = c_str_to_str(root_id) else {
        set_last_error("divi_ideas_header_create: invalid root_id");
        return std::ptr::null_mut();
    };
    let Some(ks_str) = c_str_to_str(key_slot_json) else {
        set_last_error("divi_ideas_header_create: invalid key_slot_json");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(rid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_create: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };
    let key_slot: KeySlot = match serde_json::from_str(ks_str) {
        Ok(ks) => ks,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_create: {e}"));
            return std::ptr::null_mut();
        }
    };

    let header = Header::create(pk.into(), sig.into(), uuid, key_slot);
    json_to_c(&header)
}

/// Validate a header.
///
/// Returns 0 on success, -1 on validation error.
///
/// # Safety
/// `header_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_validate(header_json: *const c_char) -> i32 {
    clear_last_error();

    let Some(hj) = c_str_to_str(header_json) else {
        set_last_error("divi_ideas_header_validate: invalid header_json");
        return -1;
    };

    let header: Header = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_validate: {e}"));
            return -1;
        }
    };

    match header.validate() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_validate: {e}"));
            -1
        }
    }
}

/// Return a header with updated modified timestamp.
///
/// Returns JSON Header. Caller must free via `divi_free_string`.
///
/// # Safety
/// `header_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_touched(header_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(hj) = c_str_to_str(header_json) else {
        set_last_error("divi_ideas_header_touched: invalid header_json");
        return std::ptr::null_mut();
    };

    let header: Header = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_touched: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&header.touched())
}

/// Check if Babel obfuscation is enabled.
///
/// Returns false on parse error.
///
/// # Safety
/// `header_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_is_babel_enabled(header_json: *const c_char) -> bool {
    let Some(hj) = c_str_to_str(header_json) else {
        return false;
    };
    let header: Header = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(_) => return false,
    };
    header.is_babel_enabled()
}

/// Check if the header has a password key slot.
///
/// Returns false on parse error.
///
/// # Safety
/// `header_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_has_password_slot(header_json: *const c_char) -> bool {
    let Some(hj) = c_str_to_str(header_json) else {
        return false;
    };
    let header: Header = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(_) => return false,
    };
    header.has_password_slot()
}

/// Get the list of public key recipients who can unlock this idea.
///
/// Returns a JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `header_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_shared_with(header_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(hj) = c_str_to_str(header_json) else {
        set_last_error("divi_ideas_header_shared_with: invalid header_json");
        return std::ptr::null_mut();
    };

    let header: Header = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_shared_with: {e}"));
            return std::ptr::null_mut();
        }
    };

    let recipients = header.shared_with();
    json_to_c(&recipients)
}

/// Get the file extension for this idea.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`, or null on error.
///
/// # Safety
/// `header_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_header_file_extension(
    header_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(hj) = c_str_to_str(header_json) else {
        set_last_error("divi_ideas_header_file_extension: invalid header_json");
        return std::ptr::null_mut();
    };

    let header: Header = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_ideas_header_file_extension: {e}"));
            return std::ptr::null_mut();
        }
    };

    string_to_c(header.file_extension())
}

// ===================================================================
// Wave 1: Validation (3 functions)
// ===================================================================

/// Validate a digit type string.
///
/// Returns 0 if valid, -1 if invalid.
///
/// # Safety
/// `type_str` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_validate_digit_type(type_str: *const c_char) -> i32 {
    clear_last_error();

    let Some(ts) = c_str_to_str(type_str) else {
        set_last_error("divi_ideas_validate_digit_type: invalid type_str");
        return -1;
    };

    match validation::validate_digit_type(ts) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_validate_digit_type: {e}"));
            -1
        }
    }
}

/// Validate a property key string.
///
/// Returns 0 if valid, -1 if invalid.
///
/// # Safety
/// `key` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_validate_property_key(key: *const c_char) -> i32 {
    clear_last_error();

    let Some(k) = c_str_to_str(key) else {
        set_last_error("divi_ideas_validate_property_key: invalid key");
        return -1;
    };

    match validation::validate_property_key(k) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_validate_property_key: {e}"));
            -1
        }
    }
}

/// Validate a local bond path.
///
/// Returns 0 if valid, -1 if invalid.
///
/// # Safety
/// `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_validate_local_bond_path(path: *const c_char) -> i32 {
    clear_last_error();

    let Some(p) = c_str_to_str(path) else {
        set_last_error("divi_ideas_validate_local_bond_path: invalid path");
        return -1;
    };

    match validation::validate_local_bond_path(p) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_validate_local_bond_path: {e}"));
            -1
        }
    }
}

// ===================================================================
// Wave 2: SchemaRegistry (12 functions)
// ===================================================================

/// Create a new empty SchemaRegistry.
///
/// Free with `divi_ideas_schema_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_ideas_schema_registry_new() -> *mut IdeasSchemaRegistry {
    Box::into_raw(Box::new(IdeasSchemaRegistry(Mutex::new(
        SchemaRegistry::new(),
    ))))
}

/// Free a SchemaRegistry.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_ideas_schema_registry_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_free(ptr: *mut IdeasSchemaRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register a schema from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `schema_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_register(
    registry: *const IdeasSchemaRegistry,
    schema_json: *const c_char,
) -> i32 {
    clear_last_error();
    let registry = unsafe { &*registry };

    let Some(sj) = c_str_to_str(schema_json) else {
        set_last_error("divi_ideas_schema_registry_register: invalid schema_json");
        return -1;
    };

    let schema_obj: DigitSchema = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_ideas_schema_registry_register: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    guard.register(schema_obj);
    0
}

/// Look up a schema by versioned type string.
///
/// Returns JSON DigitSchema, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `versioned_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_get(
    registry: *const IdeasSchemaRegistry,
    versioned_type: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };

    let Some(vt) = c_str_to_str(versioned_type) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(vt) {
        Some(s) => json_to_c(s),
        None => std::ptr::null_mut(),
    }
}

/// Look up the latest version of a schema for a given digit type.
///
/// Returns JSON DigitSchema, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `digit_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_latest(
    registry: *const IdeasSchemaRegistry,
    digit_type: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };

    let Some(dt) = c_str_to_str(digit_type) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    match guard.latest(dt) {
        Some(s) => json_to_c(s),
        None => std::ptr::null_mut(),
    }
}

/// Get all registered schemas as a JSON array.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_all(
    registry: *const IdeasSchemaRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    let schemas: Vec<&DigitSchema> = guard.all().collect();
    json_to_c(&schemas)
}

/// Get all versions of a given digit type as a JSON array.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `digit_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_versions_of(
    registry: *const IdeasSchemaRegistry,
    digit_type: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };

    let Some(dt) = c_str_to_str(digit_type) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    let versions = guard.versions_of(dt);
    json_to_c(&versions)
}

/// Get the number of schemas in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_len(
    registry: *const IdeasSchemaRegistry,
) -> u32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as u32
}

/// Whether the registry is empty.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_is_empty(
    registry: *const IdeasSchemaRegistry,
) -> bool {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.is_empty()
}

/// Resolve a schema by flattening its extends chain using the registry.
///
/// Returns JSON DigitSchema (resolved). Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `schema_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_registry_resolve(
    registry: *const IdeasSchemaRegistry,
    schema_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let registry = unsafe { &*registry };

    let Some(sj) = c_str_to_str(schema_json) else {
        set_last_error("divi_ideas_schema_registry_resolve: invalid schema_json");
        return std::ptr::null_mut();
    };

    let schema_obj: DigitSchema = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_ideas_schema_registry_resolve: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let resolved = guard.resolve(&schema_obj);
    json_to_c(&resolved)
}

// ===================================================================
// Wave 2: Schema Validation (4 functions)
// ===================================================================

/// Create a new schema at version 1 for the given digit type.
///
/// Returns JSON DigitSchema. Caller must free via `divi_free_string`.
///
/// # Safety
/// `digit_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_new(digit_type: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dt) = c_str_to_str(digit_type) else {
        set_last_error("divi_ideas_schema_new: invalid digit_type");
        return std::ptr::null_mut();
    };

    let schema_obj = DigitSchema::new(dt.into());
    json_to_c(&schema_obj)
}

/// Validate the schema itself (check defaults match types, property key rules).
///
/// Returns a JSON array of validation errors, or "[]" if valid.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `schema_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_validate_self(
    schema_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(sj) = c_str_to_str(schema_json) else {
        set_last_error("divi_ideas_schema_validate_self: invalid schema_json");
        return std::ptr::null_mut();
    };

    let schema_obj: DigitSchema = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_ideas_schema_validate_self: {e}"));
            return std::ptr::null_mut();
        }
    };

    let errors = schema_obj.validate_self();
    json_to_c(&errors)
}

/// Validate a digit against a schema.
///
/// Returns null if valid, or a JSON array of validation errors if invalid.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_validate_digit(
    digit_json: *const c_char,
    schema_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_schema_validate_digit: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(schema_json) else {
        set_last_error("divi_ideas_schema_validate_digit: invalid schema_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_schema_validate_digit: digit parse error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let schema_obj: DigitSchema = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_ideas_schema_validate_digit: schema parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    match schema::validate(&digit, &schema_obj) {
        Ok(()) => std::ptr::null_mut(), // valid = null (no errors)
        Err(errors) => json_to_c(&errors),
    }
}

/// Validate a digit against a schema with extends chain resolution.
///
/// Returns null if valid, or a JSON array of validation errors if invalid.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All pointers must be valid. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_schema_validate_composed(
    digit_json: *const c_char,
    schema_json: *const c_char,
    registry: *const IdeasSchemaRegistry,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_schema_validate_composed: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(schema_json) else {
        set_last_error("divi_ideas_schema_validate_composed: invalid schema_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!(
                "divi_ideas_schema_validate_composed: digit parse error: {e}"
            ));
            return std::ptr::null_mut();
        }
    };
    let schema_obj: DigitSchema = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!(
                "divi_ideas_schema_validate_composed: schema parse error: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);

    match schema::validate_composed(&digit, &schema_obj, &guard) {
        Ok(()) => std::ptr::null_mut(),
        Err(errors) => json_to_c(&errors),
    }
}

// ===================================================================
// Wave 2: Accessibility (2 functions)
// ===================================================================

/// Attach accessibility metadata to a digit.
///
/// Returns JSON Digit with a11y_ properties set. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_accessibility(
    digit_json: *const c_char,
    meta_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_accessibility: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(mj) = c_str_to_str(meta_json) else {
        set_last_error("divi_ideas_digit_with_accessibility: invalid meta_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_digit_with_accessibility: invalid author");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_accessibility: {e}"));
            return std::ptr::null_mut();
        }
    };
    let meta: AccessibilityMetadata = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_accessibility: meta parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let updated = accessibility::with_accessibility(digit, &meta, author_str);
    json_to_c(&updated)
}

/// Extract accessibility metadata from a digit.
///
/// Returns JSON AccessibilityMetadata, or null if no a11y metadata present.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_accessibility(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_accessibility: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_accessibility: {e}"));
            return std::ptr::null_mut();
        }
    };

    match digit.accessibility() {
        Some(meta) => json_to_c(&meta),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// Wave 2: Bonds (4 functions)
// ===================================================================

/// Create empty bonds as JSON.
///
/// Returns JSON Bonds. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_ideas_bonds_new() -> *mut c_char {
    json_to_c(&Bonds::new())
}

/// Count total references across all bond types.
///
/// Returns 0 on parse error.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_bonds_count(json: *const c_char) -> u32 {
    let Some(js) = c_str_to_str(json) else {
        return 0;
    };
    let bonds: Bonds = match serde_json::from_str(js) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    bonds.count() as u32
}

/// Check if bonds are empty.
///
/// Returns true on parse error.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_bonds_is_empty(json: *const c_char) -> bool {
    let Some(js) = c_str_to_str(json) else {
        return true;
    };
    let bonds: Bonds = match serde_json::from_str(js) {
        Ok(b) => b,
        Err(_) => return true,
    };
    bonds.is_empty()
}

/// Validate bonds (check local bond paths).
///
/// Returns 0 on success, -1 on validation error.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_bonds_validate(json: *const c_char) -> i32 {
    clear_last_error();

    let Some(js) = c_str_to_str(json) else {
        set_last_error("divi_ideas_bonds_validate: invalid json");
        return -1;
    };

    let bonds: Bonds = match serde_json::from_str(js) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(format!("divi_ideas_bonds_validate: {e}"));
            return -1;
        }
    };

    match bonds.validate() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_ideas_bonds_validate: {e}"));
            -1
        }
    }
}

// ===================================================================
// Wave 2: Binding (2 functions)
// ===================================================================

/// Attach a data binding to a digit.
///
/// Returns JSON Digit with binding_ properties set. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_with_binding(
    digit_json: *const c_char,
    binding_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_with_binding: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(bj) = c_str_to_str(binding_json) else {
        set_last_error("divi_ideas_digit_with_binding: invalid binding_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_digit_with_binding: invalid author");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_binding: {e}"));
            return std::ptr::null_mut();
        }
    };
    let data_binding: DataBinding = match serde_json::from_str(bj) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_with_binding: binding parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let updated = binding::with_data_binding(digit, &data_binding, author_str);
    json_to_c(&updated)
}

/// Extract data binding from a digit.
///
/// Returns JSON DataBinding, or null if no binding present.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `digit_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_digit_parse_binding(digit_json: *const c_char) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_digit_parse_binding: invalid digit_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_ideas_digit_parse_binding: {e}"));
            return std::ptr::null_mut();
        }
    };

    match binding::parse_data_binding(&digit) {
        Some(b) => json_to_c(&b),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// Wave 2: CRDT Operations (3 functions)
// ===================================================================

/// Create an Insert CRDT operation.
///
/// `digit_json` is a raw JSON value for the digit content.
/// `parent_id` is a UUID string, or null.
/// `vector_json` is a JSON VectorClock.
/// Returns JSON DigitOperation. Caller must free via `divi_free_string`.
///
/// # Safety
/// C strings must be valid. `parent_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_operation_insert(
    digit_json: *const c_char,
    parent_id: *const c_char,
    author: *const c_char,
    vector_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(dj) = c_str_to_str(digit_json) else {
        set_last_error("divi_ideas_operation_insert: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_operation_insert: invalid author");
        return std::ptr::null_mut();
    };
    let Some(vj) = c_str_to_str(vector_json) else {
        set_last_error("divi_ideas_operation_insert: invalid vector_json");
        return std::ptr::null_mut();
    };

    let digit_value: serde_json::Value = match serde_json::from_str(dj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_insert: digit parse error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let vector: VectorClock = match serde_json::from_str(vj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_insert: vector parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let parent = c_str_to_str(parent_id).and_then(|s| uuid::Uuid::parse_str(s).ok());

    let op = DigitOperation::insert(digit_value, parent, author_str.into(), vector);
    json_to_c(&op)
}

/// Create an Update CRDT operation.
///
/// `digit_id` is the target digit UUID.
/// `field` is the property name being updated.
/// `old_json` and `new_json` are JSON Value representations.
/// Returns JSON DigitOperation. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_operation_update(
    digit_id: *const c_char,
    field: *const c_char,
    old_json: *const c_char,
    new_json: *const c_char,
    author: *const c_char,
    vector_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(did) = c_str_to_str(digit_id) else {
        set_last_error("divi_ideas_operation_update: invalid digit_id");
        return std::ptr::null_mut();
    };
    let Some(field_str) = c_str_to_str(field) else {
        set_last_error("divi_ideas_operation_update: invalid field");
        return std::ptr::null_mut();
    };
    let Some(oj) = c_str_to_str(old_json) else {
        set_last_error("divi_ideas_operation_update: invalid old_json");
        return std::ptr::null_mut();
    };
    let Some(nj) = c_str_to_str(new_json) else {
        set_last_error("divi_ideas_operation_update: invalid new_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_operation_update: invalid author");
        return std::ptr::null_mut();
    };
    let Some(vj) = c_str_to_str(vector_json) else {
        set_last_error("divi_ideas_operation_update: invalid vector_json");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(did) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_update: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };
    let old_value: Value = match serde_json::from_str(oj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_update: old_json parse error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let new_value: Value = match serde_json::from_str(nj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_update: new_json parse error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let vector: VectorClock = match serde_json::from_str(vj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_update: vector parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let op = DigitOperation::update(uuid, field_str.into(), old_value, new_value, author_str.into(), vector);
    json_to_c(&op)
}

/// Create a Delete CRDT operation.
///
/// `digit_id` is the target digit UUID. `tombstone` indicates soft delete.
/// Returns JSON DigitOperation. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_ideas_operation_delete(
    digit_id: *const c_char,
    tombstone: bool,
    author: *const c_char,
    vector_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(did) = c_str_to_str(digit_id) else {
        set_last_error("divi_ideas_operation_delete: invalid digit_id");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_ideas_operation_delete: invalid author");
        return std::ptr::null_mut();
    };
    let Some(vj) = c_str_to_str(vector_json) else {
        set_last_error("divi_ideas_operation_delete: invalid vector_json");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(did) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_delete: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };
    let vector: VectorClock = match serde_json::from_str(vj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_ideas_operation_delete: vector parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let op = DigitOperation::delete(uuid, tombstone, author_str.into(), vector);
    json_to_c(&op)
}

// ===================================================================
// Wave 3: Domain Helpers — Macros for repetitive create/parse patterns
// ===================================================================

/// Macro for domain digit create functions.
/// Takes meta_json, deserializes to Meta type, calls constructor, returns JSON digit.
macro_rules! domain_create {
    ($fn_name:ident, $meta_type:ty, $constructor:path) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $fn_name(
            meta_json: *const c_char,
            author: *const c_char,
        ) -> *mut c_char {
            clear_last_error();

            let Some(mj) = c_str_to_str(meta_json) else {
                set_last_error(concat!(stringify!($fn_name), ": invalid meta_json"));
                return std::ptr::null_mut();
            };
            let Some(author_str) = c_str_to_str(author) else {
                set_last_error(concat!(stringify!($fn_name), ": invalid author"));
                return std::ptr::null_mut();
            };

            let meta: $meta_type = match serde_json::from_str(mj) {
                Ok(m) => m,
                Err(e) => {
                    set_last_error(format!(concat!(stringify!($fn_name), ": {e}"), e = e));
                    return std::ptr::null_mut();
                }
            };

            match $constructor(&meta, author_str) {
                Ok(digit) => json_to_c(&digit),
                Err(e) => {
                    set_last_error(format!(concat!(stringify!($fn_name), ": {e}"), e = e));
                    std::ptr::null_mut()
                }
            }
        }
    };
}

/// Macro for domain digit parse functions.
/// Takes digit_json, deserializes to Digit, calls parser, returns JSON meta.
macro_rules! domain_parse {
    ($fn_name:ident, $parser:path) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $fn_name(digit_json: *const c_char) -> *mut c_char {
            clear_last_error();

            let Some(dj) = c_str_to_str(digit_json) else {
                set_last_error(concat!(stringify!($fn_name), ": invalid digit_json"));
                return std::ptr::null_mut();
            };

            let digit: Digit = match serde_json::from_str(dj) {
                Ok(d) => d,
                Err(e) => {
                    set_last_error(format!(concat!(stringify!($fn_name), ": {e}"), e = e));
                    return std::ptr::null_mut();
                }
            };

            match $parser(&digit) {
                Ok(meta) => json_to_c(&meta),
                Err(e) => {
                    set_last_error(format!(concat!(stringify!($fn_name), ": {e}"), e = e));
                    std::ptr::null_mut()
                }
            }
        }
    };
}

/// Macro for domain schema functions.
/// Calls schema constructor, returns JSON schema.
macro_rules! domain_schema {
    ($fn_name:ident, $schema_fn:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $fn_name() -> *mut c_char {
            let schema_obj = $schema_fn();
            json_to_c(&schema_obj)
        }
    };
}

// ===================================================================
// Wave 3: Media (8 create/parse + 4 schemas)
// ===================================================================

domain_create!(divi_ideas_media_image_digit, ideas::media::ImageMeta, ideas::media::image_digit);
domain_parse!(divi_ideas_media_parse_image, ideas::media::parse_image_meta);

domain_create!(divi_ideas_media_audio_digit, ideas::media::AudioMeta, ideas::media::audio_digit);
domain_parse!(divi_ideas_media_parse_audio, ideas::media::parse_audio_meta);

domain_create!(divi_ideas_media_video_digit, ideas::media::VideoMeta, ideas::media::video_digit);
domain_parse!(divi_ideas_media_parse_video, ideas::media::parse_video_meta);

domain_create!(divi_ideas_media_stream_digit, ideas::media::StreamMeta, ideas::media::stream_digit);
domain_parse!(divi_ideas_media_parse_stream, ideas::media::parse_stream_meta);

// Media doesn't have schema functions in the crate, so no schema macros here.

// ===================================================================
// Wave 3: Sheet (4 create/parse + 2 schemas)
// ===================================================================

domain_create!(divi_ideas_sheet_sheet_digit, ideas::sheet::SheetMeta, ideas::sheet::sheet_digit);
domain_parse!(divi_ideas_sheet_parse_sheet, ideas::sheet::parse_sheet_meta);

domain_create!(divi_ideas_sheet_cell_digit, ideas::sheet::CellMeta, ideas::sheet::cell_digit);
domain_parse!(divi_ideas_sheet_parse_cell, ideas::sheet::parse_cell_meta);

domain_schema!(divi_ideas_sheet_sheet_schema, ideas::sheet::sheet_schema);
domain_schema!(divi_ideas_sheet_cell_schema, ideas::sheet::cell_schema);

// ===================================================================
// Wave 3: Slide (2 create/parse + 1 schema)
// ===================================================================

domain_create!(divi_ideas_slide_slide_digit, ideas::slide::SlideMeta, ideas::slide::slide_digit);
domain_parse!(divi_ideas_slide_parse_slide, ideas::slide::parse_slide_meta);

domain_schema!(divi_ideas_slide_slide_schema, ideas::slide::slide_schema);

// ===================================================================
// Wave 3: Form (14 create/parse + 7 schemas)
// ===================================================================

domain_create!(divi_ideas_form_input_digit, ideas::form::InputFieldMeta, ideas::form::input_field_digit);
domain_parse!(divi_ideas_form_parse_input, ideas::form::parse_input_field_meta);

domain_create!(divi_ideas_form_checkbox_digit, ideas::form::CheckboxMeta, ideas::form::checkbox_digit);
domain_parse!(divi_ideas_form_parse_checkbox, ideas::form::parse_checkbox_meta);

domain_create!(divi_ideas_form_radio_digit, ideas::form::RadioMeta, ideas::form::radio_digit);
domain_parse!(divi_ideas_form_parse_radio, ideas::form::parse_radio_meta);

domain_create!(divi_ideas_form_toggle_digit, ideas::form::ToggleMeta, ideas::form::toggle_digit);
domain_parse!(divi_ideas_form_parse_toggle, ideas::form::parse_toggle_meta);

domain_create!(divi_ideas_form_dropdown_digit, ideas::form::DropdownMeta, ideas::form::dropdown_digit);
domain_parse!(divi_ideas_form_parse_dropdown, ideas::form::parse_dropdown_meta);

domain_create!(divi_ideas_form_submit_digit, ideas::form::SubmitMeta, ideas::form::submit_digit);
domain_parse!(divi_ideas_form_parse_submit, ideas::form::parse_submit_meta);

domain_create!(divi_ideas_form_container_digit, ideas::form::FormMeta, ideas::form::form_digit);
domain_parse!(divi_ideas_form_parse_container, ideas::form::parse_form_meta);

domain_schema!(divi_ideas_form_input_schema, ideas::form::input_field_schema);
domain_schema!(divi_ideas_form_checkbox_schema, ideas::form::checkbox_schema);
domain_schema!(divi_ideas_form_radio_schema, ideas::form::radio_schema);
domain_schema!(divi_ideas_form_toggle_schema, ideas::form::toggle_schema);
domain_schema!(divi_ideas_form_dropdown_schema, ideas::form::dropdown_schema);
domain_schema!(divi_ideas_form_submit_schema, ideas::form::submit_schema);
domain_schema!(divi_ideas_form_container_schema, ideas::form::form_schema);

// ===================================================================
// Wave 3: RichText (16 create/parse + 8 schemas)
// ===================================================================

domain_create!(divi_ideas_richtext_heading_digit, ideas::richtext::HeadingMeta, ideas::richtext::heading_digit);
domain_parse!(divi_ideas_richtext_parse_heading, ideas::richtext::parse_heading_meta);

domain_create!(divi_ideas_richtext_paragraph_digit, ideas::richtext::ParagraphMeta, ideas::richtext::paragraph_digit);
domain_parse!(divi_ideas_richtext_parse_paragraph, ideas::richtext::parse_paragraph_meta);

domain_create!(divi_ideas_richtext_list_digit, ideas::richtext::ListMeta, ideas::richtext::list_digit);
domain_parse!(divi_ideas_richtext_parse_list, ideas::richtext::parse_list_meta);

domain_create!(divi_ideas_richtext_blockquote_digit, ideas::richtext::BlockquoteMeta, ideas::richtext::blockquote_digit);
domain_parse!(divi_ideas_richtext_parse_blockquote, ideas::richtext::parse_blockquote_meta);

domain_create!(divi_ideas_richtext_callout_digit, ideas::richtext::CalloutMeta, ideas::richtext::callout_digit);
domain_parse!(divi_ideas_richtext_parse_callout, ideas::richtext::parse_callout_meta);

domain_create!(divi_ideas_richtext_code_digit, ideas::richtext::CodeBlockMeta, ideas::richtext::code_block_digit);
domain_parse!(divi_ideas_richtext_parse_code, ideas::richtext::parse_code_block_meta);

domain_create!(divi_ideas_richtext_footnote_digit, ideas::richtext::FootnoteMeta, ideas::richtext::footnote_digit);
domain_parse!(divi_ideas_richtext_parse_footnote, ideas::richtext::parse_footnote_meta);

domain_create!(divi_ideas_richtext_citation_digit, ideas::richtext::CitationMeta, ideas::richtext::citation_digit);
domain_parse!(divi_ideas_richtext_parse_citation, ideas::richtext::parse_citation_meta);

domain_schema!(divi_ideas_richtext_heading_schema, ideas::richtext::heading_schema);
domain_schema!(divi_ideas_richtext_paragraph_schema, ideas::richtext::paragraph_schema);
domain_schema!(divi_ideas_richtext_list_schema, ideas::richtext::list_schema);
domain_schema!(divi_ideas_richtext_blockquote_schema, ideas::richtext::blockquote_schema);
domain_schema!(divi_ideas_richtext_callout_schema, ideas::richtext::callout_schema);
domain_schema!(divi_ideas_richtext_code_schema, ideas::richtext::code_block_schema);
domain_schema!(divi_ideas_richtext_footnote_schema, ideas::richtext::footnote_schema);
domain_schema!(divi_ideas_richtext_citation_schema, ideas::richtext::citation_schema);

// ===================================================================
// Wave 3: Interactive (8 create/parse + 4 schemas)
// ===================================================================

domain_create!(divi_ideas_interactive_button_digit, ideas::interactive::ButtonMeta, ideas::interactive::button_digit);
domain_parse!(divi_ideas_interactive_parse_button, ideas::interactive::parse_button_meta);

domain_create!(divi_ideas_interactive_navlink_digit, ideas::interactive::NavLinkMeta, ideas::interactive::nav_link_digit);
domain_parse!(divi_ideas_interactive_parse_navlink, ideas::interactive::parse_nav_link_meta);

domain_create!(divi_ideas_interactive_accordion_digit, ideas::interactive::AccordionMeta, ideas::interactive::accordion_digit);
domain_parse!(divi_ideas_interactive_parse_accordion, ideas::interactive::parse_accordion_meta);

domain_create!(divi_ideas_interactive_tabgroup_digit, ideas::interactive::TabGroupMeta, ideas::interactive::tab_group_digit);
domain_parse!(divi_ideas_interactive_parse_tabgroup, ideas::interactive::parse_tab_group_meta);

domain_schema!(divi_ideas_interactive_button_schema, ideas::interactive::button_schema);
domain_schema!(divi_ideas_interactive_navlink_schema, ideas::interactive::nav_link_schema);
domain_schema!(divi_ideas_interactive_accordion_schema, ideas::interactive::accordion_schema);
domain_schema!(divi_ideas_interactive_tabgroup_schema, ideas::interactive::tab_group_schema);

// ===================================================================
// Wave 3: Commerce (10 create/parse + 5 schemas)
// ===================================================================

domain_create!(divi_ideas_commerce_product_digit, ideas::commerce::ProductMeta, ideas::commerce::product_digit);
domain_parse!(divi_ideas_commerce_parse_product, ideas::commerce::parse_product_meta);

domain_create!(divi_ideas_commerce_storefront_digit, ideas::commerce::StorefrontMeta, ideas::commerce::storefront_digit);
domain_parse!(divi_ideas_commerce_parse_storefront, ideas::commerce::parse_storefront_meta);

domain_create!(divi_ideas_commerce_cart_item_digit, ideas::commerce::CartItemMeta, ideas::commerce::cart_item_digit);
domain_parse!(divi_ideas_commerce_parse_cart_item, ideas::commerce::parse_cart_item_meta);

domain_create!(divi_ideas_commerce_order_digit, ideas::commerce::OrderMeta, ideas::commerce::order_digit);
domain_parse!(divi_ideas_commerce_parse_order, ideas::commerce::parse_order_meta);

domain_create!(divi_ideas_commerce_review_digit, ideas::commerce::ReviewMeta, ideas::commerce::review_digit);
domain_parse!(divi_ideas_commerce_parse_review, ideas::commerce::parse_review_meta);

domain_schema!(divi_ideas_commerce_product_schema, ideas::commerce::product_schema);
domain_schema!(divi_ideas_commerce_storefront_schema, ideas::commerce::storefront_schema);
domain_schema!(divi_ideas_commerce_cart_item_schema, ideas::commerce::cart_item_schema);
domain_schema!(divi_ideas_commerce_order_schema, ideas::commerce::order_schema);
domain_schema!(divi_ideas_commerce_review_schema, ideas::commerce::review_schema);
