use std::ffi::c_char;
use std::sync::Mutex;

use polity::{
    ActionDescription, Amendment, AmendmentTrigger, Breach, BreachRegistry, BreachSeverity,
    BreachStatus, ConsentRecord, ConsentRegistry, ConsentScope, ConsentValidator,
    ConstitutionalReview, ConstitutionalReviewer, DutiesRegistry, Duty, Enactment,
    EnactmentRegistry, EnactorType, ImmutableFoundation, Protection, ProtectionsRegistry, Right,
    RightsRegistry, DEFAULT_OATH,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// ImmutableFoundation — stateless (compile-time constants)
// ===================================================================

/// Get the immutable rights as JSON.
///
/// Returns JSON array of `RightCategory` strings. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_immutable_rights() -> *mut c_char {
    json_to_c(&ImmutableFoundation::IMMUTABLE_RIGHTS)
}

/// Get the absolute prohibitions as JSON.
///
/// Returns JSON array of `ProhibitionType` strings. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_immutable_prohibitions() -> *mut c_char {
    json_to_c(&ImmutableFoundation::ABSOLUTE_PROHIBITIONS)
}

/// Get the three axioms as JSON.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_axioms() -> *mut c_char {
    json_to_c(&ImmutableFoundation::AXIOMS)
}

/// Check whether a description would violate the immutable foundation.
///
/// Returns 1 if it would violate, 0 if not, -1 on error.
///
/// # Safety
/// `description` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_would_violate(description: *const c_char) -> i32 {
    let Some(desc) = c_str_to_str(description) else {
        set_last_error("divi_polity_would_violate: invalid description");
        return -1;
    };
    if ImmutableFoundation::would_violate(desc) { 1 } else { 0 }
}

/// Check whether a right category is immutable.
///
/// `category_json` is a JSON `RightCategory` string (e.g. `"Dignity"`).
/// Returns 1 if immutable, 0 if not, -1 on error.
///
/// # Safety
/// `category_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_is_right_immutable(
    category_json: *const c_char,
) -> i32 {
    let Some(cj) = c_str_to_str(category_json) else {
        set_last_error("divi_polity_is_right_immutable: invalid category_json");
        return -1;
    };
    let category: polity::RightCategory = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_polity_is_right_immutable: {e}"));
            return -1;
        }
    };
    if ImmutableFoundation::is_right_immutable(&category) { 1 } else { 0 }
}

/// Check whether a prohibition type is absolute.
///
/// `prohibition_json` is a JSON `ProhibitionType` string (e.g. `"Surveillance"`).
/// Returns 1 if absolute, 0 if not, -1 on error.
///
/// # Safety
/// `prohibition_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_is_prohibition_absolute(
    prohibition_json: *const c_char,
) -> i32 {
    let Some(pj) = c_str_to_str(prohibition_json) else {
        set_last_error("divi_polity_is_prohibition_absolute: invalid prohibition_json");
        return -1;
    };
    let prohibition: polity::ProhibitionType = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_polity_is_prohibition_absolute: {e}"));
            return -1;
        }
    };
    if ImmutableFoundation::is_prohibition_absolute(&prohibition) { 1 } else { 0 }
}

// ===================================================================
// RightsRegistry — opaque pointer
// ===================================================================

pub struct PolityRightsRegistry(pub(crate) Mutex<RightsRegistry>);

/// Create an empty rights registry.
/// Free with `divi_polity_rights_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_rights_new() -> *mut PolityRightsRegistry {
    Box::into_raw(Box::new(PolityRightsRegistry(Mutex::new(
        RightsRegistry::new(),
    ))))
}

/// Create a rights registry pre-populated with Covenant rights.
/// Free with `divi_polity_rights_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_rights_new_with_covenant() -> *mut PolityRightsRegistry {
    Box::into_raw(Box::new(PolityRightsRegistry(Mutex::new(
        RightsRegistry::with_covenant_rights(),
    ))))
}

