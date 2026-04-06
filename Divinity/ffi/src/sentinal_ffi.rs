use std::ffi::c_char;

use sentinal::encryption;
use sentinal::key_derivation;
use sentinal::key_slot::{KeySlot, KeySlotCredential, PasswordKeySlot, PublicKeySlot};
use sentinal::password_strength;
use sentinal::recovery;

use crate::helpers::{c_str_to_str, json_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Encryption — AES-256-GCM (combined format: nonce || ciphertext || tag)
// ===================================================================

/// Encrypt plaintext with a 32-byte key.
///
/// Returns combined format bytes (nonce || ciphertext || tag).
/// Output written to `out_data`/`out_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointers must be valid. `key` must be 32 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_encrypt(
    plaintext: *const u8,
    plaintext_len: usize,
    key: *const u8,
    key_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();

    let pt = if plaintext.is_null() || plaintext_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(plaintext, plaintext_len) }
    };

    let k = if key.is_null() || key_len == 0 {
        set_last_error("divi_sentinal_encrypt: null key");
        return -1;
    } else {
        unsafe { std::slice::from_raw_parts(key, key_len) }
    };

    match encryption::encrypt_combined(pt, k) {
        Ok(combined) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(combined);
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

/// Decrypt combined-format ciphertext with a 32-byte key.
///
/// Output written to `out_data`/`out_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointers must be valid. `key` must be 32 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_decrypt(
    ciphertext: *const u8,
    ciphertext_len: usize,
    key: *const u8,
    key_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();

    let ct = if ciphertext.is_null() || ciphertext_len == 0 {
        set_last_error("divi_sentinal_decrypt: null ciphertext");
        return -1;
    } else {
        unsafe { std::slice::from_raw_parts(ciphertext, ciphertext_len) }
    };

    let k = if key.is_null() || key_len == 0 {
        set_last_error("divi_sentinal_decrypt: null key");
        return -1;
    } else {
        unsafe { std::slice::from_raw_parts(key, key_len) }
    };

    match encryption::decrypt_combined(ct, k) {
        Ok(plaintext) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(plaintext);
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

// ===================================================================
// Key Derivation
// ===================================================================

/// Derive a master key from a password using PBKDF2 (600K iterations).
///
/// If `salt` is null, generates a random 32-byte salt.
/// Outputs: key bytes (`out_key`/`out_key_len`) and salt bytes (`out_salt`/`out_salt_len`).
/// Caller must free both via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `password` must be a valid C string. Output pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_derive_master_key(
    password: *const c_char,
    salt: *const u8,
    salt_len: usize,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
    out_salt: *mut *mut u8,
    out_salt_len: *mut usize,
) -> i32 {
    clear_last_error();

    let Some(pwd) = c_str_to_str(password) else {
        set_last_error("divi_sentinal_derive_master_key: invalid password");
        return -1;
    };

    let salt_opt = if salt.is_null() || salt_len == 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(salt, salt_len) })
    };

    match key_derivation::derive_master_key(pwd, salt_opt) {
        Ok((secure_key, salt_bytes)) => {
            let key_vec = secure_key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            let (sp, sl) = crate::helpers::bytes_to_owned(salt_bytes);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
                *out_salt = sp;
                *out_salt_len = sl;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Derive a content key from a master key and a UUID string.
///
/// Output written to `out_key`/`out_key_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `master_key` must be 32 bytes. `id` must be a valid UUID C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_derive_content_key(
    master_key: *const u8,
    master_key_len: usize,
    id: *const c_char,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();

    let mk = if master_key.is_null() || master_key_len == 0 {
        set_last_error("divi_sentinal_derive_content_key: null master_key");
        return -1;
    } else {
        unsafe { std::slice::from_raw_parts(master_key, master_key_len) }
    };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_sentinal_derive_content_key: invalid id");
        return -1;
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_sentinal_derive_content_key: invalid UUID: {e}"));
            return -1;
        }
    };

    match key_derivation::derive_content_key(mk, &uuid) {
        Ok(secure_key) => {
            let key_vec = secure_key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Derive a shared key from an ECDH shared secret.
///
/// Output written to `out_key`/`out_key_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `shared_secret` must be valid for `secret_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_derive_shared_key(
    shared_secret: *const u8,
    secret_len: usize,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();

    let secret = if shared_secret.is_null() || secret_len == 0 {
        set_last_error("divi_sentinal_derive_shared_key: null shared_secret");
        return -1;
    } else {
        unsafe { std::slice::from_raw_parts(shared_secret, secret_len) }
    };

    match key_derivation::derive_shared_key(secret) {
        Ok(secure_key) => {
            let key_vec = secure_key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Generate a random salt of the given length.
///
/// Output written to `out_data`/`out_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// Output pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_generate_salt(
    length: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();

    match key_derivation::generate_salt(length) {
        Ok(salt) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(salt);
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

/// Derive a 32-byte soul encryption key from a master key using HKDF-SHA256.
///
/// Uses domain salt `"omnidea-soul-v1"` with info `"soul-data"`.
/// The resulting key is used with `divi_crown_soul_create_encrypted` and
/// `divi_crown_soul_load_encrypted` to encrypt soul.json at rest.
///
/// Output written to `out_key`/`out_key_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `master_key` must be valid for `master_key_len` bytes. Output pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_derive_soul_key(
    master_key: *const u8,
    master_key_len: usize,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();

    let mk = if master_key.is_null() || master_key_len == 0 {
        set_last_error("divi_sentinal_derive_soul_key: null master_key");
        return -1;
    } else {
        unsafe { std::slice::from_raw_parts(master_key, master_key_len) }
    };

    match key_derivation::derive_soul_key(mk) {
        Ok(secure_key) => {
            let key_vec = secure_key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
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
// Key Slots — wrapping/unwrapping content keys
// ===================================================================

/// Create a password-protected key slot wrapping a content key.
///
/// Returns JSON (KeySlot). Caller must free via `divi_free_string`.
///
/// # Safety
/// `content_key` must be 32 bytes. `password` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_key_slot_create_password(
    content_key: *const u8,
    content_key_len: usize,
    password: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let ck = if content_key.is_null() || content_key_len == 0 {
        set_last_error("divi_sentinal_key_slot_create_password: null content_key");
        return std::ptr::null_mut();
    } else {
        unsafe { std::slice::from_raw_parts(content_key, content_key_len) }
    };

    let Some(pwd) = c_str_to_str(password) else {
        set_last_error("divi_sentinal_key_slot_create_password: invalid password");
        return std::ptr::null_mut();
    };

    match PasswordKeySlot::create(ck, pwd) {
        Ok(slot) => json_to_c(&slot),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Create a public-key key slot (X25519 ECDH) wrapping a content key.
///
/// Returns JSON (KeySlot). Caller must free via `divi_free_string`.
///
/// # Safety
/// `content_key` must be 32 bytes. `recipient_pubkey` must be 32 bytes.
/// `recipient_crown_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_key_slot_create_public(
    content_key: *const u8,
    content_key_len: usize,
    recipient_pubkey: *const u8,
    recipient_crown_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let ck = if content_key.is_null() || content_key_len == 0 {
        set_last_error("divi_sentinal_key_slot_create_public: null content_key");
        return std::ptr::null_mut();
    } else {
        unsafe { std::slice::from_raw_parts(content_key, content_key_len) }
    };

    if recipient_pubkey.is_null() {
        set_last_error("divi_sentinal_key_slot_create_public: null recipient_pubkey");
        return std::ptr::null_mut();
    }
    let rpk: &[u8; 32] = unsafe {
        &*(recipient_pubkey as *const [u8; 32])
    };

    let Some(crown_id) = c_str_to_str(recipient_crown_id) else {
        set_last_error("divi_sentinal_key_slot_create_public: invalid crown_id");
        return std::ptr::null_mut();
    };

    match PublicKeySlot::create(ck, rpk, crown_id) {
        Ok(slot) => json_to_c(&slot),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Unwrap a key slot with a password credential.
///
/// `slot_json` is a serialized KeySlot. Output is the unwrapped content key bytes.
/// Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `slot_json` and `password` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_key_slot_unwrap_password(
    slot_json: *const c_char,
    password: *const c_char,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();

    let Some(json_str) = c_str_to_str(slot_json) else {
        set_last_error("divi_sentinal_key_slot_unwrap_password: invalid slot_json");
        return -1;
    };

    let Some(pwd) = c_str_to_str(password) else {
        set_last_error("divi_sentinal_key_slot_unwrap_password: invalid password");
        return -1;
    };

    let slot: KeySlot = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_sentinal_key_slot_unwrap_password: {e}"));
            return -1;
        }
    };

    match slot.unwrap(KeySlotCredential::Password(pwd)) {
        Ok(secure_key) => {
            let key_vec = secure_key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Unwrap a key slot with a private key (X25519, 32 bytes).
///
/// Output is the unwrapped content key bytes. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `slot_json` must be a valid C string. `private_key` must be 32 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_key_slot_unwrap_private(
    slot_json: *const c_char,
    private_key: *const u8,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();

    let Some(json_str) = c_str_to_str(slot_json) else {
        set_last_error("divi_sentinal_key_slot_unwrap_private: invalid slot_json");
        return -1;
    };

    if private_key.is_null() {
        set_last_error("divi_sentinal_key_slot_unwrap_private: null private_key");
        return -1;
    }
    let pk: &[u8; 32] = unsafe { &*(private_key as *const [u8; 32]) };

    let slot: KeySlot = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_sentinal_key_slot_unwrap_private: {e}"));
            return -1;
        }
    };

    match slot.unwrap(KeySlotCredential::PrivateKey(pk)) {
        Ok(secure_key) => {
            let key_vec = secure_key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
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
// Onion encryption — layered encryption for relay forwarding
// ===================================================================

/// Wrap one layer of onion encryption addressed to a relay's public key.
///
/// `relay_pubkey` must be a 32-byte X25519 public key.
/// Returns encrypted blob (ephemeral_pubkey || encrypted_combined).
/// Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// `plaintext` must be valid for `pt_len` bytes. `relay_pubkey` must be 32 bytes.
/// `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_onion_wrap(
    plaintext: *const u8,
    pt_len: usize,
    relay_pubkey: *const u8,
    key_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    let pt = if plaintext.is_null() || pt_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(plaintext, pt_len) }
    };

    if relay_pubkey.is_null() || key_len != 32 {
        set_last_error("divi_sentinal_onion_wrap: relay_pubkey must be 32 bytes");
        return std::ptr::null_mut();
    }
    let pk = unsafe { std::slice::from_raw_parts(relay_pubkey, key_len) };

    match sentinal::onion::wrap_layer(pt, pk) {
        Ok(blob) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(blob);
            unsafe { *out_len = len; }
            ptr
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Unwrap one layer of onion encryption using the relay's private key.
///
/// `relay_privkey` must be a 32-byte X25519 private key.
/// Returns the inner plaintext (which may be another onion layer).
/// Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// `ciphertext` must be valid for `ct_len` bytes. `relay_privkey` must be 32 bytes.
/// `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_onion_unwrap(
    ciphertext: *const u8,
    ct_len: usize,
    relay_privkey: *const u8,
    key_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if ciphertext.is_null() || ct_len == 0 {
        set_last_error("divi_sentinal_onion_unwrap: null or empty ciphertext");
        return std::ptr::null_mut();
    }
    let ct = unsafe { std::slice::from_raw_parts(ciphertext, ct_len) };

    if relay_privkey.is_null() || key_len != 32 {
        set_last_error("divi_sentinal_onion_unwrap: relay_privkey must be 32 bytes");
        return std::ptr::null_mut();
    }
    let sk = unsafe { std::slice::from_raw_parts(relay_privkey, key_len) };

    match sentinal::onion::unwrap_layer(ct, sk) {
        Ok(plaintext) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(plaintext);
            unsafe { *out_len = len; }
            ptr
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// AAD encryption — AES-256-GCM with additional authenticated data
// ===================================================================

/// Encrypt plaintext with AES-256-GCM and additional authenticated data (AAD).
///
/// AAD is authenticated but not encrypted. Returns combined format bytes.
/// Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// All pointers must be valid for their respective lengths. `key` must be 32 bytes.
/// `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_encrypt_with_aad(
    plaintext: *const u8,
    pt_len: usize,
    aad: *const u8,
    aad_len: usize,
    key: *const u8,
    key_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    let pt = if plaintext.is_null() || pt_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(plaintext, pt_len) }
    };

    let aad_slice = if aad.is_null() || aad_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(aad, aad_len) }
    };

    if key.is_null() || key_len == 0 {
        set_last_error("divi_sentinal_encrypt_with_aad: null key");
        return std::ptr::null_mut();
    }
    let k = unsafe { std::slice::from_raw_parts(key, key_len) };

    match sentinal::encryption::encrypt_with_aad(pt, aad_slice, k) {
        Ok(combined) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(combined);
            unsafe { *out_len = len; }
            ptr
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Decrypt combined-format ciphertext with AAD.
///
/// The `aad` must exactly match what was provided during encryption.
/// Returns decrypted plaintext bytes.
/// Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// All pointers must be valid for their respective lengths. `key` must be 32 bytes.
/// `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_decrypt_with_aad(
    combined: *const u8,
    combined_len: usize,
    aad: *const u8,
    aad_len: usize,
    key: *const u8,
    key_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if combined.is_null() || combined_len == 0 {
        set_last_error("divi_sentinal_decrypt_with_aad: null or empty combined data");
        return std::ptr::null_mut();
    }
    let ct = unsafe { std::slice::from_raw_parts(combined, combined_len) };

    let aad_slice = if aad.is_null() || aad_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(aad, aad_len) }
    };

    if key.is_null() || key_len == 0 {
        set_last_error("divi_sentinal_decrypt_with_aad: null key");
        return std::ptr::null_mut();
    }
    let k = unsafe { std::slice::from_raw_parts(key, key_len) };

    match sentinal::encryption::decrypt_with_aad(ct, aad_slice, k) {
        Ok(plaintext) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(plaintext);
            unsafe { *out_len = len; }
            ptr
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Padding — PKCS#7 block padding
// ===================================================================

/// Pad data to a multiple of `block_size` using PKCS#7 padding.
///
/// Returns padded bytes. Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// `data` must be valid for `data_len` bytes. `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_pad(
    data: *const u8,
    data_len: usize,
    block_size: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if block_size == 0 || block_size > 255 {
        set_last_error("divi_sentinal_pad: block_size must be 1..=255");
        return std::ptr::null_mut();
    }

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let padded = sentinal::pad_to_multiple(slice, block_size);
    let (ptr, len) = crate::helpers::bytes_to_owned(padded);
    unsafe { *out_len = len; }
    ptr
}

/// Remove PKCS#7 padding and recover original data.
///
/// Returns unpadded bytes. Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// `data` must be valid for `data_len` bytes. `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_unpad(
    data: *const u8,
    data_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if data.is_null() || data_len == 0 {
        set_last_error("divi_sentinal_unpad: null or empty data");
        return std::ptr::null_mut();
    }
    let slice = unsafe { std::slice::from_raw_parts(data, data_len) };

    match sentinal::unpad_from_multiple(slice) {
        Ok(original) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(original);
            unsafe { *out_len = len; }
            ptr
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Recovery — BIP-39 mnemonics
// ===================================================================

/// Generate a random 24-word recovery phrase.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
/// Returns null on error.
#[unsafe(no_mangle)]
pub extern "C" fn divi_sentinal_recovery_generate() -> *mut c_char {
    clear_last_error();

    match recovery::generate_phrase() {
        Ok(words) => json_to_c(&words),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Validate a recovery phrase (JSON array of strings).
///
/// Returns true if valid, false otherwise.
///
/// # Safety
/// `phrase_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_recovery_validate(
    phrase_json: *const c_char,
) -> bool {
    let Some(json_str) = c_str_to_str(phrase_json) else {
        return false;
    };

    let words: Vec<String> = match serde_json::from_str(json_str) {
        Ok(w) => w,
        Err(_) => return false,
    };

    recovery::validate_phrase(&words)
}

/// Convert a recovery phrase to a 64-byte seed.
///
/// Output written to `out_seed`/`out_seed_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `phrase_json` and `passphrase` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_recovery_to_seed(
    phrase_json: *const c_char,
    passphrase: *const c_char,
    out_seed: *mut *mut u8,
    out_seed_len: *mut usize,
) -> i32 {
    clear_last_error();

    let Some(json_str) = c_str_to_str(phrase_json) else {
        set_last_error("divi_sentinal_recovery_to_seed: invalid phrase_json");
        return -1;
    };

    let Some(pp) = c_str_to_str(passphrase) else {
        set_last_error("divi_sentinal_recovery_to_seed: invalid passphrase");
        return -1;
    };

    let words: Vec<String> = match serde_json::from_str(json_str) {
        Ok(w) => w,
        Err(e) => {
            set_last_error(format!("divi_sentinal_recovery_to_seed: {e}"));
            return -1;
        }
    };

    match recovery::phrase_to_seed(&words, pp) {
        Ok(secure_seed) => {
            let seed_vec = secure_seed.expose().to_vec();
            let (sp, sl) = crate::helpers::bytes_to_owned(seed_vec);
            unsafe {
                *out_seed = sp;
                *out_seed_len = sl;
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
// Identity key derivation — BIP-39 seed to secp256k1 private key
// ===================================================================

/// Derive a 32-byte identity key from a BIP-39 seed using HKDF-SHA256.
///
/// Takes a 64-byte seed (from `divi_sentinal_recovery_to_seed`) and produces
/// a 32-byte key suitable for use as a secp256k1 private key. Uses domain
/// salt `"omnidea-identity-v1"` with info `"identity-primary"`.
///
/// Output written to `out_key`/`out_key_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `seed` must be valid for `seed_len` bytes (must be >= 32 bytes, typically 64).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_derive_identity_key(
    seed: *const u8,
    seed_len: usize,
    out_key: *mut *mut u8,
    out_key_len: *mut usize,
) -> i32 {
    clear_last_error();

    if seed.is_null() || seed_len == 0 {
        set_last_error("divi_sentinal_derive_identity_key: null or empty seed");
        return -1;
    }

    let seed_slice = unsafe { std::slice::from_raw_parts(seed, seed_len) };

    match key_derivation::derive_identity_key(seed_slice) {
        Ok(key) => {
            let key_vec = key.expose().to_vec();
            let (kp, kl) = crate::helpers::bytes_to_owned(key_vec);
            unsafe {
                *out_key = kp;
                *out_key_len = kl;
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
// Password Strength Estimation
// ===================================================================

/// Estimate password strength. Returns JSON `PasswordStrength` or null on error.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `password` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_sentinal_password_strength(
    password: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(pw) = c_str_to_str(password) else {
        set_last_error("divi_sentinal_password_strength: invalid password string");
        return std::ptr::null_mut();
    };
    let result = password_strength::estimate_strength(pw);
    json_to_c(&result)
}
