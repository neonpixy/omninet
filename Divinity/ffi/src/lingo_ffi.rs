use std::ffi::c_char;
use std::sync::Mutex;

use lingo::Babel;

use crate::helpers::{c_str_to_str, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

/// Opaque wrapper for Babel (thread-safe).
pub struct LingoBabel(Mutex<Babel>);

// ===================================================================
// Lifecycle
// ===================================================================

/// Create a new Babel instance from a vocabulary seed.
///
/// `seed` is the raw seed bytes (typically 32 bytes from ECDH shared secret
/// or Sentinal key derivation).
///
/// Returns a Babel pointer. Free with `divi_lingo_babel_free`.
///
/// # Safety
/// `seed` must be valid for `seed_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_lingo_babel_new(
    seed: *const u8,
    seed_len: usize,
) -> *mut LingoBabel {
    clear_last_error();

    if seed.is_null() || seed_len == 0 {
        set_last_error("divi_lingo_babel_new: null seed");
        return std::ptr::null_mut();
    }

    let seed_slice = unsafe { std::slice::from_raw_parts(seed, seed_len) };
    let babel = Babel::new(seed_slice);
    Box::into_raw(Box::new(LingoBabel(Mutex::new(babel))))
}

/// Free a Babel instance.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_lingo_babel_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_lingo_babel_free(ptr: *mut LingoBabel) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ===================================================================
// Encode / Decode
// ===================================================================

/// Encode text into Babel symbols (hardened, non-deterministic).
///
/// Same input produces different output each time (nonce + homophones).
/// Returns a C string of Unicode symbols. Free via `divi_free_string`.
///
/// # Safety
/// `babel` must be a valid pointer. `text` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_lingo_babel_encode(
    babel: *const LingoBabel,
    text: *const c_char,
) -> *mut c_char {
    let babel = unsafe { &*babel };
    let Some(text_str) = c_str_to_str(text) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&babel.0);
    let encoded = guard.encode(text_str);
    string_to_c(encoded)
}

/// Decode Babel symbols back into text.
///
/// Returns a C string. Free via `divi_free_string`.
///
/// # Safety
/// `babel` must be a valid pointer. `encoded` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_lingo_babel_decode(
    babel: *const LingoBabel,
    encoded: *const c_char,
) -> *mut c_char {
    let babel = unsafe { &*babel };
    let Some(encoded_str) = c_str_to_str(encoded) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&babel.0);
    let decoded = guard.decode(encoded_str);
    string_to_c(decoded)
}

/// Decode Babel symbols back into text using language-aware token rejoining.
///
/// CJK/Kana/Hangul/Thai tokens are joined without spaces.
/// Latin/Arabic/Cyrillic/Devanagari tokens are joined with spaces.
/// `source_language` is a BCP 47 code (e.g., "ja", "zh-Hans", "en").
///
/// Returns a C string. Free via `divi_free_string`.
///
/// # Safety
/// `babel` must be a valid pointer. `encoded` and `source_language` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_lingo_babel_decode_for_language(
    babel: *const LingoBabel,
    encoded: *const c_char,
    source_language: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let babel = unsafe { &*babel };
    let Some(encoded_str) = c_str_to_str(encoded) else {
        set_last_error("divi_lingo_babel_decode_for_language: null encoded");
        return std::ptr::null_mut();
    };
    let Some(lang_str) = c_str_to_str(source_language) else {
        set_last_error("divi_lingo_babel_decode_for_language: null source_language");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&babel.0);
    let decoded = guard.decode_for_language(encoded_str, lang_str);
    string_to_c(decoded)
}