/// Free a rights registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_free(ptr: *mut PolityRightsRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register a new right.
///
/// `right_json` is a JSON `Right`. Returns UUID string on success,
/// null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `right_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_register(
    registry: *const PolityRightsRegistry,
    right_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(rj) = c_str_to_str(right_json) else {
        set_last_error("divi_polity_rights_register: invalid right_json");
        return std::ptr::null_mut();
    };

    let right: Right = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_polity_rights_register: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.register(right) {
        Ok(id) => string_to_c(id.to_string()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get a right by ID.
///
/// Returns JSON `Right`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_get(
    registry: *const PolityRightsRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_rights_get: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(&uuid) {
        Some(right) => json_to_c(right),
        None => std::ptr::null_mut(),
    }
}

/// Get rights by category.
///
/// `category_json` is a JSON `RightCategory` (e.g. `"Dignity"`).
/// Returns JSON array of `Right`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `category_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_by_category(
    registry: *const PolityRightsRegistry,
    category_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(cj) = c_str_to_str(category_json) else {
        set_last_error("divi_polity_rights_by_category: invalid category_json");
        return std::ptr::null_mut();
    };

    let category: polity::RightCategory = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_polity_rights_by_category: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let rights: Vec<&Right> = guard.by_category(category);
    json_to_c(&rights)
}

/// Find a right by name (case-insensitive).
///
/// Returns JSON `Right`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_find_by_name(
    registry: *const PolityRightsRegistry,
    name: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(name_str) = c_str_to_str(name) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    match guard.find_by_name(name_str) {
        Some(right) => json_to_c(right),
        None => std::ptr::null_mut(),
    }
}

/// Get all rights in the registry.
///
/// Returns JSON array of `Right`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_all(
    registry: *const PolityRightsRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.all())
}

/// Get all immutable rights in the registry.
///
/// Returns JSON array of `Right`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_immutable(
    registry: *const PolityRightsRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.immutable())
}

/// Remove a right (only if not immutable).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_remove(
    registry: *const PolityRightsRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_polity_rights_remove: invalid id");
        return -1;
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_rights_remove: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.remove(&uuid) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get the number of rights in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_rights_count(
    registry: *const PolityRightsRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as i32
}

// ===================================================================
// DutiesRegistry — opaque pointer
// ===================================================================

pub struct PolityDutiesRegistry(pub(crate) Mutex<DutiesRegistry>);

/// Create an empty duties registry.
/// Free with `divi_polity_duties_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_duties_new() -> *mut PolityDutiesRegistry {
    Box::into_raw(Box::new(PolityDutiesRegistry(Mutex::new(
        DutiesRegistry::new(),
    ))))
}

/// Create a duties registry pre-populated with Covenant duties.
/// Free with `divi_polity_duties_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_duties_new_with_covenant() -> *mut PolityDutiesRegistry {
    Box::into_raw(Box::new(PolityDutiesRegistry(Mutex::new(
        DutiesRegistry::with_covenant_duties(),
    ))))
}

/// Free a duties registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_free(ptr: *mut PolityDutiesRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register a new duty.
///
/// `duty_json` is a JSON `Duty`. Returns UUID string on success,
/// null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `duty_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_register(
    registry: *const PolityDutiesRegistry,
    duty_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(dj) = c_str_to_str(duty_json) else {
        set_last_error("divi_polity_duties_register: invalid duty_json");
        return std::ptr::null_mut();
    };

    let duty: Duty = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_polity_duties_register: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.register(duty) {
        Ok(id) => string_to_c(id.to_string()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get a duty by ID.
///
/// Returns JSON `Duty`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_get(
    registry: *const PolityDutiesRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_duties_get: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(&uuid) {
        Some(duty) => json_to_c(duty),
        None => std::ptr::null_mut(),
    }
}

/// Get duties by category.
///
/// `category_json` is a JSON `DutyCategory` (e.g. `"Steward"`).
/// Returns JSON array of `Duty`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `category_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_by_category(
    registry: *const PolityDutiesRegistry,
    category_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(cj) = c_str_to_str(category_json) else {
        set_last_error("divi_polity_duties_by_category: invalid category_json");
        return std::ptr::null_mut();
    };

    let category: polity::DutyCategory = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_polity_duties_by_category: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let duties: Vec<&Duty> = guard.by_category(category);
    json_to_c(&duties)
}

