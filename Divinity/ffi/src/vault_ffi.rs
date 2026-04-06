use std::ffi::c_char;
use std::sync::Mutex;

use uuid::Uuid;
use vault::CollectiveRole;

use crate::helpers::{bytes_to_owned, c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// Thread-safe wrapper (Vault uses &mut self, so we add Mutex at the FFI
// boundary to match the opaque-pointer pattern used throughout Divinity)
// ---------------------------------------------------------------------------

pub struct DiviVault(pub(crate) Mutex<vault::Vault>);

// ===================================================================
// Lifecycle
// ===================================================================

/// Create a new locked vault.
/// Free with `divi_vault_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_vault_new() -> *mut DiviVault {
    Box::into_raw(Box::new(DiviVault(Mutex::new(vault::Vault::new()))))
}

/// Free a vault. If the vault is still unlocked, locks it first.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_vault_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_free(ptr: *mut DiviVault) {
    if !ptr.is_null() {
        // Lock the vault before dropping to zero keys from memory.
        let wrapper = unsafe { &*ptr };
        let mut guard = lock_or_recover(&wrapper.0);
        if guard.is_unlocked() {
            let _ = guard.lock();
        }
        drop(guard);
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Whether the vault is currently unlocked.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_is_unlocked(vault: *const DiviVault) -> bool {
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);
    guard.is_unlocked()
}

// ===================================================================
// Lock / Unlock
// ===================================================================

/// Unlock the vault with a password and root directory path.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `password` and `root_path` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_unlock(
    vault: *const DiviVault,
    password: *const c_char,
    root_path: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(password_str) = c_str_to_str(password) else {
        set_last_error("divi_vault_unlock: invalid password string");
        return -1;
    };

    let Some(root_path_str) = c_str_to_str(root_path) else {
        set_last_error("divi_vault_unlock: invalid root_path string");
        return -1;
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.unlock(password_str, std::path::PathBuf::from(root_path_str)) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_unlock: {e}"));
            -1
        }
    }
}

/// Lock the vault — zeros all keys and closes the manifest.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_lock(vault: *const DiviVault) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };
    let mut guard = lock_or_recover(&vault.0);

    match guard.lock() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_lock: {e}"));
            -1
        }
    }
}

// ===================================================================
// Key Derivation
// ===================================================================

/// Derive the soul encryption key from the vault's internal master key.
///
/// The vault must be unlocked. Output is a 32-byte AES-256-GCM key
/// suitable for use with `divi_crown_soul_create_encrypted` and
/// `divi_crown_soul_load_encrypted`.
///
/// Output written to `out_key`/`out_key_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `out_key` and `out_key_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_soul_key(
    vault: *const DiviVault,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.soul_key() {
        Ok(key) => {
            let key_vec = key.expose().to_vec();
            let (kp, kl) = bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
            }
            0
        }
        Err(e) => {
            set_last_error(format!("divi_vault_soul_key: {e}"));
            -1
        }
    }
}

// ===================================================================
// Manifest Operations
// ===================================================================

/// Register a .idea entry in the manifest.
///
/// `entry_json` is a JSON ManifestEntry.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `entry_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_register_idea(
    vault: *const DiviVault,
    entry_json: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(json_str) = c_str_to_str(entry_json) else {
        set_last_error("divi_vault_register_idea: invalid entry_json");
        return -1;
    };

    let entry: vault::ManifestEntry = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_vault_register_idea: JSON parse error: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.register_idea(entry) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_register_idea: {e}"));
            -1
        }
    }
}

/// Remove a .idea entry from the manifest by ID.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_unregister_idea(
    vault: *const DiviVault,
    id: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_vault_unregister_idea: invalid id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_unregister_idea: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.unregister_idea(&uuid) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_unregister_idea: {e}"));
            -1
        }
    }
}

