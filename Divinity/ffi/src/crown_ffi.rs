use std::ffi::c_char;
use std::path::Path;
use std::sync::Mutex;

use crown::{Keyring, CrownKeypair, Preferences, Profile, Soul, SoulEncryptor};
use sentinal::SecureData;
use zeroize::Zeroize;

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// SentinalSoulEncryptor — adapter bridging Crown's SoulEncryptor trait to
// Sentinal's AES-256-GCM. Lives here in the FFI layer because Crown and
// Sentinal have no dependency on each other.
// ---------------------------------------------------------------------------

/// A `SoulEncryptor` backed by Sentinal's AES-256-GCM.
///
/// Holds a pre-derived 32-byte key (from `derive_soul_key`) in a
/// `SecureData` container that zeroizes the key material on drop.
/// Encrypt produces combined format: `nonce || ciphertext || tag`.
struct SentinalSoulEncryptor {
    key: SecureData,
}

impl SentinalSoulEncryptor {
    /// Create from raw key bytes. The key must be exactly 32 bytes.
    fn new(key: &[u8]) -> Self {
        Self { key: SecureData::from_slice(key) }
    }
}

impl SoulEncryptor for SentinalSoulEncryptor {
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        sentinal::encryption::encrypt_combined(data, self.key.expose())
            .map_err(|e| e.to_string())
    }

    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        sentinal::encryption::decrypt_combined(data, self.key.expose())
            .map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Thread-safe wrappers (Soul and Keyring use &mut self, so we add Mutex
// at the FFI boundary to match the *const T pattern used by Equipment)
// ---------------------------------------------------------------------------

pub struct CrownKeyring(pub(crate) Mutex<Keyring>);
pub struct CrownSoul(pub(crate) Mutex<Soul>);

// ===================================================================
// Keyring — cryptographic identity management
// ===================================================================

/// Create a new empty (locked) keyring.
/// Free with `divi_crown_keyring_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_crown_keyring_new() -> *mut CrownKeyring {
    Box::into_raw(Box::new(CrownKeyring(Mutex::new(Keyring::new()))))
}

/// Free a keyring.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_crown_keyring_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_free(ptr: *mut CrownKeyring) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Whether the keyring has a primary identity loaded.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_is_unlocked(keyring: *const CrownKeyring) -> bool {
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);
    guard.is_unlocked()
}