/// Find a duty by name (case-insensitive).
///
/// Returns JSON `Duty`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_find_by_name(
    registry: *const PolityDutiesRegistry,
    name: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(name_str) = c_str_to_str(name) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    match guard.find_by_name(name_str) {
        Some(duty) => json_to_c(duty),
        None => std::ptr::null_mut(),
    }
}

/// Get all duties in the registry.
///
/// Returns JSON array of `Duty`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_all(
    registry: *const PolityDutiesRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.all())
}

/// Get all absolute duties.
///
/// Returns JSON array of `Duty`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_absolute(
    registry: *const PolityDutiesRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.absolute())
}

/// Remove a duty (only if not immutable).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_remove(
    registry: *const PolityDutiesRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_polity_duties_remove: invalid id");
        return -1;
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_duties_remove: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.remove(&uuid) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get the number of duties in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_duties_count(
    registry: *const PolityDutiesRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as i32
}

// ===================================================================
// ProtectionsRegistry — opaque pointer
// ===================================================================

pub struct PolityProtectionsRegistry(pub(crate) Mutex<ProtectionsRegistry>);

/// Create an empty protections registry.
/// Free with `divi_polity_protections_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_protections_new() -> *mut PolityProtectionsRegistry {
    Box::into_raw(Box::new(PolityProtectionsRegistry(Mutex::new(
        ProtectionsRegistry::new(),
    ))))
}

/// Create a protections registry pre-populated with Covenant protections.
/// Free with `divi_polity_protections_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_protections_new_with_covenant() -> *mut PolityProtectionsRegistry {
    Box::into_raw(Box::new(PolityProtectionsRegistry(Mutex::new(
        ProtectionsRegistry::with_covenant_protections(),
    ))))
}

/// Free a protections registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_free(ptr: *mut PolityProtectionsRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register a new protection.
///
/// `protection_json` is a JSON `Protection`. Returns UUID string on success,
/// null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `protection_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_register(
    registry: *const PolityProtectionsRegistry,
    protection_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(pj) = c_str_to_str(protection_json) else {
        set_last_error("divi_polity_protections_register: invalid protection_json");
        return std::ptr::null_mut();
    };

    let protection: Protection = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_polity_protections_register: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.register(protection) {
        Ok(id) => string_to_c(id.to_string()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get a protection by ID.
///
/// Returns JSON `Protection`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_get(
    registry: *const PolityProtectionsRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_protections_get: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(&uuid) {
        Some(protection) => json_to_c(protection),
        None => std::ptr::null_mut(),
    }
}

/// Get protections by prohibition type.
///
/// `type_json` is a JSON `ProhibitionType` (e.g. `"Surveillance"`).
/// Returns JSON array of `Protection`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `type_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_by_type(
    registry: *const PolityProtectionsRegistry,
    type_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(tj) = c_str_to_str(type_json) else {
        set_last_error("divi_polity_protections_by_type: invalid type_json");
        return std::ptr::null_mut();
    };

    let prohibition_type: polity::ProhibitionType = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_polity_protections_by_type: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let protections: Vec<&Protection> = guard.by_type(prohibition_type);
    json_to_c(&protections)
}

/// Find a protection by name (case-insensitive).
///
/// Returns JSON `Protection`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_find_by_name(
    registry: *const PolityProtectionsRegistry,
    name: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(name_str) = c_str_to_str(name) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    match guard.find_by_name(name_str) {
        Some(protection) => json_to_c(protection),
        None => std::ptr::null_mut(),
    }
}

/// Get all protections in the registry.
///
/// Returns JSON array of `Protection`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_all(
    registry: *const PolityProtectionsRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.all())
}

/// Get all absolute protections.
///
/// Returns JSON array of `Protection`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_absolute(
    registry: *const PolityProtectionsRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.absolute())
}

/// Check whether an action violates any protection.
///
/// `action_json` is a JSON `ActionDescription`.
/// Returns JSON array of `Protection` that are violated. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `action_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_check_violation(
    registry: *const PolityProtectionsRegistry,
    action_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(aj) = c_str_to_str(action_json) else {
        set_last_error("divi_polity_protections_check_violation: invalid action_json");
        return std::ptr::null_mut();
    };

    let action: ActionDescription = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_protections_check_violation: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let violations: Vec<&Protection> = guard.check_violation(&action);
    json_to_c(&violations)
}