/// Get a manifest entry by ID.
///
/// Returns JSON ManifestEntry, or null if not found or on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_get_idea(
    vault: *const DiviVault,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_vault_get_idea: invalid id");
        return std::ptr::null_mut();
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_get_idea: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&vault.0);
    match guard.get_idea(&uuid) {
        Ok(Some(entry)) => json_to_c(entry),
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            set_last_error(format!("divi_vault_get_idea: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get a manifest entry by relative path.
///
/// Returns JSON ManifestEntry, or null if not found or on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_get_idea_by_path(
    vault: *const DiviVault,
    path: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_vault_get_idea_by_path: invalid path");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&vault.0);
    match guard.get_idea_by_path(path_str) {
        Ok(Some(entry)) => json_to_c(entry),
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            set_last_error(format!("divi_vault_get_idea_by_path: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// List ideas matching a filter.
///
/// `filter_json` is a JSON IdeaFilter. Returns a JSON array of ManifestEntry.
/// Returns null on error. The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `filter_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_list_ideas(
    vault: *const DiviVault,
    filter_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(json_str) = c_str_to_str(filter_json) else {
        set_last_error("divi_vault_list_ideas: invalid filter_json");
        return std::ptr::null_mut();
    };

    let filter: vault::IdeaFilter = match serde_json::from_str(json_str) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_vault_list_ideas: JSON parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&vault.0);
    match guard.list_ideas(&filter) {
        Ok(entries) => json_to_c(&entries),
        Err(e) => {
            set_last_error(format!("divi_vault_list_ideas: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// List ideas in a folder (path prefix match).
///
/// Returns a JSON array of ManifestEntry. Returns null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `folder` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_list_ideas_in_folder(
    vault: *const DiviVault,
    folder: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(folder_str) = c_str_to_str(folder) else {
        set_last_error("divi_vault_list_ideas_in_folder: invalid folder");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&vault.0);
    match guard.list_ideas_in_folder(folder_str) {
        Ok(entries) => json_to_c(&entries),
        Err(e) => {
            set_last_error(format!("divi_vault_list_ideas_in_folder: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the number of registered ideas.
///
/// Returns the count, or -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_idea_count(vault: *const DiviVault) -> i64 {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.idea_count() {
        Ok(count) => count as i64,
        Err(e) => {
            set_last_error(format!("divi_vault_idea_count: {e}"));
            -1
        }
    }
}

// ===================================================================
// Encryption
// ===================================================================

/// Encrypt data using the content key for a specific idea.
///
/// Returns 0 on success, -1 on error. Output written to `out_data`/`out_len`.
/// The caller must free `out_data` via `divi_free_bytes`.
///
/// # Safety
/// `vault` must be a valid pointer. `data` must be valid for `data_len` bytes.
/// `idea_id` must be a valid C string (UUID). `out_data` and `out_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_encrypt_for_idea(
    vault: *const DiviVault,
    data: *const u8,
    data_len: usize,
    idea_id: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_vault_encrypt_for_idea: invalid idea_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_encrypt_for_idea: invalid UUID: {e}"));
            return -1;
        }
    };

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.encrypt_for_idea(slice, &uuid) {
        Ok(encrypted) => {
            let (ptr, len) = bytes_to_owned(encrypted);
            unsafe {
                *out_data = ptr;
                *out_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(format!("divi_vault_encrypt_for_idea: {e}"));
            -1
        }
    }
}

/// Decrypt data using the content key for a specific idea.
///
/// Returns 0 on success, -1 on error. Output written to `out_data`/`out_len`.
/// The caller must free `out_data` via `divi_free_bytes`.
///
/// # Safety
/// `vault` must be a valid pointer. `data` must be valid for `data_len` bytes.
/// `idea_id` must be a valid C string (UUID). `out_data` and `out_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_decrypt_for_idea(
    vault: *const DiviVault,
    data: *const u8,
    data_len: usize,
    idea_id: *const c_char,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_vault_decrypt_for_idea: invalid idea_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_decrypt_for_idea: invalid UUID: {e}"));
            return -1;
        }
    };

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.decrypt_for_idea(slice, &uuid) {
        Ok(decrypted) => {
            let (ptr, len) = bytes_to_owned(decrypted);
            unsafe {
                *out_data = ptr;
                *out_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(format!("divi_vault_decrypt_for_idea: {e}"));
            -1
        }
    }
}

/// Get the content key for a specific idea.
///
/// Returns 0 on success, -1 on error. Output written to `out_key`/`out_key_len`.
/// The caller must free `out_key` via `divi_free_bytes`.
///
/// # Safety
/// `vault` must be a valid pointer. `idea_id` must be a valid C string (UUID).
/// `out_key` and `out_key_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_content_key(
    vault: *const DiviVault,
    idea_id: *const c_char,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_vault_content_key: invalid idea_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_content_key: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.content_key(&uuid) {
        Ok(secure_data) => {
            let (ptr, len) = bytes_to_owned(secure_data.expose().to_vec());
            unsafe {
                *out_key = ptr;
                *out_key_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(format!("divi_vault_content_key: {e}"));
            -1
        }
    }
}

/// Get the vocabulary seed for Babel obfuscation.
///
/// Returns 0 on success, -1 on error. Output written to `out_seed`/`out_seed_len`.
/// The caller must free `out_seed` via `divi_free_bytes`.
///
/// # Safety
/// `vault` must be a valid pointer. `out_seed` and `out_seed_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_vocabulary_seed(
    vault: *const DiviVault,
    out_seed: *mut *mut u8,
    out_seed_len: *mut usize,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.vocabulary_seed() {
        Ok(secure_data) => {
            let (ptr, len) = bytes_to_owned(secure_data.expose().to_vec());
            unsafe {
                *out_seed = ptr;
                *out_seed_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(format!("divi_vault_vocabulary_seed: {e}"));
            -1
        }
    }
}

// ===================================================================
// Collectives
// ===================================================================

/// Create a new collective. Generates a random 256-bit key.
///
/// Returns JSON Collective on success. Returns null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `name` and `owner_pubkey` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_create_collective(
    vault: *const DiviVault,
    name: *const c_char,
    owner_pubkey: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_vault_create_collective: invalid name");
        return std::ptr::null_mut();
    };

    let Some(pubkey_str) = c_str_to_str(owner_pubkey) else {
        set_last_error("divi_vault_create_collective: invalid owner_pubkey");
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.create_collective(name_str.to_string(), pubkey_str.to_string()) {
        Ok(collective) => json_to_c(collective),
        Err(e) => {
            set_last_error(format!("divi_vault_create_collective: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Join an existing collective with a key received externally.
///
/// `key_ptr`/`key_len` is the raw 256-bit collective key.
/// `role_json` is a JSON CollectiveRole (e.g., `"member"`).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `id` and `name` must be valid C strings.
/// `key_ptr` must be valid for `key_len` bytes. `role_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_join_collective(
    vault: *const DiviVault,
    id: *const c_char,
    name: *const c_char,
    key_ptr: *const u8,
    key_len: usize,
    role_json: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_vault_join_collective: invalid id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_join_collective: invalid UUID: {e}"));
            return -1;
        }
    };

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_vault_join_collective: invalid name");
        return -1;
    };

    if key_ptr.is_null() || key_len == 0 {
        set_last_error("divi_vault_join_collective: null or empty key");
        return -1;
    }
    let key_bytes = unsafe { std::slice::from_raw_parts(key_ptr, key_len) };
    let key = sentinal::SecureData::new(key_bytes.to_vec());

    let Some(role_str) = c_str_to_str(role_json) else {
        set_last_error("divi_vault_join_collective: invalid role_json");
        return -1;
    };

    let role: CollectiveRole = match serde_json::from_str(role_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_vault_join_collective: role parse error: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.join_collective(uuid, name_str.to_string(), key, role) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_join_collective: {e}"));
            -1
        }
    }
}

/// Leave a collective. Removes the key from memory.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_leave_collective(
    vault: *const DiviVault,
    id: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_vault_leave_collective: invalid id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_leave_collective: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.leave_collective(&uuid) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_leave_collective: {e}"));
            -1
        }
    }
}

/// List all collectives.
///
/// Returns a JSON array of Collective. Returns null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_list_collectives(
    vault: *const DiviVault,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.list_collectives() {
        Ok(collectives) => json_to_c(&collectives),
        Err(e) => {
            set_last_error(format!("divi_vault_list_collectives: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get a collective's encryption key as raw bytes.
///
/// Returns 0 on success, -1 on error. Output written to `out_key`/`out_key_len`.
/// The caller must free `out_key` via `divi_free_bytes`.
///
/// # Safety
/// `vault` must be a valid pointer. `id` must be a valid C string (UUID).
/// `out_key` and `out_key_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_collective_key(
    vault: *const DiviVault,
    id: *const c_char,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_vault_collective_key: invalid id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_collective_key: invalid UUID: {e}"));
            return -1;
        }
    };

    let guard = lock_or_recover(&vault.0);
    match guard.collective_key(&uuid) {
        Ok(secure_data) => {
            let (ptr, len) = bytes_to_owned(secure_data.expose().to_vec());
            unsafe {
                *out_key = ptr;
                *out_key_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(format!("divi_vault_collective_key: {e}"));
            -1
        }
    }
}

// ===================================================================
// Module State
// ===================================================================

/// Save a module state entry.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_save_module_state(
    vault: *const DiviVault,
    module_id: *const c_char,
    state_key: *const c_char,
    data: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(module_str) = c_str_to_str(module_id) else {
        set_last_error("divi_vault_save_module_state: invalid module_id");
        return -1;
    };

    let Some(key_str) = c_str_to_str(state_key) else {
        set_last_error("divi_vault_save_module_state: invalid state_key");
        return -1;
    };

    let Some(data_str) = c_str_to_str(data) else {
        set_last_error("divi_vault_save_module_state: invalid data");
        return -1;
    };

    let guard = lock_or_recover(&vault.0);
    match guard.save_module_state(module_str, key_str, data_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_save_module_state: {e}"));
            -1
        }
    }
}

/// Load a module state entry.
///
/// Returns the stored data as a C string, or null if not found or on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `module_id` and `state_key` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_load_module_state(
    vault: *const DiviVault,
    module_id: *const c_char,
    state_key: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(module_str) = c_str_to_str(module_id) else {
        set_last_error("divi_vault_load_module_state: invalid module_id");
        return std::ptr::null_mut();
    };

    let Some(key_str) = c_str_to_str(state_key) else {
        set_last_error("divi_vault_load_module_state: invalid state_key");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&vault.0);
    match guard.load_module_state(module_str, key_str) {
        Ok(Some(data)) => string_to_c(data),
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            set_last_error(format!("divi_vault_load_module_state: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Delete a module state entry.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `module_id` and `state_key` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_delete_module_state(
    vault: *const DiviVault,
    module_id: *const c_char,
    state_key: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(module_str) = c_str_to_str(module_id) else {
        set_last_error("divi_vault_delete_module_state: invalid module_id");
        return -1;
    };

    let Some(key_str) = c_str_to_str(state_key) else {
        set_last_error("divi_vault_delete_module_state: invalid state_key");
        return -1;
    };

    let guard = lock_or_recover(&vault.0);
    match guard.delete_module_state(module_str, key_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_delete_module_state: {e}"));
            -1
        }
    }
}

/// List all state keys for a module.
///
/// Returns a JSON array of strings. Returns null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `module_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_list_module_state_keys(
    vault: *const DiviVault,
    module_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(module_str) = c_str_to_str(module_id) else {
        set_last_error("divi_vault_list_module_state_keys: invalid module_id");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&vault.0);
    match guard.list_module_state_keys(module_str) {
        Ok(keys) => json_to_c(&keys),
        Err(e) => {
            set_last_error(format!("divi_vault_list_module_state_keys: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Search
// ===================================================================

/// Search ideas by text query via FTS5.
///
/// Returns a JSON array of `SearchHit` objects ordered by relevance.
/// `limit` is the maximum number of results; values <= 0 default to 20.
/// Returns null on error. The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_search(
    vault: *const DiviVault,
    query: *const c_char,
    limit: i32,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(query_str) = c_str_to_str(query) else {
        set_last_error("divi_vault_search: invalid query string");
        return std::ptr::null_mut();
    };

    let actual_limit = if limit <= 0 { 20usize } else { limit as usize };

    let guard = lock_or_recover(&vault.0);
    match guard.search(query_str, actual_limit) {
        Ok(hits) => json_to_c(&hits),
        Err(e) => {
            set_last_error(format!("divi_vault_search: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Index an idea's content for full-text search.
///
/// `idea_id` is a UUID string. `tags_json` is a JSON array of strings (e.g., `["tag1","tag2"]`),
/// or null for no tags.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `idea_id`, `title`, and `content_text` must be valid C strings.
/// `tags_json` must be a valid C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_index_idea(
    vault: *const DiviVault,
    idea_id: *const c_char,
    title: *const c_char,
    content_text: *const c_char,
    tags_json: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_vault_index_idea: invalid idea_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_index_idea: invalid UUID: {e}"));
            return -1;
        }
    };

    let Some(title_str) = c_str_to_str(title) else {
        set_last_error("divi_vault_index_idea: invalid title");
        return -1;
    };

    let Some(content_str) = c_str_to_str(content_text) else {
        set_last_error("divi_vault_index_idea: invalid content_text");
        return -1;
    };

    let tags: Vec<String> = if tags_json.is_null() {
        Vec::new()
    } else {
        let Some(tags_str) = c_str_to_str(tags_json) else {
            set_last_error("divi_vault_index_idea: invalid tags_json");
            return -1;
        };
        match serde_json::from_str(tags_str) {
            Ok(t) => t,
            Err(e) => {
                set_last_error(format!("divi_vault_index_idea: tags parse error: {e}"));
                return -1;
            }
        }
    };

    let guard = lock_or_recover(&vault.0);
    match guard.index_idea_for_search(&uuid, title_str, content_str, &tags) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_index_idea: {e}"));
            -1
        }
    }
}

/// Remove an idea from the search index.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `idea_id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_remove_search_index(
    vault: *const DiviVault,
    idea_id: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_vault_remove_search_index: invalid idea_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_vault_remove_search_index: invalid UUID: {e}"));
            return -1;
        }
    };

    let guard = lock_or_recover(&vault.0);
    match guard.remove_from_search_index(&uuid) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_remove_search_index: {e}"));
            -1
        }
    }
}

/// Rebuild the entire search index from the manifest table.
///
/// Returns the number of indexed entries, or -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_rebuild_search_index(
    vault: *const DiviVault,
) -> i64 {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.rebuild_search_index() {
        Ok(count) => count as i64,
        Err(e) => {
            set_last_error(format!("divi_vault_rebuild_search_index: {e}"));
            -1
        }
    }
}

// ===================================================================
// Path Resolution
// ===================================================================

/// Get the vault root path.
///
/// Returns a C string, or null if locked. The returned pointer must be freed
/// via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_root_path(vault: *const DiviVault) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.root_path() {
        Ok(path) => string_to_c(path.display().to_string()),
        Err(e) => {
            set_last_error(format!("divi_vault_root_path: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the personal ideas directory path.
///
/// Returns a C string, or null if locked. The returned pointer must be freed
/// via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_personal_path(vault: *const DiviVault) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.personal_path() {
        Ok(path) => string_to_c(path.display().to_string()),
        Err(e) => {
            set_last_error(format!("divi_vault_personal_path: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the collectives directory path.
///
/// Returns a C string, or null if locked. The returned pointer must be freed
/// via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_collectives_path(vault: *const DiviVault) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };
    let guard = lock_or_recover(&vault.0);

    match guard.collectives_path() {
        Ok(path) => string_to_c(path.display().to_string()),
        Err(e) => {
            set_last_error(format!("divi_vault_collectives_path: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Resolve a relative path within the vault root.
///
/// Returns a C string, or null if locked. The returned pointer must be freed
/// via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `relative` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_resolve_path(
    vault: *const DiviVault,
    relative: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(rel_str) = c_str_to_str(relative) else {
        set_last_error("divi_vault_resolve_path: invalid relative path");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&vault.0);
    match guard.resolve_path(rel_str) {
        Ok(path) => string_to_c(path.display().to_string()),
        Err(e) => {
            set_last_error(format!("divi_vault_resolve_path: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Collective Members
// ===================================================================

/// Add a member to a collective. Requires Admin or higher.
///
/// `role_json` is a JSON CollectiveRole (e.g., `"member"`).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `collective_id`, `pubkey`, and `role_json`
/// must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_collective_add_member(
    vault: *const DiviVault,
    collective_id: *const c_char,
    pubkey: *const c_char,
    role_json: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(collective_id) else {
        set_last_error("divi_vault_collective_add_member: invalid collective_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_vault_collective_add_member: invalid UUID: {e}"
            ));
            return -1;
        }
    };

    let Some(pubkey_str) = c_str_to_str(pubkey) else {
        set_last_error("divi_vault_collective_add_member: invalid pubkey");
        return -1;
    };

    let Some(role_str) = c_str_to_str(role_json) else {
        set_last_error("divi_vault_collective_add_member: invalid role_json");
        return -1;
    };

    let role: CollectiveRole = match serde_json::from_str(role_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!(
                "divi_vault_collective_add_member: role parse error: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.collective_add_member(&uuid, pubkey_str.to_string(), role) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_collective_add_member: {e}"));
            -1
        }
    }
}

/// Remove a member from a collective by public key. Requires Owner.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `vault` must be a valid pointer. `collective_id` and `pubkey` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_collective_remove_member(
    vault: *const DiviVault,
    collective_id: *const c_char,
    pubkey: *const c_char,
) -> i32 {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(collective_id) else {
        set_last_error("divi_vault_collective_remove_member: invalid collective_id");
        return -1;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_vault_collective_remove_member: invalid UUID: {e}"
            ));
            return -1;
        }
    };

    let Some(pubkey_str) = c_str_to_str(pubkey) else {
        set_last_error("divi_vault_collective_remove_member: invalid pubkey");
        return -1;
    };

    let mut guard = lock_or_recover(&vault.0);
    match guard.collective_remove_member(&uuid, pubkey_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_vault_collective_remove_member: {e}"));
            -1
        }
    }
}

/// Check if a public key is a member of a collective.
///
/// Returns `true` if the pubkey is a member, `false` otherwise (including on error).
///
/// # Safety
/// `vault` must be a valid pointer. `collective_id` and `pubkey` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_collective_is_member(
    vault: *const DiviVault,
    collective_id: *const c_char,
    pubkey: *const c_char,
) -> bool {
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(collective_id) else {
        return false;
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(_) => return false,
    };

    let Some(pubkey_str) = c_str_to_str(pubkey) else {
        return false;
    };

    let guard = lock_or_recover(&vault.0);
    guard.collective_is_member(&uuid, pubkey_str).unwrap_or_default()
}

/// Get a member's role in a collective.
///
/// Returns JSON CollectiveRole string, or null if not a member or on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `vault` must be a valid pointer. `collective_id` and `pubkey` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_vault_collective_member_role(
    vault: *const DiviVault,
    collective_id: *const c_char,
    pubkey: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let vault = unsafe { &*vault };

    let Some(id_str) = c_str_to_str(collective_id) else {
        set_last_error("divi_vault_collective_member_role: invalid collective_id");
        return std::ptr::null_mut();
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_vault_collective_member_role: invalid UUID: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let Some(pubkey_str) = c_str_to_str(pubkey) else {
        set_last_error("divi_vault_collective_member_role: invalid pubkey");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&vault.0);
    match guard.collective_member_role(&uuid, pubkey_str) {
        Ok(Some(role)) => json_to_c(&role),
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            set_last_error(format!("divi_vault_collective_member_role: {e}"));
            std::ptr::null_mut()
        }
    }
}