/// Generate a new random primary keypair.
///
/// Returns JSON: `{"crown_id": "cpub1...", "cpub_hex": "abcd..."}`.
/// Returns null on error (e.g., primary already exists).
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_generate_primary(
    keyring: *const CrownKeyring,
) -> *mut c_char {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let mut guard = lock_or_recover(&keyring.0);

    match guard.generate_primary() {
        Ok(kp) => {
            let result = serde_json::json!({
                "crown_id": kp.crown_id(),
                "cpub_hex": kp.public_key_hex(),
            });
            json_to_c(&result)
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Import a primary keypair from an crown_secret bech32 string.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `keyring` must be a valid pointer. `crown_secret` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_import_primary(
    keyring: *const CrownKeyring,
    crown_secret: *const c_char,
) -> i32 {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let Some(crown_secret_str) = c_str_to_str(crown_secret) else {
        set_last_error("divi_crown_keyring_import_primary: invalid crown_secret string");
        return -1;
    };

    let mut guard = lock_or_recover(&keyring.0);
    match guard.import_primary(crown_secret_str) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get the primary identity's crown_id string.
///
/// Returns null if locked. The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_public_key(
    keyring: *const CrownKeyring,
) -> *mut c_char {
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    match guard.public_key() {
        Ok(crown_id) => string_to_c(crown_id.to_string()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get the primary identity's public key as a 64-char hex string.
///
/// Returns null if locked. The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_public_key_hex(
    keyring: *const CrownKeyring,
) -> *mut c_char {
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    match guard.public_key_hex() {
        Ok(hex) => string_to_c(hex),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Sign data with the primary identity.
///
/// Returns JSON Signature: `{"data": "hex...", "signer": "cpub1...", "timestamp": "..."}`.
/// Returns null on error. The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer. `data` must be valid for `data_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_sign(
    keyring: *const CrownKeyring,
    data: *const u8,
    data_len: usize,
) -> *mut c_char {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    match guard.sign(slice) {
        Ok(sig) => json_to_c(&sig),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Export the keyring as JSON bytes (hex-encoded private keys).
///
/// The caller should encrypt these bytes before persisting.
/// Returns 0 on success, -1 on error. Output written to `out_data`/`out_len`.
/// The caller must free `out_data` via `divi_free_bytes`.
///
/// # Safety
/// `keyring` must be a valid pointer. `out_data` and `out_len` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_export(
    keyring: *const CrownKeyring,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    match guard.export() {
        Ok(bytes) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(bytes);
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

/// Load keyring state from JSON bytes (previously exported).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `keyring` must be a valid pointer. `data` must be valid for `data_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_load(
    keyring: *const CrownKeyring,
    data: *const u8,
    data_len: usize,
) -> i32 {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let mut guard = lock_or_recover(&keyring.0);

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    match guard.load(slice) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Lock the keyring — clear all keys from memory.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_lock(keyring: *const CrownKeyring) {
    let keyring = unsafe { &*keyring };
    let mut guard = lock_or_recover(&keyring.0);
    guard.lock();
}

/// List persona names as a JSON array of strings.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_list_personas(
    keyring: *const CrownKeyring,
) -> *mut c_char {
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);
    let names: Vec<&str> = guard.list_personas();
    json_to_c(&names)
}

/// Create a new persona keypair.
///
/// Returns the persona's crown_id as a string, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_create_persona(
    keyring: *const CrownKeyring,
    name: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_crown_keyring_create_persona: invalid name");
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&keyring.0);
    match guard.create_persona(name_str) {
        Ok(kp) => string_to_c(kp.crown_id().to_string()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Delete a named persona.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `keyring` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_keyring_delete_persona(
    keyring: *const CrownKeyring,
    name: *const c_char,
) -> i32 {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_crown_keyring_delete_persona: invalid name");
        return -1;
    };

    let mut guard = lock_or_recover(&keyring.0);
    match guard.delete_persona(name_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ===================================================================
// Soul — identity container (profile + preferences + social graph)
// ===================================================================

/// Create a new in-memory soul with defaults.
/// Free with `divi_crown_soul_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_crown_soul_new() -> *mut CrownSoul {
    Box::into_raw(Box::new(CrownSoul(Mutex::new(Soul::new()))))
}

/// Create a new soul at the given path, writing soul.json with defaults.
///
/// Returns null on error. Free with `divi_crown_soul_free`.
///
/// # Safety
/// `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_create(path: *const c_char) -> *mut CrownSoul {
    clear_last_error();
    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_crown_soul_create: invalid path");
        return std::ptr::null_mut();
    };

    match Soul::create(Path::new(path_str), None) {
        Ok(soul) => Box::into_raw(Box::new(CrownSoul(Mutex::new(soul)))),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Load an existing soul from a directory (reads soul.json).
///
/// Returns null on error. Free with `divi_crown_soul_free`.
///
/// # Safety
/// `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_load(path: *const c_char) -> *mut CrownSoul {
    clear_last_error();
    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_crown_soul_load: invalid path");
        return std::ptr::null_mut();
    };

    match Soul::load(Path::new(path_str), None) {
        Ok(soul) => Box::into_raw(Box::new(CrownSoul(Mutex::new(soul)))),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Create a new soul at the given path with at-rest encryption.
///
/// `key` must be a 32-byte AES-256-GCM key (from `divi_sentinal_derive_soul_key`
/// or Vault's `Custodian::soul_key()`). The soul.json file is encrypted before
/// writing and decrypted on load.
///
/// Returns null on error. Free with `divi_crown_soul_free`.
///
/// # Safety
/// `path` must be a valid C string. `key` must be valid for `key_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_create_encrypted(
    path: *const c_char,
    key: *const u8,
    key_len: usize,
) -> *mut CrownSoul {
    clear_last_error();
    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_crown_soul_create_encrypted: invalid path");
        return std::ptr::null_mut();
    };

    if key.is_null() || key_len == 0 {
        set_last_error("divi_crown_soul_create_encrypted: null or empty key");
        return std::ptr::null_mut();
    }
    let key_slice = unsafe { std::slice::from_raw_parts(key, key_len) };
    let encryptor: Box<dyn SoulEncryptor> = Box::new(SentinalSoulEncryptor::new(key_slice));

    match Soul::create(Path::new(path_str), Some(encryptor)) {
        Ok(soul) => Box::into_raw(Box::new(CrownSoul(Mutex::new(soul)))),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Load an existing soul from a directory with at-rest decryption.
///
/// `key` must be a 32-byte AES-256-GCM key. For backward compatibility,
/// if the file contains unencrypted plaintext JSON, loading still succeeds
/// (migration case). Subsequent saves will encrypt the data.
///
/// Returns null on error. Free with `divi_crown_soul_free`.
///
/// # Safety
/// `path` must be a valid C string. `key` must be valid for `key_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_load_encrypted(
    path: *const c_char,
    key: *const u8,
    key_len: usize,
) -> *mut CrownSoul {
    clear_last_error();
    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_crown_soul_load_encrypted: invalid path");
        return std::ptr::null_mut();
    };

    if key.is_null() || key_len == 0 {
        set_last_error("divi_crown_soul_load_encrypted: null or empty key");
        return std::ptr::null_mut();
    }
    let key_slice = unsafe { std::slice::from_raw_parts(key, key_len) };
    let encryptor: Box<dyn SoulEncryptor> = Box::new(SentinalSoulEncryptor::new(key_slice));

    match Soul::load(Path::new(path_str), Some(encryptor)) {
        Ok(soul) => Box::into_raw(Box::new(CrownSoul(Mutex::new(soul)))),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Free a soul.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_crown_soul_*` constructor, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_free(ptr: *mut CrownSoul) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Save the soul to disk.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `soul` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_save(soul: *const CrownSoul) -> i32 {
    clear_last_error();
    let soul = unsafe { &*soul };
    let mut guard = lock_or_recover(&soul.0);

    match guard.save() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Whether the soul has unsaved changes.
///
/// # Safety
/// `soul` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_is_dirty(soul: *const CrownSoul) -> bool {
    let soul = unsafe { &*soul };
    let guard = lock_or_recover(&soul.0);
    guard.is_dirty()
}

/// Get the profile as JSON.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `soul` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_profile(soul: *const CrownSoul) -> *mut c_char {
    let soul = unsafe { &*soul };
    let guard = lock_or_recover(&soul.0);
    json_to_c(guard.profile())
}

/// Update the profile from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `soul` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_update_profile(
    soul: *const CrownSoul,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let soul = unsafe { &*soul };
    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_crown_soul_update_profile: invalid json");
        return -1;
    };

    let profile: Profile = match serde_json::from_str(json_str) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_crown_soul_update_profile: JSON parse error: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&soul.0);
    guard.update_profile(profile);
    0
}

/// Get preferences as JSON.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `soul` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_preferences(soul: *const CrownSoul) -> *mut c_char {
    let soul = unsafe { &*soul };
    let guard = lock_or_recover(&soul.0);
    json_to_c(guard.preferences())
}

/// Update preferences from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `soul` must be a valid pointer. `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_update_preferences(
    soul: *const CrownSoul,
    json: *const c_char,
) -> i32 {
    clear_last_error();
    let soul = unsafe { &*soul };
    let Some(json_str) = c_str_to_str(json) else {
        set_last_error("divi_crown_soul_update_preferences: invalid json");
        return -1;
    };

    let prefs: Preferences = match serde_json::from_str(json_str) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!(
                "divi_crown_soul_update_preferences: JSON parse error: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&soul.0);
    guard.update_preferences(prefs);
    0
}

/// Get the social graph as JSON.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `soul` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_social_graph(soul: *const CrownSoul) -> *mut c_char {
    let soul = unsafe { &*soul };
    let guard = lock_or_recover(&soul.0);
    json_to_c(guard.social_graph())
}

/// Follow an crown_id.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_follow(soul: *const CrownSoul, crown_id: *const c_char) {
    let soul = unsafe { &*soul };
    if let Some(crown_id_str) = c_str_to_str(crown_id) {
        let mut guard = lock_or_recover(&soul.0);
        guard.follow(crown_id_str);
    }
}

/// Unfollow an crown_id.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_unfollow(soul: *const CrownSoul, crown_id: *const c_char) {
    let soul = unsafe { &*soul };
    if let Some(crown_id_str) = c_str_to_str(crown_id) {
        let mut guard = lock_or_recover(&soul.0);
        guard.unfollow(crown_id_str);
    }
}

/// Block an crown_id (also removes from following).
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_block(soul: *const CrownSoul, crown_id: *const c_char) {
    let soul = unsafe { &*soul };
    if let Some(crown_id_str) = c_str_to_str(crown_id) {
        let mut guard = lock_or_recover(&soul.0);
        guard.block(crown_id_str);
    }
}

/// Unblock an crown_id.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_unblock(soul: *const CrownSoul, crown_id: *const c_char) {
    let soul = unsafe { &*soul };
    if let Some(crown_id_str) = c_str_to_str(crown_id) {
        let mut guard = lock_or_recover(&soul.0);
        guard.unblock(crown_id_str);
    }
}

/// Check if an crown_id is followed.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_is_following(
    soul: *const CrownSoul,
    crown_id: *const c_char,
) -> bool {
    let soul = unsafe { &*soul };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        return false;
    };
    let guard = lock_or_recover(&soul.0);
    guard.social_graph().is_following(crown_id_str)
}

/// Check if an crown_id is blocked.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_is_blocked(
    soul: *const CrownSoul,
    crown_id: *const c_char,
) -> bool {
    let soul = unsafe { &*soul };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        return false;
    };
    let guard = lock_or_recover(&soul.0);
    guard.social_graph().is_blocked(crown_id_str)
}

/// Check if an crown_id is muted.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_is_muted(
    soul: *const CrownSoul,
    crown_id: *const c_char,
) -> bool {
    let soul = unsafe { &*soul };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        return false;
    };
    let guard = lock_or_recover(&soul.0);
    guard.social_graph().is_muted(crown_id_str)
}