/// Remove a protection (only if not immutable).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_remove(
    registry: *const PolityProtectionsRegistry,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_polity_protections_remove: invalid id");
        return -1;
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_protections_remove: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.remove(&uuid) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get the number of protections in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_protections_count(
    registry: *const PolityProtectionsRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as i32
}

// ===================================================================
// ConstitutionalReviewer — stateless (borrows from registries)
//
// The reviewer has a lifetime ('a), so we do NOT make it an opaque pointer.
// Instead, we lock both registries, construct a temporary reviewer, call
// the method, and return the result. Lock ordering: rights first, then
// protections — consistent across all review functions.
// ===================================================================

/// Review an action against the Covenant.
///
/// Takes pointers to both registries and an `ActionDescription` JSON.
/// Returns JSON `ConstitutionalReview`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `rights` and `protections` must be valid pointers. `action_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_review(
    rights: *const PolityRightsRegistry,
    protections: *const PolityProtectionsRegistry,
    action_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let rights = unsafe { &*rights };
    let protections = unsafe { &*protections };

    let Some(aj) = c_str_to_str(action_json) else {
        set_last_error("divi_polity_review: invalid action_json");
        return std::ptr::null_mut();
    };

    let action: ActionDescription = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_review: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Lock ordering: rights first, then protections
    let rights_guard = lock_or_recover(&rights.0);
    let protections_guard = lock_or_recover(&protections.0);
    let reviewer = ConstitutionalReviewer::new(&rights_guard, &protections_guard);
    let review = reviewer.review(&action);
    json_to_c(&review)
}

/// Quick check: does an action violate any absolute prohibition?
///
/// Returns 1 if absolutely prohibited, 0 if not, -1 on error.
///
/// # Safety
/// `rights` and `protections` must be valid pointers. `action_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_is_absolutely_prohibited(
    rights: *const PolityRightsRegistry,
    protections: *const PolityProtectionsRegistry,
    action_json: *const c_char,
) -> i32 {
    clear_last_error();

    let rights = unsafe { &*rights };
    let protections = unsafe { &*protections };

    let Some(aj) = c_str_to_str(action_json) else {
        set_last_error("divi_polity_is_absolutely_prohibited: invalid action_json");
        return -1;
    };

    let action: ActionDescription = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_is_absolutely_prohibited: {e}"));
            return -1;
        }
    };

    // Lock ordering: rights first, then protections
    let rights_guard = lock_or_recover(&rights.0);
    let protections_guard = lock_or_recover(&protections.0);
    let reviewer = ConstitutionalReviewer::new(&rights_guard, &protections_guard);
    if reviewer.is_absolutely_prohibited(&action) { 1 } else { 0 }
}

/// Convert a review breach to a formal Breach record.
///
/// `review_json` is a JSON `ConstitutionalReview`. Returns JSON `Breach` if the review
/// contained a breach, or null if the review was clean. Caller must free via `divi_free_string`.
///
/// # Safety
/// `rights` and `protections` must be valid pointers. `review_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_review_to_breach(
    rights: *const PolityRightsRegistry,
    protections: *const PolityProtectionsRegistry,
    review_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let rights = unsafe { &*rights };
    let protections = unsafe { &*protections };

    let Some(rj) = c_str_to_str(review_json) else {
        set_last_error("divi_polity_review_to_breach: invalid review_json");
        return std::ptr::null_mut();
    };

    let review: ConstitutionalReview = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_polity_review_to_breach: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Lock ordering: rights first, then protections
    let rights_guard = lock_or_recover(&rights.0);
    let protections_guard = lock_or_recover(&protections.0);
    let reviewer = ConstitutionalReviewer::new(&rights_guard, &protections_guard);
    match reviewer.to_breach(&review) {
        Some(breach) => json_to_c(&breach),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// BreachRegistry — opaque pointer
// ===================================================================

pub struct PolityBreachRegistry(pub(crate) Mutex<BreachRegistry>);

/// Create a new breach registry.
/// Free with `divi_polity_breach_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_breach_new() -> *mut PolityBreachRegistry {
    Box::into_raw(Box::new(PolityBreachRegistry(Mutex::new(
        BreachRegistry::new(),
    ))))
}

/// Free a breach registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_free(ptr: *mut PolityBreachRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Record a breach.
///
/// `breach_json` is a JSON `Breach`. Returns UUID string.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `breach_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_record(
    registry: *const PolityBreachRegistry,
    breach_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(bj) = c_str_to_str(breach_json) else {
        set_last_error("divi_polity_breach_record: invalid breach_json");
        return std::ptr::null_mut();
    };

    let breach: Breach = match serde_json::from_str(bj) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(format!("divi_polity_breach_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    let id = guard.record(breach);
    string_to_c(id.to_string())
}

/// Get a breach by ID.
///
/// Returns JSON `Breach`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_get(
    registry: *const PolityBreachRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_breach_get: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(&uuid) {
        Some(breach) => json_to_c(breach),
        None => std::ptr::null_mut(),
    }
}

/// Update a breach's status.
///
/// `status_json` is a JSON `BreachStatus` (e.g. `"Investigating"`).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_update_status(
    registry: *const PolityBreachRegistry,
    id: *const c_char,
    status_json: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_polity_breach_update_status: invalid id");
        return -1;
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_breach_update_status: invalid UUID: {e}"));
            return -1;
        }
    };

    let Some(sj) = c_str_to_str(status_json) else {
        set_last_error("divi_polity_breach_update_status: invalid status_json");
        return -1;
    };

    let status: BreachStatus = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_polity_breach_update_status: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.update_status(&uuid, status) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get breaches by actor.
///
/// Returns JSON array of `Breach`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `actor` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_by_actor(
    registry: *const PolityBreachRegistry,
    actor: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(actor_str) = c_str_to_str(actor) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    let breaches: Vec<&Breach> = guard.by_actor(actor_str);
    json_to_c(&breaches)
}

/// Get breaches by severity.
///
/// `severity_json` is a JSON `BreachSeverity` (e.g. `"Grave"`).
/// Returns JSON array of `Breach`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `severity_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_by_severity(
    registry: *const PolityBreachRegistry,
    severity_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(sj) = c_str_to_str(severity_json) else {
        set_last_error("divi_polity_breach_by_severity: invalid severity_json");
        return std::ptr::null_mut();
    };

    let severity: BreachSeverity = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_polity_breach_by_severity: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let breaches: Vec<&Breach> = guard.by_severity(severity);
    json_to_c(&breaches)
}

/// Get all active breaches (not resolved or dismissed).
///
/// Returns JSON array of `Breach`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_active(
    registry: *const PolityBreachRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.active())
}

/// Get all foundational breaches (involving immutable foundations).
///
/// Returns JSON array of `Breach`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_foundational(
    registry: *const PolityBreachRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.foundational())
}

/// Get the number of breaches in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_breach_count(
    registry: *const PolityBreachRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as i32
}

// ===================================================================
// Amendment — JSON round-trip
// ===================================================================

/// Create a new amendment.
///
/// `trigger_json` is a JSON `AmendmentTrigger` (e.g. `"Contradiction"`).
/// Returns JSON `Amendment` on success, null if the description would violate
/// the immutable foundation. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_new(
    trigger_json: *const c_char,
    title: *const c_char,
    description: *const c_char,
    proposer: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(trigger_json) else {
        set_last_error("divi_polity_amendment_new: invalid trigger_json");
        return std::ptr::null_mut();
    };
    let Some(title_str) = c_str_to_str(title) else {
        set_last_error("divi_polity_amendment_new: invalid title");
        return std::ptr::null_mut();
    };
    let Some(desc_str) = c_str_to_str(description) else {
        set_last_error("divi_polity_amendment_new: invalid description");
        return std::ptr::null_mut();
    };
    let Some(proposer_str) = c_str_to_str(proposer) else {
        set_last_error("divi_polity_amendment_new: invalid proposer");
        return std::ptr::null_mut();
    };

    let trigger: AmendmentTrigger = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    match Amendment::new(trigger, title_str, desc_str, proposer_str) {
        Ok(amendment) => json_to_c(&amendment),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Begin deliberation on an amendment.
///
/// Takes amendment JSON, returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `amendment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_begin_deliberation(
    amendment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_begin_deliberation: invalid amendment_json");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_begin_deliberation: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = amendment.begin_deliberation() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&amendment)
}