/// Mute an crown_id.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_mute(soul: *const CrownSoul, crown_id: *const c_char) {
    let soul = unsafe { &*soul };
    if let Some(crown_id_str) = c_str_to_str(crown_id) {
        let mut guard = lock_or_recover(&soul.0);
        let mut graph = guard.social_graph().clone();
        graph.mute(crown_id_str);
        guard.update_social_graph(graph);
    }
}

/// Unmute an crown_id.
///
/// # Safety
/// `soul` must be a valid pointer. `crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_soul_unmute(soul: *const CrownSoul, crown_id: *const c_char) {
    let soul = unsafe { &*soul };
    if let Some(crown_id_str) = c_str_to_str(crown_id) {
        let mut guard = lock_or_recover(&soul.0);
        let mut graph = guard.social_graph().clone();
        graph.unmute(crown_id_str);
        guard.update_social_graph(graph);
    }
}

// ===================================================================
// Blinding — context-specific keypair derivation
// ===================================================================

/// Derive a blinded keypair from a keyring's primary identity and a context.
///
/// `context_json` is a JSON BlindingContext (e.g., `{"context_id":"community:woodworkers","version":0}`).
/// Returns JSON with the blinded public key: `{"crown_id":"cpub1...","cpub_hex":"abcd..."}`.
/// Returns null on error. Caller must free via `divi_free_string`.
///
/// The blinded key is cached in the keyring for subsequent signing.
///
/// # Safety
/// `keyring` must be a valid pointer. `context_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_derive_blinded_keypair(
    keyring: *const CrownKeyring,
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let Some(cj) = c_str_to_str(context_json) else {
        set_last_error("divi_crown_derive_blinded_keypair: invalid context_json");
        return std::ptr::null_mut();
    };

    let context: crown::BlindingContext = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_crown_derive_blinded_keypair: JSON parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&keyring.0);
    match guard.get_or_derive_blinded(&context) {
        Ok(kp) => {
            let result = serde_json::json!({
                "crown_id": kp.crown_id(),
                "cpub_hex": kp.public_key_hex(),
            });
            json_to_c(&result)
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Sign data with a blinded keypair for a specific context.
///
/// The blinded key must already be in the keyring's cache (via
/// `divi_crown_derive_blinded_keypair`).
///
/// Returns JSON Signature, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer. `data` must be valid for `data_len` bytes.
/// `context_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_sign_blinded(
    keyring: *const CrownKeyring,
    data: *const u8,
    data_len: usize,
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let Some(cj) = c_str_to_str(context_json) else {
        set_last_error("divi_crown_sign_blinded: invalid context_json");
        return std::ptr::null_mut();
    };

    let context: crown::BlindingContext = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_crown_sign_blinded: JSON parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let guard = lock_or_recover(&keyring.0);
    match guard.sign_blinded(slice, &context) {
        Ok(sig) => json_to_c(&sig),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Create a blinding proof: the master key signs a binding message proving
/// ownership of the blinded key within a specific context.
///
/// `blinded_crown_id` is the bech32 crown_id of the blinded key.
/// `context_json` is a JSON BlindingContext.
/// Returns JSON BlindingProof, or null on error. Caller must free via `divi_free_string`.
///
/// # Safety
/// `keyring` must be a valid pointer. `blinded_crown_id` and `context_json` must be
/// valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_blinding_proof_create(
    keyring: *const CrownKeyring,
    blinded_crown_id: *const c_char,
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let Some(bn) = c_str_to_str(blinded_crown_id) else {
        set_last_error("divi_crown_blinding_proof_create: invalid blinded_crown_id");
        return std::ptr::null_mut();
    };
    let Some(cj) = c_str_to_str(context_json) else {
        set_last_error("divi_crown_blinding_proof_create: invalid context_json");
        return std::ptr::null_mut();
    };

    let context: crown::BlindingContext = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_crown_blinding_proof_create: JSON parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&keyring.0);

    // Extract the master keypair from the keyring.
    let keypair = match extract_keypair_from_keyring(&guard) {
        Ok(kp) => kp,
        Err(msg) => {
            set_last_error(msg);
            return std::ptr::null_mut();
        }
    };

    match crown::blinding_proof::create_blinding_proof(&keypair, bn, &context) {
        Ok(proof) => json_to_c(&proof),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Verify a blinding proof.
///
/// `proof_json` is a JSON BlindingProof.
/// Returns 1 if valid, 0 if invalid, -1 on error.
///
/// # Safety
/// `proof_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_blinding_proof_verify(
    proof_json: *const c_char,
) -> i32 {
    clear_last_error();
    let Some(pj) = c_str_to_str(proof_json) else {
        set_last_error("divi_crown_blinding_proof_verify: invalid proof_json");
        return -1;
    };

    let proof: crown::BlindingProof = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_crown_blinding_proof_verify: JSON parse: {e}"));
            return -1;
        }
    };

    match proof.verify() {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Clear all cached blinded keys from a keyring.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_clear_blinded_cache(
    keyring: *const CrownKeyring,
) -> i32 {
    clear_last_error();
    if keyring.is_null() {
        set_last_error("divi_crown_clear_blinded_cache: null keyring");
        return -1;
    }
    let keyring = unsafe { &*keyring };
    let mut guard = lock_or_recover(&keyring.0);
    guard.clear_blinded_cache();
    0
}

// ===================================================================
// ECDH — shared secret derivation
// ===================================================================

/// Compute an ECDH shared secret between a keyring's primary key and
/// another party's public key (32 bytes, x-only).
///
/// Both sides independently arrive at the same 32-byte shared secret.
/// Output written to `out_secret`/`out_secret_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `keyring` must be a valid pointer. `their_pubkey` must be 32 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_shared_secret(
    keyring: *const CrownKeyring,
    their_pubkey: *const u8,
    out_secret: *mut *mut u8,
    out_secret_len: *mut usize,
) -> i32 {
    clear_last_error();
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    if their_pubkey.is_null() {
        set_last_error("divi_crown_shared_secret: null their_pubkey");
        return -1;
    }
    let pk: &[u8; 32] = unsafe { &*(their_pubkey as *const [u8; 32]) };

    // Extract keypair from the keyring.
    let keypair = match extract_keypair_from_keyring(&guard) {
        Ok(kp) => kp,
        Err(msg) => {
            set_last_error(msg);
            return -1;
        }
    };

    match keypair.shared_secret(pk) {
        Ok(secret) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(secret.to_vec());
            unsafe {
                *out_secret = ptr;
                *out_secret_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ===================================================================
// Recovery — recover a Keyring from a raw 32-byte private key
// ===================================================================

/// Recover a Keyring from a raw 32-byte private key.
///
/// Takes the raw identity key bytes (e.g., from `divi_sentinal_derive_identity_key`)
/// and produces a new Keyring with that key imported as the primary identity.
///
/// Returns a new CrownKeyring pointer on success, null on error.
/// Caller must free via `divi_crown_keyring_free`.
///
/// # Safety
/// `secret` must be valid for `secret_len` bytes (must be exactly 32 bytes).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_crown_recover_from_secret(
    secret: *const u8,
    secret_len: usize,
) -> *mut CrownKeyring {
    clear_last_error();

    if secret.is_null() {
        set_last_error("divi_crown_recover_from_secret: null secret");
        return std::ptr::null_mut();
    }

    if secret_len != 32 {
        set_last_error(format!(
            "divi_crown_recover_from_secret: expected 32 bytes, got {secret_len}"
        ));
        return std::ptr::null_mut();
    }

    let secret_slice = unsafe { std::slice::from_raw_parts(secret, secret_len) };

    match crown::recovery::recover_from_secret(secret_slice) {
        Ok(keyring) => Box::into_raw(Box::new(CrownKeyring(Mutex::new(keyring)))),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Extract a CrownKeypair from a locked Keyring (for ECDH).
fn extract_keypair_from_keyring(keyring: &Keyring) -> Result<CrownKeypair, String> {
    let mut export = keyring
        .export()
        .map_err(|e| format!("keyring export failed: {e}"))?;

    let storage: serde_json::Value =
        serde_json::from_slice(&export).map_err(|e| format!("parse failed: {e}"))?;

    // Zeroize the raw export bytes — they contain hex-encoded private keys.
    export.zeroize();

    let mut hex_key = storage
        .get("primary_private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "no primary private key".to_string())?
        .to_string();

    let mut key_bytes = hex::decode(&hex_key).map_err(|e| format!("hex decode: {e}"))?;
    hex_key.zeroize();

    let result = CrownKeypair::from_private_key(&key_bytes).map_err(|e| format!("invalid key: {e}"));
    key_bytes.zeroize();

    result
}