/// Begin ratification on an amendment.
///
/// Takes amendment JSON, returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `amendment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_begin_ratification(
    amendment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_begin_ratification: invalid amendment_json");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_begin_ratification: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = amendment.begin_ratification() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&amendment)
}

/// Add support from a participant.
///
/// Takes amendment JSON + supporter, returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_add_support(
    amendment_json: *const c_char,
    supporter: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_add_support: invalid amendment_json");
        return std::ptr::null_mut();
    };
    let Some(supporter_str) = c_str_to_str(supporter) else {
        set_last_error("divi_polity_amendment_add_support: invalid supporter");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_add_support: {e}"));
            return std::ptr::null_mut();
        }
    };

    amendment.add_support(supporter_str);
    json_to_c(&amendment)
}

/// Add objection from a participant.
///
/// Takes amendment JSON + objector, returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_add_objection(
    amendment_json: *const c_char,
    objector: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_add_objection: invalid amendment_json");
        return std::ptr::null_mut();
    };
    let Some(objector_str) = c_str_to_str(objector) else {
        set_last_error("divi_polity_amendment_add_objection: invalid objector");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_add_objection: {e}"));
            return std::ptr::null_mut();
        }
    };

    amendment.add_objection(objector_str);
    json_to_c(&amendment)
}

/// Update the support ratio.
///
/// Takes amendment JSON + ratio (0.0 to 1.0), returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `amendment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_update_support(
    amendment_json: *const c_char,
    ratio: f64,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_update_support: invalid amendment_json");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_update_support: {e}"));
            return std::ptr::null_mut();
        }
    };

    amendment.update_support(ratio);
    json_to_c(&amendment)
}

/// Attempt to enact an amendment.
///
/// Takes amendment JSON, returns modified amendment JSON.
/// Returns null if threshold not met or invalid transition.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `amendment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_enact(
    amendment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_enact: invalid amendment_json");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_enact: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = amendment.enact() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&amendment)
}

/// Reject an amendment.
///
/// Takes amendment JSON, returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `amendment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_reject(
    amendment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_reject: invalid amendment_json");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_reject: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = amendment.reject() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&amendment)
}

/// Nullify an amendment (contradicts immutable foundations).
///
/// Takes amendment JSON, returns modified amendment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `amendment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_amendment_nullify(
    amendment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(aj) = c_str_to_str(amendment_json) else {
        set_last_error("divi_polity_amendment_nullify: invalid amendment_json");
        return std::ptr::null_mut();
    };

    let mut amendment: Amendment = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_polity_amendment_nullify: {e}"));
            return std::ptr::null_mut();
        }
    };

    amendment.nullify();
    json_to_c(&amendment)
}

// ===================================================================
// Enactment + EnactmentRegistry — hybrid (JSON + opaque pointer)
// ===================================================================

/// Create a new enactment.
///
/// `enactor_type_json` is a JSON `EnactorType` (e.g. `"Person"`).
/// Returns JSON `Enactment`. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_new(
    enactor: *const c_char,
    enactor_type_json: *const c_char,
    affirmation: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(enactor_str) = c_str_to_str(enactor) else {
        set_last_error("divi_polity_enactment_new: invalid enactor");
        return std::ptr::null_mut();
    };
    let Some(etj) = c_str_to_str(enactor_type_json) else {
        set_last_error("divi_polity_enactment_new: invalid enactor_type_json");
        return std::ptr::null_mut();
    };
    let Some(affirmation_str) = c_str_to_str(affirmation) else {
        set_last_error("divi_polity_enactment_new: invalid affirmation");
        return std::ptr::null_mut();
    };

    let enactor_type: EnactorType = match serde_json::from_str(etj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_polity_enactment_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let enactment = Enactment::new(enactor_str, enactor_type, affirmation_str);
    json_to_c(&enactment)
}

/// Suspend an enactment.
///
/// Takes enactment JSON, returns modified enactment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `enactment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_suspend(
    enactment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ej) = c_str_to_str(enactment_json) else {
        set_last_error("divi_polity_enactment_suspend: invalid enactment_json");
        return std::ptr::null_mut();
    };

    let mut enactment: Enactment = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_polity_enactment_suspend: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = enactment.suspend() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&enactment)
}

/// Reactivate a suspended enactment.
///
/// Takes enactment JSON, returns modified enactment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `enactment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_reactivate(
    enactment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ej) = c_str_to_str(enactment_json) else {
        set_last_error("divi_polity_enactment_reactivate: invalid enactment_json");
        return std::ptr::null_mut();
    };

    let mut enactment: Enactment = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_polity_enactment_reactivate: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = enactment.reactivate() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&enactment)
}

/// Withdraw from the Covenant.
///
/// Takes enactment JSON, returns modified enactment JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `enactment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_withdraw(
    enactment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ej) = c_str_to_str(enactment_json) else {
        set_last_error("divi_polity_enactment_withdraw: invalid enactment_json");
        return std::ptr::null_mut();
    };

    let mut enactment: Enactment = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_polity_enactment_withdraw: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = enactment.withdraw() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&enactment)
}

pub struct PolityEnactmentRegistry(pub(crate) Mutex<EnactmentRegistry>);

/// Create a new enactment registry.
/// Free with `divi_polity_enactment_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_enactment_registry_new() -> *mut PolityEnactmentRegistry {
    Box::into_raw(Box::new(PolityEnactmentRegistry(Mutex::new(
        EnactmentRegistry::new(),
    ))))
}

/// Free an enactment registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_registry_free(
    ptr: *mut PolityEnactmentRegistry,
) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Record an enactment in the registry.
///
/// `enactment_json` is a JSON `Enactment`. Returns UUID string on success,
/// null on error (e.g. duplicate active enactment). Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `enactment_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_registry_record(
    registry: *const PolityEnactmentRegistry,
    enactment_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(ej) = c_str_to_str(enactment_json) else {
        set_last_error("divi_polity_enactment_registry_record: invalid enactment_json");
        return std::ptr::null_mut();
    };

    let enactment: Enactment = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_polity_enactment_registry_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.record(enactment) {
        Ok(id) => string_to_c(id.to_string()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get an enactment by ID.
///
/// Returns JSON `Enactment`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_registry_get(
    registry: *const PolityEnactmentRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_enactment_registry_get: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(&uuid) {
        Some(enactment) => json_to_c(enactment),
        None => std::ptr::null_mut(),
    }
}

/// Check whether an enactor has an active enactment.
///
/// Returns 1 if enacted, 0 if not, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `enactor` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_registry_is_enacted(
    registry: *const PolityEnactmentRegistry,
    enactor: *const c_char,
) -> i32 {
    let registry = unsafe { &*registry };
    let Some(enactor_str) = c_str_to_str(enactor) else {
        set_last_error("divi_polity_enactment_registry_is_enacted: invalid enactor");
        return -1;
    };

    let guard = lock_or_recover(&registry.0);
    if guard.is_enacted(enactor_str) { 1 } else { 0 }
}

/// Get all active enactments.
///
/// Returns JSON array of `Enactment`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_registry_active(
    registry: *const PolityEnactmentRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.active())
}

/// Get the number of enactments in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_enactment_registry_count(
    registry: *const PolityEnactmentRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as i32
}

/// Get the default oath of enactment.
///
/// Returns a C string. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_default_oath() -> *mut c_char {
    string_to_c(DEFAULT_OATH.to_string())
}

// ===================================================================
// Consent + ConsentRegistry — hybrid (JSON + opaque pointer)
// ===================================================================

/// Create a new consent record.
///
/// `scope_json` is a JSON `ConsentScope`.
/// Returns JSON `ConsentRecord`. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_new(
    grantor: *const c_char,
    recipient: *const c_char,
    scope_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(grantor_str) = c_str_to_str(grantor) else {
        set_last_error("divi_polity_consent_new: invalid grantor");
        return std::ptr::null_mut();
    };
    let Some(recipient_str) = c_str_to_str(recipient) else {
        set_last_error("divi_polity_consent_new: invalid recipient");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(scope_json) else {
        set_last_error("divi_polity_consent_new: invalid scope_json");
        return std::ptr::null_mut();
    };

    let scope: ConsentScope = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_polity_consent_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let consent = ConsentRecord::new(grantor_str, recipient_str, scope);
    json_to_c(&consent)
}

/// Revoke a consent record.
///
/// Takes consent JSON + reason, returns modified consent JSON.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_revoke(
    consent_json: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(consent_json) else {
        set_last_error("divi_polity_consent_revoke: invalid consent_json");
        return std::ptr::null_mut();
    };
    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error("divi_polity_consent_revoke: invalid reason");
        return std::ptr::null_mut();
    };

    let mut consent: ConsentRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_polity_consent_revoke: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = consent.revoke(reason_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&consent)
}

pub struct PolityConsentRegistry(pub(crate) Mutex<ConsentRegistry>);

/// Create a new consent registry.
/// Free with `divi_polity_consent_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_polity_consent_registry_new() -> *mut PolityConsentRegistry {
    Box::into_raw(Box::new(PolityConsentRegistry(Mutex::new(
        ConsentRegistry::new(),
    ))))
}

/// Free a consent registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_free(ptr: *mut PolityConsentRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Record a consent in the registry.
///
/// `consent_json` is a JSON `ConsentRecord`. Returns UUID string.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `consent_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_record(
    registry: *const PolityConsentRegistry,
    consent_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(cj) = c_str_to_str(consent_json) else {
        set_last_error("divi_polity_consent_registry_record: invalid consent_json");
        return std::ptr::null_mut();
    };

    let consent: ConsentRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_polity_consent_registry_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&registry.0);
    let id = guard.record(consent);
    string_to_c(id.to_string())
}

/// Get a consent record by ID.
///
/// Returns JSON `ConsentRecord`, or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_get(
    registry: *const PolityConsentRegistry,
    id: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_consent_registry_get: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    match guard.get(&uuid) {
        Some(consent) => json_to_c(consent),
        None => std::ptr::null_mut(),
    }
}

/// Get all consent records by grantor.
///
/// Returns JSON array of `ConsentRecord`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `grantor` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_by_grantor(
    registry: *const PolityConsentRegistry,
    grantor: *const c_char,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let Some(grantor_str) = c_str_to_str(grantor) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&registry.0);
    let records: Vec<&ConsentRecord> = guard.by_grantor(grantor_str);
    json_to_c(&records)
}

/// Revoke a consent record in the registry.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_revoke(
    registry: *const PolityConsentRegistry,
    id: *const c_char,
    reason: *const c_char,
) -> i32 {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_polity_consent_registry_revoke: invalid id");
        return -1;
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_polity_consent_registry_revoke: invalid UUID: {e}"));
            return -1;
        }
    };

    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error("divi_polity_consent_registry_revoke: invalid reason");
        return -1;
    };

    let mut guard = lock_or_recover(&registry.0);
    match guard.revoke(&uuid, reason_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get all active consent records.
///
/// Returns JSON array of `ConsentRecord`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_active(
    registry: *const PolityConsentRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    json_to_c(&guard.active())
}

/// Validate consent for a given scope.
///
/// `scope_json` is a JSON `ConsentScope`.
/// Returns JSON `ConsentValidation`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_validate(
    registry: *const PolityConsentRegistry,
    grantor: *const c_char,
    recipient: *const c_char,
    scope_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let registry = unsafe { &*registry };
    let Some(grantor_str) = c_str_to_str(grantor) else {
        set_last_error("divi_polity_consent_registry_validate: invalid grantor");
        return std::ptr::null_mut();
    };
    let Some(recipient_str) = c_str_to_str(recipient) else {
        set_last_error("divi_polity_consent_registry_validate: invalid recipient");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(scope_json) else {
        set_last_error("divi_polity_consent_registry_validate: invalid scope_json");
        return std::ptr::null_mut();
    };

    let scope: ConsentScope = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_polity_consent_registry_validate: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let validation = ConsentValidator::validate(&guard, grantor_str, recipient_str, &scope);
    json_to_c(&validation)
}

/// Get the number of consent records in the registry.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_polity_consent_registry_count(
    registry: *const PolityConsentRegistry,
) -> i32 {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len() as i32
}
